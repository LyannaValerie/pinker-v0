mod common;

use common::{parse, render_cli_ir_output, render_ir};
use pinker_v0::ir;

#[test]
fn lowering_de_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let program = parse(code).unwrap();
    let lowered = ir::lower_program(&program).unwrap();
    assert_eq!(lowered.module_name, "main");
    assert_eq!(lowered.consts.len(), 0);
    assert_eq!(lowered.functions.len(), 1);
    assert_eq!(lowered.functions[0].name, "principal");
}

#[test]
fn lowering_de_constante_global() {
    let code = "\
pacote main;
eterno LIMITE: bombom = 10;
carinho principal() -> bombom { mimo LIMITE; }";
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir,
        "\
module main
consts:
  const @LIMITE: bombom = 10:bombom
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      return @LIMITE
"
    );
}

#[test]
fn lowering_de_atribuicao() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    nova mut x = 1;
    x = 2;
    mimo x;
}";
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir,
        "\
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals:
      %x#0: bombom mut
    block entry:
      let %x#0 = 1:bombom
      assign %x#0 = 2:bombom
      return %x#0
"
    );
}

#[test]
fn lowering_de_if_else() {
    let code = "\
pacote main;

carinho principal() -> bombom {
    talvez verdade {
        mimo 1;
    } senao {
        mimo 0;
    }
}";
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir,
        "\
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      if verdade:logica
        block then_0:
          return 1:bombom
        block else_1:
          return 0:bombom
"
    );
}

#[test]
fn lowering_de_chamada_de_funcao() {
    let code = "\
pacote main;
carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
carinho principal() -> bombom { mimo soma(1, 2); }";
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir,
        "\
module main
consts:
  []
functions:
  func soma -> bombom
    params:
      %x#0: bombom
      %y#0: bombom
    locals: []
    block entry:
      return add(%x#0, %y#0)
  func principal -> bombom
    params: []
    locals: []
    block entry:
      return call soma(1:bombom, 2:bombom) -> bombom
"
    );
}

#[test]
fn lowering_de_funcao_sem_retorno() {
    let code = "\
pacote main;
carinho log() { mimo; }
carinho principal() -> bombom {
    log();
    mimo 0;
}";
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir,
        "\
module main
consts:
  []
functions:
  func log -> nulo
    params: []
    locals: []
    block entry:
      return
  func principal -> bombom
    params: []
    locals: []
    block entry:
      expr call log() -> nulo
      return 0:bombom
"
    );
}

#[test]
fn ir_de_principal_tem_cabecalho_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let cli = render_cli_ir_output(code).unwrap();
    assert_eq!(
        cli,
        "\
=== IR ===
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      return 0:bombom
Análise semântica concluída sem erros.
"
    );
}

#[test]
fn lowering_de_sempre_que() {
    let code = "
pacote main;
carinho principal() -> bombom {
  nova mut x = 0;
  sempre que x < 3 {
    x = x + 1;
  }
  mimo x;
}";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("while"), "{}", ir);
    assert!(ir.contains("block loop_"), "{}", ir);
}

#[test]
fn lowering_de_sempre_que_com_quebrar() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 3 {
                quebrar;
            }
            mimo x;
        }";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("while lt(%x#0, 3:bombom)"), "{}", ir);
    assert!(ir.contains("break loop_break_join_"), "{}", ir);
}

#[test]
fn lowering_de_sempre_que_com_continuar() {
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
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("continue loop_continue_"), "{}", ir);
}

#[test]
fn lowering_de_bitwise_basico() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = 6;
            nova b = 3;
            mimo (a & b) | (a ^ b) + (a << 1) + (a >> 1);
        }";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("bitand"), "{}", ir);
    assert!(ir.contains("bitor"), "{}", ir);
    assert!(ir.contains("bitxor"), "{}", ir);
    assert!(ir.contains("shl"), "{}", ir);
    assert!(ir.contains("shr"), "{}", ir);
}
