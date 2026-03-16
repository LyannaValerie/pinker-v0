mod common;

use common::{render_backend_text, render_cli_pseudo_asm_output};

#[test]
fn emite_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_backend_text(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
text:
  func principal:
    params []
    locals []
    entry:
      ret 0
"
    );
}

#[test]
fn emite_if_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let out = render_backend_text(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
text:
  func principal:
    params []
    locals []
    entry:
      br verdade, then_0, else_1
    then_0:
      ret 1
    else_1:
      ret 0
"
    );
}

#[test]
fn emite_if_sem_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    talvez verdade { nova x = 1; }
    mimo 0;
}";
    let out = render_backend_text(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
text:
  func principal:
    params []
    locals %x#0
    entry:
      br verdade, then_0, join_0
    then_0:
      mov %x#0, 1
      jmp join_0
    join_0:
      ret 0
"
    );
}

#[test]
fn emite_chamada_direta() {
    let code = "\
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom { mimo soma(1, 2); }";
    let out = render_backend_text(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
text:
  func soma:
    params %x#0, %y#0
    locals []
    entry:
      %t0 = add %x#0, %y#0
      ret %t0
  func principal:
    params []
    locals []
    entry:
      %t0 = call soma(1, 2) -> bombom
      ret %t0
"
    );
}

#[test]
fn emite_return_vazio_e_funcao_nulo() {
    let code = "\
pacote main;
carinho log() { mimo; }
carinho principal() -> bombom {
    log();
    mimo 0;
}";
    let out = render_backend_text(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
text:
  func log:
    params []
    locals []
    entry:
      ret
  func principal:
    params []
    locals []
    entry:
      call log() -> nulo
      ret 0
"
    );
}

#[test]
fn emite_constante_global_e_principal() {
    let code = "\
pacote main;
eterno LIMITE: bombom = 10;
carinho principal() -> bombom { mimo LIMITE; }";
    let out = render_backend_text(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  global @LIMITE = 10
text:
  func principal:
    params []
    locals []
    entry:
      ret @LIMITE
"
    );
}

#[test]
fn cli_pseudo_asm_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_cli_pseudo_asm_output(code).unwrap();
    assert_eq!(
        out,
        "\
=== PSEUDO ASM ===
module main
globals:
  []
text:
  func principal:
    params []
    locals []
    entry:
      ret 0
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn validador_cfg_falha_quando_cfg_invalida() {
    let cfg = pinker_v0::cfg_ir::ProgramCfgIR {
        module_name: "main".to_string(),
        consts: vec![],
        functions: vec![pinker_v0::cfg_ir::FunctionCfgIR {
            name: "principal".to_string(),
            params: vec![],
            locals: vec![],
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            entry: "entry".to_string(),
            blocks: vec![pinker_v0::cfg_ir::BasicBlockIR {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: pinker_v0::cfg_ir::TerminatorIR::Return(None),
            }],
            span: pinker_v0::token::Span::new(
                pinker_v0::token::Position::new(1, 1),
                pinker_v0::token::Position::new(1, 1),
            ),
        }],
    };

    let err = pinker_v0::cfg_ir_validate::validate_program(&cfg).unwrap_err();
    assert!(err.to_string().contains("Erro Validação CFG IR"));
}

#[test]
fn check_ignora_flags_de_emissao() {
    use std::fs;
    use std::process::Command;

    let mut path = std::env::temp_dir();
    path.push(format!("pinker_check_{}.pink", std::process::id()));
    fs::write(
        &path,
        "pacote main; carinho principal() -> bombom { mimo 0; }",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg("--pseudo-asm")
        .arg(&path)
        .output()
        .unwrap();

    let _ = fs::remove_file(&path);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("=== PSEUDO ASM ==="));
}
