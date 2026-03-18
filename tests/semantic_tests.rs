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
