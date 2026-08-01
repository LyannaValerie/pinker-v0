mod common;

use common::native_process::process_start_time;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command as RawCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

static NEXT_CASE: AtomicU64 = AtomicU64::new(90_000);

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

fn marker(owner_pid: u32, owner_start_time: u64, created_at: u64) -> String {
    format!(
        "schema: 1\nowner_pid: {owner_pid}\nowner_start_time: {owner_start_time}\nchild_pid: null\nchild_pgid: null\nsupervisor_pid: null\ncreated_at_unix: {created_at}\ngit_head: unknown\nexecutable_sha256: pending\nstate: failed\n"
    )
}

fn write_marker(path: &Path, owner_pid: u32, owner_start_time: u64, created_at: u64) {
    fs::write(
        path.join("owner.marker"),
        marker(owner_pid, owner_start_time, created_at),
    )
    .expect("escreve marcador");
}

fn run_cleanup(argument: &str) -> std::process::Output {
    RawCommand::new("bash")
        .arg("scripts/pinker-cleanup.sh")
        .arg(argument)
        .arg("--older-than")
        .arg("0")
        .output()
        .expect("executa cleanup controlado")
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

fn kill_and_reap(child: &mut std::process::Child) {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    assert_eq!(unsafe { kill(child.id() as i32, 9) }, 0);
    let _ = child.wait().expect("reap helper");
}

#[test]
fn cleanup_distingue_stale_preserved_error_e_e_idempotente() {
    let _serial = serial();
    let root = PathBuf::from("target/pinker-exec");
    fs::create_dir_all(&root).expect("cria raiz dedicada");
    let mut fixture = Fixture::new();

    let stale = fixture.directory(&root);
    write_marker(&stale, 4_000_000, 1, 1);
    let self_start = process_start_time(std::process::id()).expect("starttime self");
    let reused_pid = fixture.directory(&root);
    write_marker(&reused_pid, std::process::id(), self_start + 1, 1);
    let live = fixture.directory(&root);
    write_marker(&live, std::process::id(), self_start, 1);

    let duplicate = fixture.directory(&root);
    fs::write(
        duplicate.join("owner.marker"),
        format!("{}owner_pid: 1\n", marker(4_000_000, 1, 1)),
    )
    .expect("marcador duplicado");
    let incomplete = fixture.directory(&root);
    fs::write(incomplete.join("owner.marker"), "schema: 1\nowner_pid: 1\n")
        .expect("marcador incompleto");
    let unmarked = fixture.directory(&root);

    let outside = PathBuf::from(format!(
        "target/pinker-cleanup-protected-{}",
        NEXT_CASE.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&outside).expect("cria conteúdo protegido");
    fs::write(outside.join("sentinel"), "preservar").expect("sentinela");
    fixture.track(outside.clone());
    let entry_link = root.join(format!(
        "exec-{}-{}",
        std::process::id(),
        NEXT_CASE.fetch_add(1, Ordering::SeqCst)
    ));
    symlink(outside.canonicalize().unwrap(), &entry_link).expect("symlink entrada");
    fixture.track(entry_link);
    let marker_link_dir = fixture.directory(&root);
    symlink(
        outside.join("sentinel").canonicalize().unwrap(),
        marker_link_dir.join("owner.marker"),
    )
    .expect("symlink marcador");

    let dry = run_cleanup("--dry-run");
    assert!(dry.status.success(), "dry-run: {dry:?}");
    let dry_stdout = String::from_utf8_lossy(&dry.stdout);
    assert!(
        dry_stdout.contains("STALE dry-run"),
        "stdout={} stderr={}",
        dry_stdout,
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(dry_stdout.contains("PRESERVED live-owner"));
    assert!(dry_stdout.contains("PRESERVED invalid-marker"));
    assert!(dry_stdout.contains("PRESERVED missing-marker"));
    assert!(stale.exists() && reused_pid.exists());

    let apply = run_cleanup("--apply");
    assert!(apply.status.success(), "apply: {apply:?}");
    let apply_stdout = String::from_utf8_lossy(&apply.stdout);
    assert!(apply_stdout.contains("STALE removed"));
    assert!(!stale.exists());
    assert!(!reused_pid.exists());
    assert!(live.exists());
    assert!(duplicate.exists());
    assert!(incomplete.exists());
    assert!(unmarked.exists());
    assert!(marker_link_dir.exists());
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "preservar"
    );

    let second = run_cleanup("--apply");
    assert!(second.status.success(), "segundo apply: {second:?}");
    assert!(!String::from_utf8_lossy(&second.stdout).contains("STALE removed"));
}

#[test]
fn cleanup_rejeita_target_e_execution_root_symlink_sem_escrita_externa() {
    let _serial = serial();
    let id = NEXT_CASE.fetch_add(1, Ordering::SeqCst);
    let base = std::env::current_dir()
        .unwrap()
        .join("target/pinker-cleanup-fake")
        .join(format!("case-{id}"));
    let fake = base.join("repo");
    let outside = base.join("outside");
    fs::create_dir_all(fake.join("scripts")).expect("scripts");
    fs::create_dir_all(&outside).expect("outside");
    fs::copy(
        "scripts/pinker-cleanup.sh",
        fake.join("scripts/pinker-cleanup.sh"),
    )
    .expect("copia script");
    fs::write(outside.join("sentinel"), "preservar").expect("sentinela");

    symlink(outside.canonicalize().unwrap(), fake.join("target")).expect("target link");
    let target = RawCommand::new("bash")
        .arg("scripts/pinker-cleanup.sh")
        .arg("--apply")
        .current_dir(&fake)
        .output()
        .expect("cleanup fake target");
    assert!(!target.status.success());
    assert!(String::from_utf8_lossy(&target.stderr).contains("ERROR root target-is-symlink"));
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "preservar"
    );

    fs::remove_file(fake.join("target")).expect("remove target link");
    fs::create_dir(fake.join("target")).expect("target real");
    symlink(
        outside.canonicalize().unwrap(),
        fake.join("target/pinker-exec"),
    )
    .expect("exec root link");
    let execution = RawCommand::new("bash")
        .arg("scripts/pinker-cleanup.sh")
        .arg("--apply")
        .current_dir(&fake)
        .output()
        .expect("cleanup fake execution");
    assert!(!execution.status.success());
    assert!(
        String::from_utf8_lossy(&execution.stderr).contains("ERROR root execution-root-is-symlink")
    );
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "preservar"
    );
    fs::remove_dir_all(&base).expect("limpa fake");
}

