mod common;

use common::ControlledCommand as Command;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_CASE: AtomicU64 = AtomicU64::new(90_000);

struct Fixture {
    paths: Vec<PathBuf>,
}

impl Fixture {
    fn new() -> Self {
        Self { paths: Vec::new() }
    }

    fn directory(&mut self, root: &Path) -> PathBuf {
        let id = NEXT_CASE.fetch_add(1, Ordering::SeqCst);
        let path = root.join(format!("exec-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("cria fixture de cleanup");
        self.paths.push(path.clone());
        path
    }

    fn track(&mut self, path: PathBuf) {
        self.paths.push(path);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in self.paths.iter().rev() {
            let _ = if path.is_dir() && !path.is_symlink() {
                fs::remove_dir_all(path)
            } else {
                fs::remove_file(path)
            };
        }
    }
}

fn process_start_time(pid: u32) -> u64 {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).expect("lê /proc/self/stat");
    stat.split_whitespace()
        .nth(21)
        .expect("campo starttime")
        .parse()
        .expect("starttime numérico")
}

fn write_marker(path: &Path, owner_pid: u32, owner_start_time: u64) {
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo posterior à época")
        .as_secs();
    let marker = format!(
        "schema: 1\nowner_pid: {owner_pid}\nowner_start_time: {owner_start_time}\nchild_pid: null\nchild_pgid: null\ncreated_at_unix: {created_at}\ngit_head: unknown\nexecutable_sha256: pending\nstate: failed\n"
    );
    fs::write(path.join("owner.marker"), marker).expect("escreve marcador");
}

fn run_cleanup(argument: &str) -> std::process::Output {
    Command::new("bash")
        .arg("scripts/pinker-cleanup.sh")
        .arg(argument)
        .arg("--older-than")
        .arg("0")
        .output()
        .expect("executa cleanup controlado")
}

#[test]
fn cleanup_e_restrito_marcado_seguro_e_idempotente() {
    let root = PathBuf::from("target/pinker-exec");
    fs::create_dir_all(&root).expect("cria raiz dedicada");
    let mut fixture = Fixture::new();

    let stale = fixture.directory(&root);
    write_marker(&stale, 4_000_000, 1);

    let reused_pid = fixture.directory(&root);
    write_marker(
        &reused_pid,
        std::process::id(),
        process_start_time(std::process::id()) + 1,
    );

    let live = fixture.directory(&root);
    write_marker(
        &live,
        std::process::id(),
        process_start_time(std::process::id()),
    );

    let invalid = fixture.directory(&root);
    fs::write(invalid.join("owner.marker"), "schema: 0\n").expect("marcador inválido");
    let unmarked = fixture.directory(&root);

    let outside = PathBuf::from(format!(
        "target/pinker-cleanup-protected-{}",
        std::process::id()
    ));
    fs::create_dir_all(&outside).expect("cria conteúdo protegido");
    fs::write(outside.join("sentinel"), "preservar").expect("cria sentinela");
    fixture.track(outside.clone());
    let link = root.join(format!(
        "exec-{}-{}",
        std::process::id(),
        NEXT_CASE.fetch_add(1, Ordering::SeqCst)
    ));
    symlink(&outside, &link).expect("cria symlink de proteção");
    fixture.track(link);

    let dry_run = run_cleanup("--dry-run");
    assert!(dry_run.status.success(), "dry-run falhou: {dry_run:?}");
    let dry_stdout = String::from_utf8_lossy(&dry_run.stdout);
    assert!(
        dry_stdout.contains(&format!("STALE {}", stale.display()))
            || dry_stdout.contains(stale.file_name().unwrap().to_str().unwrap())
    );
    assert!(stale.exists());
    assert!(reused_pid.exists());

    let apply = run_cleanup("--apply");
    assert!(apply.status.success(), "apply falhou: {apply:?}");
    let apply_stdout = String::from_utf8_lossy(&apply.stdout);
    assert!(apply_stdout.contains("REMOVED"));
    assert!(!stale.exists());
    assert!(!reused_pid.exists());
    assert!(live.exists());
    assert!(invalid.exists());
    assert!(unmarked.exists());
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).expect("sentinela preservada"),
        "preservar"
    );

    let second = run_cleanup("--apply");
    assert!(second.status.success(), "segundo apply falhou: {second:?}");
    assert!(!String::from_utf8_lossy(&second.stdout).contains("REMOVED"));

    let script = fs::read_to_string("scripts/pinker-cleanup.sh").expect("lê ferramenta");
    for required in [
        "[[ -L \"$execution_root\"",
        "[[ -L \"$directory\"",
        "[[ -L \"$marker\"",
        "PRESERVED missing-marker",
    ] {
        assert!(script.contains(required), "proteção ausente: {required}");
    }
}
