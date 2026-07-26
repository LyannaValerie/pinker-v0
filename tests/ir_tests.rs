mod common;

use common::{parse, render_cli_ir_output, render_ir};
use pinker_v0::ir;

// @pinker-nav:start evidencia.ir.lowering-programa
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita diretamente o lowering AST para IR e inspeciona a estrutura do programa resultante.
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
// @pinker-nav:end evidencia.ir.lowering-programa

// @pinker-nav:start evidencia.ir.renderizacao-estruturas-basicas
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Renderiza IR textual após lowering e compara estruturas básicas, tipos, controle e chamadas.
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
mode hospedado
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
    nova muda x = 1;
    x = 2;
    mimo x;
}";
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir,
        "\
module main
mode hospedado
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals:
      %x#0: bombom muda
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
fn lowering_de_verso_preserva_literal_e_tipo() {
    let code = r#"
pacote main;
eterno MSG: verso = "oi";
carinho eco(s: verso) -> verso { mimo s; }
carinho principal() -> bombom {
    nova a: verso = eco(MSG);
    mimo 0;
}
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("const @MSG: verso = \"oi\":verso"), "{}", ir);
    assert!(ir.contains("func eco -> verso"), "{}", ir);
    assert!(ir.contains("%s#0: verso"), "{}", ir);
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
mode hospedado
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
mode hospedado
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
mode hospedado
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
// @pinker-nav:end evidencia.ir.renderizacao-estruturas-basicas

// @pinker-nav:start evidencia.ir.renderizacao-cli
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Compara exatamente o cabeçalho e o texto de IR expostos pelo renderer de CLI.
#[test]
fn ir_de_principal_tem_cabecalho_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let cli = render_cli_ir_output(code).unwrap();
    assert_eq!(
        cli,
        "\
=== IR ===
module main
mode hospedado
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
// @pinker-nav:end evidencia.ir.renderizacao-cli

// @pinker-nav:start evidencia.ir.lowering-controle-de-laco
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita lowering estruturado de laços, quebrar e continuar e inspeciona fragmentos renderizados.
#[test]
fn lowering_de_sempre_que() {
    let code = "
pacote main;
carinho principal() -> bombom {
  nova muda x = 0;
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
            nova muda x = 0;
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
            nova muda x = 0;
            sempre que x < 3 {
                x = x + 1;
                continuar;
            }
            mimo x;
        }";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("continue loop_continue_"), "{}", ir);
}
// @pinker-nav:end evidencia.ir.lowering-controle-de-laco

// @pinker-nav:start evidencia.ir.lowering-operacoes-textuais
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita a preservação textual de asm inline e operadores lógicos na IR estruturada.
#[test]
fn lowering_preserva_inline_asm_textual() {
    let code = r#"
pacote main;
carinho principal() -> bombom {
  sussurro("mov rax, 60", "syscall");
  mimo 0;
}
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("inline_asm [mov rax, 60 | syscall]"), "{}", ir);
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
// @pinker-nav:end evidencia.ir.lowering-operacoes-textuais

// @pinker-nav:start evidencia.ir.lowering-tipos-numericos
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Inspeciona tipos inteiros fixos e operações bitwise e módulo na IR renderizada.
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
// @pinker-nav:end evidencia.ir.lowering-tipos-numericos

// @pinker-nav:start evidencia.ir.lowering-tipos-compostos
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita acessos e a preservação observada de aliases, arrays, ninhos e categorias de ponteiro na IR textual.
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

#[test]
fn fase242_referencia_de_funcao_top_level_vira_fnref() {
    let code = r#"
pacote main;
carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
carinho principal() -> bombom {
    nova operacao: carinho(bombom) -> bombom = dobrar;
    mimo 0;
}
"#;
    let ir = render_ir(code).unwrap();
    // Fase 243: fnref aponta para o wrapper sintético `__fnref_env_dobrar`
    // (aceita e ignora `__env`, convenção uniforme de toda chamada
    // indireta); `dobrar` em si nunca muda de assinatura.
    assert!(ir.contains("fnref(__fnref_env_dobrar)"), "{}", ir);
}

#[test]
fn fase242_chamada_por_variavel_vira_call_indirect() {
    let code = r#"
pacote main;
carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
carinho aplicar(operacao: carinho(bombom) -> bombom, valor: bombom) -> bombom {
    mimo operacao(valor);
}
carinho principal() -> bombom { mimo aplicar(dobrar, 1); }
"#;
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("call_indirect"), "{}", ir);
    assert!(
        !ir.contains("call operacao("),
        "chamada por variável não pode virar call direta por nome: {}",
        ir
    );
}

