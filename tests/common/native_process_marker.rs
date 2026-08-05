use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MARKER_SCHEMA: u32 = 2;
const MARKER_FIELDS: [&str; 13] = [
    "schema",
    "owner_pid",
    "owner_start_time",
    "execution_device",
    "execution_inode",
    "launcher_pid",
    "launcher_start_time",
    "guest_pid",
    "process_group_id",
    "watchdog_pid",
    "created_at_unix",
    "git_head",
    "executable_sha256",
];
const STATE_FIELD: &str = "state";
static NEXT_MARKER_TEMP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MarkerState {
    Preparing,
    LauncherReady,
    WatchdogReady,
    Running,
    Terminating,
    Finished,
    Failed,
}

impl MarkerState {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "preparing" => Self::Preparing,
            "launcher-ready" => Self::LauncherReady,
            "watchdog-ready" => Self::WatchdogReady,
            "running" => Self::Running,
            "terminating" => Self::Terminating,
            "finished" => Self::Finished,
            "failed" => Self::Failed,
            _ => return None,
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::LauncherReady => "launcher-ready",
            Self::WatchdogReady => "watchdog-ready",
            Self::Running => "running",
            Self::Terminating => "terminating",
            Self::Finished => "finished",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MarkerRecord {
    pub owner_pid: u32,
    pub owner_start_time: u64,
    pub execution_device: u64,
    pub execution_inode: u64,
    pub launcher_pid: Option<u32>,
    pub launcher_start_time: Option<u64>,
    pub guest_pid: Option<u32>,
    pub process_group_id: Option<i32>,
    pub watchdog_pid: Option<u32>,
    pub created_at_unix: u64,
    pub git_head: String,
    pub executable_sha256: String,
    pub state: MarkerState,
}

impl MarkerRecord {
    pub(super) fn validate(&self) -> bool {
        if self.owner_pid == 0
            || self.owner_start_time == 0
            || self.execution_inode == 0
            || !valid_git_head(&self.git_head)
            || !valid_executable_hash(&self.executable_sha256)
        {
            return false;
        }
        if self.launcher_pid == Some(0)
            || self.launcher_start_time == Some(0)
            || self.guest_pid == Some(0)
            || self.watchdog_pid == Some(0)
            || self.process_group_id.is_some_and(|pgid| pgid <= 0)
        {
            return false;
        }
        let none = self.launcher_pid.is_none()
            && self.launcher_start_time.is_none()
            && self.guest_pid.is_none()
            && self.process_group_id.is_none()
            && self.watchdog_pid.is_none();
        let launcher = self.launcher_pid.is_some()
            && self.launcher_start_time.is_some()
            && self.guest_pid.is_none()
            && self.process_group_id == self.launcher_pid.map(|pid| pid as i32)
            && self.watchdog_pid.is_none();
        let watchdog = self.launcher_pid.is_some()
            && self.launcher_start_time.is_some()
            && self.guest_pid.is_none()
            && self.process_group_id == self.launcher_pid.map(|pid| pid as i32)
            && self.watchdog_pid.is_some();
        let running = self.launcher_pid.is_some()
            && self.launcher_start_time.is_some()
            && self.guest_pid.is_some()
            && self.process_group_id == self.launcher_pid.map(|pid| pid as i32)
            && self.watchdog_pid.is_some();
        let shape_valid = match self.state {
            MarkerState::Preparing => none,
            MarkerState::LauncherReady => launcher,
            MarkerState::WatchdogReady => watchdog,
            MarkerState::Running | MarkerState::Terminating | MarkerState::Finished => running,
            MarkerState::Failed => none || launcher || watchdog || running,
        };
        shape_valid
            && match self.state {
                MarkerState::Preparing
                | MarkerState::LauncherReady
                | MarkerState::WatchdogReady => self.executable_sha256 == "pending",
                MarkerState::Running | MarkerState::Terminating | MarkerState::Finished => {
                    self.executable_sha256 != "pending"
                }
                MarkerState::Failed => true,
            }
    }

