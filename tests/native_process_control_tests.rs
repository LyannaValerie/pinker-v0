mod common;

use common::native_process::{
    process_identity, process_start_time, select_primary_reason, LifecycleEvent, LifecycleProbe,
    LifecycleRecord, ObservedTerminationEvents, ProcessIdentity, ProcessIdentitySnapshot,
    SandboxDisposition, ShutdownError, ShutdownFailurePoint, ShutdownSignal, ShutdownStage,
    StartupFailurePoint, TerminationReason,
};
use common::{ControlledCommand, NativeArtifactDir};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::symlink;
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as RawCommand, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn fixture_id() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
    )
}

fn execution_dirs_at(repo_root: &Path) -> usize {
    fs::read_dir(repo_root.join("target/pinker-exec"))
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

fn direct_children() -> Vec<u32> {
    fs::read_to_string("/proc/thread-self/children")
        .unwrap_or_default()
        .split_ascii_whitespace()
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("arquivo não apareceu: {}", path.display());
}

fn git_head_for_evidence() -> String {
    let head = fs::read_to_string(".git/HEAD").unwrap_or_else(|_| "unknown".to_string());
    let head = head.trim();
    match head.strip_prefix("ref: ") {
        Some(reference) => fs::read_to_string(Path::new(".git").join(reference))
            .unwrap_or_else(|_| head.to_string())
            .trim()
            .to_string(),
        None => head.to_string(),
    }
}

fn process_gone_or_zombie(pid: u32) -> bool {
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return true;
    };
    let Some((_, suffix)) = stat.rsplit_once(") ") else {
        return false;
    };
    suffix.split_ascii_whitespace().next() == Some("Z")
}

