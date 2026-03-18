mod common;

use common::{render_cfg_ir, render_cli_cfg_ir_output};

#[test]
fn cfg_ir_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let cfg = render_cfg_ir(code).unwrap();
    assert_eq!(
        cfg,
        "\
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      ret 0:bombom
"
    );
}

#[test]
fn cfg_ir_if_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    talvez verdade { mimo 1; } senao { mimo 0; }
}";
    let cfg = render_cfg_ir(code).unwrap();
    assert_eq!(
        cfg,
        "\
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      br verdade:logica, then_0, else_1
    block then_0:
      ret 1:bombom
    block else_1:
      ret 0:bombom
"
    );
}

#[test]
fn cfg_ir_if_sem_else() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    talvez verdade { nova x = 1; }
    mimo 0;
}";
    let cfg = render_cfg_ir(code).unwrap();
    assert_eq!(
        cfg,
        "\
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals:
      %x#0: bombom
    block entry:
      br verdade:logica, then_0, join_0
    block then_0:
      let %x#0 = 1:bombom
      jmp join_0
    block join_0:
      ret 0:bombom
"
    );
}

#[test]
fn cfg_ir_return_vazio_e_chamada_direta() {
    let code = "\
pacote main;
carinho log() { mimo; }
carinho principal() -> bombom {
    log();
    mimo 0;
}";
    let cfg = render_cfg_ir(code).unwrap();
    assert_eq!(
        cfg,
        "\
module main
consts:
  []
functions:
  func log -> nulo
    params: []
    locals: []
    block entry:
      ret
  func principal -> bombom
    params: []
    locals: []
    block entry:
      call log() -> nulo
      ret 0:bombom
"
    );
}

#[test]
fn cfg_ir_cli_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let cli = render_cli_cfg_ir_output(code).unwrap();
    assert_eq!(
        cli,
        "\
=== CFG IR ===
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      ret 0:bombom
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn cfg_ir_sempre_que() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 3 { x = x + 1; }
            mimo x;
        }";
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("block loop_cond_"), "{}", cfg);
    assert!(cfg.contains("block loop_"), "{}", cfg);
    assert!(cfg.contains("block loop_join_"), "{}", cfg);
}
