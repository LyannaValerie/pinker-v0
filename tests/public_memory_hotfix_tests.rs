//! Fronteira externa do hotfix de memória pública.

mod common;

use std::fs::File;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static SERIAL: Mutex<()> = Mutex::new(());
static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(0);

const MIB: u64 = 1024 * 1024;

#[repr(C)]
struct RLimit {
    current: u64,
    maximum: u64,
}

extern "C" {
    fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
}

const RLIMIT_CPU: i32 = 0;
const RLIMIT_FSIZE: i32 = 1;
const RLIMIT_CORE: i32 = 4;
const RLIMIT_AS: i32 = 9;

struct MemoryArtifacts {
    path: PathBuf,
}

impl MemoryArtifacts {
    fn create() -> Self {
        let id = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from("target/pinker-memory-hotfix")
            .join(format!("{}-{id}", std::process::id()));
        std::fs::create_dir_all(&path).expect("cria diretório estreito do teste de memória");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MemoryArtifacts {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Clone, Copy)]
struct MemoryChildLimits {
    timeout: Duration,
    address_space_bytes: u64,
    cpu_seconds: u64,
    file_bytes: u64,
}

fn run_memory_child(
    command: &mut Command,
    artifacts: &MemoryArtifacts,
    label: &str,
    limits: MemoryChildLimits,
) -> Outcome {
    let id = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
    let stdout_path = artifacts.path().join(format!("{id}.stdout"));
    let stderr_path = artifacts.path().join(format!("{id}.stderr"));
    command.stdout(Stdio::from(
        File::create(&stdout_path).expect("cria captura stdout"),
    ));
    command.stderr(Stdio::from(
        File::create(&stderr_path).expect("cria captura stderr"),
    ));
    unsafe {
        command.pre_exec(move || {
            for (resource, current, maximum) in [
                (RLIMIT_CORE, 0, 0),
                (RLIMIT_CPU, limits.cpu_seconds, limits.cpu_seconds),
                (RLIMIT_FSIZE, limits.file_bytes, limits.file_bytes),
                (
                    RLIMIT_AS,
                    limits.address_space_bytes,
                    limits.address_space_bytes,
                ),
            ] {
                let limit = RLimit { current, maximum };
                if setrlimit(resource, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }

    let mut child = command.spawn().unwrap_or_else(|error| {
        panic!("não foi possível iniciar filho de memória {label}: {error}")
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("consulta filho de memória") {
            break status;
        }
        if started.elapsed() >= limits.timeout {
            let _ = child.kill();
            let _ = child.wait();
            panic!("timeout explícito no filho de memória {label}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = std::fs::read_to_string(&stdout_path).expect("lê stdout do filho");
    let stderr = std::fs::read_to_string(&stderr_path).expect("lê stderr do filho");
    let _ = std::fs::remove_file(stdout_path);
    let _ = std::fs::remove_file(stderr_path);
    Outcome {
        code: status.code(),
        stdout,
        stderr,
    }
}

fn runtime_library() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PINKER_RT_LIB").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    let executable = std::env::current_exe().ok()?;
    let deps = executable.parent()?;
    let runtime = [Some(deps), deps.parent()]
        .into_iter()
        .flatten()
        .map(|directory| directory.join("libpinker_rt.a"))
        .find(|candidate| candidate.is_file());
    runtime
}

fn native_available() -> Option<PathBuf> {
    let reason = if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("unsupported_platform")
    } else if !["cc", "gcc", "clang"].iter().any(|candidate| {
        std::env::var_os("PATH").is_some_and(|path| {
            std::env::split_paths(&path)
                .map(|directory| directory.join(candidate))
                .any(|path| path.is_file())
        })
    }) {
        Some("cc_not_found")
    } else {
        None
    };
    let runtime = reason.is_none().then(runtime_library).flatten();
    let reason = reason.or_else(|| runtime.is_none().then_some("runtime_library_not_found"));
    if let Some(reason) = reason {
        eprintln!(
            "{{\"event\":\"native_evidence\",\"reason\":\"{reason}\",\"status\":\"unavailable\",\"test\":\"public_memory_hotfix_tests\"}}"
        );
        assert_ne!(
            std::env::var("PINKER_EXIGE_NATIVO").as_deref(),
            Ok("1"),
            "evidência nativa obrigatória indisponível: {reason}"
        );
        return None;
    }
    runtime
}

#[derive(Debug)]
struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn interpreted(example: &str) -> Outcome {
    let artifacts = MemoryArtifacts::create();
    run_memory_child(
        Command::new(env!("CARGO_BIN_EXE_pink")).args(["--run", example]),
        &artifacts,
        &format!("interpretado:{example}"),
        MemoryChildLimits {
            timeout: Duration::from_secs(10),
            address_space_bytes: 1024 * MIB,
            cpu_seconds: 8,
            file_bytes: MIB,
        },
    )
}

fn native(example: &str) -> Option<Outcome> {
    let runtime = native_available()?;
    let artifacts = MemoryArtifacts::create();
    let build = run_memory_child(
        Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["build", "--nativo", "--out-dir"])
            .arg(artifacts.path())
            .arg(example)
            .env("PINKER_RT_LIB", runtime),
        &artifacts,
        &format!("build:{example}"),
        MemoryChildLimits {
            timeout: Duration::from_secs(30),
            address_space_bytes: 2 * 1024 * MIB,
            cpu_seconds: 20,
            file_bytes: 256 * MIB,
        },
    );
    assert_eq!(build.code, Some(0), "build nativo falhou: {build:?}");
    let executable = artifacts
        .path()
        .join(Path::new(example).file_stem().expect("nome do exemplo"));
    Some(run_memory_child(
        &mut Command::new(executable),
        &artifacts,
        &format!("nativo:{example}"),
        MemoryChildLimits {
            timeout: Duration::from_secs(10),
            address_space_bytes: 1024 * MIB,
            cpu_seconds: 8,
            file_bytes: MIB,
        },
    ))
}

#[test]
fn limites_recuperacao_zero_e_paginas_esparsas_tem_paridade() {
    let _serial = SERIAL.lock().expect("serialização");
    for (example, expected_stdout) in [
        (
            "examples/hotfix_memoria_publica_limite_valido.pink",
            "256\n",
        ),
        (
            "examples/hotfix_memoria_publica_recuperacao_valido.pink",
            "2\n",
        ),
        (
            "tests/fixtures/issue449_memoria_publica_paginas_esparsas_valido.pink",
            "0\n0\n0\n24\n",
        ),
    ] {
        let interpreted = interpreted(example);
        assert_eq!(interpreted.code, Some(0), "{interpreted:?}");
        assert_eq!(interpreted.stdout, expected_stdout);
        assert!(interpreted.stderr.is_empty(), "{interpreted:?}");
        if let Some(native) = native(example) {
            assert_eq!(native.code, interpreted.code, "{native:?}");
            assert_eq!(native.stdout, interpreted.stdout, "{native:?}");
            assert_eq!(native.stderr, interpreted.stderr, "{native:?}");
        }
    }
}

#[test]
fn recusas_de_orcamento_sao_controladas_e_equivalentes() {
    let _serial = SERIAL.lock().expect("serialização");
    for (example, diagnostic) in [
        (
            "examples/hotfix_memoria_publica_limite_individual_invalido.pink",
            "E-RUNTIME-MEM-PUBLIC-SINGLE-BUDGET",
        ),
        (
            "examples/hotfix_memoria_publica_limite_vivo_invalido.pink",
            "E-RUNTIME-MEM-PUBLIC-LIVE-BUDGET",
        ),
    ] {
        let interpreted = interpreted(example);
        assert_eq!(interpreted.code, Some(1), "{interpreted:?}");
        assert!(interpreted.stderr.contains(diagnostic), "{interpreted:?}");
        if let Some(native) = native(example) {
            assert_eq!(native.code, Some(1), "{native:?}");
            assert!(native.stderr.contains(diagnostic), "{native:?}");
            assert!(!native.stderr.contains("signal"), "{native:?}");
        }
    }
}

#[test]
fn documentacao_distingue_contabilidade_realizacao_e_recuperabilidade() {
    let manual = std::fs::read_to_string("MANUAL.md").expect("lê manual");
    assert!(!manual.contains("O orçamento é explícito e equivalente nos dois modos"));
    let manual_flat = manual.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "mapeamento anônimo proporcional",
        "256 MiB",
        "8 GiB na soma vitalícia",
        "Somente os bytes vivos são recuperáveis",
        "não existe reserva antecipada de 8 GiB",
    ] {
        assert!(manual_flat.contains(required), "MANUAL omitiu: {required}");
    }

    let operations =
        std::fs::read_to_string("docs/development/runtime-public-memory-containment.md")
            .expect("lê documento operacional");
    let operations_flat = operations.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "65.536 regiões vivas",
        "E-RUNTIME-MEM-PUBLIC-MAP",
        "8 GiB acumulados",
        "O workload exato",
        "não identifica uma revisão Git",
    ] {
        assert!(
            operations_flat.contains(required),
            "documento omitiu: {required}"
        );
    }
}

#[test]
fn implementacao_nao_regride_para_arena_monolitica_ou_zeragem_ansiosa() {
    let runtime =
        std::fs::read_to_string("runtime/pinker_rt/src/lib.rs").expect("lê runtime nativo");
    for forbidden in [
        "arena_base: usize",
        "proximo_offset: usize",
        "fn reservar_arena_publica",
        "ponteiro.write_bytes(0, tamanho_usize)",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "runtime restaurou mecanismo removido: {forbidden}"
        );
    }
    for required in [
        "fn mapear_regiao_publica",
        "MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE",
        "fn tentar_alocar_publico_com",
    ] {
        assert!(
            runtime.contains(required),
            "runtime perdeu mecanismo proporcional: {required}"
        );
    }
}
