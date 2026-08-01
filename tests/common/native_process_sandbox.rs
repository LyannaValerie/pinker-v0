use super::native_process_marker::{
    parse_marker, write_marker_atomic, MarkerParse, MarkerRecord, MarkerState,
};
use super::{process_identity, process_start_time, ProcessIdentity};
use std::ffi::CString;
use std::fs::{self, File};
use std::io;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_EXECUTION: AtomicU64 = AtomicU64::new(1);
static NEXT_QUARANTINE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct ExecutionRootAuthority {
    repo_root: PathBuf,
    target: PathBuf,
    root: PathBuf,
    root_handle: File,
    root_device: u64,
    root_inode: u64,
}

impl ExecutionRootAuthority {
    fn prepare() -> io::Result<Self> {
        let repo_root = discover_repo_root()?;
        Self::prepare_at(&repo_root)
    }

    fn prepare_at(repo_root: &Path) -> io::Result<Self> {
        let repo_root = repo_root.canonicalize().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("não foi possível canonicalizar a raiz autorizada: {error}"),
            )
        })?;
        let target = repo_root.join("target");
        ensure_real_directory(&target, "target")?;
        let root = target.join("pinker-exec");
        ensure_real_directory(&root, "target/pinker-exec")?;
        let canonical_root = root.canonicalize()?;
        if !canonical_root.starts_with(&repo_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "raiz de execução escapou da raiz canônica autorizada",
            ));
        }
        let metadata = fs::symlink_metadata(&root)?;
        let root_handle = File::open(&canonical_root)?;
        let handle_metadata = root_handle.metadata()?;
        if metadata.dev() != handle_metadata.dev() || metadata.ino() != handle_metadata.ino() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "raiz de execução mudou durante a abertura",
            ));
        }
        Ok(Self {
            repo_root,
            target,
            root: canonical_root,
            root_handle,
            root_device: metadata.dev(),
            root_inode: metadata.ino(),
        })
    }

    fn revalidate(&self) -> io::Result<()> {
        validate_real_directory(&self.target, "target")?;
        validate_real_directory(&self.root, "target/pinker-exec")?;
        let canonical = self.root.canonicalize()?;
        if canonical != self.root || !canonical.starts_with(&self.repo_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "raiz de execução mudou ou escapou da raiz autorizada",
            ));
        }
        let metadata = fs::symlink_metadata(&self.root)?;
        let handle_metadata = self.root_handle.metadata()?;
        if metadata.dev() != self.root_device
            || metadata.ino() != self.root_inode
            || handle_metadata.dev() != self.root_device
            || handle_metadata.ino() != self.root_inode
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "raiz de execução foi trocada desde a preparação",
            ));
        }
        Ok(())
    }
}

fn discover_repo_root() -> io::Result<PathBuf> {
    let mut directory = std::env::current_dir()?.canonicalize()?;
    loop {
        let git = directory.join(".git");
        match fs::symlink_metadata(&git) {
            Ok(metadata)
                if !metadata.file_type().is_symlink()
                    && (metadata.is_dir() || metadata.is_file()) =>
            {
                return Ok(directory);
            }
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    ".git ambíguo na raiz candidata",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if !directory.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "raiz real do repositório Pinker não encontrada",
            ));
        }
    }
}

fn ensure_real_directory(path: &Path, label: &str) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => validate_real_directory(path, label),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            validate_real_directory(path, label)
        }
        Err(error) => Err(error),
    }
}

