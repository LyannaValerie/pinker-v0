use super::{
    process_start_time, LifecycleEvent, LifecycleRecord, ProcessIdentitySnapshot, ResourcePolicy,
    ShutdownSignal,
};
use std::fs::File;
use std::io::{self, Read as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::process::{Child, Command as StdCommand, ExitStatus};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

static LAUNCHER_FORK_WINDOW: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupFailurePoint {
    BeforeLauncher,
    AfterLauncherBeforeReady,
    AfterLauncherReady,
    BeforeWatchdog,
    WatchdogPipe,
    WatchdogFork,
    AfterWatchdogBeforeReady,
    AfterWatchdogReadyBeforeGate,
    AfterGate,
}

#[derive(Default)]
struct LifecycleState {
    next_sequence: u64,
    records: Vec<LifecycleRecord>,
}

#[derive(Clone)]
pub struct LifecycleProbe {
    state: Arc<(Mutex<LifecycleState>, Condvar)>,
    hold_guest_gate: Arc<(Mutex<bool>, Condvar)>,
    sink: Option<Arc<UnixDatagram>>,
    sink_error: Arc<Mutex<Option<io::Error>>>,
}

impl Default for LifecycleProbe {
    fn default() -> Self {
        Self {
            state: Arc::new((Mutex::new(LifecycleState::default()), Condvar::new())),
            hold_guest_gate: Arc::new((Mutex::new(false), Condvar::new())),
            sink: None,
            sink_error: Arc::new(Mutex::new(None)),
        }
    }
}

impl LifecycleProbe {
    pub fn connected_to_for_test(path: &Path) -> io::Result<Self> {
        let sink = UnixDatagram::unbound()?;
        sink.connect(path)?;
        Ok(Self {
            sink: Some(Arc::new(sink)),
            ..Self::default()
        })
    }

    pub fn records(&self) -> Vec<LifecycleRecord> {
        self.state
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .records
            .clone()
    }

    pub fn events(&self) -> Vec<LifecycleEvent> {
        self.records()
            .into_iter()
            .map(|record| record.event)
            .collect()
    }

    pub fn hold_guest_gate_for_test(&self) {
        *self
            .hold_guest_gate
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    }

    pub fn release_guest_gate_for_test(&self) {
        let mut held = self
            .hold_guest_gate
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *held = false;
        self.hold_guest_gate.1.notify_all();
    }

    pub fn record_for_test(&self, event: LifecycleEvent) {
        self.record(event);
    }

    pub fn wait_for_test(
        &self,
        label: &str,
        timeout: Duration,
        predicate: impl Fn(&LifecycleEvent) -> bool,
    ) -> io::Result<LifecycleRecord> {
        let deadline = Instant::now() + timeout;
        let mut state = self
            .state
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if let Some(record) = state.records.iter().find(|record| predicate(&record.event)) {
                return Ok(record.clone());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("evento {label} ausente; observados={:?}", state.records),
                ));
            }
            let (next, result) = self
                .state
                .1
                .wait_timeout(state, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state = next;
            if result.timed_out() && !state.records.iter().any(|record| predicate(&record.event)) {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("evento {label} ausente; observados={:?}", state.records),
                ));
            }
        }
    }

    pub(super) fn take_sink_error(&self) -> Option<io::Error> {
        self.sink_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    pub(super) fn record(&self, event: LifecycleEvent) {
        let record = {
            let mut state = self
                .state
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.next_sequence += 1;
            let record = LifecycleRecord {
                sequence: state.next_sequence,
                event,
            };
            state.records.push(record.clone());
            self.state.1.notify_all();
            record
        };
        if let Some(sink) = &self.sink {
            if let Err(error) = sink.send(record.to_wire().as_bytes()) {
                let mut saved = self
                    .sink_error
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if saved.is_none() {
                    *saved = Some(error);
                }
            }
        }
    }

    fn wait_guest_gate(&self) {
        let mut held = self
            .hold_guest_gate
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *held {
            held = self
                .hold_guest_gate
                .1
                .wait(held)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LauncherIdentity {
    pub pid: u32,
    pub start_time: u64,
    pub pgid: i32,
}

impl LauncherIdentity {
    pub(super) fn snapshot(self) -> ProcessIdentitySnapshot {
        ProcessIdentitySnapshot {
            pid: self.pid,
            start_time: self.start_time,
            pgid: Some(self.pgid),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LauncherMessage {
    kind: u8,
    pid: i32,
    value: i64,
}

impl LauncherMessage {
    const READY: u8 = 1;
    const GUEST: u8 = 2;
    const FINISHED: u8 = 3;
    const ERROR: u8 = 4;
    const TERM_SENT: u8 = 5;
    const KILL_SENT: u8 = 6;

    fn encode(self) -> [u8; 16] {
        let mut bytes = [0_u8; 16];
        bytes[0] = self.kind;
        bytes[4..8].copy_from_slice(&self.pid.to_ne_bytes());
        bytes[8..16].copy_from_slice(&self.value.to_ne_bytes());
        bytes
    }

    fn decode(bytes: [u8; 16]) -> Self {
        Self {
            kind: bytes[0],
            pid: i32::from_ne_bytes(bytes[4..8].try_into().expect("slice fixa")),
            value: i64::from_ne_bytes(bytes[8..16].try_into().expect("slice fixa")),
        }
    }
}

#[cfg(target_os = "linux")]
fn pipe_cloexec() -> io::Result<(OwnedFd, OwnedFd)> {
    extern "C" {
        fn pipe2(pipefd: *mut i32, flags: i32) -> i32;
    }
    const O_CLOEXEC: i32 = 0o2000000;
    let mut pipe = [-1_i32; 2];
    if unsafe { pipe2(pipe.as_mut_ptr(), O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe2 devolveu dois descritores novos e possuídos por este escopo.
    Ok(unsafe { (OwnedFd::from_raw_fd(pipe[0]), OwnedFd::from_raw_fd(pipe[1])) })
}

#[cfg(target_os = "linux")]
fn open_fd_snapshot() -> io::Result<Vec<i32>> {
    extern "C" {
        fn fcntl(fd: i32, command: i32, ...) -> i32;
    }
    const F_GETFD: i32 = 1;
    let mut descriptors = std::fs::read_dir("/proc/self/fd")?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
        .collect::<Vec<_>>();
    // O fd usado por read_dir já foi fechado; removê-lo evita fechar por
    // engano o pipe interno que std::process possa reutilizar nesse número.
    descriptors.retain(|&fd| unsafe { fcntl(fd, F_GETFD) } >= 0);
    descriptors.sort_unstable();
    descriptors.dedup();
    Ok(descriptors)
}

#[cfg(target_os = "linux")]
pub(super) struct ProcessLauncher {
    child: Child,
    status: File,
    control: Option<OwnedFd>,
    controller_life: Option<OwnedFd>,
    identity: LauncherIdentity,
    guest_pid: u32,
    final_status: Option<ExitStatus>,
    reaped: bool,
    probe: LifecycleProbe,
}

#[cfg(target_os = "linux")]
impl ProcessLauncher {
    pub(super) fn spawn_gated<W, F>(
        command: &mut StdCommand,
        policy: ResourcePolicy,
        failure: Option<StartupFailurePoint>,
        probe: LifecycleProbe,
        after_launcher_ready: F,
    ) -> io::Result<(Self, W)>
    where
        F: FnOnce(LauncherIdentity, i32) -> io::Result<W>,
    {
        use std::os::unix::process::CommandExt as _;
        if failure == Some(StartupFailurePoint::BeforeLauncher) {
            return Err(injected("BEFORE_LAUNCHER"));
        }
        // A tabela de descritores pertence ao processo inteiro. Serialize
        // somente o snapshot + fork + READY para que launchers concorrentes
        // não herdem os canais uns dos outros; a execução após READY continua
        // plenamente concorrente.
        let fork_window = LAUNCHER_FORK_WINDOW
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (status_read, status_write) = pipe_cloexec()?;
        let (gate_read, gate_write) = pipe_cloexec()?;
        let (control_read, control_write) = pipe_cloexec()?;
        let (life_read, life_write) = pipe_cloexec()?;
        let status_fd = status_write.as_raw_fd();
        let gate_fd = gate_read.as_raw_fd();
        let control_fd = control_read.as_raw_fd();
        let life_fd = life_read.as_raw_fd();
        let inherited_fds = open_fd_snapshot()?;
        let fail_before_ready = failure == Some(StartupFailurePoint::AfterLauncherBeforeReady);
        unsafe {
            command.pre_exec(move || {
                launcher_pre_exec(
                    status_fd,
                    gate_fd,
                    control_fd,
                    life_fd,
                    &inherited_fds,
                    policy,
                    fail_before_ready,
                )
            });
        }
        let mut status = File::from(status_read);
        let spawned = std::thread::scope(|scope| -> io::Result<(Child, W, LauncherIdentity)> {
            let spawn_thread = scope.spawn(|| {
                let result = command.spawn();
                drop((status_write, gate_read, control_read, life_read));
                result
            });
            let ready = read_message(&mut status).and_then(|message| {
                if message.kind == LauncherMessage::READY && message.pid > 0 {
                    Ok(message)
                } else {
                    Err(io::Error::other("launcher não confirmou prontidão"))
                }
            });
            let ready = match ready {
                Ok(ready) => ready,
                Err(error) => {
                    drop(gate_write);
                    let child = spawn_thread
                        .join()
                        .map_err(|_| io::Error::other("thread de spawn em panic"))??;
                    reap_cancelled_child(child, &probe)?;
                    return Err(error);
                }
            };
            drop(fork_window);
            let launcher_pid = ready.pid as u32;
            let start_time = process_start_time(launcher_pid)
                .ok_or_else(|| io::Error::other("identidade do launcher não pôde ser provada"))?;
            let identity = LauncherIdentity {
                pid: launcher_pid,
                start_time,
                pgid: ready.pid,
            };
            probe.record(LifecycleEvent::LauncherReady(identity.snapshot()));
            if failure == Some(StartupFailurePoint::AfterLauncherReady) {
                drop(gate_write);
                let child = spawn_thread
                    .join()
                    .map_err(|_| io::Error::other("thread de spawn em panic"))??;
                reap_cancelled_child(child, &probe)?;
                return Err(injected("AFTER_LAUNCHER_READY"));
            }
            let watchdog = match after_launcher_ready(identity, control_write.as_raw_fd()) {
                Ok(watchdog) => watchdog,
                Err(error) => {
                    drop(gate_write);
                    let child = spawn_thread
                        .join()
                        .map_err(|_| io::Error::other("thread de spawn em panic"))??;
                    reap_cancelled_child(child, &probe)?;
                    return Err(error);
                }
            };
            if failure == Some(StartupFailurePoint::AfterWatchdogReadyBeforeGate) {
                drop(watchdog);
                drop(gate_write);
                let child = spawn_thread
                    .join()
                    .map_err(|_| io::Error::other("thread de spawn em panic"))??;
                reap_cancelled_child(child, &probe)?;
                return Err(injected("AFTER_WATCHDOG_READY_BEFORE_GATE"));
            }
            probe.wait_guest_gate();
            write_fd(gate_write.as_raw_fd(), b'G')?;
            drop(gate_write);
            probe.record(LifecycleEvent::GuestGateOpened);
            let child = spawn_thread
                .join()
                .map_err(|_| io::Error::other("thread de spawn em panic"))??;
            Ok((child, watchdog, identity))
        });
        let (child, watchdog, identity) = spawned?;
        let guest = read_message(&mut status)?;
        if guest.kind != LauncherMessage::GUEST || guest.pid <= 0 {
            let mut child = child;
            let _ = write_fd(control_write.as_raw_fd(), b'T');
            let _ = child.wait();
            return Err(io::Error::other("launcher não confirmou o convidado"));
        }
        probe.record(LifecycleEvent::GuestStarted(ProcessIdentitySnapshot {
            pid: guest.pid as u32,
            start_time: process_start_time(guest.pid as u32).unwrap_or(0),
            pgid: Some(identity.pgid),
        }));
        Ok((
            Self {
                child,
                status,
                control: Some(control_write),
                controller_life: Some(life_write),
                identity,
                guest_pid: guest.pid as u32,
                final_status: None,
                reaped: false,
                probe,
            },
            watchdog,
        ))
    }

    pub(super) fn identity(&self) -> LauncherIdentity {
        self.identity
    }

    pub(super) fn guest_pid(&self) -> u32 {
        self.guest_pid
    }

    pub(super) fn take_stdin(&mut self) -> Option<std::process::ChildStdin> {
        self.child.stdin.take()
    }

    pub(super) fn take_stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.child.stdout.take()
    }

    pub(super) fn take_stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.child.stderr.take()
    }

    pub(super) fn request_termination(&mut self) -> io::Result<()> {
        self.probe.record(LifecycleEvent::TermRequested);
        match self.control.as_ref() {
            Some(fd) => write_fd(fd.as_raw_fd(), b'T'),
            None => Ok(()),
        }
    }

    pub(super) fn try_final(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.final_status {
            return Ok(Some(status));
        }
        while poll_readable(self.status.as_raw_fd(), 0)? {
            let message = read_message(&mut self.status)?;
            match message.kind {
                LauncherMessage::FINISHED => {
                    use std::os::unix::process::ExitStatusExt as _;
                    let status = ExitStatus::from_raw(message.value as i32);
                    self.final_status = Some(status);
                    self.probe.record(LifecycleEvent::GuestReaped);
                    return Ok(Some(status));
                }
                LauncherMessage::TERM_SENT => {
                    self.verify_anchor("term")?;
                    self.probe.record(LifecycleEvent::TermSent);
                }
                LauncherMessage::KILL_SENT => {
                    self.verify_anchor("kill")?;
                    self.probe.record(LifecycleEvent::KillSent);
                }
                LauncherMessage::ERROR => {
                    return Err(io::Error::other(format!(
                        "launcher falhou fechado: {}",
                        message.value
                    )));
                }
                _ => return Err(io::Error::other("mensagem inesperada do launcher")),
            }
        }
        Ok(None)
    }

    pub(super) fn wait_final(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.try_final()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "launcher não confirmou encerramento da árvore",
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait_ms = remaining.as_millis().min(20) as i32;
            let _ = poll_readable(self.status.as_raw_fd(), wait_ms)?;
        }
    }

    fn verify_anchor(&self, stage: &str) -> io::Result<()> {
        if process_start_time(self.identity.pid) == Some(self.identity.start_time) {
            self.probe
                .record(LifecycleEvent::LauncherAnchorVerified(match stage {
                    "term" => ShutdownSignal::Term,
                    "kill" => ShutdownSignal::Kill,
                    _ => return Err(io::Error::other("estágio de sinal inválido")),
                }));
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "âncora do launcher perdida durante {stage}"
            )))
        }
    }

    pub(super) fn reap(&mut self) -> io::Result<()> {
        if self.reaped {
            return Ok(());
        }
        let status = self.child.wait()?;
        self.reaped = true;
        self.control.take();
        self.controller_life.take();
        if status.success() {
            self.probe.record(LifecycleEvent::LauncherReaped);
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "launcher terminou sem provar a árvore: {status}"
            )))
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for ProcessLauncher {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.request_termination();
            self.controller_life.take();
            let _ = self.wait_final(Duration::from_secs(5));
            let _ = self.reap();
        }
    }
}

