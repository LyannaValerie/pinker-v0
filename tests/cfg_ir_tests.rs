mod common;

use common::{render_cfg_ir, render_cli_cfg_ir_output};

// @pinker-nav:start evidencia.cfg.lowering-e-renderizacao-basica
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita lowering IR para CFG e compara exatamente blocos, branches, saltos, retornos e chamadas renderizados.
#[test]
fn cfg_ir_funcao_simples() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let cfg = render_cfg_ir(code).unwrap();
    assert_eq!(
        cfg,
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
mode hospedado
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
mode hospedado
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
mode hospedado
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
// @pinker-nav:end evidencia.cfg.lowering-e-renderizacao-basica

// @pinker-nav:start evidencia.cfg.renderizacao-cli
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Compara exatamente o cabeçalho e a CFG textual expostos pelo renderer de CLI.
#[test]
fn cfg_ir_cli_header_estavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    let cli = render_cli_cfg_ir_output(code).unwrap();
    assert_eq!(
        cli,
        "\
=== CFG IR ===
module main
mode hospedado
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
// @pinker-nav:end evidencia.cfg.renderizacao-cli

// @pinker-nav:start evidencia.cfg.lowering-lacos
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita a formação de blocos e labels de laço, quebrar e continuar e inspeciona fragmentos textuais.
#[test]
fn cfg_ir_sempre_que() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova muda x = 0;
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
            nova muda x = 0;
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
            nova muda x = 0;
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
// @pinker-nav:end evidencia.cfg.lowering-lacos

// @pinker-nav:start evidencia.cfg.lowering-operadores-e-join
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Inspeciona operações bitwise e módulo e a formação de join quando ambos os ramos continuam.
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
fn cfg_ir_modulo_basico() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo 10 % 4;
        }";
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("mod 10:bombom, 4:bombom"), "{}", cfg);
}

#[test]
fn cfg_ir_if_else_fallthrough_ambos_ramos_gera_join_valido() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova muda x = 0;
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
// @pinker-nav:end evidencia.cfg.lowering-operadores-e-join

// @pinker-nav:start evidencia.cfg.lowering-ponteiros-e-agregados
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita casts, dereferência, indexação e campos no subset observado e espera erros nos casos fora dele.
#[test]
fn cfg_ir_cast_explicito_bombom_para_seta_bombom_e_volta() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova base: bombom = 1;
            nova p: seta<bombom> = base virar seta<bombom>;
            mimo p virar bombom;
        }
    "#;
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("cast"), "{}", cfg);
}

#[test]
fn cfg_ir_cast_ponteiro_ninho_para_bombom_fora_do_subset_falha() {
    let code = r#"
        pacote main;
        ninho Par { a: bombom; }
        carinho invalido(p: seta<Par>) -> bombom {
            mimo p virar bombom;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = render_cfg_ir(code).unwrap_err().to_string();
    assert!(
        err.contains("cast explícito inválido nesta fase"),
        "{}",
        err
    );
}

#[test]
fn cfg_ir_fragil_emite_deref_com_marcacao_operacional() {
    let code = r#"
        pacote main;
        eterno BASE: bombom = 10;
        carinho principal() -> bombom {
            nova p: fragil seta<bombom> = 1 virar fragil seta<bombom>;
            *p = 77;
            mimo *p;
        }
    "#;
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("deref_store_fragil"), "{}", cfg);
    assert!(cfg.contains("deref_fragil"), "{}", cfg);
}

#[test]
fn cfg_ir_indexacao_operacional_via_seta_array_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova base: seta<[bombom; 3]> = 1;
            mimo (*base)[1];
        }
    "#;
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("add"), "{}", cfg);
    assert!(cfg.contains(", 1:bombom"), "{}", cfg);
    assert!(cfg.contains("deref %t"), "{}", cfg);
}

#[test]
fn cfg_ir_indexacao_operacional_array_por_valor_minima() {
    let code = r#"
        pacote main;
        carinho pega(a: [bombom; 3]) -> bombom {
            mimo a[1];
        }
        carinho principal() -> bombom {
            nova base: seta<[bombom; 3]> = 1;
            mimo pega(*base);
        }
    "#;
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("call pega"), "{}", cfg);
    assert!(cfg.contains("deref %t"), "{}", cfg);
}

