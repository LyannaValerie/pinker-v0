mod common;

use common::native_process::{process_identity, process_start_time, ProcessIdentity};
use common::{ControlledCommand, NativeArtifactDir};
use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::os::unix::fs::symlink;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command as RawCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn execution_dirs() -> usize {
    fs::read_dir("target/pinker-exec")
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

fn kill_process(pid: u32, signal: i32) {
    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    assert_eq!(unsafe { kill(pid as i32, signal) }, 0);
}

#[test]
fn supervisor_fecha_descritores_gravaveis_herdados() {
    let _serial = serial();
    let artifacts = NativeArtifactDir::create().expect("diretório marcado");
    let executable = artifacts.path().join("watchdog-fd-probe");
    fs::copy("/bin/true", &executable).expect("copia executável de prova");
    let writer = OpenOptions::new()
        .write(true)
        .open(&executable)
        .expect("abre descritor gravável antes do fork");

    let status = common::native_process::watchdog_fd_hygiene_probe(|| {
        drop(writer);
        RawCommand::new(&executable).status()
    })
    .expect("supervisor de prova")
    .expect("execução não pode colher ETXTBSY");

    assert!(status.success(), "executável de prova falhou: {status}");
}

struct FakeRepo {
    root: PathBuf,
}

impl FakeRepo {
    fn new() -> Self {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst);
        let root = std::env::current_dir()
            .expect("cwd")
            .join("target/pinker-host-fixtures")
            .join(format!("repo-{}-{id}", std::process::id()));
        fs::create_dir_all(root.join(".git")).expect("cria fake repo");
        Self { root }
    }

    fn controlled(&self, script: &str) -> std::io::Result<std::process::Output> {
        ControlledCommand::new("sh")
            .args(["-c", script])
            .execution_repo_root_for_test(&self.root)
            .timeout(Duration::from_secs(3))
            .output()
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

impl Drop for FakeRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn execucao_controlada_aplica_core_zero_e_remove_sandbox() {
    let _serial = serial();
    let before = execution_dirs();
    let output = ControlledCommand::new("sh")
        .args(["-c", "ulimit -c; printf ok"])
        .logical_case("core-zero")
        .output()
        .expect("execução controlada");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\nok");
    assert_eq!(execution_dirs(), before);
}

#[test]
fn raiz_real_ausente_existente_e_segunda_execucao_sao_idempotentes() {
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
    let repo = FakeRepo::new();
    let sentinel = repo.root.join("external-sentinel");
    fs::write(&sentinel, "preservar").expect("sentinela");
    let error = repo
        .controlled(
            "root=$(dirname \"$PINKER_EXECUTION_DIR\"); mv \"$root\" \"$root.saved\"; mkdir \"$root\"; printf changed",
        )
        .expect_err("troca de inode precisa invalidar cleanup");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preservar");
    assert!(repo.root.join("target/pinker-exec.saved").is_dir());
}

#[test]
fn output_stdin_default_eof_e_configuracoes_explicitas() {
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
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
    let _serial = serial();
    let sandboxes_before = execution_dirs();
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
            .capture_limit(4096);
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
    let healthy = ControlledCommand::new("sh")
        .args(["-c", "printf healthy"])
        .output()
        .expect("execução saudável posterior");
    assert_eq!(healthy.stdout, b"healthy");
    assert_eq!(execution_dirs(), sandboxes_before, "sandbox após erro");
}

#[test]
fn captura_no_limite_preserva_prefixo_sem_crescimento() {
    let _serial = serial();
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
    let _serial = serial();
    let id = NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst);
    let child_file = PathBuf::from(format!("target/controller-child-{id}.pid"));
    let grand_file = PathBuf::from(format!("target/controller-grand-{id}.pid"));
    let _ = fs::remove_file(&child_file);
    let _ = fs::remove_file(&grand_file);
    let mut controller = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "watchdog_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_WATCHDOG_CHILD", &child_file)
        .env("PINKER_WATCHDOG_GRAND", &grand_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("controlador");
    wait_for_file(&child_file);
    wait_for_file(&grand_file);
    let child_pid: u32 = fs::read_to_string(&child_file).unwrap().parse().unwrap();
    let grand_pid: u32 = fs::read_to_string(&grand_file).unwrap().parse().unwrap();
    kill_process(controller.id(), 9);
    let _ = controller.wait().expect("reap controlador");
    wait_process_dead(child_pid);
    wait_process_dead(grand_pid);

    let cleanup = RawCommand::new("bash")
        .args(["scripts/pinker-cleanup.sh", "--apply", "--older-than", "0"])
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
    let script = "printf '%s' \"$$\" > \"$1\"; sleep 60 & printf '%s' \"$!\" > \"$2\"; wait";
    let _ = ControlledCommand::new("sh")
        .args(["-c", script, "sh"])
        .arg(child_file)
        .arg(grand_file)
        .timeout(Duration::from_secs(60))
        .output();
}

#[test]
fn supervisor_morto_e_detectado_e_arvore_terminada() {
    let _serial = serial();
    let id = NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst);
    let child_file = PathBuf::from(format!("target/supervisor-child-{id}.pid"));
    let result_file = PathBuf::from(format!("target/supervisor-result-{id}"));
    let _ = fs::remove_file(&child_file);
    let _ = fs::remove_file(&result_file);
    let mut controller = RawCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "supervisor_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_SUPERVISOR_CHILD", &child_file)
        .env("PINKER_SUPERVISOR_RESULT", &result_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("controlador");
    wait_for_file(&child_file);
    let child_pid: u32 = fs::read_to_string(&child_file).unwrap().parse().unwrap();

    let root = Path::new("target/pinker-exec");
    let marker = {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let found = fs::read_dir(root).ok().and_then(|entries| {
                entries.filter_map(Result::ok).find_map(|entry| {
                    let marker = entry.path().join("owner.marker");
                    let text = fs::read_to_string(&marker).ok()?;
                    text.contains(&format!("owner_pid: {}", controller.id()))
                        .then_some((marker, text))
                })
            });
            if let Some(found) = found {
                break found;
            }
            assert!(Instant::now() < deadline, "marcador do controlador ausente");
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    let supervisor_pid: u32 = marker
        .1
        .lines()
        .find_map(|line| line.strip_prefix("supervisor_pid: "))
        .expect("supervisor no marker")
        .parse()
        .expect("PID supervisor");
    kill_process(supervisor_pid, 9);
    wait_for_file(&result_file);
    let _ = controller.wait().expect("reap controlador");
    wait_process_dead(child_pid);
    assert!(
        fs::read_to_string(&result_file)
            .unwrap()
            .contains("watchdog_exit"),
        "falha não identificou supervisor"
    );
    let _ = fs::remove_file(child_file);
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
    let result = ControlledCommand::new("sh")
        .args(["-c", "printf '%s' \"$$\" > \"$1\"; exec sleep 60", "sh"])
        .arg(child_file)
        .timeout(Duration::from_secs(60))
        .output();
    fs::write(result_file, format!("{result:?}")).expect("publica resultado");
}

#[test]
fn proc_stat_comm_complexo_usa_starttime_real() {
    let _serial = serial();
    let ready = PathBuf::from(format!(
        "target/proc-comm-ready-{}",
        NEXT_FIXTURE.fetch_add(1, Ordering::SeqCst)
    ));
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
    kill_process(pid, 9);
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
    let _serial = serial();
    let before = execution_dirs();
    let children_before = direct_children();
    for case in 0..100 {
        let output = ControlledCommand::new("true")
            .logical_case(&format!("stress-pequeno-{case}"))
            .output()
            .expect("execução pequena controlada");
        assert!(output.status.success());
    }
    assert_eq!(execution_dirs(), before);
    assert_eq!(direct_children(), children_before, "supervisor não reaped");
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

    let helper = fs::read_to_string("tests/common/native_process.rs").expect("helper");
    for required in [
        "ProcessWatchdog",
        "pipe2",
        "root_inode",
        "parse_proc_stat_start_time",
        "Stdio::null()",
        "Stdio::inherit()",
        "cpu_seconds",
        "supervisor_pid",
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