#[cfg(target_os = "linux")]
fn reap_cancelled_child(mut child: Child, probe: &LifecycleProbe) -> io::Result<()> {
    let status = child.wait()?;
    probe.record(LifecycleEvent::LauncherReaped);
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "launcher cancelado terminou com {status}"
        )))
    }
}

#[cfg(target_os = "linux")]
fn read_message(reader: &mut File) -> io::Result<LauncherMessage> {
    let mut bytes = [0_u8; 16];
    reader.read_exact(&mut bytes)?;
    Ok(LauncherMessage::decode(bytes))
}

#[cfg(target_os = "linux")]
fn poll_readable(fd: i32, timeout_ms: i32) -> io::Result<bool> {
    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        returned: i16,
    }
    extern "C" {
        fn poll(fds: *mut PollFd, count: usize, timeout: i32) -> i32;
    }
    const POLLIN: i16 = 1;
    const POLLHUP: i16 = 16;
    let mut descriptor = PollFd {
        fd,
        events: POLLIN,
        returned: 0,
    };
    let result = unsafe { poll(&mut descriptor, 1, timeout_ms) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result > 0 && descriptor.returned & (POLLIN | POLLHUP) != 0)
    }
}

#[cfg(target_os = "linux")]
fn write_fd(fd: i32, byte: u8) -> io::Result<()> {
    extern "C" {
        fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
    }
    loop {
        let result = unsafe { write(fd, &byte, 1) };
        if result == 1 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn injected(stage: &str) -> io::Error {
    io::Error::other(format!("falha injetada em {stage}"))
}

#[cfg(target_os = "linux")]
unsafe fn launcher_pre_exec(
    status_fd: i32,
    gate_fd: i32,
    control_fd: i32,
    life_fd: i32,
    inherited_fds: &[i32],
    policy: ResourcePolicy,
    fail_before_ready: bool,
) -> io::Result<()> {
    #[repr(C)]
    struct RLimit {
        current: u64,
        maximum: u64,
    }
    extern "C" {
        fn close(fd: i32) -> i32;
        fn fork() -> i32;
        fn getpid() -> i32;
        fn getppid() -> i32;
        fn getrlimit(resource: i32, limit: *mut RLimit) -> i32;
        fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
        fn signal(signal: i32, handler: usize) -> usize;
        fn setpgid(pid: i32, pgid: i32) -> i32;
        fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
        fn _exit(status: i32) -> !;
    }
    const PR_SET_PDEATHSIG: i32 = 1;
    const PR_SET_CHILD_SUBREAPER: i32 = 36;
    const SIGKILL: u64 = 9;
    const SIGPIPE: i32 = 13;
    const SIG_IGN: usize = 1;
    const SIG_ERR: usize = usize::MAX;
    const RLIMIT_CPU: i32 = 0;
    const RLIMIT_CORE: i32 = 4;
    const RLIMIT_AS: i32 = 9;
    let controller_pid = getppid();
    raw_close_snapshot_fds(
        inherited_fds,
        &[0, 1, 2, status_fd, gate_fd, control_fd, life_fd],
    );
    // O launcher é a primeira autoridade criada por fork. Antes de publicar
    // READY, elimine descritores não-CLOEXEC e qualquer arquivo gravável e
    // seekable herdado do harness. O único descritor desconhecido que precisa
    // sobreviver até exec é o pipe CLOEXEC interno de std::process::Command.
    raw_prepare_exec_fds(&[0, 1, 2, status_fd, gate_fd, control_fd, life_fd]);
    if setpgid(0, 0) != 0 || prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) != 0 {
        _exit(120);
    }
    let guest_sigpipe = signal(SIGPIPE, SIG_IGN);
    if guest_sigpipe == SIG_ERR {
        return Err(io::Error::last_os_error());
    }
    let launcher_pid = getpid();
    if fail_before_ready {
        _exit(121);
    }
    raw_write_message(
        status_fd,
        LauncherMessage {
            kind: LauncherMessage::READY,
            pid: launcher_pid,
            value: launcher_pid as i64,
        },
    )?;
    if !raw_wait_for_gate(gate_fd, control_fd, life_fd) {
        _exit(0);
    }
    let guest = fork();
    if guest < 0 {
        let errno = io::Error::last_os_error().raw_os_error().unwrap_or(-1);
        let _ = raw_write_message(
            status_fd,
            LauncherMessage {
                kind: LauncherMessage::ERROR,
                pid: launcher_pid,
                value: errno as i64,
            },
        );
        _exit(122);
    }
    if guest == 0 {
        close(status_fd);
        close(gate_fd);
        close(control_fd);
        close(life_fd);
        // O convidado conserva stdio e o pipe CLOEXEC interno de exec somente
        // até retornar a std::process::Command. O kernel fecha o último na
        // própria fronteira; a ferramenta externa recebe apenas 0, 1 e 2.
        raw_prepare_exec_fds(&[0, 1, 2]);
        if prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) != 0 || getppid() != launcher_pid {
            return Err(io::Error::last_os_error());
        }
        if signal(SIGPIPE, guest_sigpipe) == SIG_ERR {
            return Err(io::Error::last_os_error());
        }
        let set_soft = |resource: i32, value: u64| {
            let mut limit = RLimit {
                current: 0,
                maximum: 0,
            };
            if getrlimit(resource, &mut limit) != 0 {
                return Err(io::Error::last_os_error());
            }
            limit.current = value.min(limit.maximum);
            if setrlimit(resource, &limit) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        set_soft(RLIMIT_CORE, 0)?;
        set_soft(RLIMIT_AS, policy.address_space_bytes)?;
        set_soft(RLIMIT_CPU, policy.cpu_seconds)?;
        return Ok(());
    }
    close(gate_fd);
    // Depois do fork, o launcher conserva apenas seus três canais de
    // autoridade; stdio e os canais do convidado não lhe pertencem.
    raw_close_fds_except(&[status_fd, control_fd, life_fd]);
    let _ = raw_write_message(
        status_fd,
        LauncherMessage {
            kind: LauncherMessage::GUEST,
            pid: guest,
            value: 0,
        },
    );
    let raw_status = raw_supervise_tree(
        launcher_pid,
        guest,
        status_fd,
        control_fd,
        life_fd,
        controller_pid,
    );
    match raw_status {
        Ok(status) => {
            let _ = raw_write_message(
                status_fd,
                LauncherMessage {
                    kind: LauncherMessage::FINISHED,
                    pid: guest,
                    value: status as i64,
                },
            );
            _exit(0);
        }
        Err(error) => {
            let _ = raw_write_message(
                status_fd,
                LauncherMessage {
                    kind: LauncherMessage::ERROR,
                    pid: launcher_pid,
                    value: error.raw_os_error().unwrap_or(-1) as i64,
                },
            );
            _exit(123);
        }
    }
}

#[cfg(target_os = "linux")]
unsafe fn raw_wait_for_gate(gate_fd: i32, control_fd: i32, life_fd: i32) -> bool {
    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        returned: i16,
    }
    extern "C" {
        fn poll(fds: *mut PollFd, count: usize, timeout: i32) -> i32;
        fn read(fd: i32, buffer: *mut u8, count: usize) -> isize;
    }
    const POLLIN: i16 = 1;
    const POLLHUP: i16 = 16;
    loop {
        let mut fds = [
            PollFd {
                fd: gate_fd,
                events: POLLIN,
                returned: 0,
            },
            PollFd {
                fd: control_fd,
                events: POLLIN,
                returned: 0,
            },
            PollFd {
                fd: life_fd,
                events: POLLIN,
                returned: 0,
            },
        ];
        if poll(fds.as_mut_ptr(), fds.len(), -1) < 0 {
            continue;
        }
        for (index, fd) in fds.iter().enumerate() {
            if fd.returned & (POLLIN | POLLHUP) == 0 {
                continue;
            }
            let mut byte = 0_u8;
            let count = read(fd.fd, &mut byte, 1);
            if index == 0 && count == 1 && byte == b'G' {
                return true;
            }
            return false;
        }
    }
}