fn wait_process_dead(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if process_gone_or_zombie(pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("processo {pid} permaneceu executável");
}

fn kill_process_verified(pid: u32, start_time: u64, signal: i32) {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    assert_eq!(
        process_start_time(pid),
        Some(start_time),
        "identidade mudou antes do sinal"
    );
    assert_eq!(unsafe { kill(pid as i32, signal) }, 0);
}

struct VerifiedChild {
    child: Option<Child>,
    identity: ProcessIdentitySnapshot,
}

impl VerifiedChild {
    fn new(child: Child) -> Self {
        let pid = child.id();
        let start_time = process_start_time(pid).expect("start time do controlador");
        Self {
            child: Some(child),
            identity: ProcessIdentitySnapshot {
                pid,
                start_time,
                pgid: None,
            },
        }
    }

    fn wait_with_output(mut self) -> Output {
        self.child
            .take()
            .expect("controlador presente")
            .wait_with_output()
            .expect("reap controlador")
    }

    fn kill_and_wait(mut self) -> Output {
        kill_process_verified(self.identity.pid, self.identity.start_time, 9);
        self.child
            .take()
            .expect("controlador presente")
            .wait_with_output()
            .expect("reap controlador")
    }
}

impl Drop for VerifiedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if child.try_wait().ok().flatten().is_none()
            && process_start_time(self.identity.pid) == Some(self.identity.start_time)
        {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

struct GuestGateGuard {
    probe: LifecycleProbe,
    held: bool,
}

impl GuestGateGuard {
    fn hold(probe: &LifecycleProbe) -> Self {
        probe.hold_guest_gate_for_test();
        Self {
            probe: probe.clone(),
            held: true,
        }
    }

    fn release(&mut self) {
        if self.held {
            self.probe.release_guest_gate_for_test();
            self.held = false;
        }
    }
}

impl Drop for GuestGateGuard {
    fn drop(&mut self) {
        self.release();
    }
}

struct LifecycleReceiver {
    socket: UnixDatagram,
    path: PathBuf,
    journal: PathBuf,
    records: Vec<LifecycleRecord>,
}

impl LifecycleReceiver {
    fn bind(label: &str) -> Self {
        let path = PathBuf::from(format!(
            "target/pinker-lifecycle-{label}-{}.sock",
            fixture_id()
        ));
        let _ = fs::remove_file(&path);
        let socket = UnixDatagram::bind(&path).expect("canal lifecycle exclusivo");
        let journal = path.with_extension("events");
        fs::write(&journal, []).expect("journal lifecycle");
        Self {
            socket,
            path,
            journal,
            records: Vec::new(),
        }
    }

    fn wait_for(
        &mut self,
        label: &str,
        predicate: impl Fn(&LifecycleEvent) -> bool,
    ) -> LifecycleRecord {
        self.wait_for_until(label, Instant::now() + Duration::from_secs(5), predicate)
    }

    fn wait_for_until(
        &mut self,
        label: &str,
        deadline: Instant,
        predicate: impl Fn(&LifecycleEvent) -> bool,
    ) -> LifecycleRecord {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.preserve_timeout(label);
                panic!("evento {label} ausente; observados={:?}", self.records);
            }
            self.socket
                .set_read_timeout(Some(remaining))
                .expect("timeout lifecycle");
            let mut bytes = [0_u8; 4096];
            match self.socket.recv(&mut bytes) {
                Ok(size) => {
                    let wire = std::str::from_utf8(&bytes[..size]).expect("lifecycle UTF-8");
                    let record = LifecycleRecord::from_wire(wire).expect("lifecycle estruturado");
                    let expected = self.records.len() as u64 + 1;
                    assert_eq!(
                        record.sequence, expected,
                        "eventos lifecycle fora de ordem: {:?}",
                        self.records
                    );
                    let matched = predicate(&record.event);
                    let mut journal = OpenOptions::new()
                        .append(true)
                        .open(&self.journal)
                        .expect("journal lifecycle aberto");
                    writeln!(journal, "{}", record.to_wire()).expect("journal lifecycle escreve");
                    journal.flush().expect("journal lifecycle flush");
                    self.records.push(record.clone());
                    if matched {
                        return record;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    self.preserve_timeout(label);
                    panic!("timeout aguardando {label}; observados={:?}", self.records);
                }
                Err(error) => panic!("canal lifecycle falhou aguardando {label}: {error}"),
            }
        }
    }

    fn preserve_timeout(&self, label: &str) {
        let directory = PathBuf::from(format!(
            "target/pinker-flake-evidence/lifecycle-timeout-{}",
            fixture_id()
        ));
        if fs::create_dir_all(&directory).is_ok() {
            let rendered = self
                .records
                .iter()
                .map(LifecycleRecord::to_wire)
                .collect::<Vec<_>>()
                .join("\n");
            let _ = fs::write(directory.join("events.txt"), rendered);
            let _ = fs::write(directory.join("missing-event.txt"), label);
            let _ = fs::write(directory.join("head.txt"), git_head_for_evidence());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuestTreeReady {
    child: ProcessIdentitySnapshot,
    grandchild: ProcessIdentitySnapshot,
}

struct GuestTreeReceiver {
    socket: UnixDatagram,
    path: PathBuf,
}

impl GuestTreeReceiver {
    fn bind(label: &str) -> Self {
        let path = PathBuf::from(format!(
            "target/pinker-tree-ready-{label}-{}.sock",
            fixture_id()
        ));
        let _ = fs::remove_file(&path);
        let socket = UnixDatagram::bind(&path).expect("canal exclusivo da árvore convidada");
        Self { socket, path }
    }

    fn wait_until(&self, deadline: Instant, lifecycle: &LifecycleReceiver) -> GuestTreeReady {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            lifecycle.preserve_timeout("guest_tree_ready");
            panic!(
                "prazo operacional expirou antes de guest_tree_ready; último={:?}",
                lifecycle.records.last()
            );
        }
        self.socket
            .set_read_timeout(Some(remaining))
            .expect("timeout da árvore convidada");
        let mut bytes = [0_u8; 256];
        let size = match self.socket.recv(&mut bytes) {
            Ok(size) => size,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                lifecycle.preserve_timeout("guest_tree_ready");
                panic!(
                    "timeout aguardando guest_tree_ready; último={:?}",
                    lifecycle.records.last()
                );
            }
            Err(error) => panic!("canal guest_tree_ready falhou: {error}"),
        };
        let wire = std::str::from_utf8(&bytes[..size]).expect("guest_tree_ready UTF-8");
        let fields = wire
            .split('|')
            .map(|field| field.parse::<u64>().expect("campo guest_tree_ready"))
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 4, "guest_tree_ready inválido: {wire}");
        GuestTreeReady {
            child: ProcessIdentitySnapshot {
                pid: u32::try_from(fields[0]).expect("PID do convidado"),
                start_time: fields[1],
                pgid: None,
            },
            grandchild: ProcessIdentitySnapshot {
                pid: u32::try_from(fields[2]).expect("PID do neto"),
                start_time: fields[3],
                pgid: None,
            },
        }
    }
}

impl Drop for GuestTreeReceiver {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

struct SocketPathGuard(PathBuf);

impl Drop for SocketPathGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

impl Drop for LifecycleReceiver {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        if std::thread::panicking() {
            let directory = PathBuf::from(format!(
                "target/pinker-flake-evidence/lifecycle-panic-{}",
                fixture_id()
            ));
            if fs::create_dir_all(&directory).is_ok() {
                let _ = fs::rename(&self.journal, directory.join("events.txt"));
            }
        } else {
            let _ = fs::remove_file(&self.journal);
        }
    }
}

#[test]
fn supervisor_fecha_descritores_gravaveis_herdados() {
    let output = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "fd_allowlist_entry", "--ignored", "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("helper isolado da tabela global de descritores");
    assert!(
        output.status.success(),
        "regressão ETXTBSY falhou: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "ponto de reexecução isolado para allowlist de descritores"]
fn fd_allowlist_entry() {
    let artifacts = NativeArtifactDir::create().expect("diretório marcado");
    let executable = artifacts.path().join("fd-allowlist-executable");
    fs::copy("/bin/true", &executable).expect("copia executável de regressão");
    let writer = OpenOptions::new()
        .write(true)
        .open(&executable)
        .expect("abre descritor gravável antes do fork");

    let inherited_fd = writer.as_raw_fd();
    let probe = LifecycleProbe::default();
    let mut gate = GuestGateGuard::hold(&probe);
    let runner_probe = probe.clone();
    let runner = std::thread::spawn(move || {
        ControlledCommand::new("true")
            .lifecycle_probe_for_test(&runner_probe)
            .output()
    });
    let launcher = match probe
        .wait_for_test(
            "launcher_ready para higiene de fd",
            Duration::from_secs(5),
            |event| matches!(event, LifecycleEvent::LauncherReady(_)),
        )
        .expect("launcher pronto")
        .event
    {
        LifecycleEvent::LauncherReady(identity) => identity,
        _ => unreachable!(),
    };
    let watchdog = match probe
        .wait_for_test(
            "watchdog_ready para higiene de fd",
            Duration::from_secs(5),
            |event| matches!(event, LifecycleEvent::WatchdogReady(_)),
        )
        .expect("watchdog pronto")
        .event
    {
        LifecycleEvent::WatchdogReady(identity) => identity,
        _ => unreachable!(),
    };
    let launcher_inherited = PathBuf::from(format!("/proc/{}/fd/{inherited_fd}", launcher.pid));
    assert_eq!(
        fs::read_link(&launcher_inherited)
            .err()
            .map(|error| error.kind()),
        Some(std::io::ErrorKind::NotFound),
        "launcher conservou o descritor gravável {inherited_fd}: {launcher_inherited:?}"
    );
    assert!(
        common::native_process::watchdog_fd_allowlist_probe(inherited_fd)
            .expect("prova interna da allowlist do watchdog"),
        "watchdog conservou o descritor gravável {inherited_fd}; identidade={watchdog:?}"
    );

    // A reprodução é determinística: o pai fecha sua cópia enquanto launcher
    // e watchdog permanecem vivos e o gate impede o fork do convidado. Na
    // implementação antiga, o launcher ainda possuía a abertura gravável e o
    // exec abaixo retornava ETXTBSY.
    drop(writer);
    let concurrent_exec = RawCommand::new(&executable)
        .status()
        .expect("allowlist precisa eliminar ETXTBSY");
    assert!(concurrent_exec.success());
    gate.release();
    let output = runner
        .join()
        .expect("thread da execução controlada")
        .expect("execução após liberação do gate");
    assert!(output.status.success());
}

struct FakeRepo {
    root: PathBuf,
}

impl FakeRepo {
    fn new() -> Self {
        let id = fixture_id();
        let root = std::env::current_dir()
            .expect("cwd")
            .join("target/pinker-host-fixtures")
            .join(format!("repo-{}-{id}", std::process::id()));
        fs::create_dir_all(root.join(".git")).expect("cria fake repo");
        fs::create_dir_all(root.join("scripts")).expect("cria scripts fake");
        fs::copy(
            "scripts/pinker-cleanup.sh",
            root.join("scripts/pinker-cleanup.sh"),
        )
        .expect("copia cleanup para fixture");
        Self { root }
    }

    fn command(&self, script: &str) -> ControlledCommand {
        let mut command = ControlledCommand::new("sh");
        command
            .args(["-c", script])
            .execution_repo_root_for_test(&self.root);
        command
    }

    fn controlled(&self, script: &str) -> std::io::Result<std::process::Output> {
        self.command(script).output()
    }

    fn tree(&self) -> Vec<String> {
        fn visit(base: &Path, path: &Path, out: &mut Vec<String>) {
            let Ok(entries) = fs::read_dir(path) else {
                return;
            };
            let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let relative = entry
                    .path()
                    .strip_prefix(base)
                    .expect("prefixo")
                    .to_string_lossy()
                    .into_owned();
                out.push(relative);
                if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    visit(base, &entry.path(), out);
                }
            }
        }
        let mut out = Vec::new();
        visit(&self.root, &self.root, &mut out);
        out
    }
}

#[test]
fn fixture_de_sandbox_preserva_timeout_operacional() {
    let repo = FakeRepo::new();
    let mut command = repo.command("printf fixture-ready");
    let (operational, override_timeout) = command.timeout_contract_for_test();
    assert_eq!(operational, Duration::from_secs(20));
    assert_eq!(
        override_timeout, None,
        "fixture não pode impor prazo próprio"
    );

    let probe = LifecycleProbe::default();
    command.lifecycle_probe_for_test(&probe);
    let output = command.output().expect("fixture usa contrato operacional");
    assert_eq!(output.stdout, b"fixture-ready");
    assert!(probe.events().iter().any(|event| matches!(
        event,
        LifecycleEvent::PrimaryReasonLatched(TerminationReason::GuestExited)
    )));
}

impl Drop for FakeRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn execucao_controlada_aplica_core_zero_e_remove_sandbox() {
    let repo = FakeRepo::new();
    let mut command = ControlledCommand::new("sh");
    command
        .args(["-c", "ulimit -c; printf ok"])
        .logical_case("core-zero")
        .execution_repo_root_for_test(&repo.root);
    let output = command.output().expect("execução controlada");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\nok");
    assert_eq!(execution_dirs_at(&repo.root), 0);
}

#[test]
fn raiz_real_ausente_existente_e_segunda_execucao_sao_idempotentes() {
    let repo = FakeRepo::new();
    assert!(!repo.root.join("target").exists());
    assert!(repo
        .controlled("printf first")
        .expect("primeira")
        .status
        .success());
    let first = repo.tree();
    assert!(repo.root.join("target/pinker-exec").is_dir());
    assert!(repo
        .controlled("printf second")
        .expect("segunda")
        .status
        .success());
    assert_eq!(
        repo.tree(),
        first,
        "segunda execução alterou a árvore vazia"
    );
}

#[test]
fn target_symlink_bloqueia_antes_de_entregar_ambiente_ao_filho() {
    let repo = FakeRepo::new();
    let outside = repo.root.join("outside-target");
    fs::create_dir(&outside).expect("outside");
    fs::write(outside.join("sentinel"), "preservar").expect("sentinela");
    symlink(
        outside.canonicalize().expect("canonical outside"),
        repo.root.join("target"),
    )
    .expect("target symlink");
    let evidence = outside.join("child-ran");
    let error = ControlledCommand::new("sh")
        .args([
            "-c",
            "printf '%s|%s' \"$TMPDIR\" \"$PINKER_EXECUTION_DIR\" > \"$1\"",
            "sh",
        ])
        .arg(&evidence)
        .env("TMPDIR", "/attacker/tmp")
        .env("PINKER_EXECUTION_DIR", "/attacker/exec")
        .execution_repo_root_for_test(&repo.root)
        .output()
        .expect_err("target symlink precisa bloquear");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        !evidence.exists(),
        "filho recebeu ambiente apesar da raiz inválida"
    );
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "preservar"
    );
}

#[test]
fn execution_root_symlink_e_entradas_symlink_nunca_escapam() {
    let repo = FakeRepo::new();
    let outside = repo.root.join("outside-exec");
    fs::create_dir_all(repo.root.join("target")).expect("target");
    fs::create_dir(&outside).expect("outside");
    fs::write(outside.join("sentinel"), "preservar").expect("sentinela");
    symlink(
        outside.canonicalize().expect("canonical outside"),
        repo.root.join("target/pinker-exec"),
    )
    .expect("execution root symlink");
    assert!(repo.controlled("printf never").is_err());
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "preservar"
    );

    fs::remove_file(repo.root.join("target/pinker-exec")).expect("remove link");
    assert!(repo
        .controlled("true")
        .expect("cria root real")
        .status
        .success());
    let root = repo.root.join("target/pinker-exec");
    let escaped = root.join("exec-999999-1");
    symlink(outside.canonicalize().unwrap(), &escaped).expect("entrada symlink");
    let marker_link_dir = root.join("exec-999999-2");
    fs::create_dir(&marker_link_dir).expect("dir marker");
    symlink(
        outside.join("sentinel"),
        marker_link_dir.join("owner.marker"),
    )
    .expect("marker symlink");
    assert!(repo.controlled("true").expect("scavenger").status.success());
    assert!(escaped.is_symlink());
    assert!(marker_link_dir.join("owner.marker").is_symlink());
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "preservar"
    );
}

