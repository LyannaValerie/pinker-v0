use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_CAPTURED_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_CAPTURED_STDERR_BYTES: usize = 1024 * 1024;
const STALE_EXECUTION_MIN_AGE: Duration = Duration::from_secs(60 * 60);
type HashFingerprint = (u64, SystemTime, String);
type HashCache = Mutex<BTreeMap<PathBuf, HashFingerprint>>;
static HASH_CACHE: OnceLock<HashCache> = OnceLock::new();

#[path = "native_process_launcher.rs"]
mod native_process_launcher;
#[path = "native_process_marker.rs"]
mod native_process_marker;
#[path = "native_process_model.rs"]
mod native_process_model;
#[path = "native_process_sandbox.rs"]
mod native_process_sandbox;

use native_process_launcher::{LauncherIdentity, ProcessLauncher};
pub use native_process_launcher::{LifecycleProbe, StartupFailurePoint};
// Autoridade de varredura exposta às regressões. A produção e a fonte
// sintética atravessam exatamente estes itens: não há segunda implementação da
// decisão de ancestralidade, de sinalização ou de prova de ausência.
#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub(crate) use native_process_launcher::{
    raw_ancestry, raw_parse_parent_field, scan_with, Ancestry, CandidateSource, CandidateStep,
    ParentField, ParentLookup, ParentSource, ScanSummary, SignalSink,
};
use native_process_marker::MarkerState;
#[allow(unused_imports)]
pub use native_process_marker::{
    atomic_marker_interruption_for_test, marker_fields_for_test, marker_verdict_for_test,
};
pub use native_process_model::{
    select_primary_reason, ControlledRunOutcome, LifecycleEvent, LifecycleRecord,
    ObservedTerminationEvents, ProcessIdentitySnapshot, SandboxDisposition, ShutdownError,
    ShutdownFailurePoint, ShutdownSignal, ShutdownStage, TerminationReason,
};
use native_process_sandbox::ExecutionSandbox;
#[allow(unused_imports)]
pub use native_process_sandbox::{
    quarantine_remove_for_test, rust_cleanup_verdict_for_test, QuarantineStage, RemovalVerdict,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeRunClass {
    Common,
    Pipeline,
    Toolchain,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ResourcePolicy {
    class: NativeRunClass,
    timeout: Duration,
    pub(super) address_space_bytes: u64,
    pub(super) cpu_seconds: u64,
}

impl ResourcePolicy {
    /// Autoridade única dos limites: só aqui uma classe vira valores concretos.
    fn for_class(class: NativeRunClass) -> Self {
        match class {
            NativeRunClass::Toolchain => Self {
                class,
                timeout: Duration::from_secs(120),
                address_space_bytes: 4 * 1024 * 1024 * 1024,
                cpu_seconds: 120,
            },
            NativeRunClass::Pipeline => Self {
                class,
                timeout: Duration::from_secs(60),
                address_space_bytes: 4 * 1024 * 1024 * 1024,
                cpu_seconds: 60,
            },
            NativeRunClass::Common => Self {
                class,
                timeout: Duration::from_secs(20),
                address_space_bytes: 1024 * 1024 * 1024,
                cpu_seconds: 20,
            },
        }
    }

    /// Inferência canônica pela identidade do executável. É o DEFAULT, não a
    /// única fonte: um executável gerado tem nome arbitrário e por isso o
    /// chamador precisa poder declarar a intenção explicitamente.
    fn class_for_program(program: &OsStr) -> NativeRunClass {
        let name = Path::new(program)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        if matches!(name, "cc" | "gcc" | "clang" | "cargo" | "rustc" | "git") {
            return NativeRunClass::Toolchain;
        }
        if name == "pink" || name.starts_with("pink-") {
            return NativeRunClass::Pipeline;
        }
        NativeRunClass::Common
    }

    fn resolve(program: &OsStr, explicit: Option<NativeRunClass>) -> Self {
        Self::for_class(explicit.unwrap_or_else(|| Self::class_for_program(program)))
    }
}

/// Autoridade estreita de std::process::Command para as suítes nativas.
///
/// output() usa stdin nulo e captura limitada por padrão. Configurações
/// explícitas de stdin/stdout/stderr são preservadas. status() mantém a
/// semântica herdada por padrão e nunca cria captura oculta.
pub struct ControlledCommand {
    inner: StdCommand,
    logical_case: String,
    timeout_override: Option<Duration>,
    capture_override: Option<usize>,
    stdin_configured: bool,
    stdout_configured: bool,
    stderr_configured: bool,
    execution_repo_root: Option<PathBuf>,
    startup_failure: Option<StartupFailurePoint>,
    shutdown_failure: Option<ShutdownFailurePoint>,
    lifecycle_probe: LifecycleProbe,
    resource_class: Option<NativeRunClass>,
}

impl ControlledCommand {
    pub fn new<S: AsRef<OsStr>>(program: S) -> Self {
        let program = program.as_ref();
        let logical_case = Path::new(program)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("native-process")
            .to_string();
        Self {
            inner: StdCommand::new(program),
            logical_case,
            timeout_override: None,
            capture_override: None,
            stdin_configured: false,
            stdout_configured: false,
            stderr_configured: false,
            execution_repo_root: None,
            startup_failure: None,
            shutdown_failure: None,
            lifecycle_probe: LifecycleProbe::default(),
            resource_class: None,
        }
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.inner.arg(arg);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub fn env<K: AsRef<OsStr>, V: AsRef<OsStr>>(&mut self, key: K, value: V) -> &mut Self {
        self.inner.env(key, value);
        self
    }

    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
        self.inner.env_remove(key);
        self
    }

    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Self {
        self.inner.current_dir(dir);
        self
    }

    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdin(cfg);
        self.stdin_configured = true;
        self
    }

    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdout(cfg);
        self.stdout_configured = true;
        self
    }

    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stderr(cfg);
        self.stderr_configured = true;
        self
    }

    pub fn logical_case(&mut self, logical_case: &str) -> &mut Self {
        self.logical_case = logical_case.to_string();
        self
    }

    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout_override = Some(timeout);
        self
    }

    pub fn timeout_contract_for_test(&self) -> (Duration, Option<Duration>) {
        (self.resource_policy().timeout, self.timeout_override)
    }

    /// Declara que este executável é um guest do pipeline Pinker, e não um
    /// executável arbitrário. Existe porque o produto de `pink build --nativo`
    /// tem nome escolhido pelo caso de teste: sem intenção explícita o lado
    /// nativo da paridade cairia em Common enquanto o lado interpretado, que
    /// roda `pink --run`, cai em Pipeline. O nome do arquivo nunca é
    /// autoridade; a intenção é. Os limites continuam pertencendo a
    /// `ResourcePolicy`, que é quem traduz classe em valores.
    ///
    /// É opt-in por desenho, e hoje só a paridade de processos da Parte D o
    /// declara. As demais suítes que lançam ELF gerado continuam em Common:
    /// elas não acumulam captura sob pressão de endereçamento, e Common é a
    /// contenção correta para executável cuja classe ninguém afirmou. Quem
    /// levar outra suíte ao mesmo regime declara a intenção aqui, e não por
    /// padrão de nome de arquivo.
    pub fn pinker_pipeline_guest(&mut self) -> &mut Self {
        self.resource_class = Some(NativeRunClass::Pipeline);
        self
    }

    fn resource_policy(&self) -> ResourcePolicy {
        ResourcePolicy::resolve(self.inner.get_program(), self.resource_class)
    }

    /// Contrato de recurso efetivo desta invocação: classe canônica e os
    /// limites que ela determina, antes de qualquer override de prazo.
    pub fn resource_contract_for_test(&self) -> (&'static str, u64, u64) {
        let policy = self.resource_policy();
        let class = match policy.class {
            NativeRunClass::Common => "Common",
            NativeRunClass::Pipeline => "Pipeline",
            NativeRunClass::Toolchain => "Toolchain",
        };
        (class, policy.address_space_bytes, policy.cpu_seconds)
    }

    pub fn capture_limit(&mut self, bytes_per_channel: usize) -> &mut Self {
        self.capture_override = Some(bytes_per_channel);
        self
    }

    /// Gancho exclusivo de regressão. A produção sempre descobre a raiz real a
    /// partir do cwd e nunca aceita raiz por variável de ambiente.
    pub fn execution_repo_root_for_test<P: AsRef<Path>>(&mut self, root: P) -> &mut Self {
        self.execution_repo_root = Some(root.as_ref().to_path_buf());
        self
    }

    pub fn startup_failure_for_test(&mut self, point: StartupFailurePoint) -> &mut Self {
        self.startup_failure = Some(point);
        self
    }

    pub fn lifecycle_probe_for_test(&mut self, probe: &LifecycleProbe) -> &mut Self {
        self.lifecycle_probe = probe.clone();
        self
    }

    pub fn shutdown_failure_for_test(&mut self, point: ShutdownFailurePoint) -> &mut Self {
        self.shutdown_failure = Some(point);
        self
    }

    #[cfg(unix)]
    pub unsafe fn pre_exec<F>(&mut self, function: F) -> &mut Self
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: o chamador assume o contrato async-signal-safe de pre_exec.
        unsafe { self.inner.pre_exec(function) };
        self
    }

    pub fn output(&mut self) -> io::Result<Output> {
        if !self.stdin_configured {
            self.inner.stdin(Stdio::null());
        }
        if !self.stdout_configured {
            self.inner.stdout(Stdio::piped());
        }
        if !self.stderr_configured {
            self.inner.stderr(Stdio::piped());
        }
        let outcome = controlled_run(
            &mut self.inner,
            ControlledRunConfig {
                logical_case: &self.logical_case,
                timeout_override: self.timeout_override,
                capture_override: self.capture_override,
                capture: true,
                repo_root: self.execution_repo_root.as_deref(),
                startup_failure: self.startup_failure,
                shutdown_failure: self.shutdown_failure,
                lifecycle_probe: self.lifecycle_probe.clone(),
                resource_class: self.resource_class,
            },
        )?;
        if let Some(error) = outcome.compatibility_error() {
            return Err(error);
        }
        Ok(Output {
            status: outcome.status.expect("outcome saudável possui status"),
            stdout: outcome.stdout,
            stderr: outcome.stderr,
        })
    }

    pub fn status(&mut self) -> io::Result<ExitStatus> {
        if !self.stdin_configured {
            self.inner.stdin(Stdio::inherit());
        }
        if !self.stdout_configured {
            self.inner.stdout(Stdio::inherit());
        }
        if !self.stderr_configured {
            self.inner.stderr(Stdio::inherit());
        }
        let outcome = controlled_run(
            &mut self.inner,
            ControlledRunConfig {
                logical_case: &self.logical_case,
                timeout_override: self.timeout_override,
                capture_override: self.capture_override,
                capture: false,
                repo_root: self.execution_repo_root.as_deref(),
                startup_failure: self.startup_failure,
                shutdown_failure: self.shutdown_failure,
                lifecycle_probe: self.lifecycle_probe.clone(),
                resource_class: self.resource_class,
            },
        )?;
        if let Some(error) = outcome.compatibility_error() {
            return Err(error);
        }
        outcome
            .status
            .ok_or_else(|| io::Error::other("outcome saudável sem status"))
    }

    pub fn outcome_for_test(&mut self) -> io::Result<ControlledRunOutcome> {
        if !self.stdin_configured {
            self.inner.stdin(Stdio::null());
        }
        if !self.stdout_configured {
            self.inner.stdout(Stdio::piped());
        }
        if !self.stderr_configured {
            self.inner.stderr(Stdio::piped());
        }
        controlled_run(
            &mut self.inner,
            ControlledRunConfig {
                logical_case: &self.logical_case,
                timeout_override: self.timeout_override,
                capture_override: self.capture_override,
                capture: true,
                repo_root: self.execution_repo_root.as_deref(),
                startup_failure: self.startup_failure,
                shutdown_failure: self.shutdown_failure,
                lifecycle_probe: self.lifecycle_probe.clone(),
                resource_class: self.resource_class,
            },
        )
    }
}

