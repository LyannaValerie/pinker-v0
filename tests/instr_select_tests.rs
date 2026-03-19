mod common;

use common::{render_cli_selected_output, render_selected};

#[test]
fn seleciona_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_selected(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
selected:
  func principal:
    params []
    locals []
    entry:
      term ret 0
"
    );
}

#[test]
fn seleciona_if_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let out = render_selected(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
selected:
  func principal:
    params []
    locals []
    entry:
      term br verdade, then_0, else_1
    then_0:
      term ret 1
    else_1:
      term ret 0
"
    );
}

#[test]
fn seleciona_if_sem_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  talvez verdade { nova x = 1; }
  mimo 0;
}";
    let out = render_selected(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
selected:
  func principal:
    params []
    locals %x#0
    entry:
      term br verdade, then_0, join_0
    then_0:
      isel mov %x#0, 1
      term jmp join_0
    join_0:
      term ret 0
"
    );
}

#[test]
fn seleciona_chamada_retorno_e_call_void() {
    let code = "\
pacote main;
carinho log() { mimo; }
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom {
  log();
  mimo soma(1, 2);
}";
    let out = render_selected(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
selected:
  func log:
    params []
    locals []
    entry:
      term ret
  func soma:
    params %x#0, %y#0
    locals []
    entry:
      isel add %t0, %x#0, %y#0
      term ret %t0
  func principal:
    params []
    locals []
    entry:
      isel call_void log()
      isel call %t0, soma(1, 2), bombom
      term ret %t0
"
    );
}

#[test]
fn seleciona_unario_e_binaria() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova x = 1;
  nova y = 2;
  mimo -(x + y);
}";
    let out = render_selected(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
selected:
  func principal:
    params []
    locals %x#0, %y#0
    entry:
      isel mov %x#0, 1
      isel mov %y#0, 2
      isel add %t0, %x#0, %y#0
      isel neg %t1, %t0
      term ret %t1
"
    );
}

#[test]
fn selected_cli_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_cli_selected_output(code).unwrap();
    assert_eq!(
        out,
        "\
=== SELECTED ===
module main
globals:
  []
selected:
  func principal:
    params []
    locals []
    entry:
      term ret 0
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn falha_clara_para_call_sem_destino() {
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
                instructions: vec![pinker_v0::cfg_ir::InstructionCfgIR::Call {
                    dest: None,
                    callee: "f".to_string(),
                    args: vec![],
                    ret_type: pinker_v0::ir::TypeIR::Bombom,
                }],
                terminator: pinker_v0::cfg_ir::TerminatorIR::Return(Some(
                    pinker_v0::cfg_ir::OperandIR::Int(0),
                )),
            }],
            span: pinker_v0::token::Span::new(
                pinker_v0::token::Position::new(1, 1),
                pinker_v0::token::Position::new(1, 1),
            ),
        }],
    };

    let err = pinker_v0::instr_select::lower_program(&cfg).unwrap_err();
    assert!(err.to_string().contains("instruction selection"));
}

#[test]
fn seleciona_sempre_que() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 2 { x = x + 1; }
            mimo x;
        }";
    let out = render_selected(code).unwrap();
    assert!(out.contains("loop_cond_"), "{}", out);
    assert!(out.contains("term br"), "{}", out);
}

#[test]
fn seleciona_sempre_que_com_quebrar() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 2 { quebrar; }
            mimo x;
        }";
    let out = render_selected(code).unwrap();
    assert!(out.contains("term br verdade"), "{}", out);
    assert!(out.contains("loop_join_"), "{}", out);
}

#[test]
fn seleciona_sempre_que_com_continuar() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 2 {
                x = x + 1;
                continuar;
            }
            mimo x;
        }";
    let out = render_selected(code).unwrap();
    assert!(out.contains("loop_cond_"), "{}", out);
    assert!(out.contains("loop_continue_cont"), "{}", out);
}

#[test]
fn seleciona_bitwise_basico() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = 6;
            nova b = 3;
            mimo (a & b) | (a ^ b) + (a << 1) + (a >> 1);
        }";
    let out = render_selected(code).unwrap();
    assert!(out.contains("isel bitand"), "{}", out);
    assert!(out.contains("isel bitor"), "{}", out);
    assert!(out.contains("isel bitxor"), "{}", out);
    assert!(out.contains("isel shl"), "{}", out);
    assert!(out.contains("isel shr"), "{}", out);
}

#[test]
fn seleciona_modulo_basico() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo 10 % 4;
        }";
    let out = render_selected(code).unwrap();
    assert!(out.contains("isel mod %t0, 10, 4"), "{}", out);
}