    fn render(&self) -> String {
        fn optional<T: ToString>(value: Option<T>) -> String {
            value.map_or_else(|| "null".to_string(), |value| value.to_string())
        }
        format!(
            "schema: {MARKER_SCHEMA}\nowner_pid: {}\nowner_start_time: {}\nexecution_device: {}\nexecution_inode: {}\nlauncher_pid: {}\nlauncher_start_time: {}\nguest_pid: {}\nprocess_group_id: {}\nwatchdog_pid: {}\ncreated_at_unix: {}\ngit_head: {}\nexecutable_sha256: {}\nstate: {}\n",
            self.owner_pid,
            self.owner_start_time,
            self.execution_device,
            self.execution_inode,
            optional(self.launcher_pid),
            optional(self.launcher_start_time),
            optional(self.guest_pid),
            optional(self.process_group_id),
            optional(self.watchdog_pid),
            self.created_at_unix,
            self.git_head,
            self.executable_sha256,
            self.state.as_str(),
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum MarkerParse {
    Valid(MarkerRecord),
    Legacy,
    Preserved(&'static str),
}

pub(super) fn parse_marker(marker: &str) -> MarkerParse {
    let mut fields = BTreeMap::new();
    for line in marker.lines() {
        let Some((key, value)) = line.split_once(": ") else {
            return MarkerParse::Preserved("invalid-marker");
        };
        if key.is_empty() || value.is_empty() || fields.insert(key, value).is_some() {
            return MarkerParse::Preserved("invalid-marker");
        }
    }
    if fields.get("schema") == Some(&"1") {
        return MarkerParse::Legacy;
    }
    if fields.get("schema") != Some(&"2")
        || fields.len() != MARKER_FIELDS.len() + 1
        || MARKER_FIELDS
            .iter()
            .any(|field| !fields.contains_key(field))
        || !fields.contains_key(STATE_FIELD)
    {
        return MarkerParse::Preserved("invalid-marker");
    }
    let parse_u32 = |field: &str| fields.get(field).and_then(|value| value.parse().ok());
    let parse_u64 = |field: &str| fields.get(field).and_then(|value| value.parse().ok());
    let parse_optional_u32 = |field: &str| parse_optional(fields.get(field).copied());
    let parse_optional_u64 = |field: &str| parse_optional(fields.get(field).copied());
    let parse_optional_i32 = |field: &str| parse_optional(fields.get(field).copied());
    let Some(record) = (|| {
        Some(MarkerRecord {
            owner_pid: parse_u32("owner_pid")?,
            owner_start_time: parse_u64("owner_start_time")?,
            execution_device: parse_u64("execution_device")?,
            execution_inode: parse_u64("execution_inode")?,
            launcher_pid: parse_optional_u32("launcher_pid")?,
            launcher_start_time: parse_optional_u64("launcher_start_time")?,
            guest_pid: parse_optional_u32("guest_pid")?,
            process_group_id: parse_optional_i32("process_group_id")?,
            watchdog_pid: parse_optional_u32("watchdog_pid")?,
            created_at_unix: parse_u64("created_at_unix")?,
            git_head: fields.get("git_head")?.to_string(),
            executable_sha256: fields.get("executable_sha256")?.to_string(),
            state: MarkerState::parse(fields.get("state")?)?,
        })
    })() else {
        return MarkerParse::Preserved("invalid-marker");
    };
    if record.validate() {
        MarkerParse::Valid(record)
    } else {
        MarkerParse::Preserved("invalid-marker")
    }
}

fn parse_optional<T: std::str::FromStr>(value: Option<&str>) -> Option<Option<T>> {
    match value? {
        "null" => Some(None),
        value => value.parse().ok().map(Some),
    }
}

fn valid_git_head(value: &str) -> bool {
    value == "unknown" || valid_lower_hex(value, 40)
}

fn valid_executable_hash(value: &str) -> bool {
    matches!(value, "pending" | "unknown") || valid_lower_hex(value, 64)
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn write_marker_atomic(directory: &Path, record: &MarkerRecord) -> io::Result<PathBuf> {
    write_marker_transaction(directory, record, |_| Ok(()))
}

fn write_marker_transaction<F>(
    directory: &Path,
    record: &MarkerRecord,
    before_rename: F,
) -> io::Result<PathBuf>
where
    F: FnOnce(&Path) -> io::Result<()>,
{
    if !record.validate() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "estado ou combinação de campos inválida no marcador",
        ));
    }
    let marker = directory.join("owner.marker");
    match fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "owner.marker deve ser arquivo regular",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let id = NEXT_MARKER_TEMP.fetch_add(1, Ordering::SeqCst);
    let temporary = directory.join(format!(".owner.marker.tmp-{}-{id}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(record.render().as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        before_rename(&temporary)?;
        fs::rename(&temporary, &marker)?;
        let metadata = fs::symlink_metadata(&marker)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "marcador publicado não é arquivo regular",
            ));
        }
        if let Ok(directory_file) = OpenOptions::new().read(true).open(directory) {
            directory_file.sync_all()?;
        }
        Ok(marker.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn atomic_marker_interruption_for_test(directory: &Path) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = fs::symlink_metadata(directory)?;
    let mut record = MarkerRecord {
        owner_pid: std::process::id(),
        owner_start_time: super::process_start_time(std::process::id())
            .ok_or_else(|| io::Error::other("start time ausente"))?,
        execution_device: metadata.dev(),
        execution_inode: metadata.ino(),
        launcher_pid: None,
        launcher_start_time: None,
        guest_pid: None,
        process_group_id: None,
        watchdog_pid: None,
        created_at_unix: 1,
        git_head: "unknown".to_string(),
        executable_sha256: "pending".to_string(),
        state: MarkerState::Preparing,
    };
    let marker = write_marker_atomic(directory, &record)?;
    let original = fs::read(&marker)?;
    record.created_at_unix = 2;
    let interrupted = write_marker_transaction(directory, &record, |_| {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "falha injetada antes do rename",
        ))
    });
    Ok(interrupted.is_err()
        && fs::read(&marker)? == original
        && fs::read_dir(directory)?
            .filter_map(Result::ok)
            .all(|entry| {
                !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".owner.marker.tmp-")
            }))
}

pub fn marker_verdict_for_test(marker: &str) -> &'static str {
    match parse_marker(marker) {
        MarkerParse::Valid(_) => "VALID",
        MarkerParse::Legacy | MarkerParse::Preserved(_) => "PRESERVED",
    }
}

pub fn marker_fields_for_test() -> Vec<&'static str> {
    MARKER_FIELDS
        .into_iter()
        .chain(std::iter::once(STATE_FIELD))
        .collect()
}
