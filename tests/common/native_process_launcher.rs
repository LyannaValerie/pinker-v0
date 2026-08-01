use super::{process_start_time, ResourcePolicy};
use std::fs::File;
use std::io::{self, Read as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::process::{Child, Command as StdCommand, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

#[derive(Clone, Default)]
pub struct LifecycleProbe {
    events: Arc<Mutex<Vec<String>>>,
    hold_guest_gate: Arc<AtomicBool>,
}

impl LifecycleProbe {
    pub fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn hold_guest_gate_for_test(&self) {
        self.hold_guest_gate.store(true, Ordering::SeqCst);
    }

    pub fn release_guest_gate_for_test(&self) {
        self.hold_guest_gate.store(false, Ordering::SeqCst);
    }

    pub(super) fn record(&self, event: impl Into<String>) {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event.into());
    }

    fn wait_guest_gate(&self) {
        while self.hold_guest_gate.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LauncherIdentity {
    pub pid: u32,
    pub start_time: u64,
    pub pgid: i32,
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
        let (status_read, status_write) = pipe_cloexec()?;
        let (gate_read, gate_write) = pipe_cloexec()?;
        let (control_read, control_write) = pipe_cloexec()?;
        let (life_read, life_write) = pipe_cloexec()?;
        let status_fd = status_write.as_raw_fd();
        let gate_fd = gate_read.as_raw_fd();
        let control_fd = control_read.as_raw_fd();
        let life_fd = life_read.as_raw_fd();
        let inherited_parent_fds = [
            status_read.as_raw_fd(),
            gate_write.as_raw_fd(),
            control_write.as_raw_fd(),
            life_write.as_raw_fd(),
        ];
        let fail_before_ready = failure == Some(StartupFailurePoint::AfterLauncherBeforeReady);
        unsafe {
            command.pre_exec(move || {
                launcher_pre_exec(
                    status_fd,
                    gate_fd,
                    control_fd,
                    life_fd,
                    inherited_parent_fds,
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
            let launcher_pid = ready.pid as u32;
            let start_time = process_start_time(launcher_pid)
                .ok_or_else(|| io::Error::other("identidade do launcher não pôde ser provada"))?;
            let identity = LauncherIdentity {
                pid: launcher_pid,
                start_time,
                pgid: ready.pid,
            };
            probe.record(format!("launcher_ready:{}", identity.pid));
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
            probe.record("guest_gate_opened");
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
        probe.record(format!("guest_started:{}", guest.pid));
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
        self.probe.record("termination_requested");
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
                    return Ok(Some(status));
                }
                LauncherMessage::TERM_SENT => {
                    self.verify_anchor("term")?;
                    self.probe.record("launcher_term_sent");
                }
                LauncherMessage::KILL_SENT => {
                    self.verify_anchor("kill")?;
                    self.probe.record("launcher_kill_sent_after_200ms");
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
                .record(format!("launcher_anchor_verified_{stage}"));
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
            self.probe.record("launcher_reaped");
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
    probe.record("launcher_reaped");
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
    inherited_parent_fds: [i32; 4],
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
    for fd in inherited_parent_fds {
        close(fd);
    }
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
    raw_close_except(status_fd, control_fd, life_fd);
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
unsafe fn raw_close_except(first: i32, second: i32, third: i32) {
    extern "C" {
        fn close(fd: i32) -> i32;
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
    let maximum = limit.current.min(1_048_576) as i32;
    for fd in 0..maximum {
        if fd != first && fd != second && fd != third {
            close(fd);
        }
    }
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
    raw_signal_descendants(launcher_pid, 15)?;
    let term_deadline = raw_monotonic_millis().saturating_add(200);
    while raw_monotonic_millis() < term_deadline {
        raw_reap_all(guest_pid, &mut guest_status);
        if !raw_descendants_exist(launcher_pid)? {
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
    raw_signal_descendants(launcher_pid, 9)?;
    let kill_deadline = raw_monotonic_millis().saturating_add(5_000);
    loop {
        raw_reap_all(guest_pid, &mut guest_status);
        if !raw_descendants_exist(launcher_pid)? {
            return Ok(guest_status.unwrap_or(9));
        }
        if raw_monotonic_millis() >= kill_deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "árvore permaneceu executável após KILL",
            ));
        }
        raw_signal_descendants(launcher_pid, 9)?;
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
#[derive(Clone, Copy)]
struct ProcEntry {
    pid: i32,
    parent: i32,
}

#[cfg(target_os = "linux")]
unsafe fn raw_scan_processes(entries: &mut [ProcEntry; 4096]) -> io::Result<usize> {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn open(path: *const i8, flags: i32, ...) -> i32;
        fn openat(directory: i32, path: *const i8, flags: i32, ...) -> i32;
        fn read(fd: i32, buffer: *mut u8, count: usize) -> isize;
        fn syscall(number: isize, ...) -> isize;
    }
    const O_RDONLY: i32 = 0;
    const O_DIRECTORY: i32 = 0o200000;
    const O_CLOEXEC: i32 = 0o2000000;
    const SYS_GETDENTS64: isize = 217;
    let proc_fd = open(c"/proc".as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if proc_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut count = 0_usize;
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
            if let Some(pid) = raw_parse_positive(&name_bytes[..name_len]) {
                let mut path = [0_u8; 32];
                let path_len = raw_pid_stat_path(pid, &mut path);
                let stat_fd = openat(proc_fd, path.as_ptr().cast(), O_RDONLY | O_CLOEXEC);
                if stat_fd >= 0 {
                    let mut stat = [0_u8; 4096];
                    let read_count = read(stat_fd, stat.as_mut_ptr(), stat.len());
                    close(stat_fd);
                    if read_count > 0 {
                        if let Some(parent) = raw_parse_parent(&stat[..read_count as usize]) {
                            if count == entries.len() {
                                close(proc_fd);
                                return Err(io::Error::other("limite de processos excedido"));
                            }
                            entries[count] = ProcEntry { pid, parent };
                            count += 1;
                        }
                    }
                }
                let _ = path_len;
            }
            offset += reclen;
        }
    }
    close(proc_fd);
    Ok(count)
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
fn raw_is_descendant(entries: &[ProcEntry], candidate: i32, launcher: i32) -> bool {
    let mut current = candidate;
    for _ in 0..entries.len() {
        let Some(entry) = entries.iter().find(|entry| entry.pid == current) else {
            return false;
        };
        if entry.parent == launcher {
            return true;
        }
        if entry.parent <= 1 || entry.parent == current {
            return false;
        }
        current = entry.parent;
    }
    false
}

#[cfg(target_os = "linux")]
unsafe fn raw_descendants_exist(launcher_pid: i32) -> io::Result<bool> {
    let mut entries = [ProcEntry { pid: 0, parent: 0 }; 4096];
    let count = raw_scan_processes(&mut entries)?;
    Ok(entries[..count]
        .iter()
        .any(|entry| raw_is_descendant(&entries[..count], entry.pid, launcher_pid)))
}

#[cfg(target_os = "linux")]
unsafe fn raw_signal_descendants(launcher_pid: i32, signal: i32) -> io::Result<()> {
    extern "C" {
        fn close(fd: i32) -> i32;
        fn syscall(number: isize, ...) -> isize;
    }
    const SYS_PIDFD_OPEN: isize = 434;
    const SYS_PIDFD_SEND_SIGNAL: isize = 424;
    let mut entries = [ProcEntry { pid: 0, parent: 0 }; 4096];
    let count = raw_scan_processes(&mut entries)?;
    for entry in &entries[..count] {
        if !raw_is_descendant(&entries[..count], entry.pid, launcher_pid) {
            continue;
        }
        let pidfd = syscall(SYS_PIDFD_OPEN, entry.pid, 0_u32) as i32;
        if pidfd < 0 {
            continue;
        }
        let still_descendant = {
            let mut refreshed = [ProcEntry { pid: 0, parent: 0 }; 4096];
            match raw_scan_processes(&mut refreshed) {
                Ok(refreshed_count) => {
                    raw_is_descendant(&refreshed[..refreshed_count], entry.pid, launcher_pid)
                }
                Err(error) => {
                    close(pidfd);
                    return Err(error);
                }
            }
        };
        if still_descendant {
            let _ = syscall(
                SYS_PIDFD_SEND_SIGNAL,
                pidfd,
                signal,
                std::ptr::null::<u8>(),
                0_u32,
            );
        }
        close(pidfd);
    }
    Ok(())
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
