use std::fmt::{self, Write as _};
use std::io;
use std::process::ExitStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminationReason {
    WatchdogExit,
    ControllerLost,
    LauncherFailure,
    StdoutLimit,
    StderrLimit,
    Timeout,
    StartupFailure,
    GuestExited,
}

impl TerminationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WatchdogExit => "watchdog_exit",
            Self::ControllerLost => "controller_lost",
            Self::LauncherFailure => "launcher_failure",
            Self::StdoutLimit => "stdout_limit",
            Self::StderrLimit => "stderr_limit",
            Self::Timeout => "timeout",
            Self::StartupFailure => "startup_failure",
            Self::GuestExited => "guest_exited",
        }
    }

    pub fn parse(value: &str) -> io::Result<Self> {
        match value {
            "watchdog_exit" => Ok(Self::WatchdogExit),
            "controller_lost" => Ok(Self::ControllerLost),
            "launcher_failure" => Ok(Self::LauncherFailure),
            "stdout_limit" => Ok(Self::StdoutLimit),
            "stderr_limit" => Ok(Self::StderrLimit),
            "timeout" => Ok(Self::Timeout),
            "startup_failure" => Ok(Self::StartupFailure),
            "guest_exited" => Ok(Self::GuestExited),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "causa de término inválida",
            )),
        }
    }
}

impl fmt::Display for TerminationReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObservedTerminationEvents {
    pub watchdog_exit: bool,
    pub controller_lost: bool,
    pub launcher_failure: bool,
    pub stdout_limit: bool,
    pub stderr_limit: bool,
    pub timeout: bool,
    pub startup_failure: bool,
    pub guest_exited: bool,
}