#[test]
fn fase242_chamada_direta_legada_continua_como_call_por_nome() {
    let code = "pacote main; carinho dobrar(x: bombom) -> bombom { mimo x * 2; } carinho principal() -> bombom { mimo dobrar(21); }";
    let ir = render_ir(code).unwrap();
    assert!(ir.contains("call dobrar("), "{}", ir);
    assert!(!ir.contains("call_indirect"), "{}", ir);
}

#[test]
fn fase243_closure_com_captura_vira_make_closure() {
    let code = r#"
pacote main;
carinho fabricar(base: bombom) -> carinho() -> bombom {
    mimo carinho() -> bombom {
        mimo base;
    };
}
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert!(
        ir.contains("make_closure __anon_carinho_"),
        "closure referenciada como valor deveria virar make_closure: {}",
        ir
    );
    assert!(
        ir.contains("%__env#0: seta<?>"),
        "corpo da closure deveria ganhar parâmetro __env: {}",
        ir
    );
    assert!(
        ir.contains("deref(add(%__env#0, 0:bombom))"),
        "corpo deveria carregar a captura a partir de __env: {}",
        ir
    );
}

#[test]
fn fase243_closure_sem_captura_com_nova_muda_ainda_usa_make_closure() {
    // Regressão de interação: `nova` (sem `muda`) com literal não capturante
    // continua tomando o caminho rápido pré-existente da Fase 238/239 (vira
    // `call` direta, sem `make_closure` — ver teste companheiro abaixo). Com
    // `nova muda`, o caminho geral é obrigatório e, mesmo sem capturas reais,
    // a closure passa por `resolve_closure`/`make_closure` (lista vazia) e
    // ganha `__env` (ignorado em runtime), mantendo a convenção uniforme.
    let code = r#"
pacote main;
carinho principal() -> bombom {
    nova muda f: carinho() -> bombom = carinho() -> bombom {
        mimo 7;
    };
    mimo f();
}
"#;
    let ir = render_ir(code).unwrap();
    assert!(
        ir.contains("make_closure __anon_carinho_1[]"),
        "closure sem captura com nova muda deveria usar make_closure com lista vazia: {}",
        ir
    );
    assert!(ir.contains("%__env#0: seta<?>"), "{}", ir);
}

#[test]
fn fase243_closure_sem_captura_com_nova_simples_usa_caminho_rapido_pre_existente() {
    // Companheiro do teste acima: sem `muda`, a otimização da Fase 238/239
    // permanece intacta — `f` nunca vira variável real, `f()` vira chamada
    // direta ao nome sintético, sem `call_indirect` nem `make_closure`.
    let code = r#"
pacote main;
carinho principal() -> bombom {
    nova f: carinho() -> bombom = carinho() -> bombom {
        mimo 7;
    };
    mimo f();
}
"#;
    let ir = render_ir(code).unwrap();
    assert!(!ir.contains("make_closure"), "{}", ir);
    assert!(!ir.contains("call_indirect"), "{}", ir);
    assert!(ir.contains("call __anon_carinho_1("), "{}", ir);
}

#[test]
fn fase243_closure_aninhada_gera_duas_make_closure() {
    let code = r#"
pacote main;
carinho fabricar(base: bombom) -> carinho() -> carinho() -> bombom {
    mimo carinho() -> carinho() -> bombom {
        mimo carinho() -> bombom {
            mimo base;
        };
    };
}
carinho principal() -> bombom { mimo 0; }
"#;
    let ir = render_ir(code).unwrap();
    assert_eq!(
        ir.matches("make_closure").count(),
        2,
        "closure aninhada deveria produzir duas ocorrências de make_closure: {}",
        ir
    );
}
// @pinker-nav:end evidencia.ir.lowering-tipos-compostos