#[test]
fn troca_da_raiz_antes_do_cleanup_falha_fechada_e_preserva_conteudo() {
    let repo = FakeRepo::new();
    let sentinel = repo.root.join("external-sentinel");
    fs::write(&sentinel, "preservar").expect("sentinela");
    let id = fixture_id();
    let controller_path = PathBuf::from(format!("target/root-swap-controller-{id}.sock"));
    let guest_path = PathBuf::from(format!("target/root-swap-guest-{id}.sock"));
    let _controller_guard = SocketPathGuard(controller_path.clone());
    let _guest_guard = SocketPathGuard(guest_path.clone());
    let controller_socket = UnixDatagram::bind(&controller_path).expect("canal root-swap");
    controller_socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout root-swap");
    let probe = LifecycleProbe::default();
    let runner_probe = probe.clone();
    let repo_root = repo.root.clone();
    let runner = std::thread::spawn(move || {
        ControlledCommand::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "root_swap_entry", "--ignored", "--nocapture"])
            .env("PINKER_ROOT_SWAP_CONTROLLER", controller_path)
            .env("PINKER_ROOT_SWAP_GUEST", guest_path)
            .lifecycle_probe_for_test(&runner_probe)
            .execution_repo_root_for_test(repo_root)
            .output()
    });
    let mut ready = [0_u8; 16];
    let (size, guest_address) = controller_socket
        .recv_from(&mut ready)
        .expect("guest publica prontidão para root-swap");
    assert_eq!(&ready[..size], b"ready");
    probe
        .wait_for_test(
            "sandbox_running antes de root-swap",
            Duration::from_secs(5),
            |event| matches!(event, LifecycleEvent::SandboxRunning),
        )
        .expect("marker running concluído antes da troca");
    controller_socket
        .send_to(
            b"go",
            guest_address
                .as_pathname()
                .expect("endereço guest root-swap"),
        )
        .expect("libera root-swap após startup");
    let error = runner
        .join()
        .expect("thread root-swap")
        .expect_err("troca de inode precisa invalidar cleanup");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preservar");
    assert!(repo.root.join("target/pinker-exec.saved").is_dir());
}

#[test]
#[ignore = "ponto de reexecução para troca sincronizada da raiz"]
fn root_swap_entry() {
    let Some(controller_path) = std::env::var_os("PINKER_ROOT_SWAP_CONTROLLER") else {
        return;
    };
    let Some(guest_path) = std::env::var_os("PINKER_ROOT_SWAP_GUEST") else {
        return;
    };
    let socket = UnixDatagram::bind(guest_path).expect("canal guest root-swap");
    socket
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("prazo operacional root-swap");
    socket
        .send_to(b"ready", Path::new(&controller_path))
        .expect("publica prontidão root-swap");
    let mut release = [0_u8; 2];
    let size = socket
        .recv(&mut release)
        .expect("aguarda liberação root-swap");
    assert_eq!(&release[..size], b"go");
    let execution_dir =
        PathBuf::from(std::env::var_os("PINKER_EXECUTION_DIR").expect("sandbox da execução"));
    let root = execution_dir.parent().expect("raiz de execução");
    let saved = root.with_file_name("pinker-exec.saved");
    fs::rename(root, saved).expect("substitui raiz de execução");
    fs::create_dir(root).expect("recria raiz com outra identidade");
    print!("changed");
}

#[test]
fn output_stdin_default_eof_e_configuracoes_explicitas() {
    let output = ControlledCommand::new("sh")
        .args([
            "-c",
            "if IFS= read -r line; then printf read:%s \"$line\"; else printf eof; fi",
        ])
        .output()
        .expect("output com stdin default");
    assert_eq!(output.stdout, b"eof");

    let (mut writer, reader) = UnixStream::pair().expect("pipe");
    writer.write_all(b"pinker-pipe").expect("escreve pipe");
    writer
        .shutdown(std::net::Shutdown::Write)
        .expect("fecha writer");
    let reader: OwnedFd = reader.into();
    let piped = ControlledCommand::new("cat")
        .stdin(Stdio::from(reader))
        .output()
        .expect("stdin explícito");
    assert_eq!(piped.stdout, b"pinker-pipe");

    let null = ControlledCommand::new("cat")
        .stdin(Stdio::null())
        .output()
        .expect("stdin null explícito");
    assert!(null.stdout.is_empty());
}

#[test]
fn output_nao_consome_pipe_do_controlador() {
    let (mut writer, reader) = UnixStream::pair().expect("pipe externo");
    writer.write_all(b"parent-input").expect("escreve");
    writer.shutdown(std::net::Shutdown::Write).expect("fecha");
    let reader: OwnedFd = reader.into();
    let output = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "stdio_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_STDIO_HELPER", "no-consume")
        .stdin(Stdio::from(reader))
        .output()
        .expect("helper stdio");
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("remaining=parent-input"),
        "stdin do controlador foi consumido: {output:?}"
    );
}

#[test]
fn status_herda_stdout_e_stderr_sem_captura_oculta() {
    let output = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "stdio_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_STDIO_HELPER", "status")
        .stdin(Stdio::null())
        .output()
        .expect("helper status");
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("status-out"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("status-err"));
}

#[test]
#[ignore = "ponto de reexecução para evidência de stdio"]
fn stdio_controller_entry() {
    match std::env::var("PINKER_STDIO_HELPER").as_deref() {
        Ok("no-consume") => {
            let child = ControlledCommand::new("sh")
                .args(["-c", "IFS= read -r ignored || true"])
                .output()
                .expect("filho output");
            assert!(child.status.success());
            let mut remaining = String::new();
            std::io::stdin()
                .read_to_string(&mut remaining)
                .expect("lê stdin remanescente");
            print!("remaining={remaining}");
        }
        Ok("status") => {
            let status = ControlledCommand::new("sh")
                .args(["-c", "printf status-out; printf status-err >&2"])
                .status()
                .expect("status controlado");
            assert!(status.success());
        }
        _ => {}
    }
}

#[test]
fn timeout_e_excesso_de_saida_finalizam_filho_e_neto() {
    let repo = FakeRepo::new();
    for (case, script) in [
        (
            "timeout-tree",
            "sleep 60 & grand=$!; printf '%s' \"$grand\" > \"$1\"; wait",
        ),
        (
            "stdout-tree",
            "sleep 60 & grand=$!; printf '%s' \"$grand\" > \"$1\"; yes pinker",
        ),
        (
            "stderr-tree",
            "sleep 60 & grand=$!; printf '%s' \"$grand\" > \"$1\"; yes pinker >&2",
        ),
    ] {
        let pid_file = PathBuf::from(format!("target/{case}-{}.pid", std::process::id()));
        let _ = fs::remove_file(&pid_file);
        let mut command = ControlledCommand::new("sh");
        command
            .args(["-c", script, "sh"])
            .arg(&pid_file)
            .logical_case(case)
            .timeout(if case == "timeout-tree" {
                Duration::from_millis(250)
            } else {
                Duration::from_secs(5)
            })
            .capture_limit(4096)
            .execution_repo_root_for_test(&repo.root);
        let error = command
            .output()
            .expect_err("limite precisa encerrar árvore");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        let expected_reason = if case == "timeout-tree" {
            "timeout"
        } else if case == "stdout-tree" {
            "stdout_limit"
        } else {
            "stderr_limit"
        };
        assert!(error.to_string().contains(expected_reason), "{error}");
        let pid: u32 = fs::read_to_string(&pid_file)
            .expect("PID do neto")
            .parse()
            .expect("PID numérico");
        let _ = fs::remove_file(&pid_file);
        wait_process_dead(pid);
    }
    let mut healthy_command = ControlledCommand::new("sh");
    healthy_command
        .args(["-c", "printf healthy"])
        .execution_repo_root_for_test(&repo.root);
    let healthy = healthy_command
        .output()
        .expect("execução saudável posterior");
    assert_eq!(healthy.stdout, b"healthy");
    assert_eq!(execution_dirs_at(&repo.root), 0, "sandbox após erro");
}

#[test]
fn captura_no_limite_preserva_prefixo_sem_crescimento() {
    let stdout = ControlledCommand::new("sh")
        .args(["-c", "printf 12345678"])
        .capture_limit(8)
        .output()
        .expect("captura no teto");
    assert_eq!(stdout.stdout, b"12345678");
    let stderr = ControlledCommand::new("sh")
        .args(["-c", "printf abcdefgh >&2"])
        .capture_limit(8)
        .output()
        .expect("stderr no teto");
    assert_eq!(stderr.stderr, b"abcdefgh");
}

