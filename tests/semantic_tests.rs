mod common;

use common::parse_and_check;

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
fn ouvir_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo ouvir(); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ouvir_intrinseca_rejeita_aridade_diferente_de_zero() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo ouvir(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ouvir' com aridade inválida"));
}

#[test]
fn ouvir_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ouvir_verso()); }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ouvir_verso_ou_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ouvir_verso_ou("padrao")); }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ouvir_verso_intrinseca_rejeita_aridade_diferente_de_zero() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ouvir_verso("x")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ouvir_verso' com aridade inválida"));
}

#[test]
fn ouvir_verso_ou_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ouvir_verso_ou(7)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ouvir_verso_ou'"));
}

#[test]
fn ouvir_verso_ou_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ouvir_verso_ou("a", "b")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ouvir_verso_ou' com aridade inválida"));
}

#[test]
fn argumento_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(argumento(0)); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn argumento_intrinseca_rejeita_indice_nao_bombom() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(argumento(falso)); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'argumento'"));
}

#[test]
fn argumento_ou_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(argumento_ou(0, "anonimo")); }"#;
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
        pacote main;
        carinho principal() -> bombom {
            talvez tem_chave("--saida") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tem_chave_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            talvez tem_chave(1) { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tem_chave'"));
}

#[test]
fn pedir_argumento_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_verso(pedir_argumento("--saida", "padrao"));
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn pedir_argumento_intrinseca_rejeita_padrao_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_verso(pedir_argumento("--saida", 1));
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'pedir_argumento'"));
}

#[test]
fn ambiente_ou_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ambiente_ou("HOME", "anonimo")); }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn ambiente_ou_intrinseca_rejeita_chave_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ambiente_ou(0, "anonimo")); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ambiente_ou'"));
}

#[test]
fn buscar_contexto_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_verso(buscar_contexto("--saida", "PINKER_OUT", "padrao"));
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn buscar_contexto_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_verso(buscar_contexto("--saida", 1, "padrao"));
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'buscar_contexto'"));
}

#[test]
fn buscar_contexto_intrinseca_rejeita_terceiro_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_verso(buscar_contexto("--saida", "PINKER_OUT", 1));
        }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 3 da chamada 'buscar_contexto'"));
}

#[test]
fn caminho_existe_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            talvez caminho_existe("README.md") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn caminho_existe_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { talvez caminho_existe(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'caminho_existe'"));
}

#[test]
fn e_arquivo_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            talvez e_arquivo("README.md") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn e_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { talvez e_arquivo(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'e_arquivo'"));
}

#[test]
fn e_diretorio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            talvez e_diretorio(".") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn e_diretorio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { talvez e_diretorio(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'e_diretorio'"));
}

#[test]
fn juntar_caminho_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova p: verso = juntar_caminho(".", "README.md");
            mimo tamanho_verso(p);
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn juntar_caminho_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(juntar_caminho(".", 1)); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'juntar_caminho'"));
}

#[test]
fn tamanho_arquivo_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_arquivo("README.md"); }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn tamanho_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_arquivo(1); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tamanho_arquivo'"));
}

#[test]
fn e_vazio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            talvez e_vazio("README.md") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn e_vazio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { talvez e_vazio(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'e_vazio'"));
}

#[test]
fn criar_diretorio_intrinseca_valida_sem_declaracao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            criar_diretorio("saida");
            mimo 0;
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn criar_diretorio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            remover_arquivo("temp.txt");
            mimo 0;
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn remover_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            remover_diretorio("saida");
            mimo 0;
        }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn remover_diretorio_intrinseca_rejeita_argumento_nao_verso() {
    let source = r#"
        pacote main;
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
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(diretorio_atual()); }"#;
    assert!(parse_and_check(source).is_ok());
}

#[test]
fn diretorio_atual_intrinseca_rejeita_aridade_diferente_de_zero() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(diretorio_atual("x")); }"#;
    let err = parse_and_check(source).unwrap_err().to_string();
    assert!(err.contains("chamada de 'diretorio_atual' com aridade inválida"));
}

#[test]
fn quantos_argumentos_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo quantos_argumentos(); }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn quantos_argumentos_intrinseca_rejeita_aridade_diferente_de_zero() {
    let code = "
        pacote main;
        carinho principal() -> bombom { mimo quantos_argumentos(1); }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'quantos_argumentos' com aridade inválida"));
}