fn validate_real_directory(path: &Path, label: &str) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} deve ser um diretório real, nunca symlink"),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EntryIdentity {
    device: u64,
    inode: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuarantineStage {
    BeforeQuarantine,
    AfterQuarantine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalVerdict {
    Removed,
    Preserved(&'static str),
}

pub(super) struct ExecutionSandbox {
    authority: ExecutionRootAuthority,
    directory: PathBuf,
    name: String,
    identity: EntryIdentity,
    marker: MarkerRecord,
    cleanup_authorized: bool,
    cleaned: bool,
}

impl ExecutionSandbox {
    pub(super) fn create(git_head: &str, repo_root: Option<&Path>) -> io::Result<Self> {
        let authority = match repo_root {
            Some(root) => ExecutionRootAuthority::prepare_at(root)?,
            None => ExecutionRootAuthority::prepare()?,
        };
        scavenge_stale_execution_dirs(&authority, super::STALE_EXECUTION_MIN_AGE)?;
        authority.revalidate()?;
        let id = NEXT_EXECUTION.fetch_add(1, Ordering::SeqCst);
        let name = format!("exec-{}-{id}", std::process::id());
        let directory = authority.root.join(&name);
        fs::create_dir(&directory)?;
        validate_real_directory(&directory, "diretório de execução")?;
        let metadata = fs::symlink_metadata(&directory)?;
        let created_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let owner_start_time = process_start_time(std::process::id()).ok_or_else(|| {
            io::Error::other("não foi possível provar a identidade do owner em /proc")
        })?;
        let marker = MarkerRecord {
            owner_pid: std::process::id(),
            owner_start_time,
            execution_device: metadata.dev(),
            execution_inode: metadata.ino(),
            launcher_pid: None,
            launcher_start_time: None,
            guest_pid: None,
            process_group_id: None,
            watchdog_pid: None,
            created_at_unix,
            git_head: git_head.to_string(),
            executable_sha256: "pending".to_string(),
            state: MarkerState::Preparing,
        };
        let mut sandbox = Self {
            authority,
            directory,
            name,
            identity: EntryIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
            marker,
            cleanup_authorized: true,
            cleaned: false,
        };
        if let Err(error) = sandbox.publish_marker() {
            let _ = sandbox.cleanup();
            return Err(error);
        }
        sandbox.cleanup_authorized = false;
        Ok(sandbox)
    }

    pub(super) fn path(&self) -> &Path {
        &self.directory
    }

    pub(super) fn update_marker(
        &mut self,
        state: MarkerState,
        launcher: Option<(u32, u64, i32)>,
        guest_pid: Option<u32>,
        watchdog_pid: Option<u32>,
        executable_sha256: &str,
    ) -> io::Result<()> {
        self.marker.state = state;
        self.marker.launcher_pid = launcher.map(|value| value.0);
        self.marker.launcher_start_time = launcher.map(|value| value.1);
        self.marker.process_group_id = launcher.map(|value| value.2);
        self.marker.guest_pid = guest_pid;
        self.marker.watchdog_pid = watchdog_pid;
        self.marker.executable_sha256 = executable_sha256.to_string();
        self.publish_marker()
    }

    fn publish_marker(&self) -> io::Result<()> {
        self.authority.revalidate()?;
        let metadata = fs::symlink_metadata(&self.directory)?;
        if metadata.dev() != self.identity.device || metadata.ino() != self.identity.inode {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "diretório de execução foi trocado antes do marcador",
            ));
        }
        write_marker_atomic(&self.directory, &self.marker).map(|_| ())
    }

    pub(super) fn authorize_cleanup(&mut self) {
        self.cleanup_authorized = true;
    }

    pub(super) fn mark_failed_preserving_shape(
        &mut self,
        executable_sha256: &str,
    ) -> io::Result<()> {
        self.marker.state = MarkerState::Failed;
        self.marker.executable_sha256 = executable_sha256.to_string();
        self.publish_marker()
    }

    pub(super) fn preserve(&mut self) {
        self.cleanup_authorized = false;
    }

    pub(super) fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        self.authority.revalidate()?;
        match quarantine_remove(&self.authority, &self.name, self.identity, None)? {
            RemovalVerdict::Removed => {
                self.cleaned = true;
                Ok(())
            }
            RemovalVerdict::Preserved(reason) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("PRESERVED {reason}"),
            )),
        }
    }
}

