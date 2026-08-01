use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_CAPTURED_STDOUT_BYTES: usize = 1024 * 1024;
const MAX_CAPTURED_STDERR_BYTES: usize = 1024 * 1024;
const STALE_EXECUTION_MIN_AGE: Duration = Duration::from_secs(60 * 60);
static NEXT_EXECUTION: AtomicU64 = AtomicU64::new(1);
static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(1_000_000);
type HashFingerprint = (u64, SystemTime, String);
type HashCache = Mutex<BTreeMap<PathBuf, HashFingerprint>>;
static HASH_CACHE: OnceLock<HashCache> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeRunClass {
    Common,
    Pipeline,
    Toolchain,
}

#[derive(Clone, Copy, Debug)]
struct ResourcePolicy {
    class: NativeRunClass,
    timeout: Duration,
    address_space_bytes: u64,
    cpu_seconds: u64,
}

impl ResourcePolicy {
    fn for_program(program: &OsStr) -> Self {
        let name = Path::new(program)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        if matches!(name, "cc" | "gcc" | "clang" | "cargo" | "rustc" | "git") {
            return Self {
                class: NativeRunClass::Toolchain,
                timeout: Duration::from_secs(120),
                address_space_bytes: 4 * 1024 * 1024 * 1024,
                cpu_seconds: 120,
            };
        }
        if name == "pink" || name.starts_with("pink-") {
            return Self {
                class: NativeRunClass::Pipeline,
                timeout: Duration::from_secs(60),
                address_space_bytes: 4 * 1024 * 1024 * 1024,
                cpu_seconds: 60,
            };
        }
        Self {
            class: NativeRunClass::Common,
            timeout: Duration::from_secs(20),
            address_space_bytes: 1024 * 1024 * 1024,
            cpu_seconds: 20,
        }
    }
}

/// Substituto estreito de `std::process::Command` para processos exercitados
/// pelas suítes nativas. Preserva a API de construção usada pelos testes e
/// centraliza contenção quando `output` ou `status` é solicitado.
pub struct ControlledCommand {
    inner: StdCommand,
    logical_case: String,
    timeout_override: Option<Duration>,
    capture_override: Option<usize>,
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
        self
    }

    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stdout(cfg);
        self
    }

    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Self {
        self.inner.stderr(cfg);
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

    pub fn capture_limit(&mut self, bytes_per_channel: usize) -> &mut Self {
        self.capture_override = Some(bytes_per_channel);
        self
    }

    #[cfg(unix)]
    pub unsafe fn pre_exec<F>(&mut self, function: F) -> &mut Self
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: o chamador assume o mesmo contrato async-signal-safe de
        // `std::os::unix::process::CommandExt::pre_exec`.
        unsafe { self.inner.pre_exec(function) };
        self
    }

    pub fn output(&mut self) -> io::Result<Output> {
        controlled_output(
            &mut self.inner,
            &self.logical_case,
            self.timeout_override,
            self.capture_override,
        )
    }

    pub fn status(&mut self) -> io::Result<ExitStatus> {
        self.output().map(|output| output.status)
    }
}

struct ExecutionSandbox {
    root: PathBuf,
    directory: PathBuf,
    marker: PathBuf,
    created_at_unix: u64,
    owner_start_time: u64,
}

impl ExecutionSandbox {
    fn create(git_head: &str) -> io::Result<Self> {
        let root = std::env::current_dir()?.join("target/pinker-exec");
        fs::create_dir_all(&root)?;
        if let Err(error) = scavenge_stale_execution_dirs(&root, STALE_EXECUTION_MIN_AGE) {
            eprintln!("falha parcial no scavenger Pinker: {error}");
        }
        let id = NEXT_EXECUTION.fetch_add(1, Ordering::SeqCst);
        let directory = root.join(format!("exec-{}-{id}", std::process::id()));
        fs::create_dir(&directory)?;
        let marker = directory.join("owner.marker");
        let created_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let sandbox = Self {
            root,
            directory,
            marker,
            created_at_unix,
            owner_start_time: process_start_time(std::process::id()).unwrap_or(0),
        };
        sandbox.write_marker(git_head, None, None, "preparing", "pending")?;
        Ok(sandbox)
    }

    fn write_marker(
        &self,
        git_head: &str,
        child_pid: Option<u32>,
        child_pgid: Option<i32>,
        state: &str,
        executable_sha256: &str,
    ) -> io::Result<()> {
        let marker = format!(
            "schema: 1\nowner_pid: {}\nowner_start_time: {}\nchild_pid: {}\nchild_pgid: {}\ncreated_at_unix: {}\ngit_head: {git_head}\nexecutable_sha256: {executable_sha256}\nstate: {state}\n",
            std::process::id(),
            self.owner_start_time,
            child_pid.map_or_else(|| "null".to_string(), |pid| pid.to_string()),
            child_pgid.map_or_else(|| "null".to_string(), |pgid| pgid.to_string()),
            self.created_at_unix,
        );
        fs::write(&self.marker, marker)
    }