#[test]
fn tem_argumento_intrinseca_valida_sem_declaracao() {
    let code = "
        pacote main;
        carinho principal() -> bombom {
            talvez tem_argumento(0) { mimo 1; } senao { mimo 0; }
        }";
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tem_argumento_intrinseca_rejeita_indice_nao_bombom() {
    let code = "
        pacote main;
        carinho principal() -> bombom { talvez tem_argumento(falso) { mimo 1; } senao { mimo 0; } }";
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tem_argumento'"));
}

#[test]
fn tem_chave_intrinseca_rejeita_aridade_diferente_de_um() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { talvez tem_chave("--saida", "--modo") { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'tem_chave' com aridade inválida"));
}

#[test]
fn pedir_argumento_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(pedir_argumento("--saida")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'pedir_argumento' com aridade inválida"));
}

#[test]
fn tem_flag_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { talvez tem_flag("--quiet") { mimo 1; } senao { mimo 0; } }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tem_flag_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { talvez tem_flag(1) { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'tem_flag'"));
}

#[test]
fn tem_flag_intrinseca_rejeita_aridade_diferente_de_um() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { talvez tem_flag("--quiet", "--verbose") { mimo 1; } senao { mimo 0; } }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'tem_flag' com aridade inválida"));
}

#[test]
fn buscar_contexto_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo tamanho_verso(buscar_contexto("--saida", "PINKER_OUT"));
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'buscar_contexto' com aridade inválida"));
}

#[test]
fn legado_tem_argumento_nomeado_intrinseca_permanece_valido() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            talvez tem_argumento_nomeado("--saida") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

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
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            nova v: bombom = ler_arquivo(h);
            fechar(h);
            mimo v;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_verso_arquivo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            nova t: verso = ler_verso_arquivo(h);
            fechar(h);
            mimo tamanho_verso(t);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_verso_arquivo_intrinseca_rejeita_argumento_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ler_verso_arquivo("arquivo.txt")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_verso_arquivo'"));
}

#[test]
fn ler_arquivo_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova t: verso = ler_arquivo_verso("arquivo.txt");
            falar(t);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_arquivo_verso_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ler_arquivo_verso("a", "b")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'ler_arquivo_verso' com aridade inválida"));
}

#[test]
fn ler_arquivo_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(ler_arquivo_verso(1)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_arquivo_verso'"));
}

#[test]
fn arquivo_ou_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova t: verso = arquivo_ou("arquivo.txt", "padrao");
            falar(t);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn arquivo_ou_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(arquivo_ou("a")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'arquivo_ou' com aridade inválida"));
}

#[test]
fn arquivo_ou_intrinseca_rejeita_tipos_invalidos() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(arquivo_ou(1, "ok")); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'arquivo_ou'"));
}

#[test]
fn arquivo_ou_intrinseca_rejeita_padrao_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(arquivo_ou("a.txt", 7)); }"#;
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
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            escrever(h, 42);
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn escrever_intrinseca_rejeita_segundo_argumento_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            escrever(h, "texto");
            fechar(h);
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'escrever'"));
}

#[test]
fn criar_arquivo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = criar_arquivo("arquivo.txt");
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn criar_arquivo_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo criar_arquivo(1); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'criar_arquivo'"));
}

#[test]
fn abrir_anexo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
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
        pacote main;
        carinho principal() -> bombom { mimo abrir_anexo("a.txt", "b.txt"); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'abrir_anexo' com aridade inválida"));
}

#[test]
fn anexar_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            nova h: bombom = abrir("arquivo.txt");
            truncar_arquivo(h);
            fechar(h);
            mimo 0;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn truncar_arquivo_intrinseca_rejeita_argumento_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            truncar_arquivo("arquivo.txt");
            mimo 0;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'truncar_arquivo'"));
}

#[test]
fn juntar_e_tamanho_verso_intrinsecas_validas_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova a: verso = "oi";
            nova b: verso = "!";
            nova c: verso = juntar_verso(a, b);
            nova n: bombom = tamanho_verso(c);
            mimo n;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn juntar_verso_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(juntar_verso("oi", 1)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'juntar_verso'"));
}

#[test]
fn indice_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = "paz";
            nova letra: verso = indice_verso(texto, 1);
            mimo tamanho_verso(letra);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn indice_verso_rejeita_indice_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom { mimo tamanho_verso(indice_verso("oi", falso)); }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'indice_verso'"));
}

