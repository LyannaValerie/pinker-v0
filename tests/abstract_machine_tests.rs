mod common;

use common::{render_backend_text, render_cli_machine_output, render_machine, render_selected};

#[test]
fn machine_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_machine(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
machine:
  func principal:
    params []
    locals []
    entry:
      vm push_int 0
      term ret
"
    );
}

#[test]
fn machine_if_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let out = render_machine(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
machine:
  func principal:
    params []
    locals []
    entry:
      vm push_bool verdade
      term br_true then_0, else_1
    then_0:
      vm push_int 1
      term ret
    else_1:
      vm push_int 0
      term ret
"
    );
}

#[test]
fn machine_chamada_e_call_void() {
    let code = "\
pacote main;
carinho log() { mimo; }
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom {
  log();
  mimo soma(1,2);
}";
    let out = render_machine(code).unwrap();
    assert_eq!(
        out,
        "\
module main
globals:
  []
machine:
  func log:
    params []
    locals []
    entry:
      term ret_void
  func soma:
    params %x#0, %y#0
    locals []
    entry:
      vm load_slot %x#0
      vm load_slot %y#0
      vm add
      vm store_slot %t0
      vm load_slot %t0
      term ret
  func principal:
    params []
    locals []
    entry:
      vm call_void log, 0
      vm push_int 1
      vm push_int 2
      vm call soma, 2
      vm store_slot %t0
      vm load_slot %t0
      term ret
"
    );
}

#[test]
fn machine_unaria_binaria_temporarios() {
    let code = "\
pacote main;
carinho principal() -> bombom {
  nova x = 1;
  nova y = 2;
  mimo -(x + y);
}";
    let out = render_machine(code).unwrap();
    assert!(out.contains("vm add\n      vm store_slot %t0\n"));
    assert!(out.contains("vm neg\n      vm store_slot %t1\n"));
}

#[test]
fn machine_cli_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let out = render_cli_machine_output(code).unwrap();
    assert_eq!(
        out,
        "\
=== MACHINE ===
module main
globals:
  []
machine:
  func principal:
    params []
    locals []
    entry:
      vm push_int 0
      term ret
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn machine_diferente_de_selected_e_pseudo_asm() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let machine = render_machine(code).unwrap();
    let selected = render_selected(code).unwrap();
    let backend = render_backend_text(code).unwrap();
    assert_ne!(machine, selected);
    assert_ne!(machine, backend);
}

#[test]
fn machine_falha_em_programa_invalido() {
    let program = pinker_v0::abstract_machine::MachineProgram {
        module_name: "main".to_string(),
        globals: vec![],
        functions: vec![pinker_v0::abstract_machine::MachineFunction {
            name: "principal".to_string(),
            ret_type: pinker_v0::ir::TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            blocks: vec![pinker_v0::abstract_machine::MachineBlock {
                label: "entry".to_string(),
                code: vec![],
                terminator: pinker_v0::abstract_machine::MachineTerminator::RetVoid,
            }],
        }],
    };

    let err = pinker_v0::abstract_machine_validate::validate_program(&program).unwrap_err();
    assert!(err.to_string().contains("Erro Validação Máquina Abstrata"));
}
