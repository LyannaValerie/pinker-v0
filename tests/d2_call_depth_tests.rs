//! D2 — profundidade de chamadas independente da pilha hospedada.

mod common;

use common::ControlledCommand as Command;
use pinker_v0::interpreter::{self, RuntimeValue};
use pinker_v0::{
    abstract_machine, abstract_machine_validate, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, ir, ir_validate, semantic,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_code(code: &str) -> Result<Option<RuntimeValue>, String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    semantic::check_program(&program).map_err(|error| error.to_string())?;
    let program_ir = ir::lower_program(&program).map_err(|error| error.to_string())?;
    ir_validate::validate_program(&program_ir).map_err(|error| error.to_string())?;
    let cfg = cfg_ir::lower_program(&program_ir).map_err(|error| error.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|error| error.to_string())?;
    let selected = instr_select::lower_program(&cfg).map_err(|error| error.to_string())?;
    instr_select_validate::validate_program(&selected).map_err(|error| error.to_string())?;
    let machine = abstract_machine::lower_program(&selected).map_err(|error| error.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|error| error.to_string())?;
    interpreter::run_program(&machine).map_err(|error| error.to_string())
}

fn direct_chain(depth: usize) -> String {
    let mut source = String::from("pacote main;\n");
    for index in 0..depth {
        if index + 1 == depth {
            source.push_str(&format!("carinho f{index}() -> bombom {{ mimo 73; }}\n"));
        } else {
            source.push_str(&format!(
                "carinho f{index}() -> bombom {{ mimo f{}(); }}\n",
                index + 1
            ));
        }
    }
    source.push_str("carinho principal() -> bombom { mimo f0(); }\n");
    source
}

fn indirect_chain(depth: usize) -> String {
    let mut source = String::from("pacote main;\n");
    for index in 0..depth {
        if index + 1 == depth {
            source.push_str(&format!("carinho f{index}() -> bombom {{ mimo 37; }}\n"));
        } else {
            source.push_str(&format!(
                "carinho f{index}() -> bombom {{ nova proxima: carinho() -> bombom = f{}; mimo proxima(); }}\n",
                index + 1
            ));
        }
    }
    source.push_str("carinho principal() -> bombom { mimo f0(); }\n");
    source
}

fn temporary_directory(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&path).expect("criar diretório temporário");
    path
}

#[test]
fn recursao_profunda_finita_ultrapassa_teto_historico() {
    for depth in [65, 128, 256, 512, 1024] {
        let source = format!(
            "pacote main; carinho descer(n: bombom) -> bombom {{ talvez n == 0 {{ mimo 42; }} senao {{ mimo descer(n - 1); }} }} carinho principal() -> bombom {{ mimo descer({depth}); }}"
        );
        let result = run_code(&source).unwrap_or_else(|error| panic!("depth={depth}: {error}"));
        assert_eq!(result, Some(RuntimeValue::Int(42)), "depth={depth}");
        eprintln!("depth={depth}: ok");
    }
}

#[test]
fn recursao_mutua_profunda_finita_preserva_resultado() {
    let result = run_code(
        "pacote main; carinho par(n: bombom) -> bombom { talvez n == 0 { mimo 1; } senao { mimo impar(n - 1); } } carinho impar(n: bombom) -> bombom { talvez n == 0 { mimo 0; } senao { mimo par(n - 1); } } carinho principal() -> bombom { mimo par(512); }",
    )
    .unwrap();

    assert_eq!(result, Some(RuntimeValue::Int(1)));
}

#[test]
fn cadeia_direta_profunda_nao_recursiva_retorna_corretamente() {
    let result = run_code(&direct_chain(160)).unwrap();

    assert_eq!(result, Some(RuntimeValue::Int(73)));
}

#[test]
fn retorno_desempilha_muitos_frames_sem_perder_valor() {
    let result = run_code(
        "pacote main; carinho somar(n: bombom) -> bombom { talvez n == 0 { mimo 0; } senao { mimo 1 + somar(n - 1); } } carinho principal() -> bombom { mimo somar(512); }",
    )
    .unwrap();

    assert_eq!(result, Some(RuntimeValue::Int(512)));
}

#[test]
fn erro_profundo_preserva_trace_e_nao_corrompe_execucao_seguinte() {
    let error = run_code(
        "pacote main; carinho queda(n: bombom) -> bombom { talvez n == 0 { mimo 10 / 0; } senao { mimo queda(n - 1); } } carinho principal() -> bombom { mimo queda(128); }",
    )
    .unwrap_err();
    assert!(error.contains("[runtime::divisao_por_zero]"), "{error}");
    assert!(error.contains("stack trace:"), "{error}");
    assert!(error.contains("frames omitidos"), "{error}");
    assert!(error.contains("at principal"), "{error}");
    assert!(error.contains("at queda"), "{error}");

    let next = run_code("pacote main; carinho principal() -> bombom { mimo 91; }").unwrap();
    assert_eq!(next, Some(RuntimeValue::Int(91)));
}

#[test]
fn cadeia_indireta_profunda_compartilha_frames_explicitos() {
    let result = run_code(&indirect_chain(96)).unwrap();

    assert_eq!(result, Some(RuntimeValue::Int(37)));
}

#[test]
fn recursao_profunda_tem_paridade_entre_interpretador_e_nativo() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let root = temporary_directory("pinker-d2-native-parity");
    let source_path = root.join("call_depth_parity.pink");
    let native_out = root.join("native");
    let source = r#"
        pacote main;
        carinho descer(n: bombom) -> bombom {
            talvez n == 0 { mimo 42; }
            senao { mimo descer(n - 1); }
        }
        carinho principal() -> bombom {
            falar(descer(256));
            mimo 0;
        }
    "#;
    std::fs::write(&source_path, source).expect("gravar fonte de paridade");

    let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&source_path)
        .output()
        .expect("executar interpretador");
    assert!(
        interpreted.status.success(),
        "{}",
        String::from_utf8_lossy(&interpreted.stderr)
    );

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&native_out)
        .arg(&source_path)
        .env("PINKER_RT_LIB", runtime_lib)
        .output()
        .expect("compilar fonte nativo");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );

    let native = Command::new(native_out.join("call_depth_parity"))
        .output()
        .expect("executar binário nativo");
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(interpreted.stdout, b"42\n");
    assert_eq!(native.stdout, interpreted.stdout);
    assert_eq!(native.status.code(), interpreted.status.code());

    std::fs::remove_dir_all(root).expect("limpar paridade nativa");
}