/// A causa já fixada nunca é substituída. Para eventos observados na mesma
/// iteração, perda de autoridade precede limites, que precedem saída normal.
pub fn select_primary_reason(
    current: Option<TerminationReason>,
    observed: ObservedTerminationEvents,
) -> Option<TerminationReason> {
    current.or_else(|| {
        [
            (observed.watchdog_exit, TerminationReason::WatchdogExit),
            (observed.controller_lost, TerminationReason::ControllerLost),
            (
                observed.launcher_failure,
                TerminationReason::LauncherFailure,
            ),
            (observed.stdout_limit, TerminationReason::StdoutLimit),
            (observed.stderr_limit, TerminationReason::StderrLimit),
            (observed.timeout, TerminationReason::Timeout),
            (observed.startup_failure, TerminationReason::StartupFailure),
            (observed.guest_exited, TerminationReason::GuestExited),
        ]
        .into_iter()
        .find_map(|(seen, reason)| seen.then_some(reason))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessIdentitySnapshot {
    pub pid: u32,
    pub start_time: u64,
    pub pgid: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownStage {
    MarkerRunning,
    MarkerTerminating,
    TerminationRequest,
    WaitFinal,
    StdoutJoin,
    StderrJoin,
    FinalReap,
    WatchdogFinish,
    MarkerTerminal,
    Cleanup,
    Quarantine,
    EvidenceWrite,
}

impl ShutdownStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MarkerRunning => "marker-running",
            Self::MarkerTerminating => "marker-terminating",
            Self::TerminationRequest => "termination-request",
            Self::WaitFinal => "wait-final",
            Self::StdoutJoin => "stdout-join",
            Self::StderrJoin => "stderr-join",
            Self::FinalReap => "final-reap",
            Self::WatchdogFinish => "watchdog-finish",
            Self::MarkerTerminal => "marker-terminal",
            Self::Cleanup => "cleanup",
            Self::Quarantine => "quarantine",
            Self::EvidenceWrite => "evidence-write",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShutdownError {
    pub stage: ShutdownStage,
    pub kind: io::ErrorKind,
    pub message: String,
    pub process: Option<ProcessIdentitySnapshot>,
}

impl ShutdownError {
    pub fn injected(stage: ShutdownStage, process: Option<ProcessIdentitySnapshot>) -> Self {
        Self {
            stage,
            kind: io::ErrorKind::Other,
            message: format!("falha injetada em {}", stage.as_str()),
            process,
        }
    }

    pub(crate) fn from_io(
        stage: ShutdownStage,
        error: io::Error,
        process: Option<ProcessIdentitySnapshot>,
    ) -> Self {
        Self {
            stage,
            kind: error.kind(),
            message: error.to_string(),
            process,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SandboxDisposition {
    Removed,
    Preserved(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownFailurePoint {
    MarkerTerminal,
    WatchdogFinish,
    FinalReap,
    Cleanup,
    Quarantine,
    EvidenceWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownSignal {
    Term,
    Kill,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    LauncherReady(ProcessIdentitySnapshot),
    WatchdogReady(ProcessIdentitySnapshot),
    GuestGateOpened,
    GuestStarted(ProcessIdentitySnapshot),
    SandboxRunning,
    PrimaryReasonLatched(TerminationReason),
    WatchdogExitObserved(ProcessIdentitySnapshot),
    TermRequested,
    LauncherAnchorVerified(ShutdownSignal),
    TermSent,
    KillSent,
    GuestReaped,
    LauncherReaped,
    WatchdogReaped,
    SandboxRemoved,
    SandboxPreserved(String),
    SecondaryFailure(ShutdownError),
    ResultPublished,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LifecycleRecord {
    pub sequence: u64,
    pub event: LifecycleEvent,
}

impl LifecycleRecord {
    pub fn to_wire(&self) -> String {
        let payload = match &self.event {
            LifecycleEvent::LauncherReady(identity) => {
                format!("launcher_ready|{}", identity_wire(*identity))
            }
            LifecycleEvent::WatchdogReady(identity) => {
                format!("watchdog_ready|{}", identity_wire(*identity))
            }
            LifecycleEvent::GuestGateOpened => "guest_gate_opened".to_string(),
            LifecycleEvent::GuestStarted(identity) => {
                format!("guest_started|{}", identity_wire(*identity))
            }
            LifecycleEvent::SandboxRunning => "sandbox_running".to_string(),
            LifecycleEvent::PrimaryReasonLatched(reason) => {
                format!("primary_reason_latched|{}", reason.as_str())
            }
            LifecycleEvent::WatchdogExitObserved(identity) => {
                format!("watchdog_exit_observed|{}", identity_wire(*identity))
            }
            LifecycleEvent::TermRequested => "term_requested".to_string(),
            LifecycleEvent::LauncherAnchorVerified(ShutdownSignal::Term) => {
                "launcher_anchor_verified_term".to_string()
            }
            LifecycleEvent::LauncherAnchorVerified(ShutdownSignal::Kill) => {
                "launcher_anchor_verified_kill".to_string()
            }
            LifecycleEvent::TermSent => "term_sent".to_string(),
            LifecycleEvent::KillSent => "kill_sent".to_string(),
            LifecycleEvent::GuestReaped => "guest_reaped".to_string(),
            LifecycleEvent::LauncherReaped => "launcher_reaped".to_string(),
            LifecycleEvent::WatchdogReaped => "watchdog_reaped".to_string(),
            LifecycleEvent::SandboxRemoved => "sandbox_removed".to_string(),
            LifecycleEvent::SandboxPreserved(reason) => {
                format!("sandbox_preserved|{}", hex(reason.as_bytes()))
            }
            LifecycleEvent::SecondaryFailure(error) => {
                let (pid, start_time, pgid) = error.process.map_or_else(
                    || ("none".to_string(), "none".to_string(), "none".to_string()),
                    |identity| {
                        (
                            identity.pid.to_string(),
                            identity.start_time.to_string(),
                            identity
                                .pgid
                                .map_or_else(|| "none".to_string(), |pgid| pgid.to_string()),
                        )
                    },
                );
                format!(
                    "secondary_failure|{}|{}|{}|{}|{}|{}",
                    error.stage.as_str(),
                    error_kind_name(error.kind),
                    hex(error.message.as_bytes()),
                    pid,
                    start_time,
                    pgid
                )
            }
            LifecycleEvent::ResultPublished => "result_published".to_string(),
        };
        format!("{}|{payload}", self.sequence)
    }

    pub fn from_wire(wire: &str) -> io::Result<Self> {
        let mut fields = wire.trim_end().split('|');
        let sequence = parse_field::<u64>(fields.next(), "sequência")?;
        let kind = fields
            .next()
            .ok_or_else(|| invalid_wire("evento lifecycle ausente"))?;
        let event = match kind {
            "launcher_ready" => LifecycleEvent::LauncherReady(parse_identity(&mut fields)?),
            "watchdog_ready" => LifecycleEvent::WatchdogReady(parse_identity(&mut fields)?),
            "guest_gate_opened" => LifecycleEvent::GuestGateOpened,
            "guest_started" => LifecycleEvent::GuestStarted(parse_identity(&mut fields)?),
            "sandbox_running" => LifecycleEvent::SandboxRunning,
            "primary_reason_latched" => {
                LifecycleEvent::PrimaryReasonLatched(TerminationReason::parse(
                    fields
                        .next()
                        .ok_or_else(|| invalid_wire("causa lifecycle ausente"))?,
                )?)
            }
            "watchdog_exit_observed" => {
                LifecycleEvent::WatchdogExitObserved(parse_identity(&mut fields)?)
            }
            "term_requested" => LifecycleEvent::TermRequested,
            "launcher_anchor_verified_term" => {
                LifecycleEvent::LauncherAnchorVerified(ShutdownSignal::Term)
            }
            "launcher_anchor_verified_kill" => {
                LifecycleEvent::LauncherAnchorVerified(ShutdownSignal::Kill)
            }
            "term_sent" => LifecycleEvent::TermSent,
            "kill_sent" => LifecycleEvent::KillSent,
            "guest_reaped" => LifecycleEvent::GuestReaped,
            "launcher_reaped" => LifecycleEvent::LauncherReaped,
            "watchdog_reaped" => LifecycleEvent::WatchdogReaped,
            "sandbox_removed" => LifecycleEvent::SandboxRemoved,
            "sandbox_preserved" => LifecycleEvent::SandboxPreserved(parse_hex_field(
                fields.next(),
                "motivo de preservação",
            )?),
            "secondary_failure" => {
                let stage = parse_stage(
                    fields
                        .next()
                        .ok_or_else(|| invalid_wire("estágio secundário ausente"))?,
                )?;
                LifecycleEvent::SecondaryFailure(ShutdownError {
                    stage,
                    kind: parse_error_kind(
                        fields
                            .next()
                            .ok_or_else(|| invalid_wire("tipo de erro secundário ausente"))?,
                    )?,
                    message: parse_hex_field(fields.next(), "erro secundário")?,
                    process: parse_optional_identity(&mut fields)?,
                })
            }
            "result_published" => LifecycleEvent::ResultPublished,
            _ => return Err(invalid_wire("evento lifecycle desconhecido")),
        };
        if fields.next().is_some() {
            return Err(invalid_wire("campos extras no evento lifecycle"));
        }
        Ok(Self { sequence, event })
    }
}

fn identity_wire(identity: ProcessIdentitySnapshot) -> String {
    format!(
        "{}|{}|{}",
        identity.pid,
        identity.start_time,
        identity
            .pgid
            .map_or_else(|| "none".to_string(), |pgid| pgid.to_string())
    )
}

fn parse_identity<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
) -> io::Result<ProcessIdentitySnapshot> {
    let pid = parse_field::<u32>(fields.next(), "PID")?;
    let start_time = parse_field::<u64>(fields.next(), "start time")?;
    let pgid = match fields.next().ok_or_else(|| invalid_wire("PGID ausente"))? {
        "none" => None,
        value => Some(value.parse().map_err(|_| invalid_wire("PGID inválido"))?),
    };
    Ok(ProcessIdentitySnapshot {
        pid,
        start_time,
        pgid,
    })
}

fn parse_optional_identity<'a>(
    fields: &mut impl Iterator<Item = &'a str>,
) -> io::Result<Option<ProcessIdentitySnapshot>> {
    let pid = fields
        .next()
        .ok_or_else(|| invalid_wire("PID secundário ausente"))?;
    let start_time = fields
        .next()
        .ok_or_else(|| invalid_wire("start time secundário ausente"))?;
    let pgid = fields
        .next()
        .ok_or_else(|| invalid_wire("PGID secundário ausente"))?;
    if pid == "none" && start_time == "none" && pgid == "none" {
        return Ok(None);
    }
    let pid = pid
        .parse()
        .map_err(|_| invalid_wire("PID secundário inválido"))?;
    let start_time = start_time
        .parse()
        .map_err(|_| invalid_wire("start time secundário inválido"))?;
    let pgid = if pgid == "none" {
        None
    } else {
        Some(
            pgid.parse()
                .map_err(|_| invalid_wire("PGID secundário inválido"))?,
        )
    };
    Ok(Some(ProcessIdentitySnapshot {
        pid,
        start_time,
        pgid,
    }))
}

fn parse_field<T: std::str::FromStr>(field: Option<&str>, name: &str) -> io::Result<T> {
    field
        .ok_or_else(|| invalid_wire(&format!("{name} ausente")))?
        .parse()
        .map_err(|_| invalid_wire(&format!("{name} inválido")))
}

fn parse_stage(value: &str) -> io::Result<ShutdownStage> {
    [
        ShutdownStage::MarkerRunning,
        ShutdownStage::MarkerTerminating,
        ShutdownStage::TerminationRequest,
        ShutdownStage::WaitFinal,
        ShutdownStage::StdoutJoin,
        ShutdownStage::StderrJoin,
        ShutdownStage::FinalReap,
        ShutdownStage::WatchdogFinish,
        ShutdownStage::MarkerTerminal,
        ShutdownStage::Cleanup,
        ShutdownStage::Quarantine,
        ShutdownStage::EvidenceWrite,
    ]
    .into_iter()
    .find(|stage| stage.as_str() == value)
    .ok_or_else(|| invalid_wire("estágio secundário inválido"))
}

fn hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("escrita em String");
    }
    encoded
}

fn parse_hex_field(field: Option<&str>, name: &str) -> io::Result<String> {
    let field = field.ok_or_else(|| invalid_wire(&format!("{name} ausente")))?;
    if field.len() % 2 != 0 {
        return Err(invalid_wire(&format!("{name} hexadecimal inválido")));
    }
    let bytes = (0..field.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&field[index..index + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid_wire(&format!("{name} hexadecimal inválido")))?;
    String::from_utf8(bytes).map_err(|_| invalid_wire(&format!("{name} UTF-8 inválido")))
}

fn invalid_wire(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub struct ControlledRunOutcome {
    pub status: Option<ExitStatus>,
    pub primary_reason: TerminationReason,
    pub secondary_errors: Vec<ShutdownError>,
    pub launcher_identity: ProcessIdentitySnapshot,
    pub watchdog_identity: Option<ProcessIdentitySnapshot>,
    pub tree_shutdown_proven: bool,
    pub sandbox_disposition: SandboxDisposition,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ControlledRunOutcome {
    pub(crate) fn compatibility_error(&self) -> Option<io::Error> {
        let controlled_failure = self.primary_reason != TerminationReason::GuestExited;
        if !controlled_failure && self.secondary_errors.is_empty() && self.status.is_some() {
            return None;
        }
        let kind = if self.primary_reason == TerminationReason::StartupFailure {
            self.secondary_errors
                .first()
                .map_or(io::ErrorKind::Other, |error| error.kind)
        } else if controlled_failure {
            io::ErrorKind::TimedOut
        } else {
            self.secondary_errors
                .first()
                .map_or(io::ErrorKind::Other, |error| error.kind)
        };
        let mut rendered = format!("primary={}", self.primary_reason);
        for error in &self.secondary_errors {
            rendered.push_str(&format!(
                "\nsecondary={}:{}",
                error.stage.as_str(),
                error.message.replace('\n', " ")
            ));
        }
        Some(io::Error::new(kind, rendered))
    }

    pub fn report(&self) -> String {
        use std::os::unix::process::ExitStatusExt as _;
        let mut report = String::from("schema=1\n");
        report.push_str(&format!("primary={}\n", self.primary_reason));
        report.push_str(&format!(
            "status_code={}\nstatus_signal={}\n",
            self.status
                .and_then(|status| status.code())
                .map_or_else(|| "none".to_string(), |code| code.to_string()),
            self.status
                .and_then(|status| status.signal())
                .map_or_else(|| "none".to_string(), |signal| signal.to_string())
        ));
        report.push_str(&format!(
            "launcher_pid={}\nlauncher_start_time={}\nlauncher_pgid={}\n",
            self.launcher_identity.pid,
            self.launcher_identity.start_time,
            self.launcher_identity.pgid.unwrap_or(-1)
        ));
        if let Some(watchdog) = self.watchdog_identity {
            report.push_str(&format!(
                "watchdog_pid={}\nwatchdog_start_time={}\n",
                watchdog.pid, watchdog.start_time
            ));
        } else {
            report.push_str("watchdog_pid=none\nwatchdog_start_time=none\n");
        }
        report.push_str(&format!(
            "tree_shutdown_proven={}\nsecondary_count={}\n",
            self.tree_shutdown_proven,
            self.secondary_errors.len()
        ));
        match &self.sandbox_disposition {
            SandboxDisposition::Removed => report.push_str("sandbox=removed\n"),
            SandboxDisposition::Preserved(reason) => {
                report.push_str(&format!("sandbox=preserved:{}\n", sanitize(reason)));
            }
        }
        for error in &self.secondary_errors {
            let (pid, start_time) = error.process.map_or_else(
                || ("none".to_string(), "none".to_string()),
                |identity| (identity.pid.to_string(), identity.start_time.to_string()),
            );
            report.push_str(&format!(
                "secondary={}:{}:{}:{}:{}\n",
                error.stage.as_str(),
                error_kind_name(error.kind),
                pid,
                start_time,
                sanitize(&error.message)
            ));
        }
        report
    }
}

fn sanitize(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], " ")
}

fn error_kind_name(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::NotFound => "not-found",
        io::ErrorKind::PermissionDenied => "permission-denied",
        io::ErrorKind::BrokenPipe => "broken-pipe",
        io::ErrorKind::TimedOut => "timed-out",
        io::ErrorKind::Interrupted => "interrupted",
        io::ErrorKind::UnexpectedEof => "unexpected-eof",
        io::ErrorKind::WouldBlock => "would-block",
        _ => "other",
    }
}

fn parse_error_kind(value: &str) -> io::Result<io::ErrorKind> {
    match value {
        "not-found" => Ok(io::ErrorKind::NotFound),
        "permission-denied" => Ok(io::ErrorKind::PermissionDenied),
        "broken-pipe" => Ok(io::ErrorKind::BrokenPipe),
        "timed-out" => Ok(io::ErrorKind::TimedOut),
        "interrupted" => Ok(io::ErrorKind::Interrupted),
        "unexpected-eof" => Ok(io::ErrorKind::UnexpectedEof),
        "would-block" => Ok(io::ErrorKind::WouldBlock),
        "other" => Ok(io::ErrorKind::Other),
        _ => Err(invalid_wire("tipo de erro secundário inválido")),
    }
}