#[test]
fn contem_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = contem_verso("pinker", "ink");
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
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = contem_verso("pinker", 1);
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'contem_verso'"));
}

#[test]
fn comeca_com_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
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
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = termina_com("pinker", "ker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn termina_com_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = igual_verso("pinker", "pinker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn igual_verso_intrinseca_rejeita_primeiro_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = igual_verso(1, "pinker");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'igual_verso'"));
}

#[test]
fn vazio_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = vazio_verso("");
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn aparar_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova v: verso = aparar_verso(1);
            mimo tamanho_verso(v);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'aparar_verso'"));
}

#[test]
fn minusculo_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova v: verso = minusculo_verso("PiNkEr");
            talvez igual_verso(v, "pinker") { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn maiusculo_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova v: verso = maiusculo_verso(7);
            mimo tamanho_verso(v);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'maiusculo_verso'"));
}

#[test]
fn indice_verso_em_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova idx: bombom = indice_verso_em("pinker", "ink");
            talvez idx == 1 { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn buscar_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova idx: bombom = buscar_verso("pinker", "ink");
            talvez idx == 1 { mimo 1; } senao { mimo 0; }
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn buscar_verso_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova idx: bombom = buscar_verso("pinker", 1);
            mimo idx;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'buscar_verso'"));
}

#[test]
fn formatar_verso_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova linha: verso = formatar_verso("{}={}", "idade", 7);
            mimo tamanho_verso(linha);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn formatar_verso_intrinseca_rejeita_modelo_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova linha: verso = formatar_verso(7, "idade");
            mimo tamanho_verso(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'formatar_verso'"));
}

#[test]
fn formatar_verso_intrinseca_rejeita_argumento_nao_bombom_ou_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova linha: verso = formatar_verso("{}", falso);
            mimo tamanho_verso(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'formatar_verso'"));
}

#[test]
fn formatar_verso_intrinseca_aceita_aridade_variavel() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova linha: verso = formatar_verso("{} {} {}", 1, 2, 3);
            mimo tamanho_verso(linha);
        }"#;
    parse_and_check(code).expect("formatar_verso deve aceitar aridade variável");
}

#[test]
fn ler_linha_csv_bombom_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = ler_linha_csv_bombom("7,11,13", ",");
            mimo lista_bombom_obter(itens, 1);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_linha_csv_bombom_intrinseca_rejeita_linha_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = ler_linha_csv_bombom(7, ",");
            mimo lista_bombom_tamanho(itens);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_linha_csv_bombom'"));
}

#[test]
fn emitir_linha_csv_bombom_intrinseca_rejeita_lista_fora_do_recorte() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova linha: verso = emitir_linha_csv_bombom("7,11,13", ",");
            mimo tamanho_verso(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'emitir_linha_csv_bombom'"));
}

#[test]
fn emitir_linha_csv_bombom_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova itens: lista<bombom> = lista_bombom_criar();
            nova linha: verso = emitir_linha_csv_bombom(itens, ",", ";");
            mimo tamanho_verso(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'emitir_linha_csv_bombom' com aridade inválida"));
}

#[test]
fn ler_json_plano_bombom_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova base: mapa<verso,bombom> = mapa_verso_bombom_criar();
            mapa_verso_bombom_definir(base, "idade", 7);
            nova json: verso = emitir_json_plano_bombom(base);
            nova dados: mapa<verso,bombom> = ler_json_plano_bombom(json);
            mimo mapa_verso_bombom_obter(dados, "idade");
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn ler_json_plano_bombom_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova dados: mapa<verso,bombom> = ler_json_plano_bombom(7);
            mimo mapa_verso_bombom_tamanho(dados);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'ler_json_plano_bombom'"));
}

#[test]
fn emitir_json_plano_bombom_intrinseca_rejeita_argumento_fora_do_recorte() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova linha: verso = emitir_json_plano_bombom("nao_e_mapa");
            mimo tamanho_verso(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'emitir_json_plano_bombom'"));
}

#[test]
fn emitir_json_plano_bombom_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = mapa_verso_bombom_criar();
            nova linha: verso = emitir_json_plano_bombom(m, "extra");
            mimo tamanho_verso(linha);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'emitir_json_plano_bombom' com aridade inválida"));
}

