mod common;

use common::parse_and_check;

#[test]
fn principal_valida() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn principal_invalida_sem_bombom() {
    let code = "pacote main; carinho principal() -> logica { mimo falso; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("principal"));
    assert!(err.contains("bombom"));
}

#[test]
fn principal_invalida_com_parametros() {
    let code = "pacote main; carinho principal(x: bombom) -> bombom { mimo x; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert_eq!(
        err,
        "Erro Semântico: a função 'principal' não deve ter parâmetros em 1:14..1:64"
    );
}

#[test]
fn retorno_exaustivo_com_if_else() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez verdade {
                mimo 1;
            } senao {
                mimo 2;
            }
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn retorno_nao_exaustivo_sem_else() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez verdade {
                mimo 1;
            }
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não retorna em todos os caminhos simples"));
}

#[test]
fn retorno_ausente_apos_if_incompleto() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez verdade {
                mimo 1;
            }
            nova x = 2;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não retorna em todos os caminhos simples"));
}

#[test]
fn retorno_correto_em_bloco_simples() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova x = 10;
            mimo x;
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn principal_com_retorno_errado() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo falso;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("retorno incompatível"));
}

#[test]
fn mutacao_valida() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 10;
            x = 20;
            mimo x;
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn mutacao_invalida() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova x = 10;
            x = 20;
            mimo x;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não é mutável"));
}

#[test]
fn chamada_valida() {
    let code = "
        pacote main;
        carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
        carinho principal() -> bombom { mimo soma(1, 2); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn chamada_invalida_por_aridade() {
    let code = "
        pacote main;
        carinho soma(x: bombom, y: bombom) -> bombom { mimo x + y; }
        carinho principal() -> bombom { mimo soma(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("aridade inválida"));
}

#[test]
fn chamada_invalida_por_tipo() {
    let code = "
        pacote main;
        carinho eco(x: bombom) -> bombom { mimo x; }
        carinho principal() -> bombom { mimo eco(verdade); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1"));
}

#[test]
fn chamada_de_funcao_inexistente() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo desconhecida(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("função 'desconhecida' não declarada"));
}

#[test]
fn uso_de_funcao_sem_retorno_em_expressao() {
    let code = "
        pacote main;
        carinho log() { mimo; }
        carinho principal() -> bombom {
            nova x = log();
            mimo 0;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("sem retorno"));
}

#[test]
fn mimo_vazio_valido_em_funcao_sem_retorno() {
    let code = "
        pacote main;
        carinho helper() { mimo; }
        carinho principal() -> bombom {
            helper();
            mimo 0;
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn mimo_com_valor_invalido_em_funcao_sem_retorno() {
    let code = "
        pacote main;
        carinho helper() { mimo 1; }
        carinho principal() -> bombom { mimo 0; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("mimo com valor"));
}

#[test]
fn mimo_vazio_invalido_em_funcao_com_retorno() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("mimo sem valor"));
}

#[test]
fn chamada_sem_retorno_valida_como_statement() {
    let code = "
        pacote main;
        carinho log() { mimo; }
        carinho principal() -> bombom {
            log();
            mimo 0;
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn erro_semantico_tem_formato_previsivel() {
    let err = parse_and_check("pacote main; carinho principal() -> bombom { x = 1; mimo 0; }")
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "Erro Semântico: variável 'x' não declarada para atribuição em 1:46..1:52"
    );
}

#[test]
fn sempre_que_valido_com_condicao_logica() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova mut x = 0;
            sempre que x < 3 {
                x = x + 1;
            }
            mimo x;
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn sempre_que_invalido_com_condicao_nao_logica() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            sempre que 1 {
                mimo 1;
            }
            mimo 0;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("condição de 'sempre que' deve ser 'logica'"));
}

#[test]
fn quebrar_fora_de_loop_e_invalido() {
    let code = "pacote main; carinho principal() -> bombom { quebrar; mimo 0; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("'quebrar' só pode ser usado dentro de 'sempre que'"));
}

#[test]
fn continuar_fora_de_loop_e_invalido() {
    let code = "pacote main; carinho principal() -> bombom { continuar; mimo 0; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("'continuar' só pode ser usado dentro de 'sempre que'"));
}

#[test]
fn bitwise_valido_em_bombom() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = 6;
            nova b = 3;
            mimo (a & b) | (a ^ b) + (a << 1) + (a >> 1);
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn bitwise_invalido_em_logica() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = verdade;
            nova b = falso;
            mimo a & b;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("operação aritmética/bitwise requer operandos inteiros"));
}