impl Drop for ExecutionSandbox {
    fn drop(&mut self) {
        if self.cleanup_authorized && !self.cleaned {
            if let Err(error) = self.cleanup() {
                eprintln!(
                    "PRESERVED falha ao limpar sandbox nativo {}: {error}",
                    self.directory.display()
                );
            }
        }
    }
}

fn scavenge_stale_execution_dirs(
    authority: &ExecutionRootAuthority,
    minimum_age: Duration,
) -> io::Result<usize> {
    authority.revalidate()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut removed = 0;
    for entry in fs::read_dir(&authority.root)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("PRESERVED entrada ilegível no scavenger: {error}");
                continue;
            }
        };
        match scavenge_entry(authority, &entry, now, minimum_age) {
            Ok(RemovalVerdict::Removed) => removed += 1,
            Ok(RemovalVerdict::Preserved(reason)) => {
                eprintln!("PRESERVED {}: {reason}", entry.path().display());
            }
            Err(error) => eprintln!("PRESERVED {}: {error}", entry.path().display()),
        }
    }
    Ok(removed)
}

fn scavenge_entry(
    authority: &ExecutionRootAuthority,
    entry: &fs::DirEntry,
    now: u64,
    minimum_age: Duration,
) -> io::Result<RemovalVerdict> {
    let file_type = entry.file_type()?;
    if file_type.is_symlink() || !file_type.is_dir() {
        return Ok(RemovalVerdict::Preserved("invalid-entry"));
    }
    let name = entry.file_name();
    let Some(name) = name.to_str() else {
        return Ok(RemovalVerdict::Preserved("invalid-entry"));
    };
    let Some((name_owner, _)) = parse_execution_directory_name(name) else {
        return Ok(RemovalVerdict::Preserved("invalid-entry"));
    };
    let directory = entry.path();
    let metadata = fs::symlink_metadata(&directory)?;
    let identity = EntryIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    let marker_path = directory.join("owner.marker");
    let marker_metadata = match fs::symlink_metadata(&marker_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemovalVerdict::Preserved("missing-marker"));
        }
        Err(error) => return Err(error),
    };
    if marker_metadata.file_type().is_symlink() || !marker_metadata.is_file() {
        return Ok(RemovalVerdict::Preserved("missing-marker"));
    }
    let marker = match parse_marker(&fs::read_to_string(&marker_path)?) {
        MarkerParse::Valid(marker) => marker,
        MarkerParse::Legacy => return Ok(RemovalVerdict::Preserved("legacy-marker")),
        MarkerParse::Preserved(reason) => return Ok(RemovalVerdict::Preserved(reason)),
    };
    if marker.owner_pid != name_owner {
        return Ok(RemovalVerdict::Preserved("name-owner-mismatch"));
    }
    if marker.execution_device != identity.device || marker.execution_inode != identity.inode {
        return Ok(RemovalVerdict::Preserved("identity-mismatch"));
    }
    if marker.created_at_unix > now || now - marker.created_at_unix < minimum_age.as_secs() {
        return Ok(RemovalVerdict::Preserved("too-young"));
    }
    match process_identity(marker.owner_pid, marker.owner_start_time) {
        ProcessIdentity::Live => return Ok(RemovalVerdict::Preserved("live-owner")),
        ProcessIdentity::Unknown => {
            return Ok(RemovalVerdict::Preserved("ownership-unknown"));
        }
        ProcessIdentity::Missing | ProcessIdentity::Reused => {}
    }
    authority.revalidate()?;
    quarantine_remove(authority, name, identity, None)
}

fn parse_execution_directory_name(name: &str) -> Option<(u32, u64)> {
    let ids = name.strip_prefix("exec-")?;
    let mut ids = ids.split('-');
    let owner = ids.next()?.parse().ok()?;
    let id = ids.next()?.parse().ok()?;
    ids.next().is_none().then_some((owner, id))
}