// @pinker-nav:start evidencia.ir.lowering-objetos-trato-fase244
// @pinker-nav:domain ir
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita o lowering completo da superfície da Fase 244 para a IR estruturada: materialização explícita, ordem de vtable, tamanho do snapshot, despacho dinâmico direto e qualificado e método sem retorno.

#[test]
fn fase244_lowering_materializa_objeto_e_preserva_ordem_da_vtable() {
    let code = r#"
pacote main;

trato Duplo {
    carinho primeiro(valor: si) -> bombom;
    carinho segundo(valor: si) -> bombom;
}

impl Duplo para bombom {
    carinho primeiro(valor: bombom) -> bombom {
        mimo valor;
    }

    carinho segundo(valor: bombom) -> bombom {
        mimo valor + 1;
    }
}

carinho empacotar(valor: bombom) -> trato<Duplo> {
    mimo valor virar trato<Duplo>;
}

carinho principal() -> bombom {
    mimo 0;
}
"#;

    let rendered = render_ir(code).unwrap();

    assert!(
        rendered.contains(
            "make_trait_object trato<Duplo> from %valor#0 as bombom:bombom size=8 \
vtable=[__impl_5_Duplo_6_bombom_primeiro, __impl_5_Duplo_6_bombom_segundo]"
        ),
        "IR inesperada:\n{rendered}"
    );
}

#[test]
fn fase244_lowering_despacha_metodo_dinamico_por_receiver() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si, fator: bombom) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom, fator: bombom) -> bombom {
        mimo valor * fator;
    }
}

carinho consultar(
    objeto: trato<Medivel>
) -> bombom {
    mimo objeto.medir(2);
}

carinho principal() -> bombom {
    mimo 0;
}
"#;

    let rendered = render_ir(code).unwrap();

    assert!(
        rendered.contains("trait_call trato<Medivel>.medir#0/1 %objeto#0(2:bombom) -> bombom"),
        "IR inesperada:\n{rendered}"
    );

    assert!(
        !rendered.contains("call __impl_7_Medivel"),
        "objeto de trato não pode voltar ao despacho estático:\n{rendered}"
    );
}

#[test]
fn fase244_lowering_despacha_forma_qualificada_dinamicamente() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom {
        mimo valor;
    }
}

carinho consultar(
    objeto: trato<Medivel>
) -> bombom {
    mimo Medivel.medir(objeto);
}

carinho principal() -> bombom {
    mimo 0;
}
"#;

    let rendered = render_ir(code).unwrap();

    assert!(
        rendered.contains("trait_call trato<Medivel>.medir#0/1 %objeto#0() -> bombom"),
        "IR inesperada:\n{rendered}"
    );
}

#[test]
fn fase244_lowering_preserva_chamada_dinamica_sem_retorno() {
    let code = r#"
pacote main;

trato Observavel {
    carinho observar(valor: si, codigo: bombom);
}

impl Observavel para bombom {
    carinho observar(valor: bombom, codigo: bombom) {
        falar(valor, codigo);
        mimo;
    }
}

carinho usar(
    objeto: trato<Observavel>
) {
    objeto.observar(7);
    mimo;
}

carinho principal() -> bombom {
    mimo 0;
}
"#;

    let rendered = render_ir(code).unwrap();

    assert!(
        rendered.contains("trait_call trato<Observavel>.observar#0/1 %objeto#0(7:bombom) -> nulo"),
        "IR inesperada:\n{rendered}"
    );
}