#[test]
fn controlador_sigkill_encerra_arvore_e_sandbox_e_recuperavel() {
    const OPERATIONAL_DEADLINE: Duration = Duration::from_secs(60);
    let repo = FakeRepo::new();
    let id = fixture_id();
    let child_file = PathBuf::from(format!("target/controller-child-{id}.pid"));
    let grand_file = PathBuf::from(format!("target/controller-grand-{id}.pid"));
    let _ = fs::remove_file(&child_file);
    let _ = fs::remove_file(&grand_file);
    let mut lifecycle = LifecycleReceiver::bind("controller-tree");
    let tree_ready = GuestTreeReceiver::bind("controller-death");
    let deadline = Instant::now() + OPERATIONAL_DEADLINE;
    let controller = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "watchdog_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_WATCHDOG_CHILD", &child_file)
        .env("PINKER_WATCHDOG_GRAND", &grand_file)
        .env("PINKER_WATCHDOG_REPO_ROOT", &repo.root)
        .env("PINKER_LIFECYCLE_SOCKET", &lifecycle.path)
        .env("PINKER_TREE_READY_SOCKET", &tree_ready.path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("controlador");
    let controller = VerifiedChild::new(controller);
    lifecycle.wait_for_until("launcher_ready", deadline, |event| {
        matches!(event, LifecycleEvent::LauncherReady(_))
    });
    lifecycle.wait_for_until("watchdog_ready", deadline, |event| {
        matches!(event, LifecycleEvent::WatchdogReady(_))
    });
    let guest = match lifecycle
        .wait_for_until("guest_started", deadline, |event| {
            matches!(event, LifecycleEvent::GuestStarted(_))
        })
        .event
    {
        LifecycleEvent::GuestStarted(identity) => identity,
        _ => unreachable!(),
    };
    let tree = tree_ready.wait_until(deadline, &lifecycle);
    assert_eq!(guest.pid, tree.child.pid);
    assert_eq!(guest.start_time, tree.child.start_time);
    assert_eq!(
        process_start_time(tree.child.pid),
        Some(tree.child.start_time)
    );
    assert_eq!(
        process_start_time(tree.grandchild.pid),
        Some(tree.grandchild.start_time)
    );
    let child_pid = tree.child.pid;
    let grand_pid = tree.grandchild.pid;
    assert_eq!(
        fs::read_to_string(&child_file).unwrap(),
        child_pid.to_string()
    );
    assert_eq!(
        fs::read_to_string(&grand_file).unwrap(),
        grand_pid.to_string()
    );
    let _ = controller.kill_and_wait();
    wait_process_dead(child_pid);
    wait_process_dead(grand_pid);

    let cleanup = RawCommand::new("bash")
        .args(["scripts/pinker-cleanup.sh", "--apply", "--older-than", "0"])
        .current_dir(&repo.root)
        .output()
        .expect("recupera sandbox");
    assert!(cleanup.status.success(), "{cleanup:?}");
    let _ = fs::remove_file(child_file);
    let _ = fs::remove_file(grand_file);
}

#[test]
#[ignore = "ponto de reexecução para watchdog"]
fn watchdog_controller_entry() {
    let Some(child_file) = std::env::var_os("PINKER_WATCHDOG_CHILD") else {
        return;
    };
    let Some(grand_file) = std::env::var_os("PINKER_WATCHDOG_GRAND") else {
        return;
    };
    let Some(repo_root) = std::env::var_os("PINKER_WATCHDOG_REPO_ROOT") else {
        return;
    };
    let Some(lifecycle_socket) = std::env::var_os("PINKER_LIFECYCLE_SOCKET") else {
        return;
    };
    let Some(tree_ready_socket) = std::env::var_os("PINKER_TREE_READY_SOCKET") else {
        return;
    };
    let probe = LifecycleProbe::connected_to_for_test(Path::new(&lifecycle_socket))
        .expect("canal lifecycle");
    let mut command = ControlledCommand::new(std::env::current_exe().expect("test binary"));
    command
        .args(["--exact", "guest_tree_entry", "--ignored", "--nocapture"])
        .env("PINKER_TREE_CHILD", child_file)
        .env("PINKER_TREE_GRAND", grand_file)
        .env("PINKER_TREE_READY_SOCKET", tree_ready_socket)
        .timeout(Duration::from_secs(60))
        .lifecycle_probe_for_test(&probe)
        .execution_repo_root_for_test(repo_root);
    let _ = command.output();
}

#[test]
#[ignore = "ponto de reexecução para handshake da árvore convidada"]
fn guest_tree_entry() {
    let Some(child_file) = std::env::var_os("PINKER_TREE_CHILD") else {
        return;
    };
    let Some(grand_file) = std::env::var_os("PINKER_TREE_GRAND") else {
        return;
    };
    let Some(ready_socket) = std::env::var_os("PINKER_TREE_READY_SOCKET") else {
        return;
    };
    let child_pid = std::process::id();
    let child_start = process_start_time(child_pid).expect("identidade do convidado");
    let mut grandchild = RawCommand::new("sleep")
        .arg("60")
        .spawn()
        .expect("neto da árvore convidada");
    let grandchild_pid = grandchild.id();
    let grandchild_start = process_start_time(grandchild_pid).expect("identidade do neto");
    fs::write(child_file, child_pid.to_string()).expect("publica PID do convidado");
    fs::write(grand_file, grandchild_pid.to_string()).expect("publica PID do neto");
    let ready = UnixDatagram::unbound().expect("datagrama guest_tree_ready");
    ready
        .send_to(
            format!("{child_pid}|{child_start}|{grandchild_pid}|{grandchild_start}").as_bytes(),
            Path::new(&ready_socket),
        )
        .expect("publica guest_tree_ready");
    let _ = grandchild.wait();
}

#[derive(Debug)]
struct OutcomeReport {
    primary: TerminationReason,
    secondary_count: usize,
    launcher_pid: u32,
    launcher_start_time: u64,
    watchdog_pid: u32,
    watchdog_start_time: u64,
    tree_shutdown_proven: bool,
    sandbox_removed: bool,
}

fn parse_outcome_report(text: &str) -> OutcomeReport {
    let fields = text
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| *key != "secondary")
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<BTreeMap<_, _>>();
    let field = |name: &str| {
        fields
            .get(name)
            .unwrap_or_else(|| panic!("campo {name} ausente no outcome"))
            .as_str()
    };
    assert_eq!(field("schema"), "1");
    OutcomeReport {
        primary: TerminationReason::parse(field("primary")).expect("causa tipada"),
        secondary_count: field("secondary_count").parse().expect("secondary_count"),
        launcher_pid: field("launcher_pid").parse().expect("launcher_pid"),
        launcher_start_time: field("launcher_start_time")
            .parse()
            .expect("launcher_start_time"),
        watchdog_pid: field("watchdog_pid").parse().expect("watchdog_pid"),
        watchdog_start_time: field("watchdog_start_time")
            .parse()
            .expect("watchdog_start_time"),
        tree_shutdown_proven: field("tree_shutdown_proven")
            .parse()
            .expect("tree_shutdown_proven"),
        sandbox_removed: field("sandbox") == "removed",
    }
}