#[test]
fn logico_valido_em_logica() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova a = verdade;
            nova b = falso;
            talvez a && b || !a {
                mimo 1;
            } senao {
                mimo 0;
            }
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn logico_invalido_em_bombom() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo 1 && 0;
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("operação lógica requer operandos 'logica'"));
}

#[test]
fn unsigned_fixos_validos_com_tipos_explicitos() {
    let code = r#"
        pacote main;
        eterno BASE: u32 = 40;
        carinho soma_u8(a: u8, b: u8) -> u8 { mimo a + b; }
        carinho soma_u16(a: u16, b: u16) -> u16 { mimo a + b; }
        carinho soma_u32(a: u32, b: u32) -> u32 { mimo a + b; }
        carinho soma_u64(a: u64, b: u64) -> u64 { mimo a + b; }
        carinho principal() -> bombom {
            soma_u8(1, 2);
            soma_u16(3, 4);
            soma_u32(BASE, 1);
            mimo soma_u64(40, 2);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn unsigned_fixos_rejeitam_mistura_implicita() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova a: u8 = 1;
            nova b: u16 = 2;
            nova c = a + b;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipos incompatíveis em operação binária"),
        "{}",
        err
    );
}

#[test]
fn signed_fixos_validos_com_tipos_explicitos() {
    let code = r#"
        pacote main;
        eterno BASE: i32 = 40;
        carinho soma_i8(a: i8, b: i8) -> i8 { mimo a + b; }
        carinho soma_i16(a: i16, b: i16) -> i16 { mimo a + b; }
        carinho soma_i32(a: i32, b: i32) -> i32 { mimo a + b; }
        carinho sub_i64(a: i64, b: i64) -> i64 { mimo a - b; }
        carinho principal() -> bombom {
            soma_i8(1, 2);
            soma_i16(3, 4);
            soma_i32(BASE, 1);
            nova n: i64 = 40;
            nova m: i64 = 2;
            nova r: i64 = sub_i64(-n, -m);
            sub_i64(r, m);
            mimo 42;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn signed_unsigned_rejeitam_mistura_implicita() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova a: i32 = 1;
            nova b: u32 = 2;
            nova c = (-a) + b;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipos incompatíveis em operação binária"),
        "{}",
        err
    );
}

#[test]
fn alias_de_tipo_valido_em_parametro_retorno_e_local() {
    let code = r#"
        pacote main;
        apelido Byte = u8;
        carinho id(x: Byte) -> Byte { mimo x; }
        carinho principal() -> bombom {
            nova y: Byte = id(7);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn alias_de_tipo_inexistente_falha() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: Fantasma = 1;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo 'Fantasma' não existe"), "{}", err);
}

#[test]
fn arrays_fixos_validos_em_alias_e_parametro() {
    let code = r#"
        pacote main;
        apelido Bytes16 = [u8; 16];
        carinho usa(_buf: Bytes16) -> bombom { mimo 0; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn array_fixo_com_tamanho_zero_e_invalido() {
    let code = r#"
        pacote main;
        apelido Vazio = [u8; 0];
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tamanho maior que zero"));
}

#[test]
fn modulo_valido_em_bombom() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            mimo 10 % 3;
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn modulo_invalido_em_logica() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez (verdade % falso) == 0 {
                mimo 1;
            } senao {
                mimo 0;
            }
        }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("operação aritmética/bitwise requer operandos inteiros"));
}
