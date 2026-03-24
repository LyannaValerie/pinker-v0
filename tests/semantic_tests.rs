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
