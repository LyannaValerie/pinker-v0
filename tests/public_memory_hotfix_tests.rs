//! Fronteira externa do hotfix de memória pública.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::path::Path;
use std::sync::Mutex;

static SERIAL: Mutex<()> = Mutex::new(());

#[derive(Debug)]
struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn outcome(output: std::process::Output) -> Outcome {
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn interpreted(example: &str) -> Outcome {
    outcome(
        Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run", example])
            .logical_case(&format!("memoria-publica:interpretado:{example}"))
            .output()
            .expect("executa exemplo interpretado"),
    )
}

fn native(example: &str) -> Option<Outcome> {
    let (_driver, Some(runtime)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)?
    else {
        return None;
    };
    let artifacts = NativeArtifactDir::create().expect("cria diretório nativo marcado");
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(artifacts.path())
        .arg(example)
        .env("PINKER_RT_LIB", runtime)
        .logical_case(&format!("memoria-publica:build:{example}"))
        .output()
        .expect("compila exemplo nativo");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let executable = artifacts
        .path()
        .join(Path::new(example).file_stem().expect("nome do exemplo"));
    Some(outcome(
        Command::new(executable)
            .logical_case(&format!("memoria-publica:nativo:{example}"))
            .output()
            .expect("executa exemplo nativo"),
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
            "examples/hotfix_memoria_publica_paginas_esparsas_valido.pink",
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
        std::fs::read_to_string("docs/development/runtime-public-memory-host-containment.md")
            .expect("lê documento operacional");
    let operations_flat = operations.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "Pinker v0 não produz core dump por padrão",
        "PR_SET_PDEATHSIG=SIGKILL",
        "scripts/pinker-cleanup.sh",
        "O workload exato",
        "não identifica uma revisão Git",
    ] {
        assert!(
            operations_flat.contains(required),
            "documento omitiu: {required}"
        );
    }
}