type QuarantineHook<'a> = &'a mut dyn FnMut(QuarantineStage, &Path, &Path);

fn quarantine_remove(
    authority: &ExecutionRootAuthority,
    name: &str,
    expected: EntryIdentity,
    mut hook: Option<QuarantineHook<'_>>,
) -> io::Result<RemovalVerdict> {
    authority.revalidate()?;
    let id = NEXT_QUARANTINE.fetch_add(1, Ordering::SeqCst);
    let quarantine_name = format!(
        ".pinker-quarantine-{}-{}-{id}",
        std::process::id(),
        name.trim_start_matches("exec-")
    );
    let original = authority.root.join(name);
    let quarantine = authority.root.join(&quarantine_name);
    if let Some(callback) = hook.as_mut() {
        callback(QuarantineStage::BeforeQuarantine, &original, &quarantine);
    }
    authority.revalidate()?;
    match rename_noreplace(authority.root_handle.as_raw_fd(), name, &quarantine_name) {
        Ok(()) => {}
        Err(error) if error.raw_os_error() == Some(17) => {
            return Ok(RemovalVerdict::Preserved("quarantine-exists"));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemovalVerdict::Preserved("changed-entry"));
        }
        Err(error) => return Err(error),
    }
    if let Some(callback) = hook.as_mut() {
        callback(QuarantineStage::AfterQuarantine, &original, &quarantine);
    }
    let metadata = match fs::symlink_metadata(&quarantine) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RemovalVerdict::Preserved("identity-mismatch"));
        }
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.dev() != expected.device
        || metadata.ino() != expected.inode
    {
        return Ok(RemovalVerdict::Preserved("identity-mismatch"));
    }
    fs::remove_dir_all(&quarantine)?;
    if quarantine.exists() || quarantine.is_symlink() {
        return Err(io::Error::other("quarentena permaneceu após remoção"));
    }
    Ok(RemovalVerdict::Removed)
}

fn rename_noreplace(root_fd: i32, source: &str, destination: &str) -> io::Result<()> {
    extern "C" {
        fn syscall(number: isize, ...) -> isize;
    }
    #[cfg(target_arch = "x86_64")]
    const SYS_RENAMEAT2: isize = 316;
    #[cfg(target_arch = "aarch64")]
    const SYS_RENAMEAT2: isize = 276;
    const RENAME_NOREPLACE: u32 = 1;
    let source = CString::new(source.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nome com NUL"))?;
    let destination = CString::new(destination.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "nome com NUL"))?;
    let result = unsafe {
        syscall(
            SYS_RENAMEAT2,
            root_fd,
            source.as_ptr(),
            root_fd,
            destination.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn quarantine_remove_for_test<F>(
    repo_root: &Path,
    entry_name: &str,
    mut hook: F,
) -> io::Result<RemovalVerdict>
where
    F: FnMut(QuarantineStage, &Path, &Path),
{
    let authority = ExecutionRootAuthority::prepare_at(repo_root)?;
    let directory = authority.root.join(entry_name);
    let metadata = fs::symlink_metadata(directory)?;
    let identity = EntryIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    };
    quarantine_remove(&authority, entry_name, identity, Some(&mut hook))
}

pub fn rust_cleanup_verdict_for_test(
    repo_root: &Path,
    entry_name: &str,
    minimum_age: Duration,
) -> String {
    let result = (|| {
        let authority = ExecutionRootAuthority::prepare_at(repo_root)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = fs::read_dir(&authority.root)?
            .filter_map(Result::ok)
            .find(|entry| entry.file_name() == entry_name)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "entrada ausente"))?;
        scavenge_entry(&authority, &entry, now, minimum_age)
    })();
    match result {
        Ok(RemovalVerdict::Removed) => "STALE".to_string(),
        Ok(RemovalVerdict::Preserved(_)) => "PRESERVED".to_string(),
        Err(_) => "ERROR".to_string(),
    }
}