fn publish_outcome_atomically(path: &Path, report: &str) {
    let temporary = path.with_extension(format!("tmp-{}", fixture_id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .expect("temporário exclusivo do outcome");
    file.write_all(report.as_bytes()).expect("escreve outcome");
    file.sync_all().expect("sincroniza outcome");
    fs::rename(&temporary, path).expect("publica outcome atomicamente");
}

#[test]
fn publicacao_atomica_elimina_janela_de_resultado_parcial() {
    let id = fixture_id();
    let old_path = PathBuf::from(format!("target/old-result-race-{id}"));
    let new_path = PathBuf::from(format!("target/atomic-result-{id}"));
    let (created_tx, created_rx) = std::sync::mpsc::sync_channel(0);
    let (write_tx, write_rx) = std::sync::mpsc::sync_channel(0);
    let old_for_writer = old_path.clone();
    let writer = std::thread::spawn(move || {
        let mut file = fs::File::create(&old_for_writer).expect("cria resultado antigo");
        created_tx.send(()).expect("publica criação");
        write_rx.recv().expect("libera escrita");
        file.write_all(b"primary=watchdog_exit\n")
            .expect("escreve resultado antigo");
    });
    created_rx.recv().expect("observa criação antiga");
    assert!(old_path.exists());
    assert!(
        fs::read(&old_path).expect("lê janela antiga").is_empty(),
        "a regressão precisa provar a janela do fs::write antigo"
    );
    write_tx.send(()).expect("libera escritor");
    writer.join().expect("writer antigo");

    publish_outcome_atomically(&new_path, "schema=1\nprimary=watchdog_exit\n");
    assert_eq!(
        fs::read_to_string(&new_path).expect("resultado atômico"),
        "schema=1\nprimary=watchdog_exit\n"
    );
    let _ = fs::remove_file(old_path);
    let _ = fs::remove_file(new_path);
}

#[test]
fn supervisor_morto_e_detectado_e_arvore_terminada() {
    const OPERATIONAL_DEADLINE: Duration = Duration::from_secs(60);
    let id = fixture_id();
    let child_file = PathBuf::from(format!("target/supervisor-child-{id}.pid"));
    let grand_file = PathBuf::from(format!("target/supervisor-grand-{id}.pid"));
    let result_file = PathBuf::from(format!("target/supervisor-result-{id}"));
    let _ = fs::remove_file(&child_file);
    let _ = fs::remove_file(&grand_file);
    let _ = fs::remove_file(&result_file);
    let mut lifecycle = LifecycleReceiver::bind("watchdog-death");
    let tree_ready = GuestTreeReceiver::bind("watchdog-death");
    let deadline = Instant::now() + OPERATIONAL_DEADLINE;
    let controller = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "supervisor_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_SUPERVISOR_CHILD", &child_file)
        .env("PINKER_SUPERVISOR_GRAND", &grand_file)
        .env("PINKER_SUPERVISOR_RESULT", &result_file)
        .env("PINKER_LIFECYCLE_SOCKET", &lifecycle.path)
        .env("PINKER_TREE_READY_SOCKET", &tree_ready.path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("controlador");
    let controller = VerifiedChild::new(controller);
    let launcher = match lifecycle
        .wait_for_until("launcher_ready", deadline, |event| {
            matches!(event, LifecycleEvent::LauncherReady(_))
        })
        .event
    {
        LifecycleEvent::LauncherReady(identity) => identity,
        _ => unreachable!(),
    };
    let watchdog = match lifecycle
        .wait_for_until("watchdog_ready", deadline, |event| {
            matches!(event, LifecycleEvent::WatchdogReady(_))
        })
        .event
    {
        LifecycleEvent::WatchdogReady(identity) => identity,
        _ => unreachable!(),
    };
    assert_eq!(
        process_start_time(watchdog.pid),
        Some(watchdog.start_time),
        "identidade do watchdog antes do SIGKILL"
    );
    let guest = match lifecycle
        .wait_for_until("guest_started", deadline, |event| {
            matches!(event, LifecycleEvent::GuestStarted(_))
        })
        .event
    {
        LifecycleEvent::GuestStarted(identity) => identity,
        _ => unreachable!(),
    };
    let tree = tree_ready.wait_until(deadline, &lifecycle);
    assert_eq!(guest.pid, tree.child.pid);
    assert_eq!(guest.start_time, tree.child.start_time);
    assert_eq!(
        process_start_time(tree.child.pid),
        Some(tree.child.start_time),
        "convidado não está vivo"
    );
    assert_eq!(
        process_start_time(tree.grandchild.pid),
        Some(tree.grandchild.start_time),
        "neto não está vivo"
    );
    let child_pid = tree.child.pid;
    let grand_pid = tree.grandchild.pid;
    kill_process_verified(watchdog.pid, watchdog.start_time, 9);
    let observed = lifecycle.wait_for_until("watchdog_exit_observed", deadline, |event| {
        matches!(event, LifecycleEvent::WatchdogExitObserved(_))
    });
    assert_eq!(
        observed.event,
        LifecycleEvent::WatchdogExitObserved(watchdog)
    );
    let reason = lifecycle.wait_for_until("primary_reason_latched", deadline, |event| {
        matches!(
            event,
            LifecycleEvent::PrimaryReasonLatched(TerminationReason::WatchdogExit)
        )
    });
    assert_eq!(
        reason.event,
        LifecycleEvent::PrimaryReasonLatched(TerminationReason::WatchdogExit)
    );
    lifecycle.wait_for_until("term_requested", deadline, |event| {
        matches!(event, LifecycleEvent::TermRequested)
    });
    lifecycle.wait_for_until("term_sent", deadline, |event| {
        matches!(event, LifecycleEvent::TermSent)
    });
    lifecycle.wait_for_until("launcher_reaped", deadline, |event| {
        matches!(event, LifecycleEvent::LauncherReaped)
    });
    lifecycle.wait_for_until("sandbox_removed", deadline, |event| {
        matches!(event, LifecycleEvent::SandboxRemoved)
    });
    lifecycle.wait_for_until("result_published", deadline, |event| {
        matches!(event, LifecycleEvent::ResultPublished)
    });
    assert!(
        lifecycle
            .records
            .iter()
            .any(|record| matches!(record.event, LifecycleEvent::WatchdogReaped)),
        "watchdog não foi reaped"
    );
    let controller_output = controller.wait_with_output();
    assert!(
        controller_output.status.success(),
        "controlador falhou: stdout={} stderr={}",
        String::from_utf8_lossy(&controller_output.stdout),
        String::from_utf8_lossy(&controller_output.stderr)
    );
    wait_process_dead(child_pid);
    wait_process_dead(grand_pid);
    let outcome = parse_outcome_report(&fs::read_to_string(&result_file).expect("outcome"));
    assert_eq!(outcome.primary, TerminationReason::WatchdogExit);
    assert_eq!(outcome.secondary_count, 0);
    assert_eq!(outcome.launcher_pid, launcher.pid);
    assert_eq!(outcome.launcher_start_time, launcher.start_time);
    assert_eq!(outcome.watchdog_pid, watchdog.pid);
    assert_eq!(outcome.watchdog_start_time, watchdog.start_time);
    assert!(outcome.tree_shutdown_proven);
    assert!(outcome.sandbox_removed);
    let _ = fs::remove_file(child_file);
    let _ = fs::remove_file(grand_file);
    let _ = fs::remove_file(result_file);
}

#[test]
#[ignore = "ponto de reexecução para morte do supervisor"]
fn supervisor_controller_entry() {
    let Some(child_file) = std::env::var_os("PINKER_SUPERVISOR_CHILD") else {
        return;
    };
    let Some(result_file) = std::env::var_os("PINKER_SUPERVISOR_RESULT") else {
        return;
    };
    let Some(grand_file) = std::env::var_os("PINKER_SUPERVISOR_GRAND") else {
        return;
    };
    let Some(lifecycle_socket) = std::env::var_os("PINKER_LIFECYCLE_SOCKET") else {
        return;
    };
    let Some(tree_ready_socket) = std::env::var_os("PINKER_TREE_READY_SOCKET") else {
        return;
    };
    let probe = LifecycleProbe::connected_to_for_test(Path::new(&lifecycle_socket))
        .expect("canal lifecycle");
    let outcome = ControlledCommand::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "guest_tree_entry", "--ignored", "--nocapture"])
        .env("PINKER_TREE_CHILD", child_file)
        .env("PINKER_TREE_GRAND", grand_file)
        .env("PINKER_TREE_READY_SOCKET", tree_ready_socket)
        .timeout(Duration::from_secs(60))
        .lifecycle_probe_for_test(&probe)
        .outcome_for_test()
        .expect("outcome estruturado");
    publish_outcome_atomically(Path::new(&result_file), &outcome.report());
    probe.record_for_test(LifecycleEvent::ResultPublished);
}

#[test]
fn proc_stat_comm_complexo_usa_starttime_real() {
    let ready = PathBuf::from(format!("target/proc-comm-ready-{}", fixture_id()));
    let _ = fs::remove_file(&ready);
    let mut process = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "proc_comm_helper_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_PROC_READY", &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("helper comm");
    wait_for_file(&ready);
    let pid = process.id();
    let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
        .expect("comm")
        .trim_end()
        .to_string();
    assert_eq!(comm, "pi nker) x");
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).expect("stat");
    let prefix = format!("{pid} ({comm}) ");
    let suffix = stat
        .strip_prefix(&prefix)
        .expect("autoridade independente delimitou comm conhecido");
    let expected: u64 = suffix
        .split_ascii_whitespace()
        .nth(19)
        .expect("starttime independente")
        .parse()
        .expect("numérico");
    assert_eq!(process_start_time(pid), Some(expected));
    assert_eq!(process_identity(pid, expected), ProcessIdentity::Live);
    assert_eq!(process_identity(pid, expected + 1), ProcessIdentity::Reused);
    kill_process_verified(pid, expected, 9);
    let _ = process.wait();
    wait_process_dead(pid);
    let _ = fs::remove_file(ready);
}

