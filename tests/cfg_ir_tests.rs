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

#[test]
fn cfg_ir_sempre_que_com_quebrar() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 3 {
                quebrar;
            }
            mimo x;
        }";
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("block loop_cond_"), "{}", cfg);
    assert!(cfg.contains("block loop_"), "{}", cfg);
    assert!(cfg.contains("loop_join_"), "{}", cfg);
    assert!(cfg.contains("br verdade:logica"), "{}", cfg);
}

#[test]
fn cfg_ir_sempre_que_com_continuar() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 3 {
                x = x + 1;
                continuar;
            }
            mimo x;
        }";
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("block loop_cond_"), "{}", cfg);
    assert!(cfg.contains("loop_continue_cont"), "{}", cfg);
}

#[test]
fn cfg_ir_bitwise_basico() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = 6;
            nova b = 3;
            mimo (a & b) | (a ^ b) + (a << 1) + (a >> 1);
        }";
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("bitand"), "{}", cfg);
    assert!(cfg.contains("bitor"), "{}", cfg);
    assert!(cfg.contains("shl"), "{}", cfg);
}

#[test]
fn cfg_ir_if_else_fallthrough_ambos_ramos_gera_join_valido() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            talvez verdade {
                x = x + 1;
            } senao {
                x = x + 2;
            }
            mimo x;
        }";

    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("block then_0:"), "{}", cfg);
    assert!(cfg.contains("block else_1:"), "{}", cfg);
    assert!(cfg.contains("br verdade:logica, then_0, else_1"), "{}", cfg);
    assert!(cfg.contains("jmp join_"), "{}", cfg);
    assert!(cfg.contains("block join_"), "{}", cfg);
    assert!(cfg.contains("ret %x#0"), "{}", cfg);
}
