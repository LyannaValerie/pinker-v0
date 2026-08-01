mod common;

use common::native_process::{
    atomic_marker_interruption_for_test, marker_fields_for_test, marker_verdict_for_test,
    process_start_time, quarantine_remove_for_test, rust_cleanup_verdict_for_test, QuarantineStage,
    RemovalVerdict,
};
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::symlink;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt as _;
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

    fn directory(&mut self, root: &Path, owner_pid: u32) -> PathBuf {
        let id = NEXT_CASE.fetch_add(1, Ordering::SeqCst);
        let path = root.join(format!("exec-{owner_pid}-{id}"));
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

fn marker(
    owner_pid: u32,
    owner_start_time: u64,
    created_at: u64,
    device: u64,
    inode: u64,
) -> String {
    format!(
        "schema: 2\nowner_pid: {owner_pid}\nowner_start_time: {owner_start_time}\nexecution_device: {device}\nexecution_inode: {inode}\nlauncher_pid: null\nlauncher_start_time: null\nguest_pid: null\nprocess_group_id: null\nwatchdog_pid: null\ncreated_at_unix: {created_at}\ngit_head: unknown\nexecutable_sha256: pending\nstate: failed\n"
    )
}

fn write_marker(path: &Path, owner_pid: u32, owner_start_time: u64, created_at: u64) {
    let metadata = fs::symlink_metadata(path).expect("identidade da fixture");
    fs::write(
        path.join("owner.marker"),
        marker(
            owner_pid,
            owner_start_time,
            created_at,
            metadata.dev(),
            metadata.ino(),
        ),
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

    let stale = fixture.directory(&root, 4_000_000);
    write_marker(&stale, 4_000_000, 1, 1);
    let self_start = process_start_time(std::process::id()).expect("starttime self");
    let reused_pid = fixture.directory(&root, std::process::id());
    write_marker(&reused_pid, std::process::id(), self_start + 1, 1);
    let live = fixture.directory(&root, std::process::id());
    write_marker(&live, std::process::id(), self_start, 1);

    let duplicate = fixture.directory(&root, 4_000_000);
    let duplicate_metadata = fs::symlink_metadata(&duplicate).unwrap();
    fs::write(
        duplicate.join("owner.marker"),
        format!(
            "{}owner_pid: 1\n",
            marker(
                4_000_000,
                1,
                1,
                duplicate_metadata.dev(),
                duplicate_metadata.ino()
            )
        ),
    )
    .expect("marcador duplicado");
    let incomplete = fixture.directory(&root, 1);
    fs::write(incomplete.join("owner.marker"), "schema: 2\nowner_pid: 1\n")
        .expect("marcador incompleto");
    let unmarked = fixture.directory(&root, std::process::id());

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
    let marker_link_dir = fixture.directory(&root, std::process::id());
    symlink(
        outside.join("sentinel").canonicalize().unwrap(),
        marker_link_dir.join("owner.marker"),
    )
    .expect("symlink marcador");

    let dry = run_cleanup("--dry-run");
    assert!(dry.status.success(), "dry-run: {dry:?}");
    assert!(dry.stderr.is_empty(), "ruído no stderr: {dry:?}");
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
    assert!(apply.stderr.is_empty(), "ruído no stderr: {apply:?}");
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
    let live = fixture.directory(&root, owner.id());
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
    assert!(source.contains("read -r stat_text 2>/dev/null < \"/proc/$pid/stat\""));
    assert!(source.contains("PRESERVED ownership-unknown"));
    assert!(source.contains("revalidate_root"));
    assert!(source.contains("root_identity"));
    assert!(source.contains("mv -T -n -- \"$directory\" \"$quarantine\""));
    assert!(source.contains("rm -rf -- \"$quarantine\""));
    assert!(!source.contains("rm -rf -- \"$directory\""));
}

fn vector_marker(case: &str, owner_pid: u32, device: u64, inode: u64) -> String {
    let (state, launcher, launcher_start, guest, pgid, watchdog, hash) = match case {
        "valid_launcher_ready" => (
            "launcher-ready",
            "700001",
            "1",
            "null",
            "700001",
            "null",
            "pending",
        ),
        "valid_watchdog_ready" => (
            "watchdog-ready",
            "700001",
            "1",
            "null",
            "700001",
            "700002",
            "pending",
        ),
        "valid_running" => (
            "running", "700001", "1", "700003", "700001", "700002", "unknown",
        ),
        "valid_terminating" => (
            "terminating",
            "700001",
            "1",
            "700003",
            "700001",
            "700002",
            "unknown",
        ),
        "valid_finished" => (
            "finished", "700001", "1", "700003", "700001", "700002", "unknown",
        ),
        _ => (
            if case == "valid_failed" {
                "failed"
            } else {
                "preparing"
            },
            "null",
            "null",
            "null",
            "null",
            "null",
            "pending",
        ),
    };
    let marker_owner = if case == "name_owner_mismatch" {
        owner_pid + 1
    } else {
        owner_pid
    };
    let marker_inode = if case == "inode_mismatch" {
        inode + 1
    } else {
        inode
    };
    let mut marker = format!(
        "schema: 2\nowner_pid: {marker_owner}\nowner_start_time: 1\nexecution_device: {device}\nexecution_inode: {marker_inode}\nlauncher_pid: {launcher}\nlauncher_start_time: {launcher_start}\nguest_pid: {guest}\nprocess_group_id: {pgid}\nwatchdog_pid: {watchdog}\ncreated_at_unix: 1\ngit_head: unknown\nexecutable_sha256: {hash}\nstate: {state}\n"
    );
    match case {
        "legacy_schema_1" => marker = "schema: 1\nowner_pid: 1\n".to_string(),
        "truncated" => marker.truncate(marker.len() / 2),
        "missing_field" => marker = marker.replace("git_head: unknown\n", ""),
        "extra_field" => marker.push_str("extra: field\n"),
        "duplicate_field" => marker.push_str(&format!("owner_pid: {owner_pid}\n")),
        "invalid_number" => marker = marker.replace("owner_start_time: 1", "owner_start_time: -1"),
        "invalid_hash" => {
            marker = marker.replace("executable_sha256: pending", "executable_sha256: xyz")
        }
        "invalid_state" => marker = marker.replace("state: preparing", "state: impossible"),
        "impossible_combination" => marker = marker.replace("state: preparing", "state: running"),
        _ => {}
    }
    marker
}

#[test]
fn vetores_canonicos_mantem_paridade_entre_parser_rust_e_cleanup_bash() {
    let _serial = serial();
    assert_eq!(
        marker_fields_for_test(),
        vec![
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
            "state",
        ]
    );
    let vectors =
        fs::read_to_string("tests/fixtures/native_marker_vectors.tsv").expect("vetores canônicos");
    let root = PathBuf::from("target/pinker-exec");
    fs::create_dir_all(&root).expect("raiz");
    let mut fixture = Fixture::new();
    let mut expected = Vec::new();
    for (index, line) in vectors
        .lines()
        .filter(|line| !line.starts_with('#'))
        .enumerate()
    {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3, "vetor inválido: {line}");
        let owner = 4_100_000 + index as u32;
        let directory = fixture.directory(&root, owner);
        let metadata = fs::symlink_metadata(&directory).unwrap();
        let marker = vector_marker(fields[0], owner, metadata.dev(), metadata.ino());
        assert_eq!(marker_verdict_for_test(&marker), fields[1], "{}", fields[0]);
        fs::write(directory.join("owner.marker"), marker).expect("publica vetor");
        expected.push((
            directory
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            fields[2],
        ));
    }
    let output = run_cleanup("--dry-run");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    for (name, verdict) in expected {
        let line = stdout
            .lines()
            .find(|line| line.contains(&name))
            .unwrap_or_else(|| panic!("vetor {name} sem veredicto: {stdout}"));
        assert!(line.starts_with(verdict), "{name}: {line}");
        assert_eq!(
            rust_cleanup_verdict_for_test(Path::new("."), &name, Duration::ZERO),
            verdict,
            "paridade do scavenger Rust para {name}"
        );
    }
}

#[test]
fn marcador_atomico_preserva_versao_anterior_quando_update_interrompe() {
    let _serial = serial();
    let directory = PathBuf::from(format!(
        "target/marker-atomic-{}-{}",
        std::process::id(),
        NEXT_CASE.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&directory).unwrap();
    assert!(atomic_marker_interruption_for_test(&directory).expect("transação atômica"));
    let marker = fs::read_to_string(directory.join("owner.marker")).unwrap();
    assert_eq!(marker_verdict_for_test(&marker), "VALID");
    let source = fs::read_to_string("tests/common/native_process_marker.rs").unwrap();
    for required in [
        "create_new(true)",
        "file.flush()?",
        "file.sync_all()?",
        "fs::rename",
    ] {
        assert!(
            source.contains(required),
            "etapa atômica ausente: {required}"
        );
    }
    assert!(!source.contains("fs::write(&self.marker"));
    fs::remove_dir_all(directory).unwrap();
}

struct QuarantineRepo {
    root: PathBuf,
}

impl QuarantineRepo {
    fn new(label: &str) -> Self {
        let id = NEXT_CASE.fetch_add(1, Ordering::SeqCst);
        let root = std::env::current_dir()
            .unwrap()
            .join("target/pinker-quarantine-fixtures")
            .join(format!("{label}-{}-{id}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("target/pinker-exec")).unwrap();
        Self { root }
    }

    fn entry(&self, id: u64) -> (String, PathBuf) {
        let name = format!("exec-{}-{id}", std::process::id());
        let path = self.root.join("target/pinker-exec").join(&name);
        fs::create_dir(&path).unwrap();
        fs::write(path.join("sentinel"), "original").unwrap();
        (name, path)
    }
}

impl Drop for QuarantineRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn quarentena_atomica_vincula_remocao_ao_inode_e_preserva_trocas_reais() {
    let _serial = serial();

    let success = QuarantineRepo::new("success");
    let (name, path) = success.entry(1);
    let verdict = quarantine_remove_for_test(&success.root, &name, |_, _, _| {})
        .expect("remoção por quarentena");
    assert_eq!(verdict, RemovalVerdict::Removed);
    assert!(!path.exists());

    let before = QuarantineRepo::new("before");
    let (name, path) = before.entry(2);
    let saved = path.with_extension("original");
    let verdict = quarantine_remove_for_test(&before.root, &name, |stage, original, _| {
        if stage == QuarantineStage::BeforeQuarantine {
            fs::rename(original, &saved).unwrap();
            fs::create_dir(original).unwrap();
            fs::write(original.join("unrelated"), "preservar").unwrap();
        }
    })
    .expect("troca antes");
    assert_eq!(verdict, RemovalVerdict::Preserved("identity-mismatch"));
    assert_eq!(
        fs::read_to_string(saved.join("sentinel")).unwrap(),
        "original"
    );
    let quarantine_root = before.root.join("target/pinker-exec");
    assert!(fs::read_dir(quarantine_root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".pinker-quarantine-")
                && entry.path().join("unrelated").exists()
        }));

    let after = QuarantineRepo::new("after");
    let (name, _) = after.entry(3);
    let moved = after.root.join("target/pinker-exec/saved-after");
    let verdict = quarantine_remove_for_test(&after.root, &name, |stage, _, quarantine| {
        if stage == QuarantineStage::AfterQuarantine {
            fs::rename(quarantine, &moved).unwrap();
            fs::create_dir(quarantine).unwrap();
            fs::write(quarantine.join("unrelated"), "preservar").unwrap();
        }
    })
    .expect("troca depois");
    assert_eq!(verdict, RemovalVerdict::Preserved("identity-mismatch"));
    assert_eq!(
        fs::read_to_string(moved.join("sentinel")).unwrap(),
        "original"
    );

    let collision = QuarantineRepo::new("collision");
    let (name, path) = collision.entry(4);
    let verdict = quarantine_remove_for_test(&collision.root, &name, |stage, _, quarantine| {
        if stage == QuarantineStage::BeforeQuarantine {
            fs::create_dir(quarantine).unwrap();
            fs::write(quarantine.join("unrelated"), "preservar").unwrap();
        }
    })
    .expect("colisão");
    assert_eq!(verdict, RemovalVerdict::Preserved("quarantine-exists"));
    assert!(path.join("sentinel").exists());
}

fn run_cleanup_with_internal_hook<F>(mut hook: F) -> std::process::Output
where
    F: FnMut(&str, &Path, &Path),
{
    let (mut controller, child_stream) = UnixStream::pair().expect("canal do hook");
    let child_fd = child_stream.as_raw_fd();
    let mut command = RawCommand::new("bash");
    command
        .args(["scripts/pinker-cleanup.sh", "--apply", "--older-than", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            extern "C" {
                fn dup2(old_fd: i32, new_fd: i32) -> i32;
            }
            if dup2(child_fd, 9) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    controller
        .write_all(b"PINKER_INTERNAL_CLEANUP_TEST_V1\n")
        .expect("handshake do hook");
    let child = command.spawn().expect("cleanup com hook interno");
    drop(child_stream);
    let mut writer = controller.try_clone().expect("clone do hook");
    let mut reader = BufReader::new(controller);
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).expect("mensagem do hook");
        if read == 0 {
            break;
        }
        let fields: Vec<_> = line.trim_end().split('\t').collect();
        assert_eq!(fields.len(), 3, "mensagem de hook inválida: {line:?}");
        hook(fields[0], Path::new(fields[1]), Path::new(fields[2]));
        writer.write_all(b"OK\n").expect("ack do hook");
        writer.flush().expect("flush do hook");
    }
    child.wait_with_output().expect("espera cleanup com hook")
}

#[test]
fn cleanup_bash_preserva_substituicoes_antes_e_depois_da_quarentena() {
    let _serial = serial();
    let root = PathBuf::from("target/pinker-exec");
    fs::create_dir_all(&root).unwrap();
    let mut fixture = Fixture::new();

    let before = fixture.directory(&root, 4_200_001);
    write_marker(&before, 4_200_001, 1, 1);
    fs::write(before.join("sentinel"), "original").unwrap();
    let before_saved = before.with_extension("saved-before");
    fixture.track(before_saved.clone());
    let mut before_quarantine = None;
    let output = run_cleanup_with_internal_hook(|stage, original, quarantine| {
        if original.ends_with(&before) && stage == "before-quarantine" {
            fs::rename(original, &before_saved).unwrap();
            fs::create_dir(original).unwrap();
            fs::write(original.join("unrelated"), "preservar").unwrap();
            before_quarantine = Some(quarantine.to_path_buf());
        }
    });
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("PRESERVED identity-mismatch"),
        "{output:?}"
    );
    assert_eq!(
        fs::read_to_string(before_saved.join("sentinel")).unwrap(),
        "original"
    );
    let before_quarantine = before_quarantine.expect("quarentena before");
    assert_eq!(
        fs::read_to_string(before_quarantine.join("unrelated")).unwrap(),
        "preservar"
    );
    fixture.track(before_quarantine);

    let after = fixture.directory(&root, 4_200_002);
    write_marker(&after, 4_200_002, 1, 1);
    fs::write(after.join("sentinel"), "original").unwrap();
    let after_saved = after.with_extension("saved-after");
    fixture.track(after_saved.clone());
    let mut after_quarantine = None;
    let output = run_cleanup_with_internal_hook(|stage, original, quarantine| {
        if original.ends_with(&after) && stage == "after-quarantine" {
            fs::rename(quarantine, &after_saved).unwrap();
            fs::create_dir(quarantine).unwrap();
            fs::write(quarantine.join("unrelated"), "preservar").unwrap();
            after_quarantine = Some(quarantine.to_path_buf());
        }
    });
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("PRESERVED identity-mismatch"),
        "{output:?}"
    );
    assert_eq!(
        fs::read_to_string(after_saved.join("sentinel")).unwrap(),
        "original"
    );
    let after_quarantine = after_quarantine.expect("quarentena after");
    assert_eq!(
        fs::read_to_string(after_quarantine.join("unrelated")).unwrap(),
        "preservar"
    );
    fixture.track(after_quarantine);

    let collision = fixture.directory(&root, 4_200_003);
    write_marker(&collision, 4_200_003, 1, 1);
    fs::write(collision.join("sentinel"), "original").unwrap();
    let mut collision_quarantine = None;
    let output = run_cleanup_with_internal_hook(|stage, original, quarantine| {
        if original.ends_with(&collision) && stage == "before-quarantine" {
            fs::create_dir(quarantine).unwrap();
            fs::write(quarantine.join("unrelated"), "preservar").unwrap();
            collision_quarantine = Some(quarantine.to_path_buf());
        }
    });
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("PRESERVED quarantine-exists"));
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(
        fs::read_to_string(collision.join("sentinel")).unwrap(),
        "original"
    );
    fixture.track(collision_quarantine.expect("quarentena collision"));
}