#[test]
fn tempo_unix_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova ts: bombom = tempo_unix();
            mimo ts;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn tempo_unix_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova ts: bombom = tempo_unix(1);
            mimo ts;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'tempo_unix' com aridade inválida"));
}

#[test]
fn formatar_tempo_unix_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = formatar_tempo_unix(0);
            mimo tamanho_verso(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn formatar_tempo_unix_intrinseca_rejeita_argumento_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = formatar_tempo_unix("agora");
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'formatar_tempo_unix'"));
}

#[test]
fn executar_processo_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_processo("pinker_fase162_exit0");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_processo_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_processo("pinker_fase168_argv_um", "--modo=ok");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_processo_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_processo("a", "b", "c");
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'executar_processo' com aridade inválida"));
}

#[test]
fn executar_processo_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_processo(7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'executar_processo'"));
}

#[test]
fn executar_processo_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_processo("pinker_fase168_argv_um", 7);
            mimo codigo;
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'executar_processo'"));
}

#[test]
fn executar_com_entrada_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("pinker_fase165_stdin_ok", "rosa\n");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_com_entrada_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = executar_com_entrada("pinker_fase165_stdin_ok", "argv=ok\n", "--modo=ok");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn executar_com_entrada_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            nova codigo: bombom = pipeline_minimo("pinker_fase163_stdout_ok", "pinker_fase165_stdin_ok");
            mimo codigo;
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn pipeline_minimo_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
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
        pacote main;
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
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("pinker_fase163_stdout_ok");
            mimo tamanho_verso(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stdout_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("pinker_fase163_stdout_ok", "--alvo=rosa");
            mimo tamanho_verso(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stdout_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("a", "b", "c");
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'capturar_stdout' com aridade inválida"));
}

#[test]
fn capturar_stdout_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout(7);
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'capturar_stdout'"));
}

#[test]
fn capturar_stdout_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stdout("pinker_fase163_stdout_ok", 7);
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'capturar_stdout'"));
}

#[test]
fn capturar_stderr_intrinseca_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("pinker_fase164_stderr_ok");
            mimo tamanho_verso(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stderr_intrinseca_valida_com_argv_explicito_minimo() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("pinker_fase164_stderr_ok", "--alvo=rosa");
            mimo tamanho_verso(texto);
        }"#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn capturar_stderr_intrinseca_rejeita_aridade_invalida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("a", "b", "c");
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("chamada de 'capturar_stderr' com aridade inválida"));
}

#[test]
fn capturar_stderr_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr(7);
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'capturar_stderr'"));
}

#[test]
fn capturar_stderr_intrinseca_rejeita_segundo_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova texto: verso = capturar_stderr("pinker_fase164_stderr_ok", 7);
            mimo tamanho_verso(texto);
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 2 da chamada 'capturar_stderr'"));
}

#[test]
fn nao_vazio_verso_intrinseca_rejeita_argumento_nao_verso() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova ok: logica = nao_vazio_verso(7);
            talvez ok { mimo 1; } senao { mimo 0; }
        }"#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("tipo inválido no argumento 1 da chamada 'nao_vazio_verso'"));
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
            nova p: seta<u8> = 1;
            nova _x: u8 = *p;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("apenas 'seta<bombom>'"), "{}", err);
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
            nova p: seta<u8> = 1;
            *p = 7;
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("apenas 'seta<bombom>'"), "{}", err);
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
    assert!(err.contains("apenas 'ptr + bombom'"), "{}", err);
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
    assert!(err.contains("exige 'seta<bombom> + bombom'"), "{}", err);
}

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

// ── Fase 148: escrita por índice em array fixo [bombom; N] ───────────────────

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

// ── Fase 149: lista mínima homogênea de bombom ──────────────────────────────

#[test]
fn lista_bombom_criar_anexar_obter_valida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_bombom_criar();
            lista_bombom_anexar(l, 10);
            lista_bombom_anexar(l, 20);
            mimo lista_bombom_obter(l, 1);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_bombom_rejeita_tipo_fora_do_recorte() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<verso> = lista_bombom_criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("lista<bombom>"), "{}", err);
}

#[test]
fn lista_bombom_anexar_rejeita_valor_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_bombom_criar();
            lista_bombom_anexar(l, "oi");
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("esperado 'bombom'"), "{}", err);
}