#[test]
#[ignore = "ponto de reexecução para comm complexo"]
fn proc_comm_helper_entry() {
    let Some(ready) = std::env::var_os("PINKER_PROC_READY") else {
        return;
    };
    fs::write(
        format!("/proc/self/task/{}/comm", std::process::id()),
        "pi nker) x",
    )
    .expect("define comm do líder via procfs");
    fs::write(ready, "ready").expect("ready");
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn parser_proc_rejeita_truncado_nao_numerico_e_ambiguidade() {
    use common::native_process::parse_proc_stat_start_time;
    assert_eq!(parse_proc_stat_start_time("1 (pi nker) x) S 1 2"), None);
    let mut fields = vec!["S"; 20];
    fields[19] = "not-a-number";
    assert_eq!(
        parse_proc_stat_start_time(&format!("1 (pi nker) x) {}", fields.join(" "))),
        None
    );
    fields[19] = "4242";
    assert_eq!(
        parse_proc_stat_start_time(&format!("1 (pi nker) x) {}", fields.join(" "))),
        Some(4242)
    );
    let authority = fs::read_to_string("tests/common/native_process.rs").expect("autoridade");
    assert!(
        authority.contains("None => ProcessIdentity::Unknown"),
        "parse inválido nunca pode significar owner morto"
    );
}

#[test]
fn cem_execucoes_pequenas_nao_acumulam_filhos_nem_temporarios() {
    let repo = FakeRepo::new();
    let children_before = direct_children();
    for case in 0..100 {
        let mut command = ControlledCommand::new("true");
        command
            .logical_case(&format!("stress-pequeno-{case}"))
            .execution_repo_root_for_test(&repo.root);
        let output = command.output().expect("execução pequena controlada");
        assert!(output.status.success());
    }
    assert_eq!(execution_dirs_at(&repo.root), 0);
    assert_eq!(direct_children(), children_before, "supervisor não reaped");
}

#[test]
fn falhas_injetadas_antes_do_gate_nunca_iniciam_convidado_e_sao_reaped() {
    let repo = FakeRepo::new();
    let before_children = direct_children();
    let cases = [
        StartupFailurePoint::BeforeLauncher,
        StartupFailurePoint::AfterLauncherBeforeReady,
        StartupFailurePoint::AfterLauncherReady,
        StartupFailurePoint::BeforeWatchdog,
        StartupFailurePoint::WatchdogPipe,
        StartupFailurePoint::WatchdogFork,
        StartupFailurePoint::AfterWatchdogBeforeReady,
        StartupFailurePoint::AfterWatchdogReadyBeforeGate,
    ];
    for (index, point) in cases.into_iter().enumerate() {
        let evidence = PathBuf::from(format!(
            "target/startup-gate-evidence-{}-{index}",
            std::process::id()
        ));
        let _ = fs::remove_file(&evidence);
        let probe = LifecycleProbe::default();
        let error = ControlledCommand::new("sh")
            .args(["-c", "printf guest > \"$1\"", "sh"])
            .arg(&evidence)
            .startup_failure_for_test(point)
            .lifecycle_probe_for_test(&probe)
            .execution_repo_root_for_test(&repo.root)
            .output()
            .expect_err("falha injetada precisa fechar a inicialização");
        assert!(!error.to_string().is_empty());
        assert!(!evidence.exists(), "{point:?} iniciou o convidado");
        let events = probe.events();
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, LifecycleEvent::GuestStarted(_))),
            "{point:?} liberou gate: {events:?}"
        );
        if point != StartupFailurePoint::BeforeLauncher {
            assert!(events
                .iter()
                .any(|event| matches!(event, LifecycleEvent::LauncherReaped)));
        }
        if matches!(
            point,
            StartupFailurePoint::AfterWatchdogBeforeReady
                | StartupFailurePoint::AfterWatchdogReadyBeforeGate
        ) {
            assert!(events
                .iter()
                .any(|event| matches!(event, LifecycleEvent::WatchdogReaped)));
        }
        assert_eq!(direct_children(), before_children, "{point:?} deixou filho");
    }
    assert_eq!(execution_dirs_at(&repo.root), 0);
}

#[test]
fn falha_injetada_depois_do_gate_encerra_e_reap_toda_autoridade() {
    let repo = FakeRepo::new();
    let before_children = direct_children();
    let probe = LifecycleProbe::default();
    let error = ControlledCommand::new("sh")
        .args(["-c", "sleep 60 & wait"])
        .startup_failure_for_test(StartupFailurePoint::AfterGate)
        .lifecycle_probe_for_test(&probe)
        .execution_repo_root_for_test(&repo.root)
        .outcome_for_test()
        .expect("outcome pós-gate");
    assert_eq!(error.primary_reason, TerminationReason::StartupFailure);
    let events = probe.events();
    assert!(events
        .iter()
        .any(|event| matches!(event, LifecycleEvent::GuestStarted(_))));
    assert!(events
        .iter()
        .any(|event| matches!(event, LifecycleEvent::LauncherReaped)));
    assert!(events
        .iter()
        .any(|event| matches!(event, LifecycleEvent::WatchdogReaped)));
    assert_eq!(direct_children(), before_children);
    assert_eq!(execution_dirs_at(&repo.root), 0);
}

#[test]
fn matriz_de_precedencia_da_causa_primaria_e_exata() {
    let observed = |watchdog_exit, launcher_failure, stdout_limit, stderr_limit, timeout| {
        ObservedTerminationEvents {
            watchdog_exit,
            launcher_failure,
            stdout_limit,
            stderr_limit,
            timeout,
            ..ObservedTerminationEvents::default()
        }
    };
    let cases = [
        (
            observed(true, false, false, false, false),
            TerminationReason::WatchdogExit,
        ),
        (
            observed(false, false, false, false, true),
            TerminationReason::Timeout,
        ),
        (
            observed(false, false, true, false, false),
            TerminationReason::StdoutLimit,
        ),
        (
            observed(false, false, false, true, false),
            TerminationReason::StderrLimit,
        ),
        (
            observed(true, false, false, false, true),
            TerminationReason::WatchdogExit,
        ),
        (
            observed(true, false, true, false, false),
            TerminationReason::WatchdogExit,
        ),
        (
            observed(true, false, false, true, false),
            TerminationReason::WatchdogExit,
        ),
        (
            observed(false, false, true, false, true),
            TerminationReason::StdoutLimit,
        ),
        (
            observed(false, false, false, true, true),
            TerminationReason::StderrLimit,
        ),
        (
            observed(false, false, true, true, false),
            TerminationReason::StdoutLimit,
        ),
        (
            observed(true, true, false, false, false),
            TerminationReason::WatchdogExit,
        ),
    ];
    for (events, expected) in cases {
        assert_eq!(select_primary_reason(None, events), Some(expected));
    }
    assert_eq!(
        select_primary_reason(
            Some(TerminationReason::Timeout),
            observed(true, true, true, true, false)
        ),
        Some(TerminationReason::Timeout),
        "primeira causa precisa vencer"
    );
}

#[test]
fn erros_secundarios_preservam_causa_e_disposicao() {
    let cases = [
        (
            ShutdownFailurePoint::MarkerTerminal,
            ShutdownStage::MarkerTerminal,
        ),
        (
            ShutdownFailurePoint::WatchdogFinish,
            ShutdownStage::WatchdogFinish,
        ),
        (ShutdownFailurePoint::FinalReap, ShutdownStage::FinalReap),
        (ShutdownFailurePoint::Cleanup, ShutdownStage::Cleanup),
        (ShutdownFailurePoint::Quarantine, ShutdownStage::Quarantine),
        (
            ShutdownFailurePoint::EvidenceWrite,
            ShutdownStage::EvidenceWrite,
        ),
    ];
    for (point, expected_stage) in cases {
        let repo = FakeRepo::new();
        let outcome = ControlledCommand::new("sh")
            .args(["-c", "sleep 60 & wait"])
            .timeout(Duration::from_millis(30))
            .shutdown_failure_for_test(point)
            .execution_repo_root_for_test(&repo.root)
            .outcome_for_test()
            .expect("outcome com erro secundário");
        assert_eq!(outcome.primary_reason, TerminationReason::Timeout);
        assert!(
            outcome
                .secondary_errors
                .iter()
                .any(|error| error.stage == expected_stage),
            "{point:?} não preservado"
        );
        assert!(outcome.tree_shutdown_proven);
        assert_eq!(outcome.sandbox_disposition, SandboxDisposition::Removed);
        assert_eq!(execution_dirs_at(&repo.root), 0);
    }
}

#[test]
fn outcome_estruturado_e_eventos_wire_nao_dependem_de_debug() {
    let probe = LifecycleProbe::default();
    let outcome = ControlledCommand::new("sh")
        .args(["-c", "printf structured"])
        .lifecycle_probe_for_test(&probe)
        .outcome_for_test()
        .expect("outcome estruturado");
    assert_eq!(outcome.primary_reason, TerminationReason::GuestExited);
    assert!(outcome.status.is_some_and(|status| status.success()));
    assert!(outcome.secondary_errors.is_empty());
    assert!(outcome.launcher_identity.start_time > 0);
    assert!(outcome
        .watchdog_identity
        .is_some_and(|identity| identity.start_time > 0));
    assert!(outcome.tree_shutdown_proven);
    assert_eq!(outcome.sandbox_disposition, SandboxDisposition::Removed);
    assert_eq!(outcome.stdout, b"structured");
    let records = probe.records();
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.sequence, index as u64 + 1);
        assert_eq!(
            LifecycleRecord::from_wire(&record.to_wire()).expect("roundtrip lifecycle"),
            record.clone()
        );
    }
    let secondary = LifecycleRecord {
        sequence: 99,
        event: LifecycleEvent::SecondaryFailure(ShutdownError::injected(
            ShutdownStage::FinalReap,
            Some(outcome.launcher_identity),
        )),
    };
    assert_eq!(
        LifecycleRecord::from_wire(&secondary.to_wire()).expect("roundtrip secondary"),
        secondary
    );
}

