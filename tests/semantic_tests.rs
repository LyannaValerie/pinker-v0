mod common;

use common::parse_and_check;

// @pinker-nav:start evidencia.semantica.entrada-principal
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita o contrato de `principal` e do modo `livre`: aceita `principal` válida e rejeita, nos casos presentes, `principal` sem bombom, com parâmetros e o modo livre sem entrada explícita.
#[test]
fn principal_valida() {
    let code = "pacote main; carinho principal() -> bombom { mimo 0; }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn livre_sem_principal_falha_com_boot_entry_explicito() {
    let code = "pacote main; livre; carinho boot() -> bombom { mimo 0; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("boot entry desta fase em modo `livre`"));
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
// @pinker-nav:end evidencia.semantica.entrada-principal

// @pinker-nav:start evidencia.semantica.retornos
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica nos casos presentes a exaustividade de retorno em if/else, a ausência de else, o retorno em bloco simples e o retorno incorreto de `principal`.
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
// @pinker-nav:end evidencia.semantica.retornos

// @pinker-nav:start evidencia.semantica.mutabilidade
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita aceitação e rejeição de mutação/atribuição nos dois casos presentes.
#[test]
fn mutacao_valida() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            nova muda x = 10;
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
// @pinker-nav:end evidencia.semantica.mutabilidade

// @pinker-nav:start evidencia.semantica.chamadas
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Verifica chamada válida e rejeita, nos casos presentes, aridade incorreta, tipo incorreto e função inexistente.
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
// @pinker-nav:end evidencia.semantica.chamadas

// @pinker-nav:start evidencia.semantica.intrinsecas-entrada-ambiente
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Catálogo contíguo de intrínsecas de entrada, argumentos e ambiente (ouvir, argumento, tem_chave, pedir_argumento, ambiente, buscar_contexto): aceita a assinatura sem declaração e rejeita, nos casos presentes, aridade e tipos inválidos. Verifica aceitação/rejeição semântica de assinatura, não comportamento operacional.
#[test]
fn ouvir_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main; trazer entrada.ouvir;
        carinho principal() -> bombom { mimo ouvir(); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ouvir_intrinseca_rejeita_aridade_diferente_de_zero() {
    let code = "
        pacote main; trazer entrada.ouvir;
        carinho principal() -> bombom { mimo ouvir(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ouvir' com aridade inválida"));
}

#[test]
fn ouvir_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer entrada.ouvir_verso; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ouvir_verso()); }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ouvir_verso_ou_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer entrada.ouvir_verso_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ouvir_verso_ou("padrao")); }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ouvir_verso_intrinseca_rejeita_aridade_diferente_de_zero() {
    let code = r#"
        pacote main; trazer entrada.ouvir_verso; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ouvir_verso("x")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ouvir_verso' com aridade inválida"));
}

#[test]
fn ouvir_verso_ou_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer entrada.ouvir_verso_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ouvir_verso_ou(7)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ouvir_verso_ou'"));
}

#[test]
fn ouvir_verso_ou_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer entrada.ouvir_verso_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ouvir_verso_ou("a", "b")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ouvir_verso_ou' com aridade inválida"));
}

#[test]
fn argumento_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main; trazer ambiente.argumento; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(argumento(0)); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn argumento_intrinseca_rejeita_indice_nao_bombom() {
    let code = "
        pacote main; trazer ambiente.argumento; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(argumento(falso)); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'argumento'"));
}

#[test]
fn argumento_ou_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer ambiente.argumento_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(argumento_ou(0, "anonimo")); }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn argumento_ou_intrinseca_rejeita_padrao_nao_verso() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(argumento_ou(0, 1)); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'argumento_ou'"));
}

#[test]
fn tem_chave_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave("--saida") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tem_chave_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave(1) { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tem_chave'"));
}

#[test]
fn pedir_argumento_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer ambiente.pedir_argumento; trazer texto.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(pedir_argumento("--saida", "padrao"));
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn pedir_argumento_intrinseca_rejeita_padrao_nao_verso() {
    let code = r#"
        pacote main; trazer ambiente.pedir_argumento; trazer texto.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(pedir_argumento("--saida", 1));
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'pedir_argumento'"));
}

#[test]
fn ambiente_ou_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer ambiente.variavel_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(variavel_ou("HOME", "anonimo")); }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn ambiente_ou_intrinseca_rejeita_chave_nao_verso() {
    let source = r#"
        pacote main; trazer ambiente.variavel_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(variavel_ou(0, "anonimo")); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ambiente_ou'"));
}

#[test]
fn buscar_contexto_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer ambiente.buscar_contexto; trazer texto.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(buscar_contexto("--saida", "PINKER_OUT", "padrao"));
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn buscar_contexto_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer ambiente.buscar_contexto; trazer texto.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(buscar_contexto("--saida", 1, "padrao"));
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'buscar_contexto'"));
}

#[test]
fn buscar_contexto_intrinseca_rejeita_terceiro_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer ambiente.buscar_contexto; trazer texto.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(buscar_contexto("--saida", "PINKER_OUT", 1));
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 3 da chamada 'buscar_contexto'"));
}
// @pinker-nav:end evidencia.semantica.intrinsecas-entrada-ambiente