#[test]
fn lista_bombom_definir_valida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_bombom_criar();
            lista_bombom_anexar(l, 10);
            lista_bombom_definir(l, 0, 22);
            mimo lista_bombom_obter(l, 0);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_bombom_definir_rejeita_valor_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_bombom_criar();
            lista_bombom_anexar(l, 10);
            lista_bombom_definir(l, 0, "oi");
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
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_bombom_criar();
            lista_bombom_anexar(l, 10);
            lista_bombom_anexar(l, 20);
            mimo lista_bombom_tirar_ultimo(l);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_bombom_tirar_ultimo_rejeita_argumento_fora_do_recorte() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo lista_bombom_tirar_ultimo("oi");
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(
        err.contains("argumento 1 da chamada 'lista_bombom_tirar_ultimo'"),
        "{}",
        err
    );
}

#[test]
fn mapa_verso_bombom_criar_definir_obter_tem_valida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = mapa_verso_bombom_criar();
            mapa_verso_bombom_definir(m, "idade", 7);
            talvez mapa_verso_bombom_tem(m, "idade") {
                mimo mapa_verso_bombom_obter(m, "idade");
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn mapa_verso_bombom_rejeita_tipo_fora_do_recorte() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova m: mapa<bombom,bombom> = mapa_verso_bombom_criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("mapa<verso,bombom>"), "{}", err);
}