#[cfg(target_os = "linux")]
unsafe fn raw_close_fds_except(allowed: &[i32]) {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn close_range(first: u32, last: u32, flags: u32) -> i32;
        fn getrlimit(resource: i32, limit: *mut RLimitRaw) -> i32;
    }
    #[repr(C)]
    struct RLimitRaw {
        current: u64,
        maximum: u64,
    }
    const RLIMIT_NOFILE: i32 = 7;
    let mut limit = RLimitRaw {
        current: 65_536,
        maximum: 65_536,
    };
    let _ = getrlimit(RLIMIT_NOFILE, &mut limit);
    let maximum = limit.current.min(1_048_576) as u32;
    let mut first = 0_u32;
    // Encontre o próximo fd autorizado sem alocar ou ordenar no filho
    // pós-fork. close_range fecha cada lacuna com uma syscall; o laço é apenas
    // fallback para kernels sem close_range.
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
unsafe fn raw_close_snapshot_fds(snapshot: &[i32], allowed: &[i32]) {
    extern "C" {
        fn close(fd: i32) -> i32;
    }
    for &fd in snapshot {
        if !allowed.contains(&fd) {
            close(fd);
        }
    }
}

#[cfg(target_os = "linux")]
unsafe fn raw_prepare_exec_fds(allowed: &[i32]) {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn fcntl(fd: i32, command: i32, ...) -> i32;
        fn getdents64(fd: i32, directory: *mut u8, count: usize) -> isize;
        fn lseek(fd: i32, offset: i64, whence: i32) -> i64;
        fn open(path: *const i8, flags: i32, ...) -> i32;
    }
    const F_GETFD: i32 = 1;
    const F_GETFL: i32 = 3;
    const FD_CLOEXEC: i32 = 1;
    const O_RDONLY: i32 = 0;
    const O_DIRECTORY: i32 = 0o200000;
    const O_CLOEXEC: i32 = 0o2000000;
    const O_ACCMODE: i32 = 3;
    const O_WRONLY: i32 = 1;
    const O_RDWR: i32 = 2;
    const SEEK_CUR: i32 = 1;
    const DIRENT_NAME_OFFSET: usize = 19;
    let directory_fd = open(
        b"/proc/self/fd\0".as_ptr().cast(),
        O_RDONLY | O_DIRECTORY | O_CLOEXEC,
    );
    if directory_fd < 0 {
        return;
    }
    let mut buffer = [0_u8; 4096];
    loop {
        let read = getdents64(directory_fd, buffer.as_mut_ptr(), buffer.len());
        if read <= 0 {
            break;
        }
        let mut offset = 0_usize;
        while offset + DIRENT_NAME_OFFSET <= read as usize {
            let entry = buffer.as_ptr().add(offset);
            let record_length = u16::from_ne_bytes([*entry.add(16), *entry.add(17)]) as usize;
            if record_length < DIRENT_NAME_OFFSET || offset + record_length > read as usize {
                break;
            }
            let name = std::slice::from_raw_parts(
                entry.add(DIRENT_NAME_OFFSET),
                record_length - DIRENT_NAME_OFFSET,
            );
            let mut parsed = 0_i32;
            let mut numeric = false;
            for &byte in name.iter().take_while(|&&byte| byte != 0) {
                if !byte.is_ascii_digit() {
                    numeric = false;
                    break;
                }
                numeric = true;
                parsed = parsed
                    .saturating_mul(10)
                    .saturating_add((byte - b'0') as i32);
            }
            if numeric && parsed != directory_fd && !allowed.contains(&parsed) {
                let descriptor_flags = fcntl(parsed, F_GETFD);
                let status_flags = fcntl(parsed, F_GETFL);
                let writable = matches!(status_flags & O_ACCMODE, O_WRONLY | O_RDWR);
                let seekable = writable && lseek(parsed, 0, SEEK_CUR) >= 0;
                if descriptor_flags >= 0 && (descriptor_flags & FD_CLOEXEC == 0 || seekable) {
                    close(parsed);
                }
            }
            offset += record_length;
        }
    }
    close(directory_fd);
}

