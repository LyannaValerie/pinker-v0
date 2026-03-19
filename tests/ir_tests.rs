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
fn lowering_de_cast_explicito_inteiro() {
    let code = "\
pacote main;
carinho principal() -> bombom {
    nova x: u16 = 513;
    nova y: u8 = x virar u8;
    mimo y virar bombom;
}";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("%x#0 virar u8"), "{}", ir);
    assert!(ir.contains("%y#0 virar bombom"), "{}", ir);
}

#[test]
fn lowering_de_peso_e_alinhamento_vira_literal_constante() {
    let code = r#"
pacote main;
ninho Ponto { a: u8; b: u32; c: u16; }
carinho principal() -> bombom {
    mimo peso(Ponto) + alinhamento(Ponto) + peso([u16; 3]) + alinhamento(seta<u8>);
}
"#;
    let ir = render_ir(code).unwrap();
    assert!(
        ir.contains("return add(add(add(12:bombom, 4:bombom), 6:bombom), 8:bombom)"),
        "{}",
        ir
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
fn lowering_de_logicos_basicos() {
    let code = "
pacote main;
carinho principal() -> bombom {
  nova a = verdade;
  nova b = falso;
  talvez a && b || !a { mimo 1; } senao { mimo 0; }
}";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("and("), "{}", ir);
    assert!(ir.contains("or("), "{}", ir);
}

#[test]
fn lowering_de_unsigned_fixos_preserva_tipos() {
    let code = r#"
pacote main;
carinho soma_u8(a: u8, b: u8) -> u8 { mimo a + b; }
carinho soma_u64(a: u64, b: u64) -> u64 { mimo a + b; }
carinho principal() -> bombom { mimo soma_u64(40, 2); }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func soma_u8 -> u8"), "{}", ir);
    assert!(ir.contains("%a#0: u8"), "{}", ir);
    assert!(ir.contains("func soma_u64 -> u64"), "{}", ir);
    assert!(
        ir.contains("return call soma_u64(40:bombom, 2:bombom) -> u64"),
        "{}",
        ir
    );
}

#[test]
fn lowering_de_signed_fixos_preserva_tipos() {
    let code = r#"
pacote main;
carinho soma_i8(a: i8, b: i8) -> i8 { mimo a + b; }
carinho sub_i64(a: i64, b: i64) -> i64 { mimo a - b; }
carinho principal() -> bombom {
  nova n: i64 = 40;
  nova m: i64 = 2;
  nova r: i64 = sub_i64(-n, -m);
  sub_i64(r, m);
  mimo 42;
}
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func soma_i8 -> i8"), "{}", ir);
    assert!(ir.contains("%a#0: i8"), "{}", ir);
    assert!(ir.contains("func sub_i64 -> i64"), "{}", ir);
    assert!(
        ir.contains("let %r#0 = call sub_i64(neg(%n#0), neg(%m#0)) -> i64"),
        "{}",
        ir
    );
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

#[test]
fn lowering_de_modulo_basico() {
    let code = "\
pacote main;
carinho principal() -> bombom { mimo 10 % 4; }";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("return mod(10:bombom, 4:bombom)"), "{}", ir);
}

#[test]
fn lowering_de_acesso_a_campo_e_indexacao() {
    let code = r#"
pacote main;
ninho Ponto { x: bombom; y: bombom; }
carinho combina(p: Ponto, a: [bombom; 3], i: bombom) -> bombom {
  mimo p.x + a[i];
}
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("%p#0.x"), "{}", ir);
    assert!(ir.contains("%a#0[%i#0]"), "{}", ir);
}

#[test]
fn lowering_resolve_alias_de_tipo_para_tipo_subjacente() {
    let code = r#"
pacote main;
apelido Byte = u8;
carinho id(x: Byte) -> Byte { mimo x; }
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func id -> u8"), "{}", ir);
    assert!(ir.contains("%x#0: u8"), "{}", ir);
}

#[test]
fn lowering_preserva_tipo_array_fixo_em_assinatura() {
    let code = r#"
pacote main;
apelido Bytes4 = [u8; 4];
carinho usa(buf: Bytes4) -> bombom { mimo 0; }
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func usa -> bombom"), "{}", ir);
    assert!(ir.contains("%buf#0: [u8; 4]"), "{}", ir);
}

#[test]
fn lowering_preserva_tipo_ninho_em_assinatura() {
    let code = r#"
pacote main;
ninho Ponto {
  x: bombom;
  y: bombom;
}
carinho usa(p: Ponto) -> Ponto { mimo p; }
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func usa -> struct"), "{}", ir);
    assert!(ir.contains("%p#0: struct"), "{}", ir);
}

#[test]
fn lowering_preserva_categoria_seta_em_assinatura() {
    let code = r#"
pacote main;
ninho Ponto { x: bombom; }
apelido PtrPonto = seta<Ponto>;
carinho id(p: PtrPonto) -> PtrPonto { mimo p; }
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func id -> seta<?>"), "{}", ir);
    assert!(ir.contains("%p#0: seta<?>"), "{}", ir);
}

#[test]
fn lowering_preserva_categoria_seta_fragil_em_assinatura() {
    let code = r#"
pacote main;
apelido Porta = fragil seta<u8>;
carinho id(p: Porta) -> Porta { mimo p; }
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("func id -> fragil seta<?>"), "{}", ir);
    assert!(ir.contains("%p#0: fragil seta<?>"), "{}", ir);
}