    fn cleanup(&self) -> io::Result<()> {
        if !self.directory.exists() {
            return Ok(());
        }
        let canonical_root = self.root.canonicalize()?;
        let canonical_directory = self.directory.canonicalize()?;
        if !canonical_directory.starts_with(canonical_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "diretório de execução escapou da raiz Pinker",
            ));
        }
        if fs::symlink_metadata(&canonical_directory)?
            .file_type()
            .is_symlink()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "diretório de execução não pode ser symlink",
            ));
        }
        fs::remove_dir_all(&canonical_directory)?;
        if canonical_directory.exists() {
            return Err(io::Error::other("diretório de execução não foi removido"));
        }
        Ok(())
    }
}

fn scavenge_stale_execution_dirs(root: &Path, minimum_age: Duration) -> io::Result<usize> {
    if fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "raiz de execução não pode ser symlink",
        ));
    }
    let canonical_root = root.canonicalize()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut removed = 0;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() || !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(ids) = name.strip_prefix("exec-") else {
            continue;
        };
        let mut ids = ids.split('-');
        if ids
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .is_none()
            || ids
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .is_none()
            || ids.next().is_some()
        {
            continue;
        }

        let directory = entry.path();
        let marker = directory.join("owner.marker");
        let Ok(marker_metadata) = fs::symlink_metadata(&marker) else {
            continue;
        };
        if marker_metadata.file_type().is_symlink() || !marker_metadata.is_file() {
            continue;
        }
        let Ok(marker) = fs::read_to_string(&marker) else {
            continue;
        };
        let mut fields = BTreeMap::new();
        let mut valid = true;
        for line in marker.lines() {
            let Some((key, value)) = line.split_once(": ") else {
                valid = false;
                break;
            };
            if fields.insert(key, value).is_some() {
                valid = false;
                break;
            }
        }
        let parsed = || -> Option<(u32, u64, u64)> {
            if !valid || fields.get("schema") != Some(&"1") {
                return None;
            }
            let state = *fields.get("state")?;
            if !matches!(state, "preparing" | "running" | "finished" | "failed") {
                return None;
            }
            Some((
                fields.get("owner_pid")?.parse().ok()?,
                fields.get("owner_start_time")?.parse().ok()?,
                fields.get("created_at_unix")?.parse().ok()?,
            ))
        };
        let Some((owner_pid, owner_start, created)) = parsed() else {
            continue;
        };
        if created > now || now - created < minimum_age.as_secs() {
            continue;
        }
        if process_start_time(owner_pid) == Some(owner_start) {
            continue;
        }
        let canonical_directory = directory.canonicalize()?;
        if !canonical_directory.starts_with(&canonical_root) {
            continue;
        }
        fs::remove_dir_all(canonical_directory)?;
        removed += 1;
    }
    Ok(removed)
}

impl Drop for ExecutionSandbox {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!(
                "falha ao limpar sandbox nativo {}: {error}",
                self.directory.display()
            );
        }
    }
}

/// Diretório multi-etapa para fonte, objeto e ELF que precisam sobreviver entre
/// comandos controlados. O marcador permite recuperação segura se o processo de
/// teste morrer antes de `Drop`.
pub struct NativeArtifactDir {
    sandbox: ExecutionSandbox,
}

impl NativeArtifactDir {
    pub fn create() -> io::Result<Self> {
        let git_head = read_git_head().unwrap_or_else(|| "unknown".to_string());
        let root = std::env::current_dir()?.join("target/pinker-exec");
        fs::create_dir_all(&root)?;
        let id = NEXT_ARTIFACT.fetch_add(1, Ordering::SeqCst);
        let directory = root.join(format!("exec-{}-{id}", std::process::id()));
        fs::create_dir(&directory)?;
        let marker = directory.join("owner.marker");
        let sandbox = ExecutionSandbox {
            root,
            directory,
            marker,
            created_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            owner_start_time: process_start_time(std::process::id()).unwrap_or(0),
        };
        sandbox.write_marker(&git_head, None, None, "running", "pending")?;
        Ok(Self { sandbox })
    }

    pub fn path(&self) -> &Path {
        &self.sandbox.directory
    }
}