#[cfg(target_os = "linux")]
unsafe fn raw_supervise_tree(
    launcher_pid: i32,
    guest_pid: i32,
    status_fd: i32,
    control_fd: i32,
    life_fd: i32,
    controller_pid: i32,
) -> io::Result<i32> {
    extern "C" {
        fn getppid() -> i32;
        fn poll(fds: *mut PollFdRaw, count: usize, timeout: i32) -> i32;
        fn read(fd: i32, buffer: *mut u8, count: usize) -> isize;
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
    const WNOHANG: i32 = 1;
    const POLLIN: i16 = 1;
    const POLLHUP: i16 = 16;
    let mut guest_status = None;
    let mut terminate = false;
    loop {
        let mut status = 0_i32;
        loop {
            let waited = waitpid(-1, &mut status, WNOHANG);
            if waited <= 0 {
                break;
            }
            if waited == guest_pid {
                guest_status = Some(status);
                terminate = true;
            }
        }
        if terminate {
            break;
        }
        let mut fds = [
            PollFdRaw {
                fd: control_fd,
                events: POLLIN,
                returned: 0,
            },
            PollFdRaw {
                fd: life_fd,
                events: POLLIN,
                returned: 0,
            },
        ];
        if poll(fds.as_mut_ptr(), fds.len(), 10) > 0 {
            for fd in &fds {
                if fd.returned & (POLLIN | POLLHUP) != 0 {
                    let mut byte = 0_u8;
                    let count = read(fd.fd, &mut byte, 1);
                    if count <= 0 || byte == b'T' || byte == b'D' {
                        terminate = true;
                    }
                }
            }
        }
        if getppid() != controller_pid {
            terminate = true;
        }
    }
    let _ = raw_write_message(
        status_fd,
        LauncherMessage {
            kind: LauncherMessage::TERM_SENT,
            pid: launcher_pid,
            value: 0,
        },
    );
    let _ = raw_signal_descendants(launcher_pid, 15)?;
    let term_deadline = raw_monotonic_millis().saturating_add(200);
    while raw_monotonic_millis() < term_deadline {
        raw_reap_all(guest_pid, &mut guest_status);
        if raw_descendants_exist(launcher_pid)?.proves_absence() {
            return Ok(guest_status.unwrap_or(0));
        }
        raw_sleep_millis(5);
    }
    let _ = raw_write_message(
        status_fd,
        LauncherMessage {
            kind: LauncherMessage::KILL_SENT,
            pid: launcher_pid,
            value: 0,
        },
    );
    let _ = raw_signal_descendants(launcher_pid, 9)?;
    let kill_deadline = raw_monotonic_millis().saturating_add(5_000);
    loop {
        raw_reap_all(guest_pid, &mut guest_status);
        if raw_descendants_exist(launcher_pid)?.proves_absence() {
            return Ok(guest_status.unwrap_or(9));
        }
        if raw_monotonic_millis() >= kill_deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "árvore permaneceu executável após KILL",
            ));
        }
        let _ = raw_signal_descendants(launcher_pid, 9)?;
        raw_sleep_millis(5);
    }
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct PollFdRaw {
    fd: i32,
    events: i16,
    returned: i16,
}

