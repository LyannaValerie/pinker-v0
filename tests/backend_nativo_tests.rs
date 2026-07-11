//! Testes do pipeline nativo (Eixo B do Bloco 20, fase B1):
//! emissão `.s` com init de runtime e build/link/execução de executável real.

use pinker_v0::{
    backend_s, cfg_ir, cfg_ir_validate, instr_select, instr_select_validate, ir, ir_validate,
    lexer::Lexer, parser::Parser, semantic,
};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn lower_to_selected(code: &str) -> pinker_v0::instr_select::SelectedProgram {
    let mut lexer = Lexer::new(code);
    let tokens = lexer.tokenize().expect("lex");
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("parse");
    semantic::check_program(&program).expect("semantic");
    let program_ir = ir::lower_program(&program).expect("ir");
    ir_validate::validate_program(&program_ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&program_ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("select");
    instr_select_validate::validate_program(&selected).expect("select validate");
    selected
}

#[test]
fn emissao_nativa_inclui_init_do_runtime() {
    let code = include_str!("../examples/fase212_build_nativo_fumaca_valido.pink");
    let selected = lower_to_selected(code);
    let nativo = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("emit nativo");
    assert!(nativo.contains("call pinker_rt_iniciar"), "{}", nativo);
    assert!(nativo.contains(".globl main"), "{}", nativo);
}

#[test]
fn emissao_padrao_nao_inclui_init_do_runtime() {
    let code = include_str!("../examples/fase212_build_nativo_fumaca_valido.pink");
    let selected = lower_to_selected(code);
    let padrao = backend_s::emit_external_toolchain_subset(&selected).expect("emit padrao");
    assert!(!padrao.contains("pinker_rt_iniciar"), "{}", padrao);
}

fn detect_cc_driver() -> Option<String> {
    ["cc", "gcc", "clang"].iter().find_map(|candidate| {
        let probe = Command::new(candidate).arg("--version").output().ok()?;
        if probe.status.success() {
            Some((*candidate).to_string())
        } else {
            None
        }
    })
}

#[test]
fn build_nativo_produz_executavel_real_com_runtime() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        // `make ci` roda `cargo build` antes dos testes, garantindo a staticlib;
        // em invocações avulsas sem o artefato, o teste não é conclusivo.
        eprintln!("libpinker_rt.a ausente; pulando teste de build nativo");
        return;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase212_{}", nanos));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("examples/fase212_build_nativo_fumaca_valido.pink")
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let bin_path = out_dir.join("fase212_build_nativo_fumaca_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(
        run.status.code(),
        Some(42),
        "esperava código 42 do executável nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}