fn controlled_output(
    command: &mut StdCommand,
    logical_case: &str,
    timeout_override: Option<Duration>,
    capture_override: Option<usize>,
) -> io::Result<Output> {
    let mut policy = ResourcePolicy::for_program(command.get_program());
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
    let sandbox = ExecutionSandbox::create(&git_head)?;
    command.env("TMPDIR", &sandbox.directory);
    command.env("PINKER_EXECUTION_DIR", &sandbox.directory);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(target_os = "linux")]
    prepare_linux_child(command, policy)?;

    let started_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let started = Instant::now();
    let mut child = command.spawn()?;
    let pid = child.id();
    let pgid = pid as i32;
    sandbox.write_marker(
        &git_head,
        Some(pid),
        Some(pgid),
        "running",
        &executable_hash,
    )?;

    let stdout_overflow = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let stdout_thread = bounded_reader(
        child.stdout.take().expect("stdout configurado"),
        stdout_limit,
        Arc::clone(&stdout_overflow),
    );
    let stderr_thread = bounded_reader(
        child.stderr.take().expect("stderr configurado"),
        stderr_limit,
        Arc::clone(&stderr_overflow),
    );
    drop(child.stdin.take());

    let mut termination_reason = None;
    let status = loop {
        if stdout_overflow.load(Ordering::SeqCst) {
            termination_reason = Some("stdout_limit");
            terminate_process_group(pgid, &mut child);
            break child.wait()?;
        }
        if stderr_overflow.load(Ordering::SeqCst) {
            termination_reason = Some("stderr_limit");
            terminate_process_group(pgid, &mut child);
            break child.wait()?;
        }
        if started.elapsed() >= policy.timeout {
            termination_reason = Some("timeout");
            terminate_process_group(pgid, &mut child);
            break child.wait()?;
        }
        if let Some(status) = child.try_wait()? {
            terminate_remaining_group(pgid);
            break status;
        }
        thread::sleep(Duration::from_millis(10));
    };

    let stdout = stdout_thread
        .join()
        .map_err(|_| io::Error::other("thread de stdout entrou em panic"))??;
    let stderr = stderr_thread
        .join()
        .map_err(|_| io::Error::other("thread de stderr entrou em panic"))??;
    let duration = started.elapsed();

    if termination_reason.is_some() || !status.success() {
        eprintln!(
            "native_execution_failure case={} git_head={} executable={} executable_sha256={} runtime_sha256={} pid={} pgid={} class={:?} timeout_ms={} address_space_bytes={} stdout_limit={} stderr_limit={} started_unix_ms={} duration_ms={} status={} reason={}",
            sanitize(logical_case),
            git_head,
            executable
                .as_deref()
                .and_then(Path::file_name)
                .and_then(OsStr::to_str)
                .unwrap_or("unknown"),
            executable_hash,
            runtime_hash.as_deref().unwrap_or("unknown"),
            pid,
            pgid,
            policy.class,
            policy.timeout.as_millis(),
            policy.address_space_bytes,
            stdout_limit,
            stderr_limit,
            started_unix_ms,
            duration.as_millis(),
            status,
            termination_reason.unwrap_or("exit"),
        );
    }

    sandbox.cleanup()?;
    if let Some(reason) = termination_reason {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("execução nativa controlada encerrada: {reason}"),
        ));
    }
    Ok(Output {
        status,
        stdout,
        stderr,
    })
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
fn prepare_linux_child(command: &mut StdCommand, policy: ResourcePolicy) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    let expected_parent = std::process::id() as i32;
    unsafe {
        command.pre_exec(move || {
            #[repr(C)]
            struct RLimit {
                current: u64,
                maximum: u64,
            }
            extern "C" {
                fn getppid() -> i32;
                fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
                fn setpgid(pid: i32, pgid: i32) -> i32;
                fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
            }
            const PR_SET_PDEATHSIG: i32 = 1;
            const SIGKILL: u64 = 9;
            const RLIMIT_CPU: i32 = 0;
            const RLIMIT_CORE: i32 = 4;
            const RLIMIT_AS: i32 = 9;
            let set = |resource, value| {
                let limit = RLimit {
                    current: value,
                    maximum: value,
                };
                if setrlimit(resource, &limit) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            };
            if setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if getppid() != expected_parent {
                return Err(io::Error::from_raw_os_error(3));
            }
            set(RLIMIT_CORE, 0)?;
            set(RLIMIT_AS, policy.address_space_bytes)?;
            set(RLIMIT_CPU, policy.cpu_seconds)?;
            Ok(())
        });
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn prepare_linux_child(_command: &mut StdCommand, _policy: ResourcePolicy) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn signal_group(pgid: i32, signal: i32) {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    unsafe {
        kill(-pgid, signal);
    }
}

#[cfg(not(target_os = "linux"))]
fn signal_group(_pgid: i32, _signal: i32) {}

fn terminate_process_group(pgid: i32, child: &mut std::process::Child) {
    signal_group(pgid, 15);
    let deadline = Instant::now() + Duration::from_millis(200);
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            terminate_remaining_group(pgid);
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    signal_group(pgid, 9);
}

fn terminate_remaining_group(pgid: i32) {
    signal_group(pgid, 15);
    thread::sleep(Duration::from_millis(20));
    signal_group(pgid, 9);
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

fn process_start_time(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_comm = stat.rsplit_once(") ")?.1;
    after_comm.split_whitespace().nth(19)?.parse().ok()
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
    let data = fs::read(path).ok()?;
    Some(
        sha256(&data)
            .iter()
            .fold(String::with_capacity(64), |mut output, byte| {
                write!(&mut output, "{byte:02x}").expect("escrita em String não falha");
                output
            }),
    )
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

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in padded.chunks_exact(64) {
        let mut w = [0_u32; 64];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (state, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *state = state.wrapping_add(value);
        }
    }
    let mut out = [0_u8; 32];
    for (slot, value) in out.chunks_exact_mut(4).zip(h) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
    out
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