pub struct NativeArtifactDir {
    sandbox: ExecutionSandbox,
}

impl NativeArtifactDir {
    pub fn create() -> io::Result<Self> {
        let git_head = read_git_head().unwrap_or_else(|| "unknown".to_string());
        let mut sandbox = ExecutionSandbox::create(&git_head, None)?;
        sandbox.authorize_cleanup();
        Ok(Self { sandbox })
    }

    pub fn path(&self) -> &Path {
        self.sandbox.path()
    }
}

struct ControlledRunConfig<'a> {
    logical_case: &'a str,
    timeout_override: Option<Duration>,
    capture_override: Option<usize>,
    capture: bool,
    repo_root: Option<&'a Path>,
    startup_failure: Option<StartupFailurePoint>,
    shutdown_failure: Option<ShutdownFailurePoint>,
    lifecycle_probe: LifecycleProbe,
    resource_class: Option<NativeRunClass>,
}

fn controlled_run(
    command: &mut StdCommand,
    config: ControlledRunConfig<'_>,
) -> io::Result<ControlledRunOutcome> {
    let ControlledRunConfig {
        logical_case,
        timeout_override,
        capture_override,
        capture,
        repo_root,
        startup_failure,
        shutdown_failure,
        lifecycle_probe,
        resource_class,
    } = config;
    let mut policy = ResourcePolicy::resolve(command.get_program(), resource_class);
    if let Some(timeout) = timeout_override {
        policy.timeout = timeout;
    }
    let stdout_limit = capture_override.unwrap_or(MAX_CAPTURED_STDOUT_BYTES);
    let stderr_limit = capture_override.unwrap_or(MAX_CAPTURED_STDERR_BYTES);
    let executable = resolve_executable(command.get_program());
    let executable_hash = executable
        .as_deref()
        .and_then(sha256_file_cached)
        .unwrap_or_else(|| "unknown".to_string());
    let runtime_hash = command
        .get_envs()
        .find_map(|(key, value)| (key == "PINKER_RT_LIB").then_some(value).flatten())
        .map(PathBuf::from)
        .or_else(runtime_library)
        .as_deref()
        .and_then(sha256_file_cached);
    let git_head = read_git_head().unwrap_or_else(|| "unknown".to_string());
    let mut sandbox = ExecutionSandbox::create(&git_head, repo_root)?;
    command.env("TMPDIR", sandbox.path());
    command.env("PINKER_EXECUTION_DIR", sandbox.path());

    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let started = Instant::now();
    let startup = ProcessLauncher::spawn_gated(
        command,
        policy,
        startup_failure,
        lifecycle_probe.clone(),
        |identity, control_fd| {
            sandbox.update_marker(
                MarkerState::LauncherReady,
                Some((identity.pid, identity.start_time, identity.pgid)),
                None,
                None,
                "pending",
            )?;
            if startup_failure == Some(StartupFailurePoint::BeforeWatchdog) {
                return Err(io::Error::other("falha injetada em BEFORE_WATCHDOG"));
            }
            let watchdog = ProcessWatchdog::spawn(
                identity,
                control_fd,
                startup_failure,
                lifecycle_probe.clone(),
            )?;
            sandbox.update_marker(
                MarkerState::WatchdogReady,
                Some((identity.pid, identity.start_time, identity.pgid)),
                None,
                watchdog.pid().map(|pid| pid as u32),
                "pending",
            )?;
            Ok(watchdog)
        },
    );
    let (mut launcher, mut watchdog) = match startup {
        Ok(startup) => startup,
        Err(error) => {
            let marker_result = sandbox.mark_failed_preserving_shape(&executable_hash);
            if marker_result.is_ok() {
                sandbox.authorize_cleanup();
                sandbox.cleanup()?;
            } else {
                sandbox.preserve();
            }
            return Err(error);
        }
    };
    let launcher_identity = launcher.identity();
    let launcher_snapshot = launcher_identity.snapshot();
    let watchdog_identity = watchdog.identity();
    let guest_pid = launcher.guest_pid();
    let running_marker_error = sandbox
        .update_marker(
            MarkerState::Running,
            Some((
                launcher_identity.pid,
                launcher_identity.start_time,
                launcher_identity.pgid,
            )),
            Some(guest_pid),
            watchdog.pid().map(|pid| pid as u32),
            &executable_hash,
        )
        .err();

    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let stdout_thread = if capture {
        launcher
            .take_stdout()
            .map(|stdout| bounded_reader(stdout, stdout_limit, Arc::clone(&stdout_overflow)))
    } else {
        None
    };
    let stderr_thread = if capture {
        launcher
            .take_stderr()
            .map(|stderr| bounded_reader(stderr, stderr_limit, Arc::clone(&stderr_overflow)))
    } else {
        None
    };
    drop(launcher.take_stdin());

    let running_marker_failed = running_marker_error.is_some();
    if !running_marker_failed {
        lifecycle_probe.record(LifecycleEvent::SandboxRunning);
    }
    let mut secondary_errors = Vec::new();
    let mut primary_reason = None;
    if let Some(error) = running_marker_error {
        primary_reason = Some(TerminationReason::StartupFailure);
        lifecycle_probe.record(LifecycleEvent::PrimaryReasonLatched(
            TerminationReason::StartupFailure,
        ));
        record_secondary(
            &mut secondary_errors,
            &lifecycle_probe,
            ShutdownError::from_io(ShutdownStage::MarkerRunning, error, None),
        );
    } else if startup_failure == Some(StartupFailurePoint::AfterGate) {
        primary_reason = Some(TerminationReason::StartupFailure);
        lifecycle_probe.record(LifecycleEvent::PrimaryReasonLatched(
            TerminationReason::StartupFailure,
        ));
    }

    let status = loop {
        let watchdog_exit = match watchdog.is_alive() {
            Ok(alive) => !alive,
            Err(error) => {
                record_secondary(
                    &mut secondary_errors,
                    &lifecycle_probe,
                    ShutdownError::from_io(
                        ShutdownStage::WatchdogFinish,
                        error,
                        Some(watchdog_identity),
                    ),
                );
                true
            }
        };
        let final_observation = launcher.try_final();
        let (guest_status, launcher_failure) = match final_observation {
            Ok(status) => (status, false),
            Err(error) => {
                record_secondary(
                    &mut secondary_errors,
                    &lifecycle_probe,
                    ShutdownError::from_io(
                        ShutdownStage::WaitFinal,
                        error,
                        Some(launcher_snapshot),
                    ),
                );
                (None, true)
            }
        };
        let observed = ObservedTerminationEvents {
            watchdog_exit,
            launcher_failure,
            stdout_limit: stdout_overflow.load(Ordering::SeqCst),
            stderr_limit: stderr_overflow.load(Ordering::SeqCst),
            timeout: started.elapsed() >= policy.timeout,
            startup_failure: startup_failure == Some(StartupFailurePoint::AfterGate)
                || running_marker_failed,
            guest_exited: guest_status.is_some(),
            ..ObservedTerminationEvents::default()
        };
        let selected = select_primary_reason(primary_reason, observed);
        if primary_reason.is_none() {
            if let Some(reason) = selected {
                primary_reason = Some(reason);
                lifecycle_probe.record(LifecycleEvent::PrimaryReasonLatched(reason));
            }
        }
        if primary_reason == Some(TerminationReason::GuestExited) {
            break guest_status;
        }
        if primary_reason.is_some() {
            if let Err(error) = sandbox.update_marker(
                MarkerState::Terminating,
                Some((
                    launcher_identity.pid,
                    launcher_identity.start_time,
                    launcher_identity.pgid,
                )),
                Some(guest_pid),
                watchdog.pid().map(|pid| pid as u32),
                &executable_hash,
            ) {
                record_secondary(
                    &mut secondary_errors,
                    &lifecycle_probe,
                    ShutdownError::from_io(ShutdownStage::MarkerTerminating, error, None),
                );
            }
            if let Err(error) = launcher.request_termination() {
                record_secondary(
                    &mut secondary_errors,
                    &lifecycle_probe,
                    ShutdownError::from_io(
                        ShutdownStage::TerminationRequest,
                        error,
                        Some(launcher_snapshot),
                    ),
                );
            }
            break match launcher.wait_final(Duration::from_secs(10)) {
                Ok(status) => Some(status),
                Err(error) => {
                    record_secondary(
                        &mut secondary_errors,
                        &lifecycle_probe,
                        ShutdownError::from_io(
                            ShutdownStage::WaitFinal,
                            error,
                            Some(launcher_snapshot),
                        ),
                    );
                    None
                }
            };
        }
        thread::sleep(Duration::from_millis(10));
    };

    let stdout = match join_reader(stdout_thread, "stdout") {
        Ok(stdout) => stdout,
        Err(error) => {
            record_secondary(
                &mut secondary_errors,
                &lifecycle_probe,
                ShutdownError::from_io(ShutdownStage::StdoutJoin, error, None),
            );
            Vec::new()
        }
    };
    let stderr = match join_reader(stderr_thread, "stderr") {
        Ok(stderr) => stderr,
        Err(error) => {
            record_secondary(
                &mut secondary_errors,
                &lifecycle_probe,
                ShutdownError::from_io(ShutdownStage::StderrJoin, error, None),
            );
            Vec::new()
        }
    };
    let duration = started.elapsed();
    let launcher_reaped = match launcher.reap() {
        Ok(()) => true,
        Err(error) => {
            record_secondary(
                &mut secondary_errors,
                &lifecycle_probe,
                ShutdownError::from_io(ShutdownStage::FinalReap, error, Some(launcher_snapshot)),
            );
            false
        }
    };
    if shutdown_failure == Some(ShutdownFailurePoint::FinalReap) {
        record_secondary(
            &mut secondary_errors,
            &lifecycle_probe,
            ShutdownError::injected(ShutdownStage::FinalReap, Some(launcher_snapshot)),
        );
    }
    let watchdog_reaped = match watchdog.finish() {
        Ok(()) => true,
        Err(error) => {
            record_secondary(
                &mut secondary_errors,
                &lifecycle_probe,
                ShutdownError::from_io(
                    ShutdownStage::WatchdogFinish,
                    error,
                    Some(watchdog_identity),
                ),
            );
            false
        }
    };
    if shutdown_failure == Some(ShutdownFailurePoint::WatchdogFinish) {
        record_secondary(
            &mut secondary_errors,
            &lifecycle_probe,
            ShutdownError::injected(ShutdownStage::WatchdogFinish, Some(watchdog_identity)),
        );
    }
    let tree_shutdown_proven = status.is_some() && launcher_reaped && watchdog_reaped;
    let primary_reason = primary_reason.unwrap_or(TerminationReason::LauncherFailure);

    if primary_reason != TerminationReason::GuestExited
        || status.map_or(true, |status| !status.success())
    {
        eprintln!(
            "native_execution_failure case={} git_head={} executable={} executable_sha256={} runtime_sha256={} pid={} pgid={} supervisor_pid={} policy={:?} address_space_bytes={} cpu_seconds={} timeout_ms={} capture_limit={} started_unix_ms={} duration_ms={} status={} signal={} reason={}",
            sanitize(logical_case),
            git_head,
            executable
                .as_deref()
                .and_then(Path::file_name)
                .and_then(OsStr::to_str)
                .unwrap_or("unknown"),
            executable_hash,
            runtime_hash.as_deref().unwrap_or("unknown"),
            guest_pid,
            launcher_identity.pgid,
            watchdog.reported_pid(),
            policy.class,
            policy.address_space_bytes,
            policy.cpu_seconds,
            policy.timeout.as_millis(),
            if capture { stdout_limit.max(stderr_limit) } else { 0 },
            started_unix_ms,
            duration.as_millis(),
            status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            status
                .as_ref()
                .and_then(exit_signal)
                .map(|signal| signal.to_string())
                .unwrap_or_else(|| "none".to_string()),
            primary_reason,
        );
    }

    let terminal_state = if primary_reason == TerminationReason::GuestExited
        && status.is_some_and(|status| status.success())
        && secondary_errors.is_empty()
    {
        MarkerState::Finished
    } else {
        MarkerState::Failed
    };
    if let Err(error) = sandbox.update_marker(
        terminal_state,
        Some((
            launcher_identity.pid,
            launcher_identity.start_time,
            launcher_identity.pgid,
        )),
        Some(guest_pid),
        watchdog.pid().map(|pid| pid as u32),
        &executable_hash,
    ) {
        record_secondary(
            &mut secondary_errors,
            &lifecycle_probe,
            ShutdownError::from_io(ShutdownStage::MarkerTerminal, error, None),
        );
    }
    if shutdown_failure == Some(ShutdownFailurePoint::MarkerTerminal) {
        record_secondary(
            &mut secondary_errors,
            &lifecycle_probe,
            ShutdownError::injected(ShutdownStage::MarkerTerminal, None),
        );
    }

    let sandbox_disposition = if tree_shutdown_proven {
        sandbox.authorize_cleanup();
        match sandbox.cleanup() {
            Ok(()) => {
                lifecycle_probe.record(LifecycleEvent::SandboxRemoved);
                SandboxDisposition::Removed
            }
            Err(error) => {
                let reason = error.to_string();
                sandbox.preserve();
                lifecycle_probe.record(LifecycleEvent::SandboxPreserved(reason.clone()));
                record_secondary(
                    &mut secondary_errors,
                    &lifecycle_probe,
                    ShutdownError::from_io(ShutdownStage::Cleanup, error, None),
                );
                SandboxDisposition::Preserved(reason)
            }
        }
    } else {
        let reason = "tree-shutdown-unproven".to_string();
        sandbox.preserve();
        lifecycle_probe.record(LifecycleEvent::SandboxPreserved(reason.clone()));
        SandboxDisposition::Preserved(reason)
    };
    for (point, stage) in [
        (ShutdownFailurePoint::Cleanup, ShutdownStage::Cleanup),
        (ShutdownFailurePoint::Quarantine, ShutdownStage::Quarantine),
        (
            ShutdownFailurePoint::EvidenceWrite,
            ShutdownStage::EvidenceWrite,
        ),
    ] {
        if shutdown_failure == Some(point) {
            record_secondary(
                &mut secondary_errors,
                &lifecycle_probe,
                ShutdownError::injected(stage, None),
            );
        }
    }
    if let Some(error) = lifecycle_probe.take_sink_error() {
        secondary_errors.push(ShutdownError::from_io(
            ShutdownStage::EvidenceWrite,
            error,
            None,
        ));
    }

    Ok(ControlledRunOutcome {
        status,
        primary_reason,
        secondary_errors,
        launcher_identity: launcher_snapshot,
        watchdog_identity: Some(watchdog_identity),
        tree_shutdown_proven,
        sandbox_disposition,
        stdout,
        stderr,
    })
}