#[cfg(target_os = "linux")]
unsafe fn raw_reap_all(guest_pid: i32, guest_status: &mut Option<i32>) {
    extern "C" {
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
    const WNOHANG: i32 = 1;
    loop {
        let mut status = 0_i32;
        let waited = waitpid(-1, &mut status, WNOHANG);
        if waited <= 0 {
            return;
        }
        if waited == guest_pid {
            *guest_status = Some(status);
        }
    }
}

#[cfg(target_os = "linux")]
/// Resultado de uma passagem completa de enumeração de `/proc`.
///
/// Substitui a tabela global de 4096 entradas. O teto anterior não truncava
/// silenciosamente: `raw_scan_processes` devolvia erro ao excedê-lo. O erro,
/// porém, era propagado por `?` a partir de `raw_signal_descendants`, de modo
/// que estourar o limite fazia o launcher retornar **antes de enviar o
/// primeiro TERM**. TERM e KILL deixavam de ser enviados, o laço de reaping
/// não corria e a árvore sobrevivia. Era fail-closed para relatar e fail-open
/// para conter.
#[derive(Clone, Copy)]
struct ScanSummary {
    /// Descendentes positivamente identificados nesta passagem.
    descendants: u32,
    /// Descendentes que receberam o sinal nesta passagem.
    signaled: u32,
    /// Entradas cuja ancestralidade não pôde ser estabelecida.
    unknown: u32,
}

impl ScanSummary {
    const EMPTY: Self = Self {
        descendants: 0,
        signaled: 0,
        unknown: 0,
    };

    /// A ausência só é provada por uma varredura completa e sem incógnitas.
    /// Identidade desconhecida jamais é convertida em prova de encerramento.
    fn proves_absence(&self) -> bool {
        self.descendants == 0 && self.unknown == 0
    }
}

/// Classificação de uma entrada em relação à árvore do launcher.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq)]
enum Ancestry {
    /// Cadeia de pais alcança o launcher.
    Descendant,
    /// Cadeia de pais alcança a raiz sem passar pelo launcher.
    Foreign,
    /// O processo desapareceu durante a inspeção: já não está executável.
    Gone,
    /// Permissão, parse inválido, I/O não explicado ou sequência patológica.
    Unknown,
}

#[cfg(target_os = "linux")]
enum ParentLookup {
    Parent(i32),
    Root,
    Gone,
    Unknown,
}

/// Lê o pai de um PID sob demanda, sem alocação e sem tabela.
#[cfg(target_os = "linux")]
unsafe fn raw_parent_of(proc_fd: i32, pid: i32) -> ParentLookup {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn openat(directory: i32, path: *const i8, flags: i32, ...) -> i32;
        fn read(fd: i32, buffer: *mut u8, count: usize) -> isize;
    }
    const O_RDONLY: i32 = 0;
    const O_CLOEXEC: i32 = 0o2000000;
    const ENOENT: i32 = 2;
    const ESRCH: i32 = 3;
    if pid <= 1 {
        return ParentLookup::Root;
    }
    let mut path = [0_u8; 32];
    let _ = raw_pid_stat_path(pid, &mut path);
    let stat_fd = openat(proc_fd, path.as_ptr().cast(), O_RDONLY | O_CLOEXEC);
    if stat_fd < 0 {
        let code = io::Error::last_os_error().raw_os_error().unwrap_or(0);
        // O processo desapareceu entre a enumeração e a leitura.
        if code == ENOENT || code == ESRCH {
            return ParentLookup::Gone;
        }
        return ParentLookup::Unknown;
    }
    // Buffer de leitura de `stat`, não teto de quantidade de processos.
    let mut stat = [0_u8; 4096];
    let read_count = read(stat_fd, stat.as_mut_ptr(), stat.len());
    close(stat_fd);
    if read_count <= 0 {
        return ParentLookup::Gone;
    }
    match raw_parse_parent(&stat[..read_count as usize]) {
        Some(parent) if parent <= 1 => ParentLookup::Root,
        Some(parent) => ParentLookup::Parent(parent),
        // PPID zero identifica a raiz do kernel. `raw_parse_parent` o rejeita
        // por exigir inteiro positivo, e a varredura anterior simplesmente
        // descartava a entrada em silencio. Classificar isso como incognita
        // faria toda thread de kernel, a comecar por kthreadd, impedir para
        // sempre a prova de ausencia da arvore.
        None if raw_stat_parent_is_zero(&stat[..read_count as usize]) => ParentLookup::Root,
        None => ParentLookup::Unknown,
    }
}