#[test]
fn mapa_verso_bombom_definir_rejeita_valor_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = mapa_verso_bombom_criar();
            mapa_verso_bombom_definir(m, "idade", "sete");
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
        pacote main;
        carinho principal() -> bombom {
            nova m: mapa<verso,bombom> = mapa_verso_bombom_criar();
            mapa_verso_bombom_definir(m, "idade", 7);
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

#[test]
fn aleatorio_basico_com_semente_explicita_valida_sem_declaracao() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova gerador: bombom = aleatorio_criar(42);
            nova valor: bombom = aleatorio_proximo(gerador);
            mimo valor;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn aleatorio_criar_rejeita_semente_nao_bombom() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova gerador: bombom = aleatorio_criar("oi");
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
        pacote main;
        carinho principal() -> bombom {
            nova gerador: bombom = aleatorio_criar(7);
            mimo aleatorio_proximo(gerador, 1);
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

// ── Fases 186–189 — importação por família: `tempo`, `ambiente`, `acaso` e `texto` ──

#[test]
fn trazer_tempo_familia_aceita() {
    let code = r#"
        pacote main;
        trazer tempo;
        carinho principal() -> bombom {
            nova agora: bombom = tempo_unix();
            mimo agora;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_ambiente_familia_aceita() {
    let code = r#"
        pacote main;
        trazer ambiente;
        carinho principal() -> bombom {
            nova saida: verso = buscar_contexto("--saida", "PINKER_SAIDA", "padrao.txt");
            nova origem: verso = ambiente_ou("HOME", "/tmp");
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
        pacote main;
        trazer acaso;
        carinho principal() -> bombom {
            nova gerador: bombom = aleatorio_criar(42);
            nova valor: bombom = aleatorio_proximo(gerador);
            mimo valor;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_tempo_sem_trazer_continua_valido() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova agora: bombom = tempo_unix();
            nova texto: verso = formatar_tempo_unix(agora);
            falar(texto);
            mimo agora;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_ambiente_sem_trazer_continua_valido() {
    let code = r#"
        pacote main;
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
        pacote main;
        carinho principal() -> bombom {
            nova gerador: bombom = aleatorio_criar(7);
            nova valor: bombom = aleatorio_proximo(gerador);
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
        pacote main;
        trazer texto;
        carinho principal() -> bombom {
            nova saudacao: verso = juntar_verso("rosa", " pinker");
            nova limpa: verso = aparar_verso("  texto  ");
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
        pacote main;
        carinho principal() -> bombom {
            nova saudacao: verso = juntar_verso("rosa", " pinker");
            nova n: bombom = tamanho_verso(saudacao);
            falar(saudacao);
            mimo n;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_seletivo_texto_nao_suportado_falha() {
    let code = r#"
        pacote main;
        trazer texto.juntar_verso;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("importação seletiva"), "{}", err);
}

#[test]
fn trazer_seletivo_nao_suportado_falha() {
    let code = r#"
        pacote main;
        trazer tempo.tempo_unix;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("importação seletiva"), "{}", err);
}

#[test]
fn trazer_arquivo_familia_aceita() {
    let code = r#"
        pacote main;
        trazer arquivo;
        carinho principal() -> bombom {
            nova cabo: bombom = criar_arquivo("target/teste_trazer_arquivo.txt");
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
        pacote main;
        trazer caminho;
        carinho principal() -> bombom {
            nova destino: verso = juntar_caminho("docs", "atlas.md");
            talvez caminho_existe(destino) {
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
        pacote main;
        trazer processo;
        carinho principal() -> bombom {
            nova comando: verso = argumento(0);
            nova codigo: bombom = executar_processo(comando);
            mimo codigo;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_arquivo_sem_trazer_continua_valido() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova conteudo: verso = ler_arquivo_verso("Cargo.toml");
            nova n: bombom = tamanho_verso(conteudo);
            mimo n;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn legado_global_caminho_sem_trazer_continua_valido() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova destino: verso = juntar_caminho("docs", "atlas.md");
            talvez caminho_existe(destino) {
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
        pacote main;
        carinho principal() -> bombom {
            nova comando: verso = argumento(0);
            nova codigo: bombom = executar_processo(comando);
            mimo codigo;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn trazer_seletivo_arquivo_nao_suportado_falha() {
    let code = r#"
        pacote main;
        trazer arquivo.criar_arquivo;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("importação seletiva"), "{}", err);
}

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
    assert!(
        err.contains("deve ser 'bombom', 'verso' ou um leque declarado"),
        "{}",
        err
    );
}

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
        err.contains("deve ser 'bombom', 'verso' ou um leque declarado"),
        "{}",
        err
    );
}

#[test]
fn lista_generica_de_leque_aceita() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = lista_criar();
            lista_anexar(cores, Cor.Vermelho);
            nova primeira: Cor = lista_obter(cores, 0);
            talvez primeira == Cor.Vermelho {
                mimo lista_tamanho(cores);
            }
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_generica_como_parametro_e_retorno_aceita() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho fabrica() -> lista<Cor> {
            nova cores: lista<Cor> = lista_criar();
            lista_anexar(cores, Cor.Verde);
            mimo cores;
        }
        carinho conta(cores: lista<Cor>) -> bombom {
            mimo lista_tamanho(cores);
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
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = lista_criar();
            lista_anexar(cores, Cor.Verde);
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
        pacote main;
        leque Cor { Vermelho, Verde }
        leque Fruta { Banana, Maca }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = lista_criar();
            lista_anexar(cores, Fruta.Banana);
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("exige elemento"), "{}", err);
}

#[test]
fn lista_generica_de_nao_leque_rejeitada() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova coisas: lista<Fantasma> = lista_criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("não é um leque"), "{}", err);
}

#[test]
fn lista_criar_sem_anotacao_rejeitado() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova coisas = lista_criar();
            mimo 0;
        }
    "#;
    let err = parse_and_check(code).unwrap_err().to_string();
    assert!(err.contains("anotação de tipo"), "{}", err);
}

#[test]
fn lista_criar_fora_de_nova_rejeitado() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo lista_tamanho(lista_criar());
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

#[test]
fn intrinsecas_genericas_sobre_listas_legadas_aceitas() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova numeros: lista<bombom> = lista_criar();
            lista_anexar(numeros, 7);
            nova palavras: lista<verso> = lista_criar();
            lista_anexar(palavras, "rosa");
            nova p: verso = lista_obter(palavras, 0);
            falar(p);
            mimo lista_obter(numeros, 0) + lista_tamanho(palavras);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_legada_monomorphizada_continua_valida() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova numeros: lista<bombom> = lista_bombom_criar();
            lista_bombom_anexar(numeros, 7);
            mimo lista_bombom_obter(numeros, 0);
        }
    "#;
    assert!(parse_and_check(code).is_ok());
}

#[test]
fn lista_generica_nao_aceita_intrinseca_monomorphizada_de_bombom() {
    let code = r#"
        pacote main;
        leque Cor { Vermelho, Verde }
        carinho principal() -> bombom {
            nova cores: lista<Cor> = lista_criar();
            lista_bombom_anexar(cores, 1);
            mimo 0;
        }
    "#;
    assert!(parse_and_check(code).is_err());
}

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

#[test]
fn carinho_anonimo_nao_capturante_aceito() {
    let code = include_str!("../examples/fase225_carinho_anonimo_valido.pink");
    assert!(parse_and_check(code).is_ok());
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
        err.contains("exige primeiro parâmetro do método com tipo 'u32'"),
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