fn record_secondary(
    secondary_errors: &mut Vec<ShutdownError>,
    lifecycle_probe: &LifecycleProbe,
    error: ShutdownError,
) {
    lifecycle_probe.record(LifecycleEvent::SecondaryFailure(error.clone()));
    secondary_errors.push(error);
}

fn join_reader(
    reader: Option<thread::JoinHandle<io::Result<Vec<u8>>>>,
    channel: &str,
) -> io::Result<Vec<u8>> {
    match reader {
        Some(reader) => reader
            .join()
            .map_err(|_| io::Error::other(format!("thread de {channel} entrou em panic")))?,
        None => Ok(Vec::new()),
    }
}

fn bounded_reader<R: Read + Send + 'static>(
    mut reader: R,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut captured = Vec::with_capacity(limit.min(64 * 1024));
        let mut buffer = [0_u8; 8192];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let remaining = limit.saturating_sub(captured.len());
            captured.extend_from_slice(&buffer[..read.min(remaining)]);
            if read > remaining {
                overflow.store(true, Ordering::SeqCst);
            }
        }
        Ok(captured)
    })
}

#[cfg(target_os = "linux")]
struct ProcessWatchdog {
    pid: i32,
    reported_pid: i32,
    identity: ProcessIdentitySnapshot,
    life_fd: Option<i32>,
    reaped: bool,
    probe: LifecycleProbe,
    launcher: LauncherIdentity,
}