#[test]
fn fase244_lowering_calcula_snapshot_de_ninho_com_layout_real() {
    let code = r#"
pacote main;

ninho Ponto {
    x: u32;
    y: u64;
}

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para Ponto {
    carinho medir(valor: Ponto) -> bombom {
        mimo valor.y;
    }
}

carinho empacotar(valor: Ponto) -> trato<Medivel> {
    mimo valor virar trato<Medivel>;
}

carinho principal() -> bombom {
    mimo 0;
}
"#;

    let rendered = render_ir(code).unwrap();

    assert!(
        rendered.contains(
            "make_trait_object trato<Medivel> from %valor#0 as Ponto:struct size=16 \
vtable=[__impl_7_Medivel_5_Ponto_medir]"
        ),
        "IR inesperada:\n{rendered}"
    );
}

#[test]
fn fase244_lowering_preserva_identidade_de_trato_por_aliases_em_todos_os_fluxos() {
    let code = r#"
pacote main;

trato Medivel {
    carinho medir(valor: si) -> bombom;
}

impl Medivel para bombom {
    carinho medir(valor: bombom) -> bombom { mimo valor; }
}

apelido ObjetoBase = trato<Medivel>;
apelido ObjetoPublico = ObjetoBase;
apelido Numero = bombom;

carinho usar_base(objeto: ObjetoBase) -> bombom {
    mimo objeto.medir();
}

carinho usar_publico(objeto: ObjetoPublico) -> bombom {
    mimo objeto.medir();
}

carinho criar_base(valor: bombom) -> ObjetoBase {
    mimo valor virar trato<Medivel>;
}

carinho criar_publico(valor: bombom) -> ObjetoPublico {
    mimo valor virar trato<Medivel>;
}

trato Fabrica {
    carinho criar(valor: si) -> ObjetoPublico;
}

impl Fabrica para bombom {
    carinho criar(valor: bombom) -> ObjetoPublico {
        mimo valor virar trato<Medivel>;
    }
}

carinho principal() -> bombom {
    nova direto: trato<Medivel> = 7 virar trato<Medivel>;
    nova base: ObjetoBase = 11 virar trato<Medivel>;
    nova publico: ObjetoPublico = 13 virar trato<Medivel>;
    nova copia = publico;
    nova numero: Numero = 5;
    nova fabrica: trato<Fabrica> = 41 virar trato<Fabrica>;
    falar(direto.medir());
    falar(usar_base(base));
    falar(usar_publico(copia));
    falar(copia.medir());
    falar(criar_base(17).medir());
    falar(criar_publico(19).medir());
    falar(fabrica.criar().medir());
    falar(numero);
    mimo 0;
}
"#;

    let rendered = render_ir(code).unwrap();
    assert_eq!(
        rendered.matches("trait_call trato<Medivel>.medir").count(),
        7,
        "parâmetros, retornos, local, cópia e encadeamento devem preservar Medivel:\n{rendered}"
    );
    assert!(
        rendered.contains("let %numero#0 = 5:bombom"),
        "alias não-trato deve permanecer um bombom comum:\n{rendered}"
    );
}

#[test]
fn fase244_lowering_rejeita_ciclo_e_alias_inexistente_sem_fallback_silencioso() {
    let ciclo = parse(
        r#"
pacote main;
apelido A = B;
apelido B = A;
carinho usar(valor: A) -> bombom { mimo 0; }
carinho principal() -> bombom { mimo 0; }
"#,
    )
    .unwrap();
    let err = ir::lower_program(&ciclo)
        .expect_err("ciclo deve falhar antes de qualquer fallback nominal")
        .to_string();
    assert!(
        err.contains("alias de tipo recursivo"),
        "erro inesperado: {err}"
    );

    let ausente = parse(
        r#"
pacote main;
carinho usar(valor: Ausente) -> bombom { mimo 0; }
carinho principal() -> bombom { mimo 0; }
"#,
    )
    .unwrap();
    let err = ir::lower_program(&ausente)
        .expect_err("alias inexistente deve falhar antes de qualquer fallback nominal")
        .to_string();
    assert!(
        err.contains("tipo 'Ausente' não existe"),
        "erro inesperado: {err}"
    );
}

// @pinker-nav:end evidencia.ir.lowering-objetos-trato-fase244