/// Reconhece `ppid` igual a zero, a raiz do kernel.
#[cfg(target_os = "linux")]
fn raw_stat_parent_is_zero(stat: &[u8]) -> bool {
    let Some(close) = stat.windows(2).rposition(|window| window == b") ") else {
        return false;
    };
    let mut fields = stat[close + 2..]
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|value| !value.is_empty());
    if fields.next().is_none() {
        return false;
    }
    matches!(fields.next(), Some(b"0"))
}

/// Determina a ancestralidade sob demanda, percorrendo a cadeia de pais.
///
/// Iterativo, nunca recursivo. Sequências patológicas são detectadas por
/// ponteiros lento e rápido sobre a mesma cadeia, em espaço constante, sem
/// reintroduzir teto arbitrário de quantidade de processos.
#[cfg(target_os = "linux")]
unsafe fn raw_ancestry(proc_fd: i32, candidate: i32, launcher: i32) -> Ancestry {
    if candidate == launcher || candidate <= 1 {
        return Ancestry::Foreign;
    }
    // Uma cadeia pode quebrar porque um ancestral saiu durante a caminhada.
    // Em sistema ativo isso e rotina e nao constitui incognita sobre a arvore.
    //
    // O launcher e subreaper, de modo que um orfao pertencente a arvore e
    // reparentado ao proprio launcher: repetir a caminhada passa a encontra-lo
    // diretamente. Um processo alheio, reparentado a init ou a outro subreaper,
    // resolve como estrangeiro. Sobra como incognita apenas o que persiste,
    // tipicamente permissao ou parse invalido.
    const TENTATIVAS: u32 = 3;
    for tentativa in 0..TENTATIVAS {
        match raw_walk_ancestry(proc_fd, candidate, launcher) {
            Ancestry::Unknown if tentativa + 1 < TENTATIVAS => {
                // Se o proprio candidato ja nao existe, nada ha a conter.
                if let ParentLookup::Gone = raw_parent_of(proc_fd, candidate) {
                    return Ancestry::Gone;
                }
            }
            outcome => return outcome,
        }
    }
    Ancestry::Unknown
}

