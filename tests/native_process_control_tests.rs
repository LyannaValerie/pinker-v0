mod common;

use common::ControlledCommand;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

fn serial() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().expect("lock")
}

fn execution_dirs() -> usize {
    fs::read_dir("target/pinker-exec")
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
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
fn termino_por_sinal_e_observado_sem_core_ou_residuo() {
    let _serial = serial();
    let before = execution_dirs();
    let output = ControlledCommand::new("sh")
        .args(["-c", "kill -SEGV $$"])
        .logical_case("signal-sem-core")
        .output()
        .expect("status por sinal continua observável");
    assert!(output.status.code().is_none());
    assert_eq!(execution_dirs(), before);
    let controlled_root = Path::new("target/pinker-exec");
    let core = fs::read_dir(controlled_root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with("core"));
    assert!(!core, "execução controlada não pode criar core na raiz");

    let healthy = ControlledCommand::new("sh")
        .args(["-c", "printf healthy"])
        .logical_case("apos-sinal")
        .output()
        .expect("execução seguinte");
    assert!(healthy.status.success());
    assert_eq!(healthy.stdout, b"healthy");
}

#[test]
fn timeout_finaliza_grupo_inteiro_e_reaproveita_filho() {
    let _serial = serial();
    let pid_file = Path::new("target/pinker-timeout-grandchild.pid");
    let _ = fs::remove_file(pid_file);
    let error = ControlledCommand::new("sh")
        .args([
            "-c",
            "sleep 60 & child=$!; printf '%s' \"$child\" > target/pinker-timeout-grandchild.pid; wait",
        ])
        .logical_case("timeout-com-neto")
        .timeout(Duration::from_millis(250))
        .output()
        .expect_err("timeout precisa ser controlado");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    let pid: u32 = fs::read_to_string(pid_file)
        .expect("PID do neto")
        .parse()
        .expect("PID numérico");
    let _ = fs::remove_file(pid_file);
    for _ in 0..50 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("neto {pid} permaneceu vivo após timeout do grupo");
}

#[test]
fn saida_acima_do_teto_encerra_execucao_sem_crescimento_ilimitado() {
    let _serial = serial();
    let error = ControlledCommand::new("sh")
        .args(["-c", "yes pinker"])
        .logical_case("stdout-ilimitado")
        .capture_limit(4096)
        .timeout(Duration::from_secs(5))
        .output()
        .expect_err("stdout acima do teto precisa falhar");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    assert!(error.to_string().contains("stdout_limit"), "{error}");
}

#[test]
fn cem_execucoes_pequenas_nao_acumulam_filhos_nem_temporarios() {
    let _serial = serial();
    let before = execution_dirs();
    for case in 0..100 {
        let output = ControlledCommand::new("true")
            .logical_case(&format!("stress-pequeno-{case}"))
            .output()
            .expect("execução pequena controlada");
        assert!(output.status.success());
    }
    assert_eq!(execution_dirs(), before);
}

#[test]
fn scavenger_automatico_remove_somente_residuo_marcado_antigo() {
    let _serial = serial();
    let stale =
        Path::new("target/pinker-exec").join(format!("exec-{}-7777777", std::process::id()));
    let _ = fs::remove_dir_all(&stale);
    fs::create_dir_all(&stale).expect("cria resíduo simulado");
    fs::write(
        stale.join("owner.marker"),
        "schema: 1\nowner_pid: 4000000\nowner_start_time: 1\nchild_pid: null\nchild_pgid: null\ncreated_at_unix: 1\ngit_head: unknown\nexecutable_sha256: pending\nstate: failed\n",
    )
    .expect("marca resíduo simulado");

    let output = ControlledCommand::new("true")
        .logical_case("aciona-scavenger")
        .output()
        .expect("execução saudável");
    assert!(output.status.success());
    assert!(!stale.exists(), "scavenger deixou resíduo elegível");
}

#[test]
#[ignore = "ponto de reexecução usado apenas pelo teste de parent-death"]
fn pdeath_controller_entry() {
    let Some(pid_file) = std::env::var_os("PINKER_PDEATH_PID_FILE") else {
        return;
    };
    use std::os::unix::process::CommandExt as _;

    extern "C" {
        fn getpid() -> i32;
        fn getppid() -> i32;
        fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
        fn setpgid(pid: i32, pgid: i32) -> i32;
    }
    const PR_SET_PDEATHSIG: i32 = 1;
    const SIGKILL: i32 = 9;

    let original_parent = unsafe { getpid() };
    let mut child = std::process::Command::new("sh");
    child.args(["-c", "exec sleep 60"]);
    child
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    unsafe {
        child.pre_exec(move || {
            if setpgid(0, 0) != 0 || prctl(PR_SET_PDEATHSIG, SIGKILL as u64, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if getppid() != original_parent {
                return Err(std::io::Error::other(
                    "pai morreu durante configuração de PR_SET_PDEATHSIG",
                ));
            }
            Ok(())
        });
    }
    let child = child.spawn().expect("cria filho da sonda");
    fs::write(pid_file, child.id().to_string()).expect("publica PID da sonda");
    drop(child);
}

#[test]
fn parent_death_signal_impede_filho_orfao() {
    let _serial = serial();
    let pid_file = format!("target/pinker-pdeath-{}.pid", std::process::id());
    let _ = fs::remove_file(&pid_file);
    let output = ControlledCommand::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "pdeath_controller_entry",
            "--ignored",
            "--nocapture",
        ])
        .env("PINKER_PDEATH_PID_FILE", &pid_file)
        .logical_case("pdeath-controller")
        .timeout(Duration::from_secs(5))
        .output()
        .expect("controlador termina normalmente");
    assert!(output.status.success(), "controlador: {output:?}");
    let pid: u32 = fs::read_to_string(&pid_file)
        .expect("PID do filho PDEATH")
        .parse()
        .expect("PID numérico");
    let _ = fs::remove_file(&pid_file);
    for _ in 0..100 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("filho {pid} permaneceu vivo após morte do controlador");
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
        "tests/public_memory_hotfix_tests.rs",
        "tests/uniao_contabilidade_paridade_tests.rs",
    ];
    for path in mapped {
        let source = fs::read_to_string(path).expect("lê suíte nativa mapeada");
        let direct_command = source.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with('.') && line.contains("std::process::Command::new(")
        });
        assert!(!direct_command, "{path} escapou da autoridade comum");
        assert!(
            !source.contains("use std::process::Command"),
            "{path} importou Command direto"
        );
    }

    let runtime = fs::read_to_string("runtime/pinker_rt/src/lib.rs").expect("lê runtime");
    assert!(
        !runtime.contains("write_bytes(0, tamanho"),
        "zeragem ansiosa retornou"
    );
    assert!(
        !runtime.contains("mmap_publico_monolitico(MAX_PUBLIC_LIFETIME_VIRTUAL_BYTES"),
        "arena monolítica retornou"
    );
    let helper = fs::read_to_string("tests/common/native_process.rs").expect("lê helper");
    for required in [
        "PR_SET_PDEATHSIG",
        "RLIMIT_CORE",
        "setpgid",
        "terminate_process_group",
        "MAX_CAPTURED_STDOUT_BYTES",
        "executable_sha256",
        "started.elapsed() >= policy.timeout",
        "sandbox.cleanup()?",
        "let git_head = read_git_head()",
        "signal_group(pgid, 15)",
    ] {
        assert!(helper.contains(required), "política ausente: {required}");
    }
}