#[test]
fn cleanup_preserva_owner_vivo_com_comm_espaco_e_parentese_em_dry_e_apply() {
    let _serial = serial();
    let ready = PathBuf::from(format!(
        "target/cleanup-proc-ready-{}",
        NEXT_CASE.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_file(&ready);
    let mut owner = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "cleanup_proc_comm_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_CLEANUP_PROC_READY", &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("owner complexo");
    wait_for_file(&ready);
    let start = process_start_time(owner.id()).expect("starttime real");

    let root = PathBuf::from("target/pinker-exec");
    fs::create_dir_all(&root).expect("root");
    let mut fixture = Fixture::new();
    let live = fixture.directory(&root);
    write_marker(&live, owner.id(), start, 1);

    for mode in ["--dry-run", "--apply"] {
        let output = run_cleanup(mode);
        assert!(output.status.success(), "{mode}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("PRESERVED live-owner"),
            "{mode} não preservou comm complexo"
        );
        assert!(live.exists());
    }

    kill_and_reap(&mut owner);
    let _ = fs::remove_file(ready);
}

#[test]
#[ignore = "ponto de reexecução para comm complexo do cleanup"]
fn cleanup_proc_comm_helper() {
    let Some(ready) = std::env::var_os("PINKER_CLEANUP_PROC_READY") else {
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
fn script_nao_contem_parser_whitespace_fail_open() {
    let source = fs::read_to_string("scripts/pinker-cleanup.sh").expect("script");
    assert!(!source.contains("awk '{print $22}'"));
    assert!(source.contains(r"suffix=${stat_text##*) }"));
    assert!(source.contains("PRESERVED ownership-unknown"));
    assert!(source.contains("revalidate_root"));
    assert!(source.contains("root_identity"));
    assert!(source.contains("rm -rf -- \"$directory\""));
}