/// Uma unica caminhada da cadeia de pais, sem repeticao.
#[cfg(target_os = "linux")]
unsafe fn raw_walk_ancestry(proc_fd: i32, candidate: i32, launcher: i32) -> Ancestry {
    let mut slow = candidate;
    let mut fast = candidate;
    loop {
        match raw_parent_of(proc_fd, slow) {
            ParentLookup::Parent(parent) => slow = parent,
            ParentLookup::Root => return Ancestry::Foreign,
            ParentLookup::Gone => {
                return if slow == candidate {
                    Ancestry::Gone
                } else {
                    // Um ancestral sumiu: a cadeia não pode ser fechada.
                    Ancestry::Unknown
                };
            }
            ParentLookup::Unknown => return Ancestry::Unknown,
        }
        if slow == launcher {
            return Ancestry::Descendant;
        }
        for _ in 0..2 {
            match raw_parent_of(proc_fd, fast) {
                ParentLookup::Parent(parent) => fast = parent,
                ParentLookup::Root => return Ancestry::Foreign,
                ParentLookup::Gone => {
                    return if fast == candidate {
                        Ancestry::Gone
                    } else {
                        Ancestry::Unknown
                    };
                }
                ParentLookup::Unknown => return Ancestry::Unknown,
            }
            if fast == launcher {
                return Ancestry::Descendant;
            }
        }
        if slow == fast {
            return Ancestry::Unknown;
        }
    }
}