#[test]
fn handshake_ordena_launcher_watchdog_gate_e_convidado() {
    let probe = LifecycleProbe::default();
    let output = ControlledCommand::new("sh")
        .args(["-c", "printf gated"])
        .lifecycle_probe_for_test(&probe)
        .output()
        .expect("execução saudável com handshake");
    assert_eq!(output.stdout, b"gated");
    let events = probe.events();
    let position = |predicate: fn(&LifecycleEvent) -> bool, label: &str| {
        events
            .iter()
            .position(predicate)
            .unwrap_or_else(|| panic!("evento {label} ausente: {events:?}"))
    };
    let launcher = position(
        |event| matches!(event, LifecycleEvent::LauncherReady(_)),
        "launcher_ready",
    );
    let watchdog = position(
        |event| matches!(event, LifecycleEvent::WatchdogReady(_)),
        "watchdog_ready",
    );
    let gate = position(
        |event| matches!(event, LifecycleEvent::GuestGateOpened),
        "guest_gate_opened",
    );
    let guest = position(
        |event| matches!(event, LifecycleEvent::GuestStarted(_)),
        "guest_started",
    );
    let launcher_reaped = position(
        |event| matches!(event, LifecycleEvent::LauncherReaped),
        "launcher_reaped",
    );
    let watchdog_reaped = position(
        |event| matches!(event, LifecycleEvent::WatchdogReaped),
        "watchdog_reaped",
    );
    assert!(launcher < watchdog);
    assert!(watchdog < gate);
    assert!(gate < guest);
    assert!(guest < launcher_reaped);
    assert!(launcher_reaped < watchdog_reaped);
}

#[test]
fn launcher_ancora_pgid_durante_term_espera_e_kill_sem_sinalizar_externo() {
    let mut external = RawCommand::new("sleep")
        .arg("60")
        .spawn()
        .expect("processo externo");
    let probe = LifecycleProbe::default();
    let error = ControlledCommand::new("sh")
        .args([
            "-c",
            "trap '' TERM; sh -c 'trap \"\" TERM; sleep 60' & wait",
        ])
        .timeout(Duration::from_millis(100))
        .lifecycle_probe_for_test(&probe)
        .output()
        .expect_err("timeout precisa exercitar TERM e KILL");
    assert!(error.to_string().contains("timeout"));
    let events = probe.events();
    for (required, label) in [
        (
            LifecycleEvent::LauncherAnchorVerified(ShutdownSignal::Term),
            "anchor term",
        ),
        (LifecycleEvent::TermSent, "term"),
        (
            LifecycleEvent::LauncherAnchorVerified(ShutdownSignal::Kill),
            "anchor kill",
        ),
        (LifecycleEvent::KillSent, "kill"),
        (LifecycleEvent::LauncherReaped, "launcher reap"),
    ] {
        assert!(
            events.iter().any(|event| event == &required),
            "{label}: {events:?}"
        );
    }
    let external_alive = external.try_wait().expect("consulta externo").is_none();
    let external_start = process_start_time(external.id()).expect("identidade externa");
    kill_process_verified(external.id(), external_start, 9);
    let _ = external.wait();
    assert!(external_alive);
}

#[test]
fn controlador_sigkill_antes_do_gate_cancela_launcher_e_watchdog_sem_convidado() {
    let repo = FakeRepo::new();
    let id = fixture_id();
    let evidence = PathBuf::from(format!("target/controller-pregate-guest-{id}"));
    let _ = fs::remove_file(&evidence);
    let mut lifecycle = LifecycleReceiver::bind("controller-pregate");
    let controller = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "controller_before_gate_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_PREGATE_GUEST", &evidence)
        .env("PINKER_LIFECYCLE_SOCKET", &lifecycle.path)
        .env("PINKER_PREGATE_REPO_ROOT", &repo.root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("controlador pré-gate");
    let controller = VerifiedChild::new(controller);
    let launcher = match lifecycle
        .wait_for("launcher_ready pré-gate", |event| {
            matches!(event, LifecycleEvent::LauncherReady(_))
        })
        .event
    {
        LifecycleEvent::LauncherReady(identity) => identity,
        _ => unreachable!(),
    };
    let watchdog = match lifecycle
        .wait_for("watchdog_ready pré-gate", |event| {
            matches!(event, LifecycleEvent::WatchdogReady(_))
        })
        .event
    {
        LifecycleEvent::WatchdogReady(identity) => identity,
        _ => unreachable!(),
    };
    assert_eq!(
        process_start_time(launcher.pid),
        Some(launcher.start_time),
        "identidade do launcher pré-gate"
    );
    assert_eq!(
        process_start_time(watchdog.pid),
        Some(watchdog.start_time),
        "identidade do watchdog pré-gate"
    );
    let controller_output = controller.kill_and_wait();
    assert!(!controller_output.status.success());
    wait_process_dead(launcher.pid);
    wait_process_dead(watchdog.pid);
    assert!(!evidence.exists());
    let cleanup = RawCommand::new("bash")
        .args(["scripts/pinker-cleanup.sh", "--apply", "--older-than", "0"])
        .current_dir(&repo.root)
        .output()
        .expect("cleanup pós-controlador");
    assert!(cleanup.status.success(), "{cleanup:?}");
}

#[test]
#[ignore = "ponto de reexecução para morte pré-gate do controlador"]
fn controller_before_gate_entry() {
    let Some(evidence) = std::env::var_os("PINKER_PREGATE_GUEST") else {
        return;
    };
    let Some(lifecycle_socket) = std::env::var_os("PINKER_LIFECYCLE_SOCKET") else {
        return;
    };
    let Some(repo_root) = std::env::var_os("PINKER_PREGATE_REPO_ROOT") else {
        return;
    };
    let probe = LifecycleProbe::connected_to_for_test(Path::new(&lifecycle_socket))
        .expect("canal lifecycle");
    probe.hold_guest_gate_for_test();
    let mut command = ControlledCommand::new("sh");
    command
        .args(["-c", "printf guest > \"$1\"", "sh"])
        .arg(evidence)
        .lifecycle_probe_for_test(&probe)
        .execution_repo_root_for_test(repo_root);
    let _ = command.output();
}

#[test]
fn caminhos_nativos_mapeados_usam_a_autoridade_controlada() {
    let mapped = [
        "tests/backend_nativo_tests.rs",
        "tests/backend_s_external_toolchain_tests.rs",
        "tests/d1_leque_carga_lista_tests.rs",
        "tests/hotfix_r5_sigpipe_tests.rs",
        "tests/hotfix_sussurro_atribuicao_tests.rs",
        "tests/hotfix_v4_chamadas_ponteiro_tests.rs",
        "tests/hotfix_v4_ponteiro_fabricado_tests.rs",
        "tests/phase245_246_tests.rs",
        "tests/phase247_248_tests.rs",
        "tests/pr411_hr3_terminal_evidence_tests.rs",
        "tests/pr411_hr3_union_payload_tests.rs",
        "tests/uniao_contabilidade_paridade_tests.rs",
    ];
    for path in mapped {
        let source = fs::read_to_string(path).expect("lê suíte nativa mapeada");
        assert!(
            source.contains("ControlledCommand"),
            "{path} não declara autoridade"
        );
        assert!(
            !source.contains("use std::process::Command"),
            "{path} importou Command direto"
        );
        let code = rust_code_without_strings_and_comments(&source);
        assert!(
            !code.contains("std::process::Command::new("),
            "{path} criou processo direto"
        );
    }

    let memory = fs::read_to_string("tests/public_memory_hotfix_tests.rs").expect("memória");
    assert!(memory.contains("fn run_memory_child("));
    assert!(memory.contains("RLIMIT_AS"));
    assert!(memory.contains("RLIMIT_CORE"));
    assert!(
        !memory.contains("Command::new(\"cc\")"),
        "ferramenta externa escapou"
    );

    let helper = [
        "tests/common/native_process.rs",
        "tests/common/native_process_launcher.rs",
        "tests/common/native_process_marker.rs",
        "tests/common/native_process_model.rs",
        "tests/common/native_process_sandbox.rs",
    ]
    .into_iter()
    .map(|path| fs::read_to_string(path).expect("helper"))
    .collect::<Vec<_>>()
    .join("\n");
    for required in [
        "ProcessWatchdog",
        "pipe2",
        "root_inode",
        "execution_inode",
        "LauncherReady",
        "LifecycleEvent::GuestGateOpened",
        "PR_SET_CHILD_SUBREAPER",
        "SYS_PIDFD_SEND_SIGNAL",
        "RENAME_NOREPLACE",
        "parse_proc_stat_start_time",
        "Stdio::null()",
        "Stdio::inherit()",
        "cpu_seconds",
        "watchdog_pid",
        "sandbox.cleanup()?",
        "started.elapsed() >= policy.timeout",
    ] {
        assert!(helper.contains(required), "política ausente: {required}");
    }
    assert!(
        !helper.contains("self.output().map"),
        "status não pode descartar output"
    );
}