#[cfg(target_os = "linux")]
impl ProcessWatchdog {
    fn spawn(
        launcher: LauncherIdentity,
        launcher_control_fd: i32,
        failure: Option<StartupFailurePoint>,
        probe: LifecycleProbe,
    ) -> io::Result<Self> {
        extern "C" {
            fn pipe2(pipefd: *mut i32, flags: i32) -> i32;
            fn fork() -> i32;
            fn close(fd: i32) -> i32;
            fn read(fd: i32, buffer: *mut u8, count: usize) -> isize;
            fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
            fn _exit(status: i32) -> !;
        }
        const O_CLOEXEC: i32 = 0o2000000;
        if failure == Some(StartupFailurePoint::WatchdogPipe) {
            return Err(io::Error::other("falha injetada em WATCHDOG_PIPE"));
        }
        let mut life_pipe = [-1_i32; 2];
        let mut ready_pipe = [-1_i32; 2];
        if unsafe { pipe2(life_pipe.as_mut_ptr(), O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        if unsafe { pipe2(ready_pipe.as_mut_ptr(), O_CLOEXEC) } != 0 {
            let error = io::Error::last_os_error();
            unsafe {
                close(life_pipe[0]);
                close(life_pipe[1]);
            }
            return Err(error);
        }
        if failure == Some(StartupFailurePoint::WatchdogFork) {
            unsafe {
                close(life_pipe[0]);
                close(life_pipe[1]);
                close(ready_pipe[0]);
                close(ready_pipe[1]);
            }
            return Err(io::Error::other("falha injetada em WATCHDOG_FORK"));
        }
        let supervisor = unsafe { fork() };
        if supervisor < 0 {
            unsafe {
                close(life_pipe[0]);
                close(life_pipe[1]);
                close(ready_pipe[0]);
                close(ready_pipe[1]);
            }
            return Err(io::Error::last_os_error());
        }
        if supervisor == 0 {
            unsafe {
                close(life_pipe[1]);
                close(ready_pipe[0]);
                if failure == Some(StartupFailurePoint::AfterWatchdogBeforeReady) {
                    _exit(124);
                }
                // O watchdog não executa ferramentas nem o convidado: sua
                // allowlist contém somente vida, prontidão e controle do
                // launcher. A higiene termina antes de WATCHDOG_READY.
                close_watchdog_fds_except(&[launcher_control_fd, life_pipe[0], ready_pipe[1]]);
                let ready = [b'R'];
                if write(ready_pipe[1], ready.as_ptr(), 1) != 1 {
                    _exit(125);
                }
                close(ready_pipe[1]);
                let mut byte = 0_u8;
                let read_result = read(life_pipe[0], &mut byte, 1);
                close(life_pipe[0]);
                if read_result == 0 {
                    let terminate = [b'T'];
                    let _ = write(launcher_control_fd, terminate.as_ptr(), 1);
                }
                _exit(0);
            }
        }
        unsafe {
            close(life_pipe[0]);
            close(ready_pipe[1]);
        }
        let mut ready = 0_u8;
        let ready_result = unsafe { read(ready_pipe[0], &mut ready, 1) };
        unsafe {
            close(ready_pipe[0]);
        }
        if ready_result != 1 || ready != b'R' {
            let mut status = 0_i32;
            unsafe {
                waitpid_blocking(supervisor, &mut status);
                close(life_pipe[1]);
            }
            probe.record(LifecycleEvent::WatchdogReaped);
            return Err(io::Error::other("watchdog não confirmou o canal de vida"));
        }
        let start_time = process_start_time(supervisor as u32).ok_or_else(|| {
            unsafe {
                close(life_pipe[1]);
            }
            let mut status = 0_i32;
            unsafe {
                waitpid_blocking(supervisor, &mut status);
            }
            probe.record(LifecycleEvent::WatchdogReaped);
            io::Error::other("identidade do watchdog não pôde ser provada")
        })?;
        let identity = ProcessIdentitySnapshot {
            pid: supervisor as u32,
            start_time,
            pgid: None,
        };
        probe.record(LifecycleEvent::WatchdogReady(identity));
        Ok(Self {
            pid: supervisor,
            reported_pid: supervisor,
            identity,
            life_fd: Some(life_pipe[1]),
            reaped: false,
            probe,
            launcher,
        })
    }

    fn pid(&self) -> Option<i32> {
        Some(self.reported_pid)
    }

    fn reported_pid(&self) -> i32 {
        self.reported_pid
    }

    fn identity(&self) -> ProcessIdentitySnapshot {
        self.identity
    }

    fn is_alive(&mut self) -> io::Result<bool> {
        if self.reaped {
            return Ok(false);
        }
        extern "C" {
            fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        }
        const WNOHANG: i32 = 1;
        loop {
            let mut status = 0_i32;
            let waited = unsafe { waitpid(self.pid, &mut status, WNOHANG) };
            if waited == 0 {
                return Ok(true);
            }
            if waited == self.pid {
                self.reaped = true;
                self.probe
                    .record(LifecycleEvent::WatchdogExitObserved(self.identity));
                self.probe.record(LifecycleEvent::WatchdogReaped);
                return Ok(false);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    fn finish(&mut self) -> io::Result<()> {
        if self.reaped {
            if let Some(fd) = self.life_fd.take() {
                extern "C" {
                    fn close(fd: i32) -> i32;
                }
                unsafe {
                    close(fd);
                }
            }
            return Ok(());
        }
        let mut first_error = None;
        if let Some(fd) = self.life_fd.take() {
            extern "C" {
                fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
                fn close(fd: i32) -> i32;
            }
            let graceful = [b'D'];
            if unsafe { write(fd, graceful.as_ptr(), 1) } < 0 {
                first_error = Some(io::Error::last_os_error());
                unsafe {
                    close(fd);
                }
            } else {
                unsafe {
                    close(fd);
                }
            }
        }
        match self.reap() {
            Ok(()) => self.probe.record(LifecycleEvent::WatchdogReaped),
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(error) => {
                let earlier = first_error.take().expect("erro anterior presente");
                first_error = Some(io::Error::other(format!(
                    "{earlier}; reap do watchdog também falhou: {error}"
                )));
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn reap(&mut self) -> io::Result<()> {
        if self.reaped {
            return Ok(());
        }
        extern "C" {
            fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        }
        let mut status = 0_i32;
        let waited = unsafe { waitpid(self.pid, &mut status, 0) };
        if waited == self.pid {
            self.reaped = true;
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(target_os = "linux")]
unsafe fn close_watchdog_fds_except(allowed: &[i32]) {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn close_range(first: u32, last: u32, flags: u32) -> i32;
    }
    let maximum = 65_536_u32;
    let mut first = 0_u32;
    loop {
        let Some(kept) = allowed
            .iter()
            .copied()
            .filter(|&fd| fd >= 0 && fd as u32 >= first)
            .map(|fd| fd as u32)
            .min()
        else {
            break;
        };
        if first < kept && close_range(first, kept - 1, 0) != 0 {
            for fd in first..kept.min(maximum) {
                close(fd as i32);
            }
        }
        first = kept.saturating_add(1);
    }
    if first < maximum && close_range(first, u32::MAX, 0) != 0 {
        for fd in first..maximum {
            close(fd as i32);
        }
    }
}

#[cfg(target_os = "linux")]
pub fn watchdog_fd_allowlist_probe(inherited_fd: i32) -> io::Result<bool> {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn fcntl(fd: i32, command: i32, ...) -> i32;
        fn fork() -> i32;
        fn pipe2(pipefd: *mut i32, flags: i32) -> i32;
        fn read(fd: i32, buffer: *mut u8, count: usize) -> isize;
        fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
        fn _exit(status: i32) -> !;
    }
    const F_GETFD: i32 = 1;
    const O_CLOEXEC: i32 = 0o2000000;
    let mut probe = [-1_i32; 2];
    if unsafe { pipe2(probe.as_mut_ptr(), O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let child = unsafe { fork() };
    if child < 0 {
        let error = io::Error::last_os_error();
        unsafe {
            close(probe[0]);
            close(probe[1]);
        }
        return Err(error);
    }
    if child == 0 {
        unsafe {
            close(probe[0]);
            close_watchdog_fds_except(&[probe[1]]);
            let result = if fcntl(inherited_fd, F_GETFD) < 0 {
                b'C'
            } else {
                b'O'
            };
            let _ = write(probe[1], &result, 1);
            close(probe[1]);
            _exit(0);
        }
    }
    unsafe {
        close(probe[1]);
    }
    let mut result = 0_u8;
    let read_result = unsafe { read(probe[0], &mut result, 1) };
    unsafe {
        close(probe[0]);
    }
    let mut status = 0_i32;
    unsafe {
        waitpid_blocking(child, &mut status);
    }
    if read_result != 1 {
        return Err(io::Error::other("watchdog não publicou prova da allowlist"));
    }
    Ok(result == b'C')
}

#[cfg(not(target_os = "linux"))]
pub fn watchdog_fd_allowlist_probe(_inherited_fd: i32) -> io::Result<bool> {
    Ok(true)
}

#[cfg(target_os = "linux")]
unsafe fn waitpid_blocking(pid: i32, status: *mut i32) {
    extern "C" {
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
    while waitpid(pid, status, 0) < 0 {
        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            break;
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for ProcessWatchdog {
    fn drop(&mut self) {
        if let Some(fd) = self.life_fd.take() {
            extern "C" {
                fn close(fd: i32) -> i32;
            }
            unsafe {
                close(fd);
            }
        }
        if !self.reaped && self.reap().is_ok() {
            self.probe.record(LifecycleEvent::WatchdogReaped);
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct ProcessWatchdog;

#[cfg(not(target_os = "linux"))]
impl ProcessWatchdog {
    fn spawn(_pgid: i32) -> io::Result<Self> {
        Ok(Self)
    }
    fn pid(&self) -> Option<i32> {
        None
    }
    fn reported_pid(&self) -> i32 {
        -1
    }
    fn identity(&self) -> ProcessIdentitySnapshot {
        ProcessIdentitySnapshot {
            pid: 0,
            start_time: 0,
            pgid: None,
        }
    }
    fn is_alive(&mut self) -> io::Result<bool> {
        Ok(true)
    }
    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessIdentity {
    Live,
    Reused,
    Missing,
    Unknown,
}

pub fn parse_proc_stat_start_time(stat: &str) -> Option<u64> {
    let close = stat.rfind(") ")?;
    let suffix = stat.get(close + 2..)?;
    let fields: Vec<&str> = suffix.split_ascii_whitespace().collect();
    if fields.len() < 20 || fields[0].len() != 1 || !fields[0].is_ascii() {
        return None;
    }
    fields[19].parse().ok()
}

pub fn process_identity(pid: u32, expected_start_time: u64) -> ProcessIdentity {
    if !Path::new("/proc").is_dir() {
        return ProcessIdentity::Unknown;
    }
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => match parse_proc_stat_start_time(&stat) {
            Some(actual) if actual == expected_start_time => ProcessIdentity::Live,
            Some(_) => ProcessIdentity::Reused,
            None => ProcessIdentity::Unknown,
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => ProcessIdentity::Missing,
        Err(_) => ProcessIdentity::Unknown,
    }
}

pub fn process_start_time(pid: u32) -> Option<u64> {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => parse_proc_stat_start_time(&stat),
        Err(_) => None,
    }
}

#[cfg(unix)]
fn exit_signal(status: &ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &ExitStatus) -> Option<i32> {
    None
}

fn resolve_executable(program: &OsStr) -> Option<PathBuf> {
    let path = PathBuf::from(program);
    if path.components().count() > 1 {
        return path.canonicalize().ok();
    }
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        let candidate = dir.join(&path);
        candidate
            .is_file()
            .then(|| candidate.canonicalize().ok())
            .flatten()
    })
}

fn runtime_library() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PINKER_RT_LIB").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    let candidate = std::env::current_dir()
        .ok()?
        .join("target/debug/libpinker_rt.a");
    candidate.is_file().then_some(candidate)
}

fn read_git_head() -> Option<String> {
    let mut directory = std::env::current_dir().ok()?;
    loop {
        let dot_git = directory.join(".git");
        if dot_git.is_dir() {
            return resolve_head(&dot_git);
        }
        if dot_git.is_file() {
            let text = fs::read_to_string(&dot_git).ok()?;
            let git_dir = PathBuf::from(text.trim().strip_prefix("gitdir: ")?);
            return resolve_head(&git_dir);
        }
        if !directory.pop() {
            return None;
        }
    }
}

fn resolve_head(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if !head.starts_with("ref: ") {
        return Some(head.to_string());
    }
    let reference = head.strip_prefix("ref: ")?;
    if let Ok(value) = fs::read_to_string(git_dir.join(reference)) {
        return Some(value.trim().to_string());
    }
    let common = fs::read_to_string(git_dir.join("commondir")).ok()?;
    fs::read_to_string(git_dir.join(common.trim()).join(reference))
        .ok()
        .map(|value| value.trim().to_string())
}

fn sha256_file(path: &Path) -> Option<String> {
    // Delegado ao núcleo compartilhado da Parte E2: era uma cópia manual do
    // algoritmo, e SHA-256 duplicado diverge em silêncio.
    let data = fs::read(path).ok()?;
    Some(pinker_sha256_contract::sha256_hex(&data))
}

fn sha256_file_cached(path: &Path) -> Option<String> {
    let path = path.canonicalize().ok()?;
    let metadata = fs::metadata(&path).ok()?;
    let fingerprint = (metadata.len(), metadata.modified().ok()?);
    let cache = HASH_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some((length, modified, hash)) = cache.lock().ok()?.get(&path) {
        if (*length, *modified) == fingerprint {
            return Some(hash.clone());
        }
    }
    let hash = sha256_file(&path)?;
    cache
        .lock()
        .ok()?
        .insert(path, (fingerprint.0, fingerprint.1, hash.clone()));
    Some(hash)
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || "._-".contains(ch) {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