// @pinker-nav:start evidencia.semantica.intrinsecas-caminhos-e-sistema
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Catálogo contíguo de intrínsecas de caminho e sistema de arquivos (caminho_existe, e_arquivo, e_diretorio, juntar_caminho, tamanho_arquivo, e_vazio, criar/remover diretório e arquivo, diretorio_atual): aceita assinatura sem declaração e rejeita casos de tipo/aridade presentes. Declaração de intrínseca não implica execução real de arquivos.
#[test]
fn caminho_existe_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.existe;
        carinho principal() -> bombom {
            talvez existe("README.md") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn caminho_existe_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.existe;
        carinho principal() -> bombom { talvez existe(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'caminho_existe'"));
}

#[test]
fn e_arquivo_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.e_arquivo;
        carinho principal() -> bombom {
            talvez e_arquivo("README.md") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn e_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.e_arquivo;
        carinho principal() -> bombom { talvez e_arquivo(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'e_arquivo'"));
}

#[test]
fn e_diretorio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.e_diretorio;
        carinho principal() -> bombom {
            talvez e_diretorio(".") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn e_diretorio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.e_diretorio;
        carinho principal() -> bombom { talvez e_diretorio(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'e_diretorio'"));
}

#[test]
fn juntar_caminho_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.juntar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova p: verso = juntar(".", "README.md");
            mimo tamanho(p);
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn juntar_caminho_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.juntar; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(juntar(".", 1)); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'juntar_caminho'"));
}

#[test]
fn tamanho_arquivo_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.tamanho_arquivo;
        carinho principal() -> bombom { mimo tamanho_arquivo("README.md"); }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn tamanho_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.tamanho_arquivo;
        carinho principal() -> bombom { mimo tamanho_arquivo(1); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tamanho_arquivo'"));
}

#[test]
fn e_vazio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.arquivo_vazio;
        carinho principal() -> bombom {
            talvez arquivo_vazio("README.md") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn e_vazio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.arquivo_vazio;
        carinho principal() -> bombom { talvez arquivo_vazio(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'e_vazio'"));
}

#[test]
fn criar_diretorio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.criar_diretorio;
        carinho principal() -> bombom {
            criar_diretorio("saida");
            mimo 0;
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn criar_diretorio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.criar_diretorio;
        carinho principal() -> bombom {
            criar_diretorio(1);
            mimo 0;
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'criar_diretorio'"));
}

#[test]
fn remover_arquivo_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.remover_arquivo;
        carinho principal() -> bombom {
            remover_arquivo("temp.txt");
            mimo 0;
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn remover_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.remover_arquivo;
        carinho principal() -> bombom {
            remover_arquivo(1);
            mimo 0;
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'remover_arquivo'"));
}

#[test]
fn remover_diretorio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.remover_diretorio;
        carinho principal() -> bombom {
            remover_diretorio("saida");
            mimo 0;
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn remover_diretorio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main; trazer caminho.remover_diretorio;
        carinho principal() -> bombom {
            remover_diretorio(1);
            mimo 0;
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'remover_diretorio'"));
}

#[test]
fn diretorio_atual_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main; trazer caminho.diretorio_atual; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(diretorio_atual()); }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn diretorio_atual_intrinseca_rejeita_aridade_diferente_de_zero() {
    let source = r#"
        pacote main; trazer caminho.diretorio_atual; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(diretorio_atual("x")); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("chamada de 'diretorio_atual' com aridade inválida"));
}
// @pinker-nav:end evidencia.semantica.intrinsecas-caminhos-e-sistema

// @pinker-nav:start evidencia.semantica.intrinsecas-argumentos-e-contexto
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Catálogo contíguo de intrínsecas de argumentos e contexto de invocação (quantos_argumentos, tem_argumento, tem_chave, pedir_argumento, tem_flag, buscar_contexto e legado nomeado): aceita assinatura sem declaração e rejeita aridade/tipo nos casos presentes.
#[test]
fn quantos_argumentos_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main; trazer ambiente.quantos_argumentos;
        carinho principal() -> bombom { mimo quantos_argumentos(); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn quantos_argumentos_intrinseca_rejeita_aridade_diferente_de_zero() {
    let code = "
        pacote main; trazer ambiente.quantos_argumentos;
        carinho principal() -> bombom { mimo quantos_argumentos(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'quantos_argumentos' com aridade inválida"));
}

#[test]
fn tem_argumento_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main; trazer ambiente.tem_argumento;
        carinho principal() -> bombom {
            talvez tem_argumento(0) { mimo 1; } senao { mimo 0; }
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tem_argumento_intrinseca_rejeita_indice_nao_bombom() {
    let code = "
        pacote main; trazer ambiente.tem_argumento;
        carinho principal() -> bombom { talvez tem_argumento(falso) { mimo 1; } senao { mimo 0; } }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tem_argumento'"));
}

#[test]
fn tem_chave_intrinseca_rejeita_aridade_diferente_de_um() {
    let code = r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom { talvez tem_chave("--saida", "--modo") { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'tem_chave' com aridade inválida"));
}

#[test]
fn pedir_argumento_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer ambiente.pedir_argumento; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(pedir_argumento("--saida")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'pedir_argumento' com aridade inválida"));
}

#[test]
fn tem_flag_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom { talvez tem_flag("--quiet") { mimo 1; } senao { mimo 0; } }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tem_flag_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom { talvez tem_flag(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tem_flag'"));
}

#[test]
fn tem_flag_intrinseca_rejeita_aridade_diferente_de_um() {
    let code = r#"
        pacote main; trazer ambiente.tem_flag;
        carinho principal() -> bombom { talvez tem_flag("--quiet", "--verbose") { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'tem_flag' com aridade inválida"));
}

#[test]
fn buscar_contexto_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer ambiente.buscar_contexto; trazer texto.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(buscar_contexto("--saida", "PINKER_OUT"));
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'buscar_contexto' com aridade inválida"));
}

#[test]
fn legado_tem_argumento_nomeado_intrinseca_permanece_valido() {
    let code = r#"
        pacote main; trazer ambiente.tem_chave;
        carinho principal() -> bombom {
            talvez tem_chave("--saida") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}
// @pinker-nav:end evidencia.semantica.intrinsecas-argumentos-e-contexto

// @pinker-nav:start evidencia.semantica.intrinsecas-arquivos-io
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Catálogo contíguo de intrínsecas de E/S de arquivos e de saída de processo (sair, abrir/ler/fechar, ler_verso, escrever, criar_arquivo, abrir_anexo, anexar_verso, escrever_verso, truncar_arquivo): aceita assinatura sem declaração e rejeita aridade/tipo nos casos presentes. Aceitação de assinatura não é comportamento de runtime.
#[test]
fn sair_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main;
        carinho principal() -> bombom { sair(1); mimo 0; }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn sair_intrinseca_rejeita_argumento_nao_bombom() {
    let code = "
        pacote main;
        carinho principal() -> bombom { sair(verdade); mimo 0; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'sair'"));
}

#[test]
fn abrir_ler_fechar_intrinsecas_validas_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_bombom;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            nova v: bombom = ler_bombom(h);
            fechar(h);
            mimo v;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_verso_arquivo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            nova t: verso = ler_verso(h);
            fechar(h);
            mimo tamanho(t);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_verso_arquivo_intrinseca_rejeita_argumento_nao_bombom() {
    let code = r#"
        pacote main; trazer arquivo.ler_verso; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ler_verso("arquivo.txt")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_verso_arquivo'"));
}

#[test]
fn ler_arquivo_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_verso;
        carinho principal() -> bombom {
            nova t: verso = ler_caminho_verso("arquivo.txt");
            falar(t);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_arquivo_verso_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_verso; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ler_caminho_verso("a", "b")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ler_arquivo_verso' com aridade inválida"));
}

#[test]
fn ler_arquivo_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_verso; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ler_caminho_verso(1)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_arquivo_verso'"));
}

#[test]
fn arquivo_ou_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_ou;
        carinho principal() -> bombom {
            nova t: verso = ler_caminho_ou("arquivo.txt", "padrao");
            falar(t);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn arquivo_ou_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ler_caminho_ou("a")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'arquivo_ou' com aridade inválida"));
}

#[test]
fn arquivo_ou_intrinseca_rejeita_tipos_invalidos() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ler_caminho_ou(1, "ok")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'arquivo_ou'"));
}

#[test]
fn arquivo_ou_intrinseca_rejeita_padrao_nao_verso() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_ou; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(ler_caminho_ou("a.txt", 7)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'arquivo_ou'"));
}

#[test]
fn abrir_intrinseca_rejeita_argumento_nao_verso() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo abrir(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'abrir'"));
}

#[test]
fn escrever_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.escrever_bombom; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            escrever_bombom(h, 42);
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn escrever_intrinseca_rejeita_segundo_argumento_nao_bombom() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.escrever_bombom; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            escrever_bombom(h, "texto");
            fechar(h);
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'escrever'"));
}

#[test]
fn criar_arquivo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.criar; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = criar("arquivo.txt");
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn criar_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer arquivo.criar;
        carinho principal() -> bombom { mimo criar(1); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'criar_arquivo'"));
}

#[test]
fn abrir_anexo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir_anexo("arquivo.txt");
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn abrir_anexo_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer arquivo.abrir_anexo;
        carinho principal() -> bombom { mimo abrir_anexo("a.txt", "b.txt"); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'abrir_anexo' com aridade inválida"));
}

#[test]
fn anexar_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.anexar_verso; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir_anexo("arquivo.txt");
            anexar_verso(h, "texto");
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn anexar_verso_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.anexar_verso; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir_anexo("arquivo.txt");
            anexar_verso(h);
            fechar(h);
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'anexar_verso' com aridade inválida"));
}

#[test]
fn anexar_verso_intrinseca_rejeita_handle_nao_bombom() {
    let code = r#"
        pacote main; trazer arquivo.anexar_verso;
        carinho principal() -> bombom {
            anexar_verso("arquivo.txt", "texto");
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'anexar_verso'"));
}

#[test]
fn anexar_verso_intrinseca_rejeita_texto_nao_verso() {
    let code = r#"
        pacote main; trazer arquivo.abrir_anexo; trazer arquivo.anexar_verso; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir_anexo("arquivo.txt");
            anexar_verso(h, 7);
            fechar(h);
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'anexar_verso'"));
}

#[test]
fn escrever_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.escrever_verso; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            escrever_verso(h, "texto");
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn escrever_verso_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.escrever_verso; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            escrever_verso(h, 7);
            fechar(h);
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'escrever_verso'"));
}

#[test]
fn truncar_arquivo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer arquivo.abrir; trazer arquivo.fechar; trazer arquivo.truncar;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            truncar(h);
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn truncar_arquivo_intrinseca_rejeita_argumento_nao_bombom() {
    let code = r#"
        pacote main; trazer arquivo.truncar;
        carinho principal() -> bombom {
            truncar("arquivo.txt");
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'truncar_arquivo'"));
}
// @pinker-nav:end evidencia.semantica.intrinsecas-arquivos-io

// @pinker-nav:start evidencia.semantica.intrinsecas-texto-e-estruturados
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Catálogo contíguo de intrínsecas de texto/verso e de dados estruturados (juntar, índice, contém, começa/termina, igual, vazio, aparar, minúsculo/maiúsculo, buscar, formatar, CSV, JSON, tempo_unix): aceita assinatura sem declaração e rejeita tipos/aridade nos casos presentes; formatar aceita aridade variável. Verifica a assinatura, não a formatação real.
#[test]
fn juntar_e_tamanho_verso_intrinsecas_validas_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.juntar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova a: verso = "oi";
            nova b: verso = "!";
            nova c: verso = juntar(a, b);
            nova n: bombom = tamanho(c);
            mimo n;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn juntar_verso_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.juntar; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(juntar("oi", 1)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'juntar_verso'"));
}

#[test]
fn indice_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.indice; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = "paz";
            nova letra: verso = indice(texto, 1);
            mimo tamanho(letra);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn indice_verso_rejeita_indice_nao_bombom() {
    let code = r#"
        pacote main; trazer texto.indice; trazer texto.tamanho;
        carinho principal() -> bombom { mimo tamanho(indice("oi", falso)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'indice_verso'"));
}

#[test]
fn contem_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.contem;
        carinho principal() -> bombom {
            nova ok: logica = contem("pinker", "ink");
            talvez ok {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn contem_verso_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.contem;
        carinho principal() -> bombom {
            nova ok: logica = contem("pinker", 1);
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'contem_verso'"));
}

#[test]
fn comeca_com_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.comeca_com;
        carinho principal() -> bombom {
            nova ok: logica = comeca_com("pinker", "pin");
            talvez ok {
                mimo 1;
            } senao {
                mimo 0;
            }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn comeca_com_intrinseca_rejeita_primeiro_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.comeca_com;
        carinho principal() -> bombom {
            nova ok: logica = comeca_com(1, "pin");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'comeca_com'"));
}

#[test]
fn termina_com_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.termina_com;
        carinho principal() -> bombom {
            nova ok: logica = termina_com("pinker", "ker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn termina_com_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.termina_com;
        carinho principal() -> bombom {
            nova ok: logica = termina_com("pinker", 1);
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'termina_com'"));
}

#[test]
fn igual_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.igual;
        carinho principal() -> bombom {
            nova ok: logica = igual("pinker", "pinker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn igual_verso_intrinseca_rejeita_primeiro_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.igual;
        carinho principal() -> bombom {
            nova ok: logica = igual(1, "pinker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'igual_verso'"));
}

#[test]
fn vazio_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.vazio;
        carinho principal() -> bombom {
            nova ok: logica = vazio("");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn aparar_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.aparar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova v: verso = aparar(1);
            mimo tamanho(v);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'aparar_verso'"));
}

#[test]
fn minusculo_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.igual; trazer texto.minusculo;
        carinho principal() -> bombom {
            nova v: verso = minusculo("PiNkEr");
            talvez igual(v, "pinker") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn maiusculo_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.maiusculo; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova v: verso = maiusculo(7);
            mimo tamanho(v);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'maiusculo_verso'"));
}

#[test]
fn indice_verso_em_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.indice_em;
        carinho principal() -> bombom {
            nova idx: bombom = indice_em("pinker", "ink");
            talvez idx == 1 { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn buscar_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.buscar;
        carinho principal() -> bombom {
            nova idx: bombom = buscar("pinker", "ink");
            talvez idx == 1 { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn buscar_verso_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.buscar;
        carinho principal() -> bombom {
            nova idx: bombom = buscar("pinker", 1);
            mimo idx;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'buscar_verso'"));
}

#[test]
fn formatar_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer texto.formatar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova linha: verso = formatar("{}={}", "idade", 7);
            mimo tamanho(linha);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn formatar_verso_intrinseca_rejeita_modelo_nao_verso() {
    let code = r#"
        pacote main; trazer texto.formatar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova linha: verso = formatar(7, "idade");
            mimo tamanho(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'formatar_verso'"));
}

#[test]
fn formatar_verso_intrinseca_rejeita_argumento_nao_bombom_ou_verso() {
    let code = r#"
        pacote main; trazer texto.formatar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova linha: verso = formatar("{}", falso);
            mimo tamanho(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'formatar_verso'"));
}

#[test]
fn formatar_verso_intrinseca_aceita_aridade_variavel() {
    let code = r#"
        pacote main; trazer texto.formatar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova linha: verso = formatar("{} {} {}", 1, 2, 3);
            mimo tamanho(linha);
        }"#;
    parse_and_check(code).expect("formatar_verso deve aceitar aridade variável");
}

#[test]
fn ler_linha_csv_bombom_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer csv.ler_linha_bombom; trazer lista.bombom_obter;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = ler_linha_bombom("7,11,13", ",");
            mimo bombom_obter(itens, 1);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_linha_csv_bombom_intrinseca_rejeita_linha_nao_verso() {
    let code = r#"
        pacote main; trazer csv.ler_linha_bombom; trazer lista.bombom_tamanho;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = ler_linha_bombom(7, ",");
            mimo bombom_tamanho(itens);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_linha_csv_bombom'"));
}

#[test]
fn emitir_linha_csv_bombom_intrinseca_rejeita_lista_fora_do_recorte() {
    let code = r#"
        pacote main; trazer csv.emitir_linha_bombom; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova linha: verso = emitir_linha_bombom("7,11,13", ",");
            mimo tamanho(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'emitir_linha_csv_bombom'"));
}

#[test]
fn emitir_linha_csv_bombom_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer csv.emitir_linha_bombom; trazer lista.bombom_criar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = bombom_criar();
            nova linha: verso = emitir_linha_bombom(itens, ",", ";");
            mimo tamanho(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'emitir_linha_csv_bombom' com aridade inválida"));
}

#[test]
fn ler_json_plano_bombom_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer json.emitir_plano_bombom; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter;
        carinho principal() -> bombom {
            nova base: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(base, "idade", 7);
            nova json: verso = emitir_plano_bombom(base);
            nova dados: mapa<verso,bombom> = ler_plano_bombom(json);
            mimo verso_bombom_obter(dados, "idade");
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_json_plano_bombom_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer json.ler_plano_bombom; trazer mapa.verso_bombom_tamanho;
        carinho principal() -> bombom {
            nova dados: mapa<verso,bombom> = ler_plano_bombom(7);
            mimo verso_bombom_tamanho(dados);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_json_plano_bombom'"));
}

#[test]
fn emitir_json_plano_bombom_intrinseca_rejeita_argumento_fora_do_recorte() {
    let code = r#"
        pacote main; trazer json.emitir_plano_bombom; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova linha: verso = emitir_plano_bombom("nao_e_mapa");
            mimo tamanho(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'emitir_json_plano_bombom'"));
}

#[test]
fn emitir_json_plano_bombom_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer json.emitir_plano_bombom; trazer mapa.verso_bombom_criar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = verso_bombom_criar();
            nova linha: verso = emitir_plano_bombom(m, "extra");
            mimo tamanho(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'emitir_json_plano_bombom' com aridade inválida"));
}

#[test]
fn tempo_unix_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer tempo.unix;
        carinho principal() -> bombom {
            nova ts: bombom = unix();
            mimo ts;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tempo_unix_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer tempo.unix;
        carinho principal() -> bombom {
            nova ts: bombom = unix(1);
            mimo ts;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'tempo_unix' com aridade inválida"));
}

#[test]
fn formatar_tempo_unix_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer tempo.formatar_unix; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = formatar_unix(0);
            mimo tamanho(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn formatar_tempo_unix_intrinseca_rejeita_argumento_nao_bombom() {
    let code = r#"
        pacote main; trazer tempo.formatar_unix; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = formatar_unix("agora");
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'formatar_tempo_unix'"));
}
// @pinker-nav:end evidencia.semantica.intrinsecas-texto-e-estruturados

// @pinker-nav:start evidencia.semantica.intrinsecas-processos
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Catálogo contíguo de intrínsecas de processos (executar_processo, executar_com_entrada, pipeline_minimo, capturar_stdout/stderr) mais nao_vazio_verso: aceita assinatura sem declaração, aceita argv explícito mínimo e rejeita aridade/tipo nos casos presentes. Não executa processos reais.
#[test]
fn executar_processo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("pinker_fase162_exit0");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_processo_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("pinker_fase168_argv_um", "--modo=ok");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_processo_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("a", "b", "c");
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'executar_processo' com aridade inválida"));
}

#[test]
fn executar_processo_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar(7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'executar_processo'"));
}

#[test]
fn executar_processo_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.executar;
        carinho principal() -> bombom {
            nova codigo: bombom = executar("pinker_fase168_argv_um", 7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'executar_processo'"));
}

#[test]
fn executar_com_entrada_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("pinker_fase165_stdin_ok", "rosa\n");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_com_entrada_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("pinker_fase165_stdin_ok", "argv=ok\n", "--modo=ok");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_com_entrada_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("a", "b", "c", "d");
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'executar_com_entrada' com aridade inválida"));
}

#[test]
fn executar_com_entrada_intrinseca_rejeita_comando_nao_verso() {
    let code = r#"
        pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada(7, "rosa\n");
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'executar_com_entrada'"));
}

#[test]
fn executar_com_entrada_intrinseca_rejeita_entrada_nao_verso() {
    let code = r#"
        pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("pinker_fase165_stdin_ok", 7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'executar_com_entrada'"));
}

#[test]
fn executar_com_entrada_intrinseca_rejeita_terceiro_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.executar_com_entrada;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("pinker_fase165_stdin_ok", "argv=ok\n", 7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 3 da chamada 'executar_com_entrada'"));
}

#[test]
fn pipeline_minimo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("pinker_fase163_stdout_ok", "pinker_fase165_stdin_ok");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn pipeline_minimo_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("a");
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'pipeline_minimo' com aridade inválida"));
}

#[test]
fn pipeline_minimo_intrinseca_rejeita_produtor_nao_verso() {
    let code = r#"
        pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo(7, "pinker_fase165_stdin_ok");
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'pipeline_minimo'"));
}

#[test]
fn pipeline_minimo_intrinseca_rejeita_consumidor_nao_verso() {
    let code = r#"
        pacote main; trazer processo.pipeline_minimo;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("pinker_fase163_stdout_ok", 7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'pipeline_minimo'"));
}

#[test]
fn capturar_stdout_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("pinker_fase163_stdout_ok");
            mimo tamanho(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stdout_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("pinker_fase163_stdout_ok", "--alvo=rosa");
            mimo tamanho(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stdout_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("a", "b", "c");
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'capturar_stdout' com aridade inválida"));
}

#[test]
fn capturar_stdout_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout(7);
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'capturar_stdout'"));
}

#[test]
fn capturar_stdout_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.capturar_stdout; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("pinker_fase163_stdout_ok", 7);
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'capturar_stdout'"));
}

#[test]
fn capturar_stderr_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("pinker_fase164_stderr_ok");
            mimo tamanho(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stderr_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("pinker_fase164_stderr_ok", "--alvo=rosa");
            mimo tamanho(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stderr_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("a", "b", "c");
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'capturar_stderr' com aridade inválida"));
}

#[test]
fn capturar_stderr_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr(7);
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'capturar_stderr'"));
}

#[test]
fn capturar_stderr_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer processo.capturar_stderr; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("pinker_fase164_stderr_ok", 7);
            mimo tamanho(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'capturar_stderr'"));
}

#[test]
fn nao_vazio_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main; trazer texto.nao_vazio;
        carinho principal() -> bombom {
            nova ok: logica = nao_vazio(7);
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'nao_vazio_verso'"));
}
// @pinker-nav:end evidencia.semantica.intrinsecas-processos

// @pinker-nav:start evidencia.semantica.funcoes-sem-retorno
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita funções sem retorno e o tipo unitário: rejeita uso de função sem retorno em expressão, aceita/rejeita `verso` e `mimo` conforme o contexto e aceita chamada sem retorno como statement, nos casos presentes.
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
fn verso_valido_em_parametro_retorno_e_local() {
    let code = r#"
        pacote main;
        carinho eco(msg: verso) -> verso { mimo msg; }
        carinho principal() -> bombom {
            nova texto: verso = "olá";
            nova copia: verso = eco(texto);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn verso_rejeita_atribuicao_de_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = 10;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo de inicialização incompatível para 'texto'"));
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
// @pinker-nav:end evidencia.semantica.funcoes-sem-retorno

// @pinker-nav:start evidencia.semantica.controle-fluxo-e-diagnostico
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita o formato previsível do diagnóstico semântico, controle de fluxo (`sempre_que`, `quebrar`, `continuar` fora de laço) e `sussurro`, aceitando e rejeitando os casos presentes.
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
            nova muda x = 0;
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
fn sussurro_valido_com_strings_literais() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("mov rax, 60", "syscall");
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn sussurro_invalido_com_string_vazia() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("");
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não pode conter string vazia"));
}
// @pinker-nav:end evidencia.semantica.controle-fluxo-e-diagnostico

// @pinker-nav:start evidencia.semantica.operadores-logicos-e-bitwise
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita operadores bitwise e lógicos: aceita uso sobre o tipo correto e rejeita a mistura com o tipo incorreto nos casos presentes.
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
// @pinker-nav:end evidencia.semantica.operadores-logicos-e-bitwise

// @pinker-nav:start evidencia.semantica.acesso-campos-e-indexacao
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita acesso a campo de ninho e indexação de array fixo: aceita casos válidos (incluindo base via deref de seta) e rejeita campo inexistente, base não-struct, índice não-inteiro, índice signed fora do subset e base não-array, nos casos presentes.
#[test]
fn acesso_a_campo_de_ninho_valido() {
    let code = r#"
        pacote main;
        ninho Ponto { x: bombom; y: bombom; }
        carinho pega_x(p: Ponto) -> bombom { mimo p.x; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn acesso_a_campo_inexistente_falha() {
    let code = r#"
        pacote main;
        ninho Ponto { x: bombom; }
        carinho pega_y(p: Ponto) -> bombom { mimo p.y; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("campo 'y' não existe"));
}

#[test]
fn acesso_a_campo_em_base_nao_struct_falha() {
    let code = r#"
        pacote main;
        carinho invalida(v: bombom) -> bombom { mimo v.x; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("acesso de campo exige base do tipo 'ninho'"));
}

#[test]
fn indexacao_de_array_fixo_valida() {
    let code = r#"
        pacote main;
        carinho pega(a: [bombom; 3], i: bombom) -> bombom { mimo a[i]; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn indexacao_com_indice_nao_inteiro_falha() {
    let code = r#"
        pacote main;
        carinho pega(a: [bombom; 3], ok: logica) -> bombom { mimo a[ok]; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("índice nesta fase deve ser 'bombom'"));
}

#[test]
fn indexacao_com_base_deref_seta_array_bombom_valida() {
    let code = r#"
        pacote main;
        carinho pega(a: seta<[bombom; 3]>, i: bombom) -> bombom { mimo (*a)[i]; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn indexacao_com_indice_signed_fora_do_subset_falha() {
    let code = r#"
        pacote main;
        carinho pega(a: [bombom; 3], i: i32) -> bombom { mimo a[i]; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("índice nesta fase deve ser 'bombom'"));
}

#[test]
fn indexacao_em_base_nao_array_falha() {
    let code = r#"
        pacote main;
        carinho pega(v: bombom) -> bombom { mimo v[0]; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("indexação exige base de array fixo nesta fase"));
}
// @pinker-nav:end evidencia.semantica.acesso-campos-e-indexacao

// @pinker-nav:start evidencia.semantica.casts
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita casts entre inteiros e entre bombom e seta bombom, rejeitando cast de lógica para inteiro e de ponteiro não-bombom nesta fase, nos casos presentes.
#[test]
fn cast_inteiro_para_inteiro_valido() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: u16 = 300;
            nova y: u8 = x virar u8;
            mimo y virar bombom;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn cast_logica_para_inteiro_falha_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova b = verdade;
            mimo b virar bombom;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("cast explícito inválido nesta fase"));
}

#[test]
fn cast_bombom_para_seta_bombom_e_seta_bombom_para_bombom_valido() {
    let code = r#"
        pacote main;
        carinho ida(x: bombom) -> seta<bombom> {
            mimo x virar seta<bombom>;
        }
        carinho volta(p: seta<bombom>) -> bombom {
            mimo p virar bombom;
        }
        carinho principal() -> bombom {
            nova p: seta<bombom> = ida(1);
            mimo volta(p);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn cast_ponteiro_nao_bombom_para_inteiro_falha_nesta_fase() {
    let code = r#"
        pacote main;
        ninho Ponto { x: bombom; }
        carinho invalido(p: seta<Ponto>) -> bombom {
            mimo p virar bombom;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("cast explícito inválido nesta fase"));
}
// @pinker-nav:end evidencia.semantica.casts

// @pinker-nav:start evidencia.semantica.peso-e-alinhamento
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita peso e alinhamento de tipos escalares, array fixo, alias e ninho, rejeitando peso de tipo inexistente nos casos presentes.
#[test]
fn peso_e_alinhamento_de_tipos_escalares_sao_validos() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo peso(bombom) + peso(logica) + alinhamento(u32);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn peso_de_array_fixo_e_alias_sao_validos() {
    let code = r#"
        pacote main;
        apelido Bytes = [u8; 16];
        carinho principal() -> bombom {
            mimo peso(Bytes) + alinhamento(Bytes);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn peso_e_alinhamento_de_ninho_sao_validos() {
    let code = r#"
        pacote main;
        ninho Ponto { x: u8; y: u32; }
        carinho principal() -> bombom {
            mimo peso(Ponto) + alinhamento(Ponto);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn peso_de_tipo_inexistente_falha() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo peso(TipoQueNaoExiste);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo 'TipoQueNaoExiste' não existe"));
}
// @pinker-nav:end evidencia.semantica.peso-e-alinhamento

// @pinker-nav:start evidencia.semantica.tipos-numericos-largura-fixa
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita inteiros de largura fixa: cast via alias, aceitação de unsigned/signed com tipos explícitos e rejeição de mistura implícita, nos casos presentes.
#[test]
fn cast_com_alias_inteiro_funciona_via_tipo_subjacente() {
    let code = r#"
        pacote main;
        apelido Byte = u8;
        carinho principal() -> bombom {
            nova x: bombom = 511;
            nova y: Byte = x virar Byte;
            mimo y virar bombom;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
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
// @pinker-nav:end evidencia.semantica.tipos-numericos-largura-fixa

// @pinker-nav:start evidencia.semantica.aliases-arrays-e-ninhos
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita aliases de tipo, arrays fixos e ninhos em assinatura/alias: aceita casos válidos e rejeita alias inexistente e array de tamanho zero, nos casos presentes.
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
fn ninho_valido_em_assinatura_e_alias() {
    let code = r#"
        pacote main;
        ninho Ponto {
            x: bombom;
            y: bombom;
        }
        apelido VetorPontos = [Ponto; 2];
        carinho usa(_p: Ponto, _v: VetorPontos) -> bombom { mimo 0; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}
// @pinker-nav:end evidencia.semantica.aliases-arrays-e-ninhos

// @pinker-nav:start evidencia.semantica.ponteiros-e-aritmetica
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita, via parse_and_check, ponteiros `seta`, `frágil seta`, dereferência, escrita indireta e aritmética de ponteiro: aceita os casos válidos desta fase e rejeita base inexistente, seta de seta, tipos não-bombom e combinações inválidas nos casos presentes; parte das rejeições de sintaxe de tipo (ex.: `frágil` fora de `seta`) é emitida já no parser, não pelo checker semântico.
#[test]
fn seta_valida_em_alias_array_struct_e_assinatura() {
    let code = r#"
        pacote main;
        ninho Ponto { x: bombom; }
        apelido PtrPonto = seta<Ponto>;
        apelido PtrBytes = seta<[u8; 8]>;
        carinho usa(_a: PtrPonto, _b: PtrBytes, _c: seta<u64>) -> bombom { mimo 0; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn seta_falha_com_tipo_base_inexistente() {
    let code =
        "pacote main; carinho principal() -> bombom { nova _x: seta<Desconhecido> = 0; mimo 0; }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo 'Desconhecido' não existe"));
}

#[test]
fn seta_de_seta_ainda_nao_suportada() {
    let code = r#"
        pacote main;
        apelido Ptr = seta<bombom>;
        apelido PtrPtr = seta<Ptr>;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("seta de seta ainda não é suportada nesta fase"));
}

#[test]
fn fragil_seta_valida_em_alias_e_assinatura() {
    let code = r#"
        pacote main;
        apelido Porta = fragil seta<u8>;
        carinho id(p: Porta) -> Porta { mimo p; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fragil_em_tipo_nao_seta_e_invalido() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: fragil u8 = 1;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("'fragil' só pode qualificar tipo seta"),
        "{}",
        err
    );
}

#[test]
fn dereferencia_seta_bombom_valida_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<bombom> = 1;
            mimo *p;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn dereferencia_seta_nao_bombom_falha_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<verso> = 1;
            nova _x: verso = *p;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("dereferência aceita ponteiro para escalar público, array suportado ou ninho"),
        "{}",
        err
    );
}

#[test]
fn dereferencia_seta_ninho_valida_quando_usada_em_acesso_a_campo() {
    let code = r#"
        pacote main;
        ninho Par { a: bombom; b: bombom; }
        carinho pega(p: seta<Par>) -> bombom {
            mimo (*p).a;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn escrita_indireta_seta_bombom_valida_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<bombom> = 1;
            *p = 42;
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn escrita_indireta_seta_nao_bombom_falha_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<verso> = 1;
            *p = "fora";
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("escrita indireta aceita ponteiros para escalares públicos de uma palavra"),
        "{}",
        err
    );
}

#[test]
fn aritmetica_ponteiro_ptr_add_bombom_valida_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<bombom> = 1;
            nova q: seta<bombom> = p + 1;
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn aritmetica_ponteiro_ptr_sub_bombom_valida_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<bombom> = 3;
            nova q: seta<bombom> = p - 2;
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn aritmetica_ponteiro_bombom_add_ptr_falha_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: seta<bombom> = 1;
            nova q: seta<bombom> = 1 + p;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("apenas 'seta<T> + bombom'"), "{}", err);
}

#[test]
fn aritmetica_ponteiro_ptr_ptr_falha_nesta_fase() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova a: seta<bombom> = 1;
            nova b: seta<bombom> = 2;
            nova c: seta<bombom> = a + b;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("exige 'seta<T> + bombom'"), "{}", err);
}
// @pinker-nav:end evidencia.semantica.ponteiros-e-aritmetica

// @pinker-nav:start evidencia.semantica.ninhos-diagnostico
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Rejeita, nos casos presentes, ninho com campo duplicado, tipo de campo inexistente e recursão direta.
#[test]
fn ninho_falha_com_campo_duplicado() {
    let code = r#"
        pacote main;
        ninho Ponto {
            x: bombom;
            x: bombom;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("campo 'x' duplicado"));
}

#[test]
fn ninho_falha_com_tipo_de_campo_inexistente() {
    let code = r#"
        pacote main;
        ninho Ponto {
            x: Fantasma;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo 'Fantasma' não existe"), "{}", err);
}

#[test]
fn ninho_falha_com_recursao_direta() {
    let code = r#"
        pacote main;
        ninho Node {
            prox: Node;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("recursão direta"), "{}", err);
}
// @pinker-nav:end evidencia.semantica.ninhos-diagnostico

// @pinker-nav:start evidencia.semantica.aritmetica-modulo-e-literais
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita módulo (aceito em bombom, rejeitado em lógica) e limites de literais inteiros de largura fixa (u8/u16/i8 no limite e fora de range; bombom sem limite), nos casos presentes.
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

// --- HF-4: validação de range de literais inteiros ---

#[test]
fn literal_u8_fora_de_range_e_rejeitado() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: u8 = 300;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("u8"), "{}", err);
    assert!(err.contains("300"), "{}", err);
}

#[test]
fn literal_u8_no_limite_aceito() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: u8 = 255;
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn literal_u16_fora_de_range_e_rejeitado() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: u16 = 70000;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("u16"), "{}", err);
}

#[test]
fn literal_i8_fora_de_range_e_rejeitado() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: i8 = 200;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("i8"), "{}", err);
}

#[test]
fn literal_em_chamada_fora_de_range_e_rejeitado() {
    let code = r#"
        pacote main;
        carinho soma(a: u8, b: u8) -> u8 { mimo a; }
        carinho principal() -> bombom {
            soma(256, 1);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("u8"), "{}", err);
    assert!(err.contains("256"), "{}", err);
}

#[test]
fn literal_bombom_sem_limite_aceito() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: bombom = 999999999999;
            mimo x;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}
// @pinker-nav:end evidencia.semantica.aritmetica-modulo-e-literais

// ── Fase 148: escrita por índice em array fixo [bombom; N] ───────────────────

// @pinker-nav:start evidencia.semantica.escrita-por-indice
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita escrita por índice em array fixo de bombom: aceita o caso válido e rejeita índice não-bombom, base não-array e valor não-bombom, nos casos presentes.
#[test]
fn escrita_por_indice_em_array_fixo_bombom_valida() {
    let code = r#"
        pacote main;
        carinho escreve(a: [bombom; 3], i: bombom) { a[i] = 5; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn escrita_por_indice_com_indice_nao_bombom_falha() {
    let code = r#"
        pacote main;
        carinho escreve(a: [bombom; 3], ok: logica) { a[ok] = 5; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("índice de escrita nesta fase deve ser 'bombom'"),
        "{}",
        err
    );
}

#[test]
fn escrita_por_indice_em_base_nao_array_falha() {
    let code = r#"
        pacote main;
        carinho escreve(v: bombom, i: bombom) { v[i] = 5; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("escrita por índice exige base de array fixo nesta fase"),
        "{}",
        err
    );
}

#[test]
fn escrita_por_indice_com_valor_nao_bombom_falha() {
    let code = r#"
        pacote main;
        carinho escreve(a: [bombom; 3], i: bombom, ok: logica) { a[i] = ok; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipo incompatível na escrita por índice"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.escrita-por-indice

// ── Fase 149: lista mínima homogênea de bombom ──────────────────────────────

// @pinker-nav:start evidencia.semantica.listas
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita listas homogêneas de bombom (criar, anexar, obter, definir, tirar_ultimo): aceita casos válidos e rejeita tipo fora do recorte e valores/argumentos inválidos, nos casos presentes.
#[test]
fn lista_bombom_criar_anexar_obter_valida() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_obter;
        carinho principal() -> bombom {
            nova l: lista<bombom> = bombom_criar();
            bombom_anexar(l, 10);
            bombom_anexar(l, 20);
            mimo bombom_obter(l, 1);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_bombom_rejeita_tipo_fora_do_recorte() {
    let code = r#"
        pacote main; trazer lista.bombom_criar;
        carinho principal() -> bombom {
            nova l: lista<verso> = bombom_criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("lista<bombom>"), "{}", err);
}

#[test]
fn lista_bombom_anexar_rejeita_valor_nao_bombom() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar;
        carinho principal() -> bombom {
            nova l: lista<bombom> = bombom_criar();
            bombom_anexar(l, "oi");
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("esperado 'bombom'"), "{}", err);
}

#[test]
fn lista_bombom_definir_valida() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_definir; trazer lista.bombom_obter;
        carinho principal() -> bombom {
            nova l: lista<bombom> = bombom_criar();
            bombom_anexar(l, 10);
            bombom_definir(l, 0, 22);
            mimo bombom_obter(l, 0);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_bombom_definir_rejeita_valor_nao_bombom() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_definir;
        carinho principal() -> bombom {
            nova l: lista<bombom> = bombom_criar();
            bombom_anexar(l, 10);
            bombom_definir(l, 0, "oi");
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("argumento 3 da chamada 'lista_bombom_definir'"),
        "{}",
        err
    );
}

#[test]
fn lista_bombom_tirar_ultimo_valida() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_tirar_ultimo;
        carinho principal() -> bombom {
            nova l: lista<bombom> = bombom_criar();
            bombom_anexar(l, 10);
            bombom_anexar(l, 20);
            mimo bombom_tirar_ultimo(l);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_bombom_tirar_ultimo_rejeita_argumento_fora_do_recorte() {
    let code = r#"
        pacote main; trazer lista.bombom_tirar_ultimo;
        carinho principal() -> bombom {
            mimo bombom_tirar_ultimo("oi");
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("argumento 1 da chamada 'lista_bombom_tirar_ultimo'"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.listas

// @pinker-nav:start evidencia.semantica.mapas
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita mapas verso->bombom (criar, definir, obter, tem): aceita o caso válido e rejeita tipo fora do recorte, valor inválido e superfície não pública, nos casos presentes.
#[test]
fn mapa_verso_bombom_criar_definir_obter_tem_valida() {
    let code = r#"
        pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir; trazer mapa.verso_bombom_obter; trazer mapa.verso_bombom_tem;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(m, "idade", 7);
            talvez verso_bombom_tem(m, "idade") {
                mimo verso_bombom_obter(m, "idade");
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn mapa_verso_bombom_rejeita_tipo_fora_do_recorte() {
    let code = r#"
        pacote main; trazer mapa.verso_bombom_criar;
        carinho principal() -> bombom {
            nova m: mapa<bombom,bombom> = verso_bombom_criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("mapa<verso,bombom>"), "{}", err);
}

#[test]
fn mapa_verso_bombom_definir_rejeita_valor_nao_bombom() {
    let code = r#"
        pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(m, "idade", "sete");
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("argumento 3 da chamada 'mapa_verso_bombom_definir'"),
        "{}",
        err
    );
}

#[test]
fn mapa_verso_bombom_chave_indice_nao_e_superficie_publica_na_fase155() {
    let code = r#"
        pacote main; trazer mapa.verso_bombom_criar; trazer mapa.verso_bombom_definir;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = verso_bombom_criar();
            verso_bombom_definir(m, "idade", 7);
            nova chave: verso = mapa_verso_bombom_chave_indice(m, 0);
            falar(chave);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("função 'mapa_verso_bombom_chave_indice' não declarada"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.mapas

// @pinker-nav:start evidencia.semantica.acaso
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita intrínsecas de aleatoriedade com semente explícita: aceita o caso básico e rejeita semente não-bombom, aridade inválida e a API ampla fora do recorte, nos casos presentes.
#[test]
fn aleatorio_basico_com_semente_explicita_valida_sem_declaracao() {
    let code = r#"
        pacote main; trazer acaso.criar; trazer acaso.proximo;
        carinho principal() -> bombom {
            nova gerador: bombom = criar(42);
            nova valor: bombom = proximo(gerador);
            mimo valor;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn aleatorio_criar_rejeita_semente_nao_bombom() {
    let code = r#"
        pacote main; trazer acaso.criar;
        carinho principal() -> bombom {
            nova gerador: bombom = criar("oi");
            mimo gerador;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("argumento 1 da chamada 'aleatorio_criar'"),
        "{}",
        err
    );
}

#[test]
fn aleatorio_proximo_rejeita_aridade_invalida() {
    let code = r#"
        pacote main; trazer acaso.criar; trazer acaso.proximo;
        carinho principal() -> bombom {
            nova gerador: bombom = criar(7);
            mimo proximo(gerador, 1);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("chamada de 'aleatorio_proximo' com aridade inválida"),
        "{}",
        err
    );
}

#[test]
fn api_ampla_de_aleatoriedade_permanece_fora_do_recorte() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo aleatorio_intervalo(1, 10);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("função 'aleatorio_intervalo' não declarada"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.acaso

// ── Fases 186–189 — importação por família: `tempo`, `ambiente`, `acaso` e `texto` ──

// @pinker-nav:start evidencia.semantica.imports-por-familia
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita importação por família via `trazer` (tempo, ambiente, acaso, texto, arquivo, caminho, processo) e o acesso legado global sem trazer, rejeitando família desconhecida e membro que a família não exporta, nos casos presentes. Importação aceita não implica uso real do símbolo. A superfície por família em si — grafias qualificada e seletiva, canonicalização e paridade — tem evidência própria em `tests/part_g_familia_superficie_tests.rs`.
#[test]
fn trazer_tempo_familia_aceita() {
    let code = r#"
        pacote main; trazer tempo.unix;
        carinho principal() -> bombom {
            nova agora: bombom = unix();
            mimo agora;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_ambiente_familia_aceita() {
    let code = r#"
        pacote main; trazer ambiente.buscar_contexto; trazer ambiente.quantos_argumentos; trazer ambiente.tem_flag; trazer ambiente.variavel_ou;
        carinho principal() -> bombom {
            nova saida: verso = buscar_contexto("--saida", "PINKER_SAIDA", "padrao.txt");
            nova origem: verso = variavel_ou("HOME", "/tmp");
            talvez tem_flag("--quiet") {
                falar(saida, origem);
            }
            mimo quantos_argumentos();
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_acaso_familia_aceita() {
    let code = r#"
        pacote main; trazer acaso.criar; trazer acaso.proximo;
        carinho principal() -> bombom {
            nova gerador: bombom = criar(42);
            nova valor: bombom = proximo(gerador);
            mimo valor;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_tempo_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer tempo.formatar_unix; trazer tempo.unix;
        carinho principal() -> bombom {
            nova agora: bombom = unix();
            nova texto: verso = formatar_unix(agora);
            falar(texto);
            mimo agora;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_ambiente_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer ambiente.buscar_contexto; trazer ambiente.quantos_argumentos; trazer caminho.diretorio_atual;
        carinho principal() -> bombom {
            nova saida: verso = buscar_contexto("--saida", "PINKER_SAIDA", "padrao.txt");
            nova cwd: verso = diretorio_atual();
            falar(saida, cwd);
            mimo quantos_argumentos();
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_acaso_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer acaso.criar; trazer acaso.proximo;
        carinho principal() -> bombom {
            nova gerador: bombom = criar(7);
            nova valor: bombom = proximo(gerador);
            mimo valor;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_familia_desconhecida_falha() {
    let code = r#"
        pacote main;
        trazer colecao;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("família 'colecao' não é reconhecida"),
        "{}",
        err
    );
}

#[test]
fn trazer_texto_familia_aceita() {
    let code = r#"
        pacote main; trazer texto.aparar; trazer texto.juntar;
        carinho principal() -> bombom {
            nova saudacao: verso = juntar("rosa", " pinker");
            nova limpa: verso = aparar("  texto  ");
            falar(saudacao);
            falar(limpa);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_texto_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer texto.juntar; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova saudacao: verso = juntar("rosa", " pinker");
            nova n: bombom = tamanho(saudacao);
            falar(saudacao);
            mimo n;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

/// A recusa deixou de ser categórica: o que se recusa é um membro que a
/// família não exporta. `texto` continua importável inteira e continua sem
/// membro nenhum a selecionar nesta fase.
#[test]
fn trazer_seletivo_de_familia_sem_membros_falha() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("membro 'juntar_verso' não existe na família 'texto'"),
        "{}",
        err
    );
    assert!(err.contains("não exporta membros nesta fase"), "{}", err);
}

#[test]
fn trazer_seletivo_de_familia_de_tempo_falha() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("membro 'tempo_unix' não existe na família 'tempo'"),
        "{}",
        err
    );
}

#[test]
fn trazer_arquivo_familia_aceita() {
    let code = r#"
        pacote main; trazer arquivo.criar; trazer arquivo.escrever_verso; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova cabo: bombom = criar("target/teste_trazer_arquivo.txt");
            escrever_verso(cabo, "rosa");
            fechar(cabo);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_caminho_familia_aceita() {
    let code = r#"
        pacote main; trazer caminho.existe; trazer caminho.juntar;
        carinho principal() -> bombom {
            nova destino: verso = juntar("docs", "atlas.md");
            talvez existe(destino) {
                falar(destino);
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_processo_familia_aceita() {
    let code = r#"
        pacote main; trazer ambiente.argumento; trazer processo.executar;
        carinho principal() -> bombom {
            nova comando: verso = argumento(0);
            nova codigo: bombom = executar(comando);
            mimo codigo;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_arquivo_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer arquivo.ler_caminho_verso; trazer texto.tamanho;
        carinho principal() -> bombom {
            nova conteudo: verso = ler_caminho_verso("Cargo.toml");
            nova n: bombom = tamanho(conteudo);
            mimo n;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_caminho_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer caminho.existe; trazer caminho.juntar;
        carinho principal() -> bombom {
            nova destino: verso = juntar("docs", "atlas.md");
            talvez existe(destino) {
                falar(destino);
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_processo_sem_trazer_continua_valido() {
    let code = r#"
        pacote main; trazer ambiente.argumento; trazer processo.executar;
        carinho principal() -> bombom {
            nova comando: verso = argumento(0);
            nova codigo: bombom = executar(comando);
            mimo codigo;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

/// `arquivo` exporta membros, mas `criar_arquivo` é o nome GLOBAL: sob a
/// família o membro se chama `criar`. O diagnóstico precisa dizer isso, e não
/// procurar `arquivo.pink`.
#[test]
fn trazer_seletivo_de_nome_global_em_vez_de_membro_falha() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("membro 'criar_arquivo' não existe na família 'arquivo'"),
        "{}",
        err
    );
    assert!(err.contains("'criar'"), "{}", err);
}

/// O contrapositivo: o membro aprovado é aceito.
#[test]
fn trazer_seletivo_de_membro_aprovado_e_aceito() {
    let code = r#"
        pacote main; trazer arquivo.criar; trazer arquivo.fechar;
        carinho principal() -> bombom {
            nova h: bombom = criar("alvo.txt");
            fechar(h);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}
// @pinker-nav:end evidencia.semantica.imports-por-familia

// @pinker-nav:start evidencia.semantica.leques-simples
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita leques nominais simples: declaração e uso, parâmetro/retorno, `escolha` por variante e `virar bombom`; rejeita tipos nominais diferentes, comparação entre leques distintos, variante inexistente, conversão implícita, ordem, variante duplicada, leque vazio e colisão com ninho, nos casos presentes.
#[test]
fn leque_declaracao_e_uso_nominal_aceitos() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde, Azul }
        carinho principal() -> bombom {
            nova escolhida: Cor = Cor.Verde;
            talvez escolhida == Cor.Verde {
                mimo 1;
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_como_parametro_e_retorno_aceito() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho troca(cor: Cor) -> Cor {
            talvez cor == Cor.Vermelho {
                mimo Cor.Verde;
            }
            mimo Cor.Vermelho;
        }
        carinho principal() -> bombom {
            nova c: Cor = troca(Cor.Verde);
            talvez c == Cor.Vermelho {
                mimo 0;
            }
            mimo 1;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_escolha_despacha_por_variante() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde, Azul }
        carinho principal() -> bombom {
            nova c: Cor = Cor.Azul;
            escolha c {
                caso Cor.Vermelho { mimo 1; }
                caso Cor.Verde { mimo 2; }
                senao { mimo 3; }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_virar_bombom_aceito() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova disc: bombom = Cor.Verde virar bombom;
            mimo disc;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_tipos_nominais_diferentes_rejeitados() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        leque Fruta { Banana, Maca }
        carinho principal() -> bombom {
            nova c: Cor = Fruta.Banana;
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn leque_comparacao_entre_leques_diferentes_rejeitada() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        leque Fruta { Banana, Maca }
        carinho principal() -> bombom {
            talvez Cor.Vermelho == Fruta.Banana {
                mimo 1;
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipos incompatíveis"), "{}", err);
}

#[test]
fn leque_variante_inexistente_rejeitada() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova c: Cor = Cor.Rosa;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("variante 'Rosa' não existe"), "{}", err);
}

#[test]
fn leque_inteiro_nao_vira_leque_implicitamente() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova c: Cor = 1;
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn leque_comparacao_de_ordem_rejeitada() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            talvez Cor.Vermelho < Cor.Verde {
                mimo 1;
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("comparação de ordem"), "{}", err);
}

#[test]
fn leque_variante_duplicada_rejeitada() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Vermelho }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("duplicada"), "{}", err);
}

#[test]
fn leque_vazio_rejeitado() {
    let code = r#"
        pacote main;
        leque Cor { }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn leque_nome_colide_com_ninho_rejeitado() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho }
        ninho Cor { valor: bombom; }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("já utilizado"), "{}", err);
}
// @pinker-nav:end evidencia.semantica.leques-simples

// @pinker-nav:start evidencia.semantica.leques-com-carga
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita leques com carga: construção e encaixe, parâmetro/retorno; rejeita tipo/aridade errados, variante com carga sem construção, variante sem carga com chamada, igualdade, virar e tipo de carga não suportado, nos casos presentes.
#[test]
fn leque_carga_construcao_e_encaixe_aceitos() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Palavra(verso), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Numero(42);
            encaixe t {
                caso Token.Numero(n) { falar(n); }
                caso Token.Palavra(p) { falar(p); }
                caso Token.Fim { falar("fim"); }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_carga_como_parametro_e_retorno_aceito() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho fabrica(valor: bombom) -> Token {
            mimo Token.Numero(valor);
        }
        carinho principal() -> bombom {
            nova t: Token = fabrica(7);
            encaixe t {
                caso Token.Numero(n) { mimo n; }
                caso Token.Fim { mimo 0; }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_carga_tipo_errado_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Numero("texto");
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("carga 1 inválida"), "{}", err);
}

#[test]
fn leque_carga_aridade_errada_rejeitada() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Numero(1, 2);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("exige 1 argumento(s)"), "{}", err);
}

#[test]
fn leque_variante_com_carga_sem_construcao_rejeitada() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Numero;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("carrega valor"), "{}", err);
}

#[test]
fn leque_variante_sem_carga_com_chamada_rejeitada() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim(1);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não carrega valor"), "{}", err);
}

#[test]
fn leque_com_carga_igualdade_rejeitada() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova a: Token = Token.Fim;
            nova b: Token = Token.Fim;
            talvez a == b {
                mimo 1;
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("use 'encaixe'"), "{}", err);
}

#[test]
fn leque_com_carga_virar_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim;
            nova d: bombom = t virar bombom;
            mimo d;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("'virar' não é suportado"), "{}", err);
}

#[test]
fn leque_carga_tipo_nao_suportado_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Ativo(logica) }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    // A mensagem descreve o contrato atualizado (D1) e carrega o código estável
    // da recusa; `logica` continua fora do contrato.
    assert!(
        err.contains(pinker_v0::enum_payload::CONTRATO_CARGAS),
        "{}",
        err
    );
    assert!(
        err.contains("E-SEMANTIC-ENUM-PAYLOAD-UNSUPPORTED"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.leques-com-carga

// @pinker-nav:start evidencia.semantica.encaixe-e-bindings
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita, via parse_and_check, `encaixe`: exaustividade com/sem `senão`, bindings em variantes com/sem carga, leque não declarado, mistura de leques, escrutínio de tipo errado e variante repetida, nos casos presentes; algumas rejeições (ex.: mistura de leques) surgem já no parse/desugaring de `encaixe`, outras no checker.
#[test]
fn encaixe_nao_exaustivo_sem_senao_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Palavra(verso), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim;
            encaixe t {
                caso Token.Numero(n) { falar(n); }
                caso Token.Fim { falar("fim"); }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não cobre a variante 'Palavra'"), "{}", err);
}

#[test]
fn encaixe_nao_exaustivo_com_senao_aceito() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Palavra(verso), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim;
            encaixe t {
                caso Token.Numero(n) { falar(n); }
                senao { falar("outro"); }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn encaixe_binding_em_variante_sem_carga_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim;
            encaixe t {
                caso Token.Numero(n) { falar(n); }
                caso Token.Fim(x) { falar(x); }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não carrega valor"), "{}", err);
}

#[test]
fn encaixe_sem_binding_em_variante_com_carga_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim;
            encaixe t {
                caso Token.Numero { falar("n"); }
                caso Token.Fim { falar("fim"); }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("carrega 1 valor(es)"), "{}", err);
}

#[test]
fn encaixe_leque_nao_declarado_rejeitado() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova t: bombom = 1;
            encaixe t {
                caso Fantasma.Algo { falar("x"); }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não declarado"), "{}", err);
}

#[test]
fn encaixe_mistura_de_leques_rejeitada() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        leque Fruta { Banana, Maca }
        carinho principal() -> bombom {
            nova c: Cor = Cor.Verde;
            encaixe c {
                caso Cor.Vermelho { falar("v"); }
                caso Fruta.Banana { falar("b"); }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("mistura leques"), "{}", err);
}

#[test]
fn encaixe_escrutinio_de_tipo_errado_rejeitado() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova x: bombom = 5;
            encaixe x {
                caso Token.Numero(n) { falar(n); }
                caso Token.Fim { falar("fim"); }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn encaixe_variante_repetida_rejeitada() {
    let code = r#"
        pacote main;
        leque Token { Numero(bombom), Fim }
        carinho principal() -> bombom {
            nova t: Token = Token.Fim;
            encaixe t {
                caso Token.Fim { falar("a"); }
                caso Token.Fim { falar("b"); }
                caso Token.Numero(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("repetida"), "{}", err);
}
// @pinker-nav:end evidencia.semantica.encaixe-e-bindings

// @pinker-nav:start evidencia.semantica.leques-recursivos-e-multiplas-cargas
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita leques recursivos, mutuamente recursivos e com múltiplas cargas, além de bindings de encaixe correlatos; rejeita carga de leque errado, aridade errada e tipo de carga desconhecido, nos casos presentes.
#[test]
fn leque_recursivo_aceito() {
    let code = r#"
        pacote main;
        leque Expr { Lit(bombom), Dobro(Expr) }
        carinho avalia(e: Expr) -> bombom {
            encaixe e {
                caso Expr.Lit(n) { mimo n; }
                caso Expr.Dobro(interno) { mimo 2 * avalia(interno); }
            }
            mimo 0;
        }
        carinho principal() -> bombom {
            mimo avalia(Expr.Dobro(Expr.Lit(21)));
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_mutuamente_recursivo_aceito() {
    let code = r#"
        pacote main;
        leque Par { Fim, Passo(Impar) }
        leque Impar { Passo(Par) }
        carinho principal() -> bombom {
            nova p: Par = Par.Passo(Impar.Passo(Par.Fim));
            encaixe p {
                caso Par.Fim { mimo 0; }
                caso Par.Passo(i) { mimo 1; }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_multiplas_cargas_aceito() {
    let code = r#"
        pacote main;
        leque Expr { Lit(bombom), Soma(Expr, Expr), Rotulo(verso, Expr) }
        carinho principal() -> bombom {
            nova e: Expr = Expr.Rotulo("r", Expr.Soma(Expr.Lit(1), Expr.Lit(2)));
            encaixe e {
                caso Expr.Lit(n) { mimo n; }
                caso Expr.Soma(a, b) { mimo 1; }
                caso Expr.Rotulo(nome, corpo) { falar(nome); mimo 2; }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn leque_carga_de_leque_errado_rejeitada() {
    let code = r#"
        pacote main;
        leque Expr { Lit(bombom), Dobro(Expr) }
        leque Outro { Coisa(bombom) }
        carinho principal() -> bombom {
            nova e: Expr = Expr.Dobro(Outro.Coisa(1));
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("carga 1 inválida"), "{}", err);
}

#[test]
fn leque_multiplas_cargas_aridade_errada_rejeitada() {
    let code = r#"
        pacote main;
        leque Expr { Soma(Expr, Expr), Lit(bombom) }
        carinho principal() -> bombom {
            nova e: Expr = Expr.Soma(Expr.Lit(1));
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("exige 2 argumento(s)"), "{}", err);
}

#[test]
fn encaixe_bindings_em_numero_errado_rejeitado() {
    let code = r#"
        pacote main;
        leque Expr { Soma(Expr, Expr), Lit(bombom) }
        carinho principal() -> bombom {
            nova e: Expr = Expr.Lit(1);
            encaixe e {
                caso Expr.Soma(a) { mimo 1; }
                caso Expr.Lit(n) { mimo n; }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("liga 1 nome(s)"), "{}", err);
}

#[test]
fn leque_carga_de_tipo_desconhecido_rejeitada() {
    let code = r#"
        pacote main;
        leque Expr { Guarda(Fantasma) }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains(pinker_v0::enum_payload::CONTRATO_CARGAS),
        "{}",
        err
    );
    assert!(
        err.contains("E-SEMANTIC-ENUM-PAYLOAD-UNRESOLVED"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.leques-recursivos-e-multiplas-cargas

// @pinker-nav:start evidencia.semantica.genericos
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita coleções e funções genéricas (lista e mapa genéricos, impl homônimos, função genérica de usuário, para_cada, monomorfização legada): aceita casos válidos e rejeita tipo incompatível, elemento de outro leque, não-leque, ausência de anotação e criação fora de `nova`, nos casos presentes. Vários casos usam exemplos por include_str!, observados como casos exemplares.
#[test]
fn lista_generica_de_leque_aceita() {
    let code = r#"
        pacote main; trazer lista.anexar; trazer lista.criar; trazer lista.obter; trazer lista.tamanho;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = criar();
            anexar(cores, Cor.Vermelho);
            nova primeira: Cor = obter(cores, 0);
            talvez primeira == Cor.Vermelho {
                mimo tamanho(cores);
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase233_mapa_generico_aceito() {
    let code = include_str!("../examples/fase233_mapa_generico_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase234_impl_homonimos_aceito() {
    let code = include_str!("../examples/fase234_impl_homonimos_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase235_mapa_generico_expressoes_aceito() {
    let code = include_str!("../examples/fase235_mapa_generico_expressoes_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase236_funcao_generica_usuario_aceita() {
    let code = include_str!("../examples/fase236_funcao_generica_usuario_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase236_funcao_generica_usuario_rejeita_tipo_incompativel() {
    let code = r#"
        pacote demo;
        carinho identidade<T>(valor: T) -> T {
            mimo valor;
        }
        carinho principal() -> bombom {
            mimo identidade<bombom>("texto");
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipo inválido no argumento"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase235_mapa_generico_rejeita_primeiro_argumento_nao_mapa() {
    let code = r#"
        pacote demo; trazer mapa.obter;
        carinho principal() -> bombom {
            mimo obter(1, "idade");
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("exige mapa como primeiro argumento"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase234_impl_homonimos_exigem_qualificacao_quando_ambiguo() {
    let code = r#"
        pacote demo;

        trato Exibivel { carinho valor(valor: bombom) -> bombom; }
        trato Medivel { carinho valor(valor: bombom) -> bombom; }

        impl Exibivel para bombom {
            carinho valor(valor: bombom) -> bombom { mimo valor + 1; }
        }

        impl Medivel para bombom {
            carinho valor(valor: bombom) -> bombom { mimo valor + valor; }
        }

        carinho principal() -> bombom {
            mimo 20.valor();
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("é ambíguo"), "erro inesperado: {err}");
}

#[test]
fn lista_generica_como_parametro_e_retorno_aceita() {
    let code = r#"
        pacote main; trazer lista.anexar; trazer lista.criar; trazer lista.tamanho;
        leque Cor { Vermelho, Verde }
        carinho fabrica() -> lista<Cor> {
            nova cores: lista<Cor> = criar();
            anexar(cores, Cor.Verde);
            mimo cores;
        }
        carinho conta(cores: lista<Cor>) -> bombom {
            mimo tamanho(cores);
        }
        carinho principal() -> bombom {
            mimo conta(fabrica());
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_generica_para_cada_aceito() {
    let code = r#"
        pacote main; trazer lista.anexar; trazer lista.criar;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = criar();
            anexar(cores, Cor.Verde);
            para cada cor em cores {
                talvez cor == Cor.Verde {
                    falar("verde");
                }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_generica_elemento_de_outro_leque_rejeitado() {
    let code = r#"
        pacote main; trazer lista.anexar; trazer lista.criar;
        leque Cor { Vermelho, Verde }
        leque Fruta { Banana, Maca }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = criar();
            anexar(cores, Fruta.Banana);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("exige elemento"), "{}", err);
}

#[test]
fn lista_generica_de_nao_leque_rejeitada() {
    let code = r#"
        pacote main; trazer lista.criar;
        carinho principal() -> bombom {
            nova coisas: lista<Fantasma> = criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não é um leque"), "{}", err);
}

#[test]
fn lista_criar_sem_anotacao_rejeitado() {
    let code = r#"
        pacote main; trazer lista.criar;
        carinho principal() -> bombom {
            nova coisas = criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("anotação de tipo"), "{}", err);
}

#[test]
fn lista_criar_fora_de_nova_rejeitado() {
    let code = r#"
        pacote main; trazer lista.criar; trazer lista.tamanho;
        carinho principal() -> bombom {
            mimo tamanho(criar());
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn intrinsecas_genericas_sobre_listas_legadas_aceitas() {
    let code = r#"
        pacote main; trazer lista.anexar; trazer lista.criar; trazer lista.obter; trazer lista.tamanho;
        carinho principal() -> bombom {
            nova numeros: lista<bombom> = criar();
            anexar(numeros, 7);
            nova palavras: lista<verso> = criar();
            anexar(palavras, "rosa");
            nova p: verso = obter(palavras, 0);
            falar(p);
            mimo obter(numeros, 0) + tamanho(palavras);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_legada_monomorphizada_continua_valida() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.bombom_criar; trazer lista.bombom_obter;
        carinho principal() -> bombom {
            nova numeros: lista<bombom> = bombom_criar();
            bombom_anexar(numeros, 7);
            mimo bombom_obter(numeros, 0);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_generica_nao_aceita_intrinseca_monomorphizada_de_bombom() {
    let code = r#"
        pacote main; trazer lista.bombom_anexar; trazer lista.criar;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = criar();
            bombom_anexar(cores, 1);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_err());
}
// @pinker-nav:end evidencia.semantica.genericos

// @pinker-nav:start evidencia.semantica.tratamento-de-erro
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita, via parse_and_check, tratamento estruturado de erro (`encaixe` em leque sem carga, `tentar`, `propagar` e propagar curto): aceita casos válidos e rejeita sucesso/falha ausentes, falha ambígua e variantes indistintas, nos casos presentes; as rejeições de forma de `tentar`/`propagar` são emitidas no desugaring do parser, não pelo checker. Casos usam include_str! como exemplos observados.
#[test]
fn encaixe_em_leque_sem_carga_aceito() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde, Azul }
        carinho principal() -> bombom {
            nova c: Cor = Cor.Azul;
            encaixe c {
                caso Cor.Vermelho { falar("quente"); }
                caso Cor.Verde { falar("fria"); }
                caso Cor.Azul { falar("fria"); }
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tentar_error_handling_estruturado_aceito() {
    let code = include_str!("../examples/fase223_error_handling_tentar_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tentar_exige_sucesso_e_falha() {
    let code = r#"
        pacote main;
        leque Resultado { Ok(bombom), Erro(verso) }
        carinho principal() -> bombom {
            nova r: Resultado = Resultado.Ok(1);
            tentar r {
                sucesso Resultado.Ok(v) { mimo v; }
            }
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tentar exige exatamente um braço 'sucesso' e um braço 'falha'"),
        "{}",
        err
    );
}

#[test]
fn propagar_error_handling_estruturado_aceito() {
    let code = include_str!("../examples/fase224_error_handling_propagar_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase231_propagar_valor_nomeado_aceito() {
    let code = include_str!("../examples/fase231_propagar_valor_nomeado_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase237_propagar_curto_aceito() {
    let code = include_str!("../examples/fase237_propagar_curto_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase237_propagar_curto_rejeita_falha_ambigua() {
    let code = r#"
        pacote main;
        leque Resultado { Ok(bombom), Erro(verso), Cancelado(verso) }
        carinho validar() -> Resultado { mimo Resultado.Ok(1); }
        carinho principal() -> bombom {
            propagar? validar() como Resultado.Ok(v);
            mimo v;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("propagar? é ambíguo"), "{}", err);
}

#[test]
fn propagar_exige_variantes_distintas() {
    let code = r#"
        pacote main;
        leque Resultado { Ok(bombom), Erro(verso) }
        carinho validar() -> Resultado { mimo Resultado.Ok(1); }
        carinho principal() -> bombom {
            propagar validar() como Resultado.Ok(v) senao Resultado.Ok(e);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("propagar exige variantes distintas para sucesso e falha"),
        "{}",
        err
    );
}
// @pinker-nav:end evidencia.semantica.tratamento-de-erro

// @pinker-nav:start evidencia.semantica.funcoes-locais-e-carinho
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita, via parse_and_check, funções locais e carinho anônimo não capturante, função como valor, parâmetro estático e leque genérico de resultado: aceita casos válidos e rejeita aridade de tipo inválida, assinatura incompatível, tipo incompatível e captura de escopo externo, nos casos presentes; parte das rejeições (ex.: tipo de função local incompatível) é emitida já no parser. Casos usam include_str! como exemplos observados.
#[test]
fn carinho_anonimo_nao_capturante_aceito() {
    let code = include_str!("../examples/fase225_carinho_anonimo_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase238_funcao_local_valor_aceita() {
    let code = include_str!("../examples/fase238_funcao_local_valor_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase239_funcao_parametro_estatica_aceita() {
    let code = include_str!("../examples/fase239_funcao_parametro_estatica_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase240_leque_generico_resultado_aceita() {
    let code = include_str!("../examples/fase240_leque_generico_resultado_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase240_leque_generico_rejeita_aridade_de_tipo_invalida() {
    let code = r#"
        pacote main;
        leque Resultado<T, E> { Ok(T), Erro(E) }
        apelido Ruim = Resultado<bombom>;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("leque genérico 'Resultado' exige 2 argumento(s) de tipo"),
        "{}",
        err
    );
}

#[test]
fn fase239_funcao_parametro_estatica_rejeita_assinatura_incompativel() {
    let code = r#"
        pacote main;

        carinho aplicar(f: carinho(bombom) -> bombom, x: bombom) -> bombom {
            mimo f(x);
        }

        carinho principal() -> bombom {
            nova tamanho: carinho(verso) -> bombom = carinho(s: verso) -> bombom {
                mimo tamanho(s);
            };
            mimo aplicar(tamanho, 1);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("callback") && err.contains("incompatível"),
        "{}",
        err
    );
}

#[test]
fn fase238_funcao_local_valor_rejeita_tipo_incompativel() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova f: carinho(verso) -> bombom = carinho(x: bombom) -> bombom {
                mimo x;
            };
            mimo f("x");
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipo da função local é incompatível"),
        "{}",
        err
    );
}

#[test]
fn carinho_anonimo_nao_captura_escopo_externo() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova base: bombom = 10;
            nova valor: bombom = carinho(x: bombom) -> bombom {
                mimo x + base;
            }(1);
            mimo valor;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("identificador 'base' não declarado"),
        "{}",
        err
    );
}

// --- Fase 241: leque padrão `Resultado<T, E>` predeclarado ---

#[test]
fn fase241_resultado_predeclarado_aceita() {
    let code = include_str!("../examples/fase241_resultado_predeclarado_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase241_resultado_predeclarado_constroi_decompoe_sem_leque() {
    let code = r#"
        pacote main;
        apelido RBV = Resultado<bombom, verso>;
        carinho principal() -> bombom {
            nova ok: RBV = RBV.Ok(42);
            nova erro: RBV = RBV.Erro("x");
            nova muda total: bombom = 0;
            encaixe ok {
                caso RBV.Ok(v) { total += v; }
                caso RBV.Erro(m) { falar(m); }
            }
            encaixe erro {
                caso RBV.Ok(v) { total += v; }
                caso RBV.Erro(m) { falar(m); }
            }
            mimo total;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase241_predeclarado_rejeita_carga_incompativel() {
    let code = r#"
        pacote main;
        apelido RBV = Resultado<bombom, verso>;
        carinho usa() -> RBV { mimo RBV.Ok("texto"); }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("esperado 'bombom'") && err.contains("encontrado 'verso'"),
        "{}",
        err
    );
}

#[test]
fn fase241_predeclarado_rejeita_aridade_de_tipo_invalida() {
    let code = r#"
        pacote main;
        apelido Ruim = Resultado<bombom>;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("leque genérico 'Resultado' exige 2 argumento(s) de tipo"),
        "{}",
        err
    );
}

#[test]
fn fase241_usuario_resultado_nao_generico_suprime_predeclarado() {
    // Regressão crítica: programas da Fase 223 (leque Resultado NÃO-genérico do
    // usuário) continuam válidos; o predeclarado é suprimido, não materializado.
    let code = include_str!("../examples/fase223_error_handling_tentar_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase241_usuario_resultado_generico_substitui_predeclarado() {
    // Regressão: a declaração genérica do usuário (Fase 240) substitui o
    // predeclarado sem erro de duplicata.
    let code = include_str!("../examples/fase240_leque_generico_resultado_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase241_duas_declaracoes_usuario_resultado_falham() {
    let code = r#"
        pacote main;
        leque Resultado<T, E> { Ok(T), Erro(E) }
        leque Resultado<T, E> { Ok(T), Erro(E) }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("leque genérico 'Resultado' já declarado"),
        "{}",
        err
    );
}

#[test]
fn fase241_usuario_resultado_nao_herda_variantes_predeclaradas() {
    // O usuário redefine Resultado como leque não-genérico sem `Ok`; usar
    // `Resultado.Ok` deve falhar — nada é herdado do predeclarado.
    let code = r#"
        pacote main;
        leque Resultado { Falha(verso) }
        carinho principal() -> bombom {
            nova r: Resultado = Resultado.Ok(1);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn fase241_ninho_resultado_suprime_predeclarado() {
    // `ninho Resultado` redefine o nome; o uso aplicado `Resultado<...>` deixa de
    // resolver contra o template predeclarado (suprimido) e é inválido.
    let code = r#"
        pacote main;
        ninho Resultado { campo: bombom }
        apelido X = Resultado<bombom, verso>;
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn fase241_propagar_curto_sobre_predeclarado_aceito() {
    let code = r#"
        pacote main;
        apelido RBV = Resultado<bombom, verso>;
        carinho etapa(v: bombom, ok: logica) -> RBV {
            talvez ok { mimo RBV.Ok(v); }
            mimo RBV.Erro("e");
        }
        carinho dobro(a: bombom, ok: logica) -> RBV {
            propagar? etapa(a, ok) como RBV.Ok(x);
            mimo RBV.Ok(x + x);
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase241_exemplo_nao_declara_leque_resultado_manualmente() {
    // O valor da Fase 241 é usar `Resultado<T,E>` SEM declará-lo; o exemplo não
    // pode esconder uma declaração manual do leque (nenhuma linha de código, fora
    // de comentário, pode começar por `leque Resultado`).
    let code = include_str!("../examples/fase241_resultado_predeclarado_valido.pink");
    let declara_manual = code.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with("//") && trimmed.starts_with("leque Resultado")
    });
    assert!(
        !declara_manual,
        "o exemplo da Fase 241 não pode declarar `leque Resultado` manualmente"
    );
}

#[test]
fn fase241_exemplo_exercita_superficie_completa() {
    // Trava a fatia vertical do exemplo: mantém a prova de tentar, propagar,
    // propagar? e encaixe e das DUAS especializações distintas. Impede que uma
    // mutação enfraqueça silenciosamente a prova (ex.: remover `propagar?`).
    let code = include_str!("../examples/fase241_resultado_predeclarado_valido.pink");
    for marca in [
        "Resultado<bombom, verso>",
        "Resultado<verso, bombom>",
        "tentar ",
        "propagar ",
        "propagar? ",
        "encaixe ",
        ".Ok(",
        ".Erro(",
    ] {
        assert!(
            code.contains(marca),
            "o exemplo da Fase 241 deveria exercitar `{marca}`"
        );
    }
}

#[test]
fn fase242_funcao_indireta_aceita() {
    let code = include_str!("../examples/fase242_funcao_indireta_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase242_chamada_indireta_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
        carinho aplicar(operacao: carinho(bombom) -> bombom, valor: bombom) -> bombom {
            mimo operacao(valor, valor);
        }
        carinho principal() -> bombom { mimo aplicar(dobrar, 1); }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("chamada indireta de 'operacao' com aridade inválida"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase242_chamada_indireta_rejeita_tipo_de_argumento_invalido() {
    let code = r#"
        pacote main;
        carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
        carinho aplicar(operacao: carinho(bombom) -> bombom, valor: verso) -> bombom {
            mimo operacao(valor);
        }
        carinho principal() -> bombom { mimo aplicar(dobrar, "x"); }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipo inválido no argumento 1 da chamada indireta de 'operacao'"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase242_rejeita_chamar_valor_nao_callable() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova x: bombom = 1;
            mimo x(1);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("'x' não é chamável"), "erro inesperado: {err}");
}

#[test]
fn fase242_binding_rejeita_assinatura_incompativel() {
    let code = r#"
        pacote main;
        carinho tamanho(s: verso) -> bombom { mimo 0; }
        carinho principal() -> bombom {
            nova operacao: carinho(bombom) -> bombom = tamanho;
            mimo operacao(1);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipo de inicialização incompatível"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase242_binding_rejeita_retorno_incompativel() {
    let code = r#"
        pacote main;
        carinho verifica(x: bombom) -> logica { mimo verdade; }
        carinho principal() -> bombom {
            nova operacao: carinho(bombom) -> bombom = verifica;
            mimo operacao(1);
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("tipo de inicialização incompatível"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase242_retorno_de_callable_aceita_e_verifica_assinatura() {
    let code = r#"
        pacote main;
        carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
        carinho fabricar() -> carinho(bombom) -> bombom {
            mimo dobrar;
        }
        carinho principal() -> bombom {
            nova f: carinho(bombom) -> bombom = fabricar();
            mimo f(21);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase242_variavel_local_callable_tem_precedencia_sobre_funcao_top_level() {
    let code = r#"
        pacote main;
        carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
        carinho triplicar(x: bombom) -> bombom { mimo x * 3; }
        carinho principal() -> bombom {
            nova dobrar: carinho(bombom) -> bombom = triplicar;
            mimo dobrar(10);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase242_regressao_fase239_ainda_aceita_e_rejeita_igual() {
    let code = include_str!("../examples/fase239_funcao_parametro_estatica_valido.pink");
    assert!(parse_and_check(code).is_ok());

    let code_incompativel = r#"
        pacote main;

        carinho aplicar(f: carinho(bombom) -> bombom, x: bombom) -> bombom {
            mimo f(x);
        }

        carinho principal() -> bombom {
            nova tamanho: carinho(verso) -> bombom = carinho(s: verso) -> bombom {
                mimo tamanho(s);
            };
            mimo aplicar(tamanho, 1);
        }
    "#;
    let err = parse_and_check(code_incompativel).unwrap_err().to_string();
    assert!(
        err.contains("callback") && err.contains("incompatível"),
        "erro inesperado: {err}"
    );
}
// @pinker-nav:end evidencia.semantica.funcoes-locais-e-carinho

// @pinker-nav:start evidencia.semantica.closures-captura-imutavel
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fase 243: exercita, via parse_and_check, closures com captura imutável por valor (`carinho(...) {...}` referenciado como valor, nunca chamado imediatamente): aceita captura simples e múltipla, sombreamento por parâmetro e por local, closure aninhada capturando do avô léxico e closure passada como argumento; rejeita atribuição a captura e captura de tipo maior que uma palavra (ninho por valor), nos casos presentes. O idioma de chamada imediata (`carinho(...) {...}(x)`, Fase 225) permanece não capturante mesmo referenciando escopo externo — regressão coberta em `carinho_anonimo_nao_captura_escopo_externo`.
#[test]
fn fase243_closure_captura_imutavel_aceita() {
    let code = include_str!("../examples/fase243_closure_captura_imutavel_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase243_exemplo_canonico_prova_escape_do_escopo_criador() {
    // `fabricar_somador` só constrói e retorna a closure — nunca a chama —
    // provando que a evidência de "executa depois do retorno do criador"
    // não vem de uma chamada escondida dentro do próprio criador.
    // `principal` é quem chama `somar_2`/`somar_10`, e só depois de
    // `fabricar_somador` já ter retornado (a chamada `fabricar_somador(2)`
    // completa antes de `somar_2` existir).
    let code = include_str!("../examples/fase243_closure_captura_imutavel_valido.pink");
    let start = code
        .find("carinho fabricar_somador")
        .expect("fabricar_somador presente");
    let end = code[start..]
        .find("\ncarinho principal")
        .map(|offset| start + offset)
        .expect("função principal após fabricar_somador");
    let corpo_fabricar_somador = &code[start..end];
    assert!(
        !corpo_fabricar_somador.contains("}("),
        "fabricar_somador não pode chamar a closure imediatamente após criá-la (idioma de chamada imediata da Fase 225) — só construir e retornar: {corpo_fabricar_somador}"
    );
    let corpo_principal = &code[end..];
    assert!(
        corpo_principal.contains("somar_2(40)") && corpo_principal.contains("somar_10(32)"),
        "principal deve chamar as duas closures já retornadas por fabricar_somador"
    );
}

#[test]
fn fase243_closure_captura_multipla_de_tipos_distintos_aceita() {
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom, ligado: logica, rotulo: verso) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                talvez ligado {
                    mimo base + tamanho_verso(rotulo);
                } senao {
                    mimo base;
                }
            };
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar(10, verdade, "ab");
            mimo f();
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase243_closure_rejeita_atribuicao_a_captura() {
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                base = base + 1;
                mimo base;
            };
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar(1);
            mimo f();
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("reatribuição inválida") && err.contains("'base'"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase243_closure_rejeita_captura_de_tipo_maior_que_uma_palavra() {
    let code = r#"
        pacote main;
        ninho Par { a: bombom; }
        carinho fabricar(p: Par) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                mimo p.a;
            };
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("captura de 'p'") && err.contains("não suportada nesta fase"),
        "erro inesperado: {err}"
    );
}

#[test]
fn fase243_closure_parametro_sombreia_captura_aceita() {
    // `base` é parâmetro da closure, não captura: a closure não depende do
    // `base` externo (18) — usa exclusivamente o próprio parâmetro (5).
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho(bombom) -> bombom {
            mimo carinho(base: bombom) -> bombom {
                mimo base;
            };
        }
        carinho principal() -> bombom {
            nova f: carinho(bombom) -> bombom = fabricar(18);
            mimo f(5);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase243_closure_local_sombreia_captura_aceita() {
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                nova base: bombom = base + 1000;
                mimo base;
            };
        }
        carinho principal() -> bombom {
            nova f: carinho() -> bombom = fabricar(1);
            mimo f();
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase243_closure_aninhada_captura_do_escopo_avo() {
    let code = r#"
        pacote main;
        carinho fabricar(base: bombom) -> carinho() -> carinho() -> bombom {
            mimo carinho() -> carinho() -> bombom {
                mimo carinho() -> bombom {
                    mimo base;
                };
            };
        }
        carinho principal() -> bombom {
            nova externa: carinho() -> carinho() -> bombom = fabricar(7);
            nova interna: carinho() -> bombom = externa();
            mimo interna();
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase243_closure_passada_como_argumento_aceita() {
    let code = r#"
        pacote main;
        carinho aplicar(f: carinho() -> bombom) -> bombom {
            mimo f();
        }
        carinho fabricar(base: bombom) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                mimo base;
            };
        }
        carinho principal() -> bombom {
            mimo aplicar(fabricar(9));
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase244_followup_closure_aceita_handle_de_trato_e_callable_de_uma_palavra() {
    let code = r#"
        pacote main;
        trato Medivel { carinho medir(valor: si) -> bombom; }
        impl Medivel para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        carinho criar(valor: bombom) -> trato<Medivel> {
            mimo valor virar trato<Medivel>;
        }
        carinho fabricar(
            objeto: trato<Medivel>,
            fabrica: carinho(bombom) -> trato<Medivel>
        ) -> carinho() -> bombom {
            mimo carinho() -> bombom {
                mimo objeto.medir() + fabrica(1).medir();
            };
        }
        carinho principal() -> bombom {
            nova objeto: trato<Medivel> = 2 virar trato<Medivel>;
            nova executar: carinho() -> bombom = fabricar(objeto, criar);
            mimo executar();
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase244_followup_captura_usa_classificacao_canonica_sem_atalho_applied() {
    let source = include_str!("../src/semantic.rs");
    let start = source
        .find("fn resolve_closure_value")
        .expect("resolver de closure presente");
    let end = source[start..]
        .find("fn check_closure_function")
        .map(|offset| start + offset)
        .expect("fim do resolver de closure presente");
    let resolver = &source[start..end];
    assert!(
        resolver.contains("is_closure_environment_word"),
        "capturas devem consultar a representação canônica"
    );
    assert!(
        !resolver.contains("Type::Applied { .. } => true"),
        "não se pode admitir todo Type::Applied como captura"
    );
}

#[test]
fn fase244_followup_callable_rejeita_retorno_de_trato_nominal_incompativel() {
    let code = r#"
        pacote main;
        trato A { carinho medir(valor: si) -> bombom; }
        trato B { carinho medir(valor: si) -> bombom; }
        impl A para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        impl B para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        carinho criar_b(valor: bombom) -> trato<B> {
            mimo valor virar trato<B>;
        }
        carinho principal() -> bombom {
            nova fabrica: carinho(bombom) -> trato<A> = criar_b;
            mimo fabrica(1).medir();
        }
    "#;
    let err = parse_and_check(code)
        .expect_err("retornos de tratos nominais distintos devem ser incompatíveis")
        .to_string();
    assert!(
        err.contains("tipo de inicialização incompatível"),
        "diagnóstico inesperado: {err}"
    );
}

// @pinker-nav:end evidencia.semantica.closures-captura-imutavel

// @pinker-nav:start evidencia.semantica.tratos-e-impls
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita tratos e impls (função compatível e chamada de método, receiver explícito, resolução nominal, ninho nominal, cobertura completa, múltiplos contratos): aceita casos válidos e rejeita receiver de tipo errado, método faltante e método extra, nos casos presentes. Casos usam include_str! como exemplos observados.
#[test]
fn trato_com_funcao_compativel_e_chamada_metodo_aceito() {
    let code = include_str!("../examples/fase226_trato_metodo_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trato_exige_funcao_compativel() {
    let code = r#"
        pacote main;
        trato Dobravel {
            carinho dobrar(x: bombom) -> bombom;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("trato 'Dobravel' exige função 'dobrar' compatível"),
        "{}",
        err
    );
}

#[test]
fn fase227_impl_trato_com_receiver_explicito_aceito() {
    let code = include_str!("../examples/fase227_impl_trato_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase228_impl_resolucao_nominal_prefere_impl_a_funcao_global() {
    let code = include_str!("../examples/fase228_impl_resolucao_nominal_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase229_impl_ninho_nominal_aceito() {
    let code = include_str!("../examples/fase229_impl_ninho_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase230_impl_cobertura_completa_aceita() {
    let code = include_str!("../examples/fase230_impl_cobertura_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn fase232_impl_multiplos_contratos_aceito() {
    let code = include_str!("../examples/fase232_impl_multiplos_contratos_valido.pink");
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn impl_trato_exige_receiver_do_tipo_alvo() {
    let code = r#"
        pacote demo;
        trato Dobravel { carinho dobrar(valor: bombom) -> bombom; }
        impl Dobravel para u32 {
            carinho dobrar(valor: bombom) -> bombom { mimo valor; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("receiver do método 'dobrar' no impl 'Dobravel' para 'u32' usa 'bombom'"),
        "erro inesperado: {err}"
    );
}

#[test]
fn impl_trato_rejeita_metodo_faltante() {
    let code = r#"
        pacote demo;
        trato Aritmetico {
            carinho dobrar(valor: bombom) -> bombom;
            carinho triplicar(valor: bombom) -> bombom;
        }
        impl Aritmetico para bombom {
            carinho dobrar(valor: bombom) -> bombom { mimo valor + valor; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("não implementa método 'triplicar'"),
        "erro inesperado: {err}"
    );
}

#[test]
fn impl_trato_rejeita_metodo_extra() {
    let code = r#"
        pacote demo;
        trato Aritmetico {
            carinho dobrar(valor: bombom) -> bombom;
        }
        impl Aritmetico para bombom {
            carinho dobrar(valor: bombom) -> bombom { mimo valor + valor; }
            carinho triplicar(valor: bombom) -> bombom { mimo valor + valor + valor; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("declara método 'triplicar' que não existe no trato"),
        "erro inesperado: {err}"
    );
}

#[test]
fn impl_trato_exige_trato_declarado_antes() {
    let code = r#"
        pacote demo;
        impl Inexistente para bombom {
            carinho dobrar(valor: bombom) -> bombom { mimo valor; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("impl usa trato 'Inexistente' não declarado antes deste ponto"),
        "erro inesperado: {err}"
    );
}
// @pinker-nav:end evidencia.semantica.tratos-e-impls

// @pinker-nav:start evidencia.semantica.objetos-trato-fase244
// @pinker-nav:domain semantica
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita a semântica nominal dos objetos de trato da Fase 244: receiver contextual `si`, materialização por `virar`, object safety, impls e coerções; preserva callables compatíveis em reatribuições condicionais e rejeita braços não-callable, retornos incompatíveis, tratos distintos e igualdade pública.

#[test]
fn fase244_semantica_aceita_objetos_do_mesmo_trato_para_tipos_distintos() {
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

        impl Medivel para u64 {
            carinho medir(valor: u64) -> bombom {
                mimo 64;
            }
        }

        carinho consumir(valor: trato<Medivel>) -> bombom {
            mimo 1;
        }

        apelido ObjetoBase = trato<Medivel>;
        apelido Objeto = ObjetoBase;

        carinho criar(valor: bombom) -> Objeto {
            mimo valor virar trato<Medivel>;
        }

        carinho principal() -> bombom {
            nova a: bombom = 21;
            nova b: u64 = 42;

            nova objeto_a: trato<Medivel> =
                a virar trato<Medivel>;

            nova objeto_b: trato<Medivel> =
                b virar trato<Medivel>;
            nova aliasado: Objeto = criar(a);
            nova copia: Objeto = aliasado;

            mimo consumir(objeto_a) + consumir(objeto_b) + consumir(copia);
        }
    "#;

    assert!(
        parse_and_check(code).is_ok(),
        "dois tipos concretos devem formar o mesmo tipo nominal de objeto"
    );
}

#[test]
fn fase244_semantica_rejeita_todas_as_comparacoes_de_objetos_de_trato_por_alias() {
    for operador in ["==", "!=", "<", "<=", ">", ">="] {
        for (esquerda, direita) in [("original", "duplicado"), ("criar(9)", "criar(8)")] {
            let code = format!(
                r#"
                pacote main;

                trato Medivel {{
                    carinho medir(valor: si) -> bombom;
                }}

                impl Medivel para bombom {{
                    carinho medir(valor: bombom) -> bombom {{
                        mimo valor;
                    }}
                }}

                apelido ObjetoBase = trato<Medivel>;
                apelido Objeto = ObjetoBase;

                carinho criar(valor: bombom) -> Objeto {{
                    mimo valor virar trato<Medivel>;
                }}

                carinho principal() -> bombom {{
                    nova original: Objeto = criar(7);
                    nova duplicado: Objeto = original;
                    nova comparou: logica = {esquerda} {operador} {direita};
                    talvez comparou {{
                        mimo 1;
                    }}
                    mimo 0;
                }}
                "#
            );

            let err = parse_and_check(&code)
                .expect_err("comparação de objeto de trato deve parar na semântica");
            match err {
                pinker_v0::error::PinkerError::Semantic { msg, span } => {
                    assert!(
                        msg.contains("comparação entre objetos de trato não é suportada"),
                        "diagnóstico inesperado para '{operador}': {msg}"
                    );
                    assert_eq!(
                        span.start.line, 24,
                        "span deve apontar a expressão comparada para '{operador}'"
                    );
                }
                other => panic!("estágio inesperado para '{operador}': {other}"),
            }
        }
    }
}

#[test]
fn fase244_semantica_rejeita_trato_inexistente_como_tipo_de_objeto() {
    let code = r#"
        pacote main;

        carinho usar(valor: trato<Fantasma>) -> bombom {
            mimo 0;
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("trato inexistente deve ser recusado")
        .to_string();

    assert!(
        err.contains("trato 'Fantasma' não declarado"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_semantica_rejeita_objetificacao_de_trato_estatico_legado() {
    let code = r#"
        pacote main;

        trato Dobravel {
            carinho dobrar(valor: bombom) -> bombom;
        }

        carinho dobrar(valor: bombom) -> bombom {
            mimo valor * 2;
        }

        carinho usar(valor: trato<Dobravel>) -> bombom {
            mimo 0;
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("trato estático sem receiver `si` não deve ser objetificável")
        .to_string();

    assert!(
        err.contains("não é objetificável"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_semantica_rejeita_si_fora_da_posicao_de_receiver() {
    let code = r#"
        pacote main;

        trato Invalido {
            carinho combinar(valor: si, outro: si) -> bombom;
        }

        impl Invalido para bombom {
            carinho combinar(valor: bombom, outro: bombom) -> bombom {
                mimo valor + outro;
            }
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("`si` adicional deve violar object safety")
        .to_string();

    assert!(
        err.contains("só pode usar 'si' como primeiro parâmetro receiver"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_semantica_rejeita_si_fora_de_trato() {
    let code = r#"
        pacote main;

        carinho usar(valor: si) -> bombom {
            mimo 0;
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("`si` não é tipo nominal fora de assinatura de trato")
        .to_string();

    assert!(
        err.contains("tipo 'si' não existe"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_semantica_rejeita_materializacao_sem_impl_correspondente() {
    let code = r#"
        pacote main;

        trato Medivel {
            carinho medir(valor: si) -> bombom;
        }

        impl Medivel para u64 {
            carinho medir(valor: u64) -> bombom {
                mimo 64;
            }
        }

        carinho principal() -> bombom {
            nova origem: bombom = 1;
            nova objeto: trato<Medivel> =
                origem virar trato<Medivel>;
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("tipo concreto sem impl não deve formar objeto")
        .to_string();

    assert!(
        err.contains("não implementa o trato 'Medivel'"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_semantica_rejeita_familias_fora_de_escalar_e_ninho() {
    let cases = [
        r#"
        pacote main; trazer lista.criar;
        trato Medivel { carinho medir(valor: si) -> bombom; }
        impl Medivel para lista<bombom> {
            carinho medir(valor: lista<bombom>) -> bombom { mimo 1; }
        }
        carinho principal() -> bombom {
            nova origem: lista<bombom> = criar();
            nova objeto: trato<Medivel> = origem virar trato<Medivel>;
            mimo 0;
        }
        "#,
        r#"
        pacote main;
        leque Cor { Vermelho }
        trato Medivel { carinho medir(valor: si) -> bombom; }
        impl Medivel para Cor {
            carinho medir(valor: Cor) -> bombom { mimo 1; }
        }
        carinho principal() -> bombom {
            nova origem: Cor = Cor.Vermelho;
            nova objeto: trato<Medivel> = origem virar trato<Medivel>;
            mimo 0;
        }
        "#,
    ];

    for code in cases {
        let err = parse_and_check(code)
            .expect_err("materialização dinâmica deve rejeitar família fora do recorte")
            .to_string();
        assert!(
            err.contains("aceita tipo concreto escalar ou ninho"),
            "diagnóstico inesperado: {err}"
        );
    }
}

#[test]
fn fase244_semantica_rejeita_coercao_implicita_para_objeto_de_trato() {
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

        carinho principal() -> bombom {
            nova origem: bombom = 1;
            nova objeto: trato<Medivel> = origem;
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("materialização deve exigir `virar trato<...>`")
        .to_string();

    assert!(
        err.contains("tipo de inicialização incompatível"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_despacho_semantico_aceita_metodo_com_retorno() {
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

        carinho consultar(objeto: trato<Medivel>) -> bombom {
            mimo objeto.medir(2);
        }

        carinho principal() -> bombom {
            nova origem: bombom = 21;
            nova objeto: trato<Medivel> =
                origem virar trato<Medivel>;

            mimo consultar(objeto);
        }
    "#;

    assert!(
        parse_and_check(code).is_ok(),
        "método dinâmico com retorno deveria ser semanticamente válido"
    );
}

#[test]
fn fase244_despacho_semantico_aceita_metodo_sem_retorno() {
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

        carinho usar(objeto: trato<Observavel>) {
            objeto.observar(7);
            mimo;
        }

        carinho principal() -> bombom {
            nova origem: bombom = 35;
            nova objeto: trato<Observavel> =
                origem virar trato<Observavel>;

            usar(objeto);
            mimo 0;
        }
    "#;

    assert!(
        parse_and_check(code).is_ok(),
        "método dinâmico sem retorno deveria ser válido como comando"
    );
}

#[test]
fn fase244_despacho_semantico_aceita_forma_qualificada_sobre_objeto() {
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

        carinho consultar(objeto: trato<Medivel>) -> bombom {
            mimo Medivel.medir(objeto);
        }

        carinho principal() -> bombom {
            nova origem: bombom = 42;
            nova objeto: trato<Medivel> =
                origem virar trato<Medivel>;

            mimo consultar(objeto);
        }
    "#;

    assert!(
        parse_and_check(code).is_ok(),
        "forma qualificada deve preservar despacho dinâmico quando o receiver já é objeto"
    );
}

#[test]
fn fase244_despacho_semantico_rejeita_metodo_ausente() {
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

        carinho consultar(objeto: trato<Medivel>) -> bombom {
            mimo objeto.inexistente();
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("método ausente deve ser recusado")
        .to_string();

    assert!(
        err.contains("não existe no trato objetificável 'Medivel'"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_despacho_semantico_rejeita_aridade_invalida() {
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

        carinho consultar(objeto: trato<Medivel>) -> bombom {
            mimo objeto.medir();
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("aridade dinâmica inválida deve ser recusada")
        .to_string();

    assert!(
        err.contains("chamada dinâmica de 'Medivel.medir' com aridade inválida"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_despacho_semantico_rejeita_tipo_de_argumento() {
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

        carinho consultar(objeto: trato<Medivel>) -> bombom {
            mimo objeto.medir("dois");
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("tipo de argumento dinâmico inválido deve ser recusado")
        .to_string();

    assert!(
        err.contains("tipo inválido no argumento 1 da chamada dinâmica 'Medivel.medir'"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_despacho_semantico_rejeita_retorno_nulo_usado_como_valor() {
    let code = r#"
        pacote main;

        trato Observavel {
            carinho observar(valor: si);
        }

        impl Observavel para bombom {
            carinho observar(valor: bombom) {
                mimo;
            }
        }

        carinho consultar(objeto: trato<Observavel>) -> bombom {
            mimo objeto.observar();
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("método nulo não pode ser usado como valor")
        .to_string();

    assert!(
        err.contains("resultado de função sem retorno não pode ser retornado como valor"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_metodo_objetificavel_rejeita_parametro_multi_palavra_antes_da_ir() {
    let code = r#"
        pacote main;

        trato Invalido {
            carinho usar(valor: si, dados: [bombom; 2]) -> bombom;
        }

        impl Invalido para bombom {
            carinho usar(valor: bombom, dados: [bombom; 2]) -> bombom {
                mimo valor + dados[0];
            }
        }

        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("array multi-palavra deve ser rejeitado pela semântica")
        .to_string();
    assert!(
        err.contains("exige representação multi-palavra sem transporte nativo"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_ternario_rejeita_tratos_nominais_diferentes_na_semantica() {
    let code = r#"
        pacote main;

        trato A { carinho medir(valor: si) -> bombom; }
        trato B { carinho medir(valor: si) -> bombom; }

        impl A para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        impl B para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }

        carinho principal() -> bombom {
            nova a: trato<A> = 0 virar trato<A>;
            nova b: trato<B> = 0 virar trato<B>;
            nova invalido = verdade ? a : b;
            mimo 0;
        }
    "#;

    let err = parse_and_check(code)
        .expect_err("ternário de tratos nominais diferentes deve falhar")
        .to_string();
    assert!(
        err.contains("ramos da expressão ternária devem ter o mesmo tipo"),
        "diagnóstico inesperado: {err}"
    );
}

#[test]
fn fase244_reatribuicao_condicional_de_callable_valida_bracos_e_preserva_regressoes() {
    let valido = r#"
        pacote main;

        trato Medivel { carinho medir(valor: si) -> bombom; }
        impl Medivel para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }

        carinho um() -> trato<Medivel> { mimo 1 virar trato<Medivel>; }
        carinho dois() -> trato<Medivel> { mimo 2 virar trato<Medivel>; }
        carinho numero_um() -> bombom { mimo 10; }
        carinho numero_dois() -> bombom { mimo 20; }

        carinho principal() -> bombom {
            nova inferido_um = um;
            nova inferido_dois = dois;
            nova copia_um = inferido_um;
            nova copia_dois = inferido_dois;
            nova muda f = um;
            f = verdade ? inferido_um : inferido_dois;
            f = falso ? copia_um : copia_dois;
            f = verdade ? (falso ? um : dois) : um;
            nova muda comum: carinho() -> bombom = numero_um;
            comum = falso ? numero_um : numero_dois;
            mimo f().medir() + comum();
        }
    "#;
    parse_and_check(valido).expect("casos válidos devem passar pela semântica");

    let nominal_diferente = r#"
        pacote main;
        trato A { carinho medir(valor: si) -> bombom; }
        trato B { carinho medir(valor: si) -> bombom; }
        impl A para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        impl B para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        carinho a() -> trato<A> { mimo 1 virar trato<A>; }
        carinho b() -> trato<B> { mimo 2 virar trato<B>; }
        carinho principal() -> bombom {
            nova muda f: carinho() -> trato<A> = a;
            f = verdade ? a : b;
            mimo 0;
        }
    "#;
    let err = parse_and_check(nominal_diferente)
        .expect_err("callables com tratos nominais diferentes devem falhar")
        .to_string();
    assert!(
        err.contains("ramos da expressão ternária devem ter o mesmo tipo"),
        "{err}"
    );

    let nao_callable = r#"
        pacote main;
        carinho um() -> bombom { mimo 1; }
        carinho principal() -> bombom {
            nova muda f: carinho() -> bombom = um;
            f = verdade ? um : 2;
            mimo 0;
        }
    "#;
    let err = parse_and_check(nao_callable)
        .expect_err("um braço não-callable deve falhar")
        .to_string();
    assert!(
        err.contains("ramos da expressão ternária devem ter o mesmo tipo"),
        "{err}"
    );

    let retornos_diferentes = r#"
        pacote main;
        carinho numero() -> bombom { mimo 1; }
        carinho texto() -> verso { mimo "um"; }
        carinho principal() -> bombom {
            nova muda f: carinho() -> bombom = numero;
            f = verdade ? numero : texto;
            mimo 0;
        }
    "#;
    let err = parse_and_check(retornos_diferentes)
        .expect_err("callables com retornos estruturais diferentes devem falhar")
        .to_string();
    assert!(
        err.contains("ramos da expressão ternária devem ter o mesmo tipo"),
        "{err}"
    );

    let comparacao = r#"
        pacote main;
        trato Medivel { carinho medir(valor: si) -> bombom; }
        impl Medivel para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        carinho principal() -> bombom {
            nova a: trato<Medivel> = 1 virar trato<Medivel>;
            nova b: trato<Medivel> = 2 virar trato<Medivel>;
            nova igual = a == b;
            mimo 0;
        }
    "#;
    let err = parse_and_check(comparacao)
        .expect_err("igualdade pública de objetos de trato deve continuar rejeitada")
        .to_string();
    assert!(
        err.contains("comparação entre objetos de trato não é suportada"),
        "{err}"
    );
}

// @pinker-nav:end evidencia.semantica.objetos-trato-fase244