#[test]
fn cfg_ir_acesso_campo_operacional_via_ponteiro_para_ninho() {
    let code = r#"
        pacote main;
        ninho Par { a: bombom; b: bombom; }
        carinho principal() -> bombom {
            nova p: seta<Par> = 1;
            mimo (*p).b;
        }
    "#;
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("add"), "{}", cfg);
    assert!(cfg.contains(", 8:bombom"), "{}", cfg);
    assert!(cfg.contains("deref %t0:bombom"), "{}", cfg);
}

#[test]
fn cfg_ir_acesso_campo_em_valor_struct_ainda_fora_do_subset_operacional() {
    let code = r#"
        pacote main;
        ninho Par { a: bombom; b: bombom; }
        carinho pega(p: Par) -> bombom {
            mimo p.b;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = render_cfg_ir(code).unwrap_err().to_string();
    assert!(err.contains("(*ptr).campo"), "{}", err);
}
// @pinker-nav:end evidencia.cfg.lowering-ponteiros-e-agregados

// @pinker-nav:start evidencia.cfg.lowering-limite-asm
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Espera a rejeição observada de asm inline no lowering para CFG.
#[test]
fn cfg_ir_inline_asm_ainda_fora_do_escopo_operacional() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("mov rax, 60");
            mimo 0;
        }
    "#;
    let err = render_cfg_ir(code).unwrap_err().to_string();
    assert!(err.contains("ainda não lowera inline asm"));
}
// @pinker-nav:end evidencia.cfg.lowering-limite-asm

// @pinker-nav:start evidencia.cfg.lowering-verso
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita verso em constante, local, parâmetro, retorno, chamada e falar e inspeciona a CFG renderizada.
#[test]
fn cfg_ir_verso_constante_global_operacional() {
    let code = r#"
        pacote main;
        eterno MSG: verso = "oi";
        carinho principal() -> bombom {
            mimo 0;
        }
    "#;
    let result = render_cfg_ir(code).unwrap();
    assert!(result.contains("const @MSG: verso"));
}

#[test]
fn cfg_ir_verso_operacional_minimo_em_local_parametro_retorno() {
    let code = r#"
        pacote main;
        carinho eco(msg: verso) -> verso { mimo msg; }
        carinho principal() -> bombom {
            nova texto: verso = "oi";
            nova copia: verso = eco(texto);
            falar(copia);
            mimo 0;
        }
    "#;
    let cfg = render_cfg_ir(code).expect("cfg-ir deve aceitar verso no recorte mínimo");
    assert!(cfg.contains("let %texto#0 = \"oi\":verso"), "{}", cfg);
    assert!(cfg.contains("call eco(%texto#0) -> verso"), "{}", cfg);
    assert!(cfg.contains("falar %copia#0:verso"), "{}", cfg);
}
// @pinker-nav:end evidencia.cfg.lowering-verso

// @pinker-nav:start evidencia.cfg.lowering-curto-circuito
// @pinker-nav:domain cfg
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita curto-circuito lógico como controle e valor e inspeciona seus blocos de branch e join.
#[test]
fn cfg_ir_logicos_viram_branch_de_curto_circuito() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = verdade;
            nova b = falso;
            talvez a && b || !a { mimo 1; } senao { mimo 0; }
        }";
    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("logic_rhs_"), "{}", cfg);
    assert!(cfg.contains("logic_short_"), "{}", cfg);
    assert!(cfg.contains("logic_join_"), "{}", cfg);
}

#[test]
fn cfg_ir_logico_com_chamada_pode_ser_usado_como_valor() {
    let code = r#"
        pacote main;

        carinho aceita(ok: logica) -> logica {
            mimo ok;
        }

        carinho principal() -> bombom {
            nova fase: bombom = 239;
            nova ok: logica = aceita(fase > 0 && contem_verso("Fase 239", "239"));
            talvez ok { mimo 0; } senao { mimo 1; }
        }
    "#;

    let cfg = render_cfg_ir(code).unwrap();
    assert!(cfg.contains("logic_join_"), "{}", cfg);
    assert!(cfg.contains("call aceita("), "{}", cfg);
}
// @pinker-nav:end evidencia.cfg.lowering-curto-circuito