/// Uma passagem completa e incremental sobre `/proc`.
///
/// Nenhuma entrada é armazenada: cada PID é classificado e, quando exigido,
/// sinalizado antes de a próxima ser lida. Erro em uma entrada nunca impede a
/// sinalização das demais, e a passagem só termina após percorrer o diretório
/// inteiro.
#[cfg(target_os = "linux")]
unsafe fn raw_scan_pass(launcher_pid: i32, signal: Option<i32>) -> io::Result<ScanSummary> {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn open(path: *const i8, flags: i32, ...) -> i32;
        fn syscall(number: isize, ...) -> isize;
    }
    const O_RDONLY: i32 = 0;
    const O_DIRECTORY: i32 = 0o200000;
    const O_CLOEXEC: i32 = 0o2000000;
    const SYS_GETDENTS64: isize = 217;
    const SYS_PIDFD_OPEN: isize = 434;
    const SYS_PIDFD_SEND_SIGNAL: isize = 424;

    let proc_fd = open(c"/proc".as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if proc_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut summary = ScanSummary::EMPTY;
    let mut directory_buffer = [0_u8; 8192];
    loop {
        let bytes = syscall(
            SYS_GETDENTS64,
            proc_fd,
            directory_buffer.as_mut_ptr(),
            directory_buffer.len(),
        );
        if bytes < 0 {
            let error = io::Error::last_os_error();
            close(proc_fd);
            return Err(error);
        }
        if bytes == 0 {
            break;
        }
        let mut offset = 0_usize;
        while offset < bytes as usize {
            if offset + 19 > bytes as usize {
                close(proc_fd);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "getdents truncado",
                ));
            }
            let reclen =
                u16::from_ne_bytes([directory_buffer[offset + 16], directory_buffer[offset + 17]])
                    as usize;
            if reclen < 20 || offset + reclen > bytes as usize {
                close(proc_fd);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "dirent inválido",
                ));
            }
            let name_bytes = &directory_buffer[offset + 19..offset + reclen];
            let name_len = name_bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(name_bytes.len());
            // Entrada não numérica de /proc é ignorada, não é incógnita.
            if let Some(pid) = raw_parse_positive(&name_bytes[..name_len]) {
                match raw_ancestry(proc_fd, pid, launcher_pid) {
                    Ancestry::Descendant => {
                        summary.descendants += 1;
                        if let Some(signal) = signal {
                            let pidfd = syscall(SYS_PIDFD_OPEN, pid, 0_u32) as i32;
                            if pidfd >= 0 {
                                // Revalida a ancestralidade imediatamente antes
                                // do sinal: o pidfd fixa a identidade, e a
                                // reconfirmação impede sinalizar um PID
                                // reutilizado fora da árvore.
                                if raw_ancestry(proc_fd, pid, launcher_pid) == Ancestry::Descendant
                                {
                                    let sent = syscall(
                                        SYS_PIDFD_SEND_SIGNAL,
                                        pidfd,
                                        signal,
                                        std::ptr::null::<u8>(),
                                        0_u32,
                                    );
                                    if sent == 0 {
                                        summary.signaled += 1;
                                    }
                                }
                                close(pidfd);
                            }
                        }
                    }
                    Ancestry::Unknown => summary.unknown += 1,
                    // Estrangeiro ou já não executável: nada a conter.
                    Ancestry::Foreign | Ancestry::Gone => {}
                }
            }
            offset += reclen;
        }
    }
    close(proc_fd);
    Ok(summary)
}

#[cfg(target_os = "linux")]
fn raw_pid_stat_path(pid: i32, buffer: &mut [u8; 32]) -> usize {
    let mut digits = [0_u8; 12];
    let mut value = pid as u32;
    let mut length = 0;
    loop {
        digits[length] = b'0' + (value % 10) as u8;
        length += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for index in 0..length {
        buffer[index] = digits[length - index - 1];
    }
    buffer[length..length + 5].copy_from_slice(b"/stat");
    buffer[length + 5] = 0;
    length + 5
}

#[cfg(target_os = "linux")]
fn raw_parse_positive(bytes: &[u8]) -> Option<i32> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0_i32;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((byte - b'0') as i32)?;
    }
    (value > 0).then_some(value)
}

#[cfg(target_os = "linux")]
fn raw_parse_parent(stat: &[u8]) -> Option<i32> {
    let close = stat.windows(2).rposition(|window| window == b") ")?;
    let suffix = &stat[close + 2..];
    let mut fields = suffix
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|v| !v.is_empty());
    fields.next()?;
    raw_parse_positive(fields.next()?)
}

#[cfg(target_os = "linux")]
unsafe fn raw_descendants_exist(launcher_pid: i32) -> io::Result<ScanSummary> {
    raw_scan_pass(launcher_pid, None)
}

#[cfg(target_os = "linux")]
unsafe fn raw_signal_descendants(launcher_pid: i32, signal: i32) -> io::Result<ScanSummary> {
    raw_scan_pass(launcher_pid, Some(signal))
}

#[cfg(target_os = "linux")]
unsafe fn raw_write_message(fd: i32, message: LauncherMessage) -> io::Result<()> {
    extern "C" {
        fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
    }
    let bytes = message.encode();
    let mut offset = 0;
    while offset < bytes.len() {
        let written = write(fd, bytes[offset..].as_ptr(), bytes.len() - offset);
        if written < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        offset += written as usize;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
unsafe fn raw_monotonic_millis() -> u64 {
    #[repr(C)]
    struct Timespec {
        seconds: i64,
        nanoseconds: i64,
    }
    extern "C" {
        fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
    }
    const CLOCK_MONOTONIC: i32 = 1;
    let mut value = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    let _ = clock_gettime(CLOCK_MONOTONIC, &mut value);
    value.seconds.max(0) as u64 * 1000 + (value.nanoseconds.max(0) as u64 / 1_000_000)
}

#[cfg(target_os = "linux")]
unsafe fn raw_sleep_millis(milliseconds: u64) {
    #[repr(C)]
    struct Timespec {
        seconds: i64,
        nanoseconds: i64,
    }
    extern "C" {
        fn nanosleep(request: *const Timespec, remaining: *mut Timespec) -> i32;
    }
    let request = Timespec {
        seconds: (milliseconds / 1000) as i64,
        nanoseconds: ((milliseconds % 1000) * 1_000_000) as i64,
    };
    let _ = nanosleep(&request, std::ptr::null_mut());
}

#[cfg(not(target_os = "linux"))]
pub(super) struct ProcessLauncher;

#[cfg(not(target_os = "linux"))]
impl ProcessLauncher {
    pub(super) fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "launcher permanente requer Linux",
        )
    }
}