fn rust_code_without_strings_and_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    for line in source.lines() {
        let mut in_string = false;
        let mut escaped = false;
        let mut characters = line.chars().peekable();
        while let Some(character) = characters.next() {
            if !in_string && character == '/' && characters.peek() == Some(&'/') {
                break;
            }
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                output.push(' ');
            } else if character == '"' {
                in_string = true;
                output.push(' ');
            } else {
                output.push(character);
            }
        }
        output.push('\n');
    }
    output
}

fn containment_guard(sources: &std::collections::BTreeMap<&str, String>) -> bool {
    let debug_result = ["format!(\"", "{result:?}", "\")"].concat();
    let debug_reason_assertion = [".contains(\"", "watchdog_exit", "\")"].concat();
    let global_marker_scan = ["read_dir(\"", "target/pinker-exec", "\")"].concat();
    let global_lock = ["fn ", "serial", "()"].concat();
    let required = [
        ("launcher", "raw_wait_for_gate"),
        ("launcher", "probe.wait_guest_gate()"),
        ("launcher", "LifecycleEvent::GuestGateOpened"),
        ("launcher", "PR_SET_CHILD_SUBREAPER"),
        ("launcher", "raw_supervise_tree"),
        ("launcher", "SYS_PIDFD_SEND_SIGNAL"),
        ("launcher", "self.verify_anchor(\"kill\")"),
        ("launcher", "reap_cancelled_child"),
        ("launcher", "open_fd_snapshot"),
        ("launcher", "LAUNCHER_FORK_WINDOW"),
        ("process", "ProcessWatchdog::spawn"),
        ("process", "WatchdogExitObserved"),
        ("process", "launcher.wait_final"),
        ("process", "primary_reason.is_none()"),
        ("process", "record_secondary"),
        ("process", "ControlledRunOutcome"),
        ("model", "current.or_else"),
        ("model", "observed.watchdog_exit"),
        ("model", "sequence: u64"),
        ("test", "connected_to_for_test"),
        ("test", "process_start_time(watchdog.pid)"),
        ("test", "publish_outcome_atomically"),
        ("test", "LifecycleRecord::from_wire"),
        ("test", "record.sequence, expected"),
        ("test", "LifecycleEvent::ResultPublished"),
        ("test", "preserve_timeout"),
        ("test", "fixture_id()"),
        ("runner", "preserve_failure"),
        ("runner", "pinker-flake-evidence"),
        ("runner", "rm -rf -- \"$tmp\""),
        ("sandbox", "execution_device"),
        ("sandbox", "execution_inode"),
        ("sandbox", "NEXT_EXECUTION.fetch_add"),
        ("sandbox", "RENAME_NOREPLACE"),
        ("sandbox", "fs::remove_dir_all(&quarantine)"),
        ("marker", "fields.len() != MARKER_FIELDS.len() + 1"),
        ("marker", "create_new(true)"),
        ("marker", "file.sync_all()?"),
        ("marker", "fs::rename(&temporary, &marker)"),
        ("bash", "parse_marker"),
        ("bash", "execution_device|execution_inode|launcher_pid"),
        ("bash", "mv -T -n -- \"$directory\" \"$quarantine\""),
        ("bash", "quarantined_identity"),
        ("bash", "read -r stat_text 2>/dev/null"),
    ];
    required
        .iter()
        .all(|(source, token)| sources.get(source).is_some_and(|text| text.contains(token)))
        && !sources
            .get("sandbox")
            .is_some_and(|text| text.contains("remove_dir_all(&self.directory)"))
        && !sources
            .get("process")
            .is_some_and(|text| text.contains("ProcessWatchdog::spawn(pgid)"))
        && !sources
            .get("process")
            .is_some_and(|text| text.contains("termination_reason = Some"))
        && !sources
            .get("test")
            .is_some_and(|text| text.contains(&debug_result))
        && !sources
            .get("test")
            .is_some_and(|text| text.contains(&debug_reason_assertion))
        && !sources
            .get("test")
            .is_some_and(|text| text.contains(&global_lock))
        && !sources
            .get("test")
            .is_some_and(|text| text.contains(&global_marker_scan))
        && !sources
            .get("bash")
            .is_some_and(|text| text.contains("rm -rf -- \"$directory\""))
}

#[test]
fn sensibilidade_detecta_variacoes_e_restaura_fontes_byte_a_byte() {
    let paths = [
        ("process", "tests/common/native_process.rs"),
        ("launcher", "tests/common/native_process_launcher.rs"),
        ("sandbox", "tests/common/native_process_sandbox.rs"),
        ("marker", "tests/common/native_process_marker.rs"),
        ("model", "tests/common/native_process_model.rs"),
        ("bash", "scripts/pinker-cleanup.sh"),
        ("test", "tests/native_process_control_tests.rs"),
        ("runner", "scripts/pinker-flake-runner.sh"),
    ];
    let originals: std::collections::BTreeMap<_, _> = paths
        .into_iter()
        .map(|(key, path)| {
            (
                key,
                fs::read_to_string(path).expect("fonte de sensibilidade"),
            )
        })
        .collect();
    assert!(containment_guard(&originals));
    let variations = [
        ("launcher", "raw_wait_for_gate"),
        ("launcher", "probe.wait_guest_gate()"),
        ("launcher", "LifecycleEvent::GuestGateOpened"),
        ("launcher", "PR_SET_CHILD_SUBREAPER"),
        ("launcher", "raw_supervise_tree"),
        ("launcher", "SYS_PIDFD_SEND_SIGNAL"),
        ("launcher", "self.verify_anchor(\"kill\")"),
        ("launcher", "reap_cancelled_child"),
        ("launcher", "open_fd_snapshot"),
        ("launcher", "LAUNCHER_FORK_WINDOW"),
        ("process", "ProcessWatchdog::spawn"),
        ("process", "WatchdogExitObserved"),
        ("process", "launcher.wait_final"),
        ("process", "primary_reason.is_none()"),
        ("process", "record_secondary"),
        ("process", "ControlledRunOutcome"),
        ("model", "current.or_else"),
        ("model", "observed.watchdog_exit"),
        ("model", "sequence: u64"),
        ("test", "connected_to_for_test"),
        ("test", "process_start_time(watchdog.pid)"),
        ("test", "publish_outcome_atomically"),
        ("test", "LifecycleRecord::from_wire"),
        ("test", "record.sequence, expected"),
        ("test", "LifecycleEvent::ResultPublished"),
        ("test", "preserve_timeout"),
        ("test", "fixture_id()"),
        ("runner", "preserve_failure"),
        ("runner", "pinker-flake-evidence"),
        ("runner", "rm -rf -- \"$tmp\""),
        ("sandbox", "execution_device"),
        ("sandbox", "execution_inode"),
        ("sandbox", "NEXT_EXECUTION.fetch_add"),
        ("sandbox", "RENAME_NOREPLACE"),
        ("sandbox", "fs::remove_dir_all(&quarantine)"),
        ("marker", "fields.len() != MARKER_FIELDS.len() + 1"),
        ("marker", "create_new(true)"),
        ("marker", "file.sync_all()?"),
        ("marker", "fs::rename(&temporary, &marker)"),
        ("bash", "parse_marker"),
        ("bash", "execution_device|execution_inode|launcher_pid"),
        ("bash", "mv -T -n -- \"$directory\" \"$quarantine\""),
        ("bash", "quarantined_identity"),
        ("bash", "read -r stat_text 2>/dev/null"),
    ];
    for (source, token) in variations {
        let mut mutated = originals.clone();
        let text = mutated.get_mut(source).expect("fonte da variação");
        assert!(text.contains(token), "token de variação ausente: {token}");
        *text = text.replace(token, "");
        assert!(
            !containment_guard(&mutated),
            "variação não detectada: {token}"
        );
        mutated = originals.clone();
        assert_eq!(mutated, originals, "restauração em memória não foi exata");
    }
    let forbidden_variations = [
        (
            "process",
            ["termination_", "reason = Some(TerminationReason::Timeout)"].concat(),
        ),
        ("process", ["ProcessWatchdog::", "spawn(pgid)"].concat()),
        ("test", ["format!(\"", "{result:?}", "\")"].concat()),
        ("test", [".contains(\"", "watchdog_exit", "\")"].concat()),
        ("test", ["fn ", "serial", "()"].concat()),
        (
            "test",
            ["read_dir(\"", "target/pinker-exec", "\")"].concat(),
        ),
        (
            "sandbox",
            ["remove_dir_all(", "&self.directory", ")"].concat(),
        ),
    ];
    for (source, injection) in forbidden_variations {
        let mut mutated = originals.clone();
        mutated
            .get_mut(source)
            .expect("fonte da variação proibida")
            .push_str(&injection);
        assert!(
            !containment_guard(&mutated),
            "variação proibida não detectada: {injection}"
        );
        mutated = originals.clone();
        assert_eq!(mutated, originals, "restauração proibida não foi exata");
    }
    for (key, path) in paths {
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            originals[key],
            "fonte {key} mudou durante sensibilidade"
        );
    }
}
