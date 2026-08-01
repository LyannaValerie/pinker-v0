//! D1 — listas como cargas tipadas de variantes de `leque`.
//!
//! Três contratos são guardados aqui, e nenhum deles se reduz aos outros:
//!
//! 1. **Representação**: uma lista guardada numa variante continua sendo um
//!    handle opaco de uma palavra, copiado de forma rasa. O que entra e o que
//!    sai é o mesmo handle lógico, observável pelos aliases.
//! 2. **Identidade**: `lista<bombom>`, `lista<verso>`, `lista<Cor>` e
//!    `lista<Token>` compartilham a classe operacional e são quatro tipos
//!    distintos. A validade de uma carga nunca é decidida por `TypeIR`.
//! 3. **Paridade**: interpretador e nativo produzem o mesmo stdout, o mesmo
//!    exit e o mesmo braço selecionado, reutilizando os símbolos de runtime já
//!    existentes — sem mudança de ABI.

mod common;

use common::parse_and_check;
use pinker_v0::enum_payload::{self, EnumPayloadClass};
use pinker_v0::ir::{self, EnumPayloadMetaIR, ProgramIR, TypeIR};
use pinker_v0::{ir_validate, semantic};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Auxiliares
// ---------------------------------------------------------------------------

fn lower(code: &str) -> ProgramIR {
    let program = common::parse(code).expect("parse");
    semantic::check_program(&program).expect("semântica");
    let program_ir = ir::lower_program(&program).expect("lowering");
    ir_validate::validate_program(&program_ir).expect("validação de IR");
    program_ir
}

fn carga(program: &ProgramIR, leque: &str, variante: &str, indice: usize) -> EnumPayloadMetaIR {
    program
        .enum_variants
        .iter()
        .find(|meta| meta.enum_name == leque && meta.variant_name == variante)
        .unwrap_or_else(|| panic!("variante '{leque}.{variante}' ausente da metadata"))
        .payloads
        .get(indice)
        .unwrap_or_else(|| panic!("carga {indice} ausente em '{leque}.{variante}'"))
        .clone()
}

fn recusa(code: &str) -> String {
    parse_and_check(code)
        .expect_err("o programa deveria ser recusado")
        .to_string()
}

fn interpretado(exemplo: &str) -> (Vec<String>, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", exemplo])
        .output()
        .expect("execução do interpretador");
    assert!(
        output.stderr.is_empty(),
        "stderr interpretado deveria ser vazio: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let linhas = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    (linhas, output.status.code())
}

fn nativo(exemplo: &str) -> Option<(Vec<String>, Option<i32>)> {
    let (_driver, Some(runtime_lib)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)?
    else {
        return None;
    };
    let pink = env!("CARGO_BIN_EXE_pink");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_d1_{}_{nanos}", std::process::id()));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou para {exemplo}: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nome = std::path::Path::new(exemplo)
        .file_stem()
        .expect("nome do exemplo");
    let run = Command::new(out_dir.join(nome))
        .output()
        .expect("falha ao executar binário nativo");
    assert!(
        run.stderr.is_empty(),
        "stderr nativo deveria ser vazio: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let linhas = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    let codigo = run.status.code();
    let _ = std::fs::remove_dir_all(&out_dir);
    Some((linhas, codigo))
}

fn paridade(exemplo: &str, esperado: &[&str]) {
    let (linhas_interpretadas, exit_interpretado) = interpretado(exemplo);
    assert_eq!(
        linhas_interpretadas, esperado,
        "stdout interpretado divergente em {exemplo}"
    );
    let Some((linhas_nativas, exit_nativo)) = nativo(exemplo) else {
        return;
    };
    assert_eq!(
        linhas_nativas, esperado,
        "stdout nativo divergente em {exemplo}"
    );
    assert_eq!(
        exit_interpretado, exit_nativo,
        "os dois backends devem concordar no exit de {exemplo}"
    );
}

// @pinker-nav:start evidencia.leques.carga-lista-matriz-positiva
// @pinker-nav:domain leques
// @pinker-nav:layer evidencia
// @pinker-nav:summary Matriz positiva de cargas de lista com paridade entre interpretador e binário nativo: listas vazias, de um e de vários elementos, `lista<verso>`, listas de leque sem e com carga, apelidos simples e encadeados, especializações genéricas monomorfizadas, variantes com cargas misturadas, leque recursivo por `lista<si>`, passagem e retorno da lista extraída, mutação antes, depois e pelo binding, iteração, e instâncias independentes da mesma variante.

/// Matriz positiva principal: representação, aliasing raso e independência
/// entre instâncias, com o mesmo stdout nos dois backends.
#[test]
fn matriz_positiva_de_cargas_de_lista_tem_paridade() {
    paridade(
        "examples/d1_leque_carga_lista_valido.pink",
        &[
            "0", "2", "42", "129", "3", "3", "6", "2", "azul", "rosa", "azul", "alfa", "anonimo",
            "7", "3", "fim", "2", "9", "10", "2",
        ],
    );
}

/// Apelidos (simples e encadeados), especializações genéricas com listas de
/// escalar, texto e leque, e recursão por `lista<Arvore>`.
#[test]
fn apelidos_genericos_e_recursao_por_lista_tem_paridade() {
    paridade(
        "examples/d1_leque_carga_lista_alias_generico_valido.pink",
        &[
            "2", "5", "azul", "rosa", "2", "eco", "2", "2", "eco", "2", "21",
        ],
    );
}

/// Construção e extração fora de `principal`: função auxiliar, parâmetro,
/// retorno, chamada direta e chamada indireta por função local tipada.
#[test]
fn construcao_e_extracao_em_funcoes_tem_paridade() {
    paridade(
        "examples/d1_leque_carga_lista_funcoes_valido.pink",
        &["2", "3", "4", "3", "1", "rosa"],
    );
}

/// A cópia é rasa **por contrato**: a variante guarda o handle, e as três
/// janelas de mutação (antes, depois e pelo binding extraído) são observadas
/// por todos os aliases do mesmo handle.
#[test]
fn copia_do_handle_e_rasa_e_visivel_pelos_aliases() {
    let code = r#"
        pacote main;
        leque Caixa { Valores(lista<bombom>) }
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_criar();
            lista_anexar(l, 1);
            nova c: Caixa = Caixa.Valores(l);
            lista_anexar(l, 2);
            encaixe c {
                caso Caixa.Valores(v) {
                    lista_anexar(v, 3);
                    falar(lista_tamanho(v));
                }
            }
            falar(lista_tamanho(l));
            mimo 0;
        }
    "#;
    let saida = executa_fonte(code);
    assert_eq!(saida, vec!["3".to_string(), "3".to_string()]);
}

/// Uma lista vazia criada por contexto continua vazia depois de atravessar a
/// variante: nenhuma cópia profunda é materializada no caminho.
#[test]
fn lista_vazia_atravessa_a_variante_sem_materializacao() {
    let code = r#"
        pacote main;
        leque Caixa { Valores(lista<bombom>) }
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_criar();
            nova c: Caixa = Caixa.Valores(l);
            encaixe c {
                caso Caixa.Valores(v) { falar(lista_tamanho(v)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(executa_fonte(code), vec!["0".to_string()]);
}
// @pinker-nav:end evidencia.leques.carga-lista-matriz-positiva

/// Executa um fonte pelo interpretador, gravando-o num arquivo temporário para
/// usar exatamente o mesmo caminho de CLI dos exemplos versionados.
fn executa_fonte(code: &str) -> Vec<String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let caminho = std::env::temp_dir().join(format!(
        "pinker_d1_fonte_{}_{nanos}.pink",
        std::process::id()
    ));
    std::fs::write(&caminho, code).expect("gravar fonte temporário");
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&caminho)
        .output()
        .expect("execução do interpretador");
    let _ = std::fs::remove_file(&caminho);
    assert!(
        output.status.success(),
        "execução falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

// @pinker-nav:start evidencia.leques.carga-lista-matriz-negativa
// @pinker-nav:domain leques
// @pinker-nav:layer evidencia
// @pinker-nav:summary Matriz negativa das cargas de lista: os tipos que continuam fora do contrato (`mapa<K,V>`, `seta<T>`, `ninho`, array fixo, função, objeto de trato, união estrutural, `nulo`, genérico não resolvido, tipo inexistente, aridade genérica errada), as trocas de identidade entre listas de mesma representação, a aridade de cargas e de bindings, a variante e o leque inexistentes, o argumento sem valor e o uso do binding por operação de lista incompatível — cada um com diagnóstico estável.

/// Os tipos fora do contrato são recusados com o código estável e com a
/// descrição fiel do contrato — nunca com a enumeração antiga.
#[test]
fn tipos_fora_do_contrato_continuam_recusados() {
    let casos: [(&str, &str); 10] = [
        ("mapa<verso, bombom>", "mapa"),
        ("seta<bombom>", "seta"),
        ("Ninho", "ninho"),
        ("[bombom; 3]", "array fixo"),
        ("carinho(bombom) -> bombom", "função"),
        ("trato<Falante>", "objeto de trato"),
        ("uniao<bombom, verso>", "união estrutural"),
        ("nulo", "nulo"),
        ("T", "genérico não resolvido"),
        ("Fantasma", "tipo inexistente"),
    ];
    for (tipo, rotulo) in casos {
        let code = format!(
            r#"
            pacote main;
            ninho Ninho {{ a: bombom; }}
            trato Falante {{ carinho falar_algo(self: bombom) -> bombom; }}
            carinho falar_algo(self: bombom) -> bombom {{ mimo self; }}
            leque Alvo {{ X({tipo}) }}
            carinho principal() -> bombom {{ mimo 0; }}
        "#
        );
        let err = recusa(&code);
        assert!(
            err.contains(enum_payload::CONTRATO_CARGAS),
            "{rotulo}: a mensagem deve descrever o contrato atualizado: {err}"
        );
        assert!(
            err.contains("E-SEMANTIC-ENUM-PAYLOAD-"),
            "{rotulo}: a mensagem deve carregar o código estável: {err}"
        );
    }
}

/// A mensagem antiga, que enumerava apenas `'bombom', 'verso' ou leque`, não
/// pode voltar: ela deixou de ser fiel quando as listas entraram no contrato.
#[test]
fn mensagem_antiga_do_contrato_nao_reaparece() {
    let code = r#"
        pacote main;
        leque Alvo { X(logica) }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(
        !err.contains("deve ser 'bombom', 'verso' ou um leque declarado"),
        "{err}"
    );
}

/// Duas listas de leques diferentes têm a mesma representação e identidades
/// distintas: a construção recusa a troca, e o diagnóstico nomeia os dois
/// elementos em vez de imprimir `lista<leque>` dos dois lados.
#[test]
fn lista_de_leque_nao_aceita_lista_de_outro_leque() {
    let code = r#"
        pacote main;
        leque Cor { Rosa }
        leque Token { Fim }
        leque A { X(lista<Cor>) }
        carinho principal() -> bombom {
            nova tokens: lista<Token> = lista_criar();
            nova valor: A = A.X(tokens);
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(err.contains("esperado 'lista<Cor>'"), "{err}");
    assert!(err.contains("encontrado 'lista<Token>'"), "{err}");
}

/// `lista<verso>` não é `lista<bombom>` — nem `verso`.
#[test]
fn lista_de_bombom_nao_aceita_lista_de_verso() {
    let code = r#"
        pacote main;
        leque Pacote { Numeros(lista<bombom>) }
        carinho principal() -> bombom {
            nova textos: lista<verso> = lista_criar();
            nova p: Pacote = Pacote.Numeros(textos);
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(err.contains("esperado 'lista<bombom>'"), "{err}");
    assert!(err.contains("encontrado 'lista<verso>'"), "{err}");
}

/// Um apelido nominal errado no elemento não vira uma lista nova: o alvo é que
/// decide a identidade.
#[test]
fn lista_de_apelido_nominal_incorreto_e_recusada() {
    let code = r#"
        pacote main;
        leque Cor { Rosa }
        apelido CorAlias = Cor;
        apelido Errado = bombom;
        leque A { X(lista<CorAlias>) }
        carinho principal() -> bombom {
            nova ns: lista<Errado> = lista_criar();
            nova valor: A = A.X(ns);
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(err.contains("esperado 'lista<Cor>'"), "{err}");
}

/// Um argumento sem valor não pode ser carga.
#[test]
fn argumento_sem_valor_nao_pode_ser_carga() {
    let code = r#"
        pacote main;
        leque Pacote { Numeros(lista<bombom>) }
        carinho nada() -> nulo { mimo; }
        carinho principal() -> bombom {
            nova p: Pacote = Pacote.Numeros(nada());
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(!err.is_empty(), "deveria haver diagnóstico");
}

/// Quantidade errada de cargas na construção.
#[test]
fn quantidade_errada_de_cargas_e_recusada() {
    let code = r#"
        pacote main;
        leque Evento { Dados(bombom, lista<bombom>, verso) }
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_criar();
            nova ev: Evento = Evento.Dados(1, l);
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(err.contains("argumento(s) de carga"), "{err}");
}

/// Variante inexistente e leque inexistente continuam recusados.
#[test]
fn variante_e_leque_inexistentes_sao_recusados() {
    let variante = r#"
        pacote main;
        leque Pacote { Numeros(lista<bombom>) }
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_criar();
            nova p: Pacote = Pacote.Ausente(l);
            mimo 0;
        }
    "#;
    assert!(recusa(variante).contains("Ausente"), "{}", recusa(variante));

    let leque = r#"
        pacote main;
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_criar();
            nova p: bombom = Inexistente.X(l);
            mimo 0;
        }
    "#;
    assert!(!recusa(leque).is_empty());
}

/// Extração com quantidade errada de bindings.
#[test]
fn extracao_com_aridade_errada_de_bindings_e_recusada() {
    let code = r#"
        pacote main;
        leque Evento { Dados(bombom, lista<bombom>, verso), Encerrar }
        carinho principal() -> bombom {
            nova l: lista<bombom> = lista_criar();
            nova ev: Evento = Evento.Dados(1, l, "ok");
            encaixe ev {
                caso Evento.Dados(n, x) { falar(n); }
                caso Evento.Encerrar { falar(0); }
            }
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(err.contains("liga 2 nome(s)"), "{err}");
}

/// O binding extraído conserva o tipo exato da carga: uma operação de lista
/// incompatível é recusada pelo tipo do binding, não pela representação.
#[test]
fn binding_extraido_recusa_operacao_de_lista_incompativel() {
    let code = r#"
        pacote main;
        leque Pacote { Textos(lista<verso>) }
        carinho principal() -> bombom {
            nova t: lista<verso> = lista_criar();
            nova p: Pacote = Pacote.Textos(t);
            encaixe p {
                caso Pacote.Textos(v) { lista_anexar(v, 42); }
            }
            mimo 0;
        }
    "#;
    let err = recusa(code);
    assert!(err.contains("exige elemento 'verso'"), "{err}");
}

/// Especialização genérica com aridade errada.
#[test]
fn especializacao_generica_com_aridade_errada_e_recusada() {
    let code = r#"
        pacote main;
        leque Opcao<T> { Algum(T), Nenhum }
        apelido Errada = Opcao<lista<bombom>, lista<verso>>;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(err.contains("argumento(s) de tipo"), "{err}");
}
// @pinker-nav:end evidencia.leques.carga-lista-matriz-negativa

// @pinker-nav:start evidencia.leques.carga-lista-estrutura-ir
// @pinker-nav:domain leques
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência estrutural sobre a IR das cargas de lista: a metadata publicada conserva representação operacional e identidade semântica ao mesmo tempo, `lista<Cor>` e `lista<Token>` não compartilham identidade, apelidos convergem para a identidade do alvo, nenhum parâmetro genérico residual sobrevive à monomorfização, o helper de runtime deriva da classe de representação, listas não são encaminhadas pelo helper de `verso`, e o validador de IR recusa metadata fabricada inconsistente.

/// A carga conserva as duas dimensões: representação operacional de uma
/// palavra **e** identidade semântica resolvida com o elemento nomeado.
#[test]
fn metadata_conserva_representacao_e_identidade() {
    let program = lower(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque Pacote { Numeros(lista<bombom>), Textos(lista<verso>), Cores(lista<Cor>) }
        carinho principal() -> bombom { mimo 0; }
    "#,
    );

    let numeros = carga(&program, "Pacote", "Numeros", 0);
    assert_eq!(numeros.operational_type, TypeIR::ListBombom);
    assert_eq!(numeros.class, EnumPayloadClass::OpaqueWordHandle);
    assert_eq!(numeros.canonical_key, "lista<bombom>");
    assert!(numeros.element_type_id.is_none());

    let textos = carga(&program, "Pacote", "Textos", 0);
    assert_eq!(textos.operational_type, TypeIR::ListVerso);
    assert_eq!(textos.class, EnumPayloadClass::OpaqueWordHandle);
    assert_eq!(textos.canonical_key, "lista<verso>");

    let cores = carga(&program, "Pacote", "Cores", 0);
    // Mesma representação de `lista<bombom>`, identidade diferente, e a
    // identidade concreta do elemento presente.
    assert_eq!(cores.operational_type, TypeIR::ListBombom);
    assert_eq!(cores.canonical_key, "lista<leque>:3:Cor");
    let elemento = cores.element_type_id.expect("identidade do elemento");
    let entrada = program
        .resolved_types
        .iter()
        .find(|entry| entry.id == elemento)
        .expect("elemento internado");
    assert_eq!(entrada.nominal_name.as_deref(), Some("Cor"));
}

/// `lista<Cor>` e `lista<Token>` não compartilham identidade — e nenhuma das
/// duas compartilha identidade com `lista<bombom>`.
#[test]
fn listas_de_leques_diferentes_nao_compartilham_identidade() {
    let program = lower(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque Token { Fim }
        leque A { X(lista<Cor>), Y(lista<Token>), Z(lista<bombom>) }
        carinho principal() -> bombom { mimo 0; }
    "#,
    );
    let x = carga(&program, "A", "X", 0);
    let y = carga(&program, "A", "Y", 0);
    let z = carga(&program, "A", "Z", 0);
    assert_eq!(x.operational_type, y.operational_type);
    assert_eq!(x.operational_type, z.operational_type);
    assert_ne!(x.resolved_type_id, y.resolved_type_id);
    assert_ne!(x.resolved_type_id, z.resolved_type_id);
    assert_ne!(y.resolved_type_id, z.resolved_type_id);
    assert_ne!(x.canonical_key, y.canonical_key);
}

/// Apelidos — simples e encadeados, inclusive no elemento — convergem para a
/// identidade do alvo.
#[test]
fn apelidos_convergem_para_a_identidade_do_alvo() {
    let program = lower(
        r#"
        pacote main;
        leque Cor { Rosa }
        apelido CorAlias = Cor;
        apelido ListaCor = lista<CorAlias>;
        apelido ListaCor2 = ListaCor;
        apelido Numeros = lista<bombom>;
        leque Caixa { Direta(lista<Cor>), PorAlias(ListaCor2), Ns(Numeros), Diretos(lista<bombom>) }
        carinho principal() -> bombom { mimo 0; }
    "#,
    );
    let direta = carga(&program, "Caixa", "Direta", 0);
    let por_alias = carga(&program, "Caixa", "PorAlias", 0);
    assert_eq!(direta.resolved_type_id, por_alias.resolved_type_id);
    assert_eq!(direta.canonical_key, "lista<leque>:3:Cor");

    let ns = carga(&program, "Caixa", "Ns", 0);
    let diretos = carga(&program, "Caixa", "Diretos", 0);
    assert_eq!(ns.resolved_type_id, diretos.resolved_type_id);
    assert_eq!(ns.canonical_key, "lista<bombom>");
}

/// Depois da monomorfização, nenhuma carga carrega parâmetro genérico
/// residual: nem na chave canônica, nem como apelido não resolvido.
#[test]
fn metadata_nao_contem_parametro_generico_residual() {
    let program = lower(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque Resultado2<T, E> { Ok(T), Erro(E) }
        apelido RLista = Resultado2<lista<bombom>, lista<verso>>;
        apelido RCores = Resultado2<lista<Cor>, verso>;
        carinho principal() -> bombom {
            nova ns: lista<bombom> = lista_criar();
            nova r: RLista = RLista.Ok(ns);
            encaixe r {
                caso RLista.Ok(v) { falar(lista_tamanho(v)); }
                caso RLista.Erro(e) { falar(lista_tamanho(e)); }
            }
            mimo 0;
        }
    "#,
    );
    for variante in &program.enum_variants {
        for payload in &variante.payloads {
            assert!(
                !payload.canonical_key.starts_with('?'),
                "identidade não resolvida em {}.{}: {}",
                variante.enum_name,
                variante.variant_name,
                payload.canonical_key
            );
            for parametro in ["T", "E"] {
                assert!(
                    payload.canonical_key != parametro
                        && !payload.canonical_key.contains(&format!(":{parametro}")),
                    "parâmetro genérico residual em {}.{}: {}",
                    variante.enum_name,
                    variante.variant_name,
                    payload.canonical_key
                );
            }
        }
    }
    // As especializações concretas existem e carregam os tipos substituídos em
    // profundidade.
    let ok = program
        .enum_variants
        .iter()
        .find(|meta| meta.variant_name == "Ok" && meta.enum_name.contains("Resultado2"))
        .expect("especialização de Ok");
    assert!(ok.payloads.iter().all(
        |p| p.class == EnumPayloadClass::OpaqueWordHandle || p.class == EnumPayloadClass::Verso
    ));
}

/// A substituição também atravessa uma lista declarada no template genérico:
/// remover o braço `ListEnum` de `substitute_type` deixa `lista<T>` residual.
#[test]
fn substituicao_generica_atravessa_lista_do_template() {
    let program = lower(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque Caixa<T> { Muitos(lista<T>) }
        apelido CaixaBombons = Caixa<bombom>;
        apelido CaixaCores = Caixa<Cor>;
        carinho principal() -> bombom { mimo 0; }
    "#,
    );
    let keys: Vec<_> = program
        .enum_variants
        .iter()
        .filter(|meta| meta.variant_name == "Muitos")
        .map(|meta| meta.payloads[0].canonical_key.as_str())
        .collect();
    assert!(keys.contains(&"lista<bombom>"), "{keys:?}");
    assert!(keys.contains(&"lista<leque>:3:Cor"), "{keys:?}");
    assert!(keys.iter().all(|key| !key.contains(":T")), "{keys:?}");
}

/// A escolha do helper de runtime deriva da classe de representação, e nunca
/// de um `match` parcial sobre o tipo-fonte.
#[test]
fn helper_de_runtime_deriva_da_classe_de_representacao() {
    use pinker_v0::ast::Type;
    use pinker_v0::token::{Position, Span};
    use std::collections::{HashMap, HashSet};

    let span = Span::single(Position::new(1, 1));
    let mut enums = HashSet::new();
    enums.insert("Cor".to_string());
    let aliases: HashMap<String, Type> = HashMap::new();
    let structs: HashSet<String> = HashSet::new();

    let casos: [(Type, EnumPayloadClass, &str, &str); 5] = [
        (
            Type::Bombom(span),
            EnumPayloadClass::ImmediateDiscriminant,
            enum_payload::ANEXAR_IMEDIATO,
            enum_payload::CARGA_IMEDIATO,
        ),
        (
            Type::Verso(span),
            EnumPayloadClass::Verso,
            enum_payload::ANEXAR_VERSO,
            enum_payload::CARGA_VERSO,
        ),
        (
            Type::ListBombom(span),
            EnumPayloadClass::OpaqueWordHandle,
            enum_payload::ANEXAR_LISTA_BOMBOM,
            enum_payload::CARGA_LISTA_BOMBOM,
        ),
        (
            Type::ListVerso(span),
            EnumPayloadClass::OpaqueWordHandle,
            enum_payload::ANEXAR_LISTA_VERSO,
            enum_payload::CARGA_LISTA_VERSO,
        ),
        (
            Type::ListEnum {
                element: "Cor".to_string(),
                span,
            },
            EnumPayloadClass::OpaqueWordHandle,
            enum_payload::ANEXAR_LISTA_BOMBOM,
            enum_payload::CARGA_LISTA_BOMBOM,
        ),
    ];

    for (ty, classe, anexar, extrair) in casos {
        let shape = enum_payload::classify_enum_payload(&ty, &aliases, &enums, &structs)
            .expect("classificação");
        assert_eq!(shape.class, classe, "classe de {}", ty.display_name());
        assert_eq!(shape.anexar_intrinsic(), anexar);
        assert_eq!(shape.carga_intrinsic(), extrair);
    }
}

/// Listas nunca são encaminhadas pelo helper de `verso`: nem `lista<verso>`,
/// que compartilha o elemento textual.
#[test]
fn listas_nao_usam_o_helper_de_verso() {
    let ir_texto = common::render_ir(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque P { N(lista<bombom>), T(lista<verso>), C(lista<Cor>), S(verso) }
        carinho principal() -> bombom {
            nova t: lista<verso> = lista_criar();
            nova p: P = P.T(t);
            encaixe p {
                caso P.N(v) { falar(lista_tamanho(v)); }
                caso P.T(v) { falar(lista_tamanho(v)); }
                caso P.C(v) { falar(lista_tamanho(v)); }
                caso P.S(v) { falar(v); }
            }
            mimo 0;
        }
    "#,
    )
    .expect("IR");

    // Os helpers de lista aparecem; e nenhuma chamada de anexo textual é
    // emitida, porque a única carga construída é uma lista.
    assert!(
        ir_texto.contains(enum_payload::CARGA_LISTA_VERSO),
        "{ir_texto}"
    );
    assert!(
        ir_texto.contains(enum_payload::ANEXAR_LISTA_VERSO),
        "{ir_texto}"
    );
    assert!(
        !ir_texto.contains(enum_payload::ANEXAR_VERSO),
        "uma lista não pode ser anexada pelo helper de verso: {ir_texto}"
    );
}

/// O validador de IR recusa metadata fabricada: representação de uma palavra
/// com identidade de outro tipo, e classe que não corresponde à representação.
#[test]
fn validador_recusa_metadata_fabricada() {
    let base = lower(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque P { C(lista<Cor>), N(lista<bombom>) }
        carinho principal() -> bombom { mimo 0; }
    "#,
    );

    // 1. Identidade trocada entre duas cargas de mesma representação.
    let mut trocado = base.clone();
    let identidade_n = carga(&base, "P", "N", 0).resolved_type_id;
    for variante in &mut trocado.enum_variants {
        if variante.variant_name == "C" {
            variante.payloads[0].resolved_type_id = identidade_n;
        }
    }
    let erro = ir_validate::validate_program(&trocado)
        .expect_err("identidade divergente deveria ser recusada")
        .to_string();
    assert!(erro.contains("E-IR-ENUM-PAYLOAD-METADATA"), "{erro}");

    // 2. Classe incoerente com a representação.
    let mut classe_errada = base.clone();
    for variante in &mut classe_errada.enum_variants {
        if variante.variant_name == "N" {
            variante.payloads[0].class = EnumPayloadClass::Verso;
        }
    }
    let erro = ir_validate::validate_program(&classe_errada)
        .expect_err("classe divergente deveria ser recusada")
        .to_string();
    assert!(erro.contains("E-IR-ENUM-PAYLOAD-METADATA"), "{erro}");

    // 3. Elemento inventado numa carga que não é lista de leque.
    let mut elemento_inventado = base.clone();
    let elemento = carga(&base, "P", "C", 0)
        .element_type_id
        .expect("elemento de lista<Cor>");
    for variante in &mut elemento_inventado.enum_variants {
        if variante.variant_name == "N" {
            variante.payloads[0].element_type_id = Some(elemento);
        }
    }
    let erro = ir_validate::validate_program(&elemento_inventado)
        .expect_err("elemento inventado deveria ser recusado")
        .to_string();
    assert!(erro.contains("E-IR-ENUM-PAYLOAD-METADATA"), "{erro}");
}
// @pinker-nav:end evidencia.leques.carga-lista-estrutura-ir

// @pinker-nav:start evidencia.leques.carga-lista-abi-runtime
// @pinker-nav:domain leques
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova de que o caminho de carga de uma palavra já transporta handles de lista sem alteração de ABI: as quatro intrínsecas internas de anexo e extração colapsam nos dois símbolos de runtime já existentes (`pinker_leque_anexar` e `pinker_leque_carga`), e a emissão nativa de um programa com cargas de lista não introduz símbolo de runtime novo.

/// O backend nativo reutiliza os símbolos existentes: nenhuma carga de lista
/// cria símbolo novo, e a ABI permanece a de uma palavra.
#[test]
fn cargas_de_lista_reutilizam_os_simbolos_de_runtime_existentes() {
    let asm = common::render_backend_s_external_subset_nativo(
        r#"
        pacote main;
        leque Cor { Rosa }
        leque P { N(lista<bombom>), T(lista<verso>), C(lista<Cor>) }
        carinho principal() -> bombom {
            nova ns: lista<bombom> = lista_criar();
            nova p: P = P.N(ns);
            encaixe p {
                caso P.N(v) { falar(lista_tamanho(v)); }
                caso P.T(v) { falar(lista_tamanho(v)); }
                caso P.C(v) { falar(lista_tamanho(v)); }
            }
            mimo 0;
        }
    "#,
    )
    .expect("emissão nativa");

    assert!(asm.contains("call pinker_leque_anexar"), "{asm}");
    assert!(asm.contains("call pinker_leque_carga"), "{asm}");
    // Nenhum símbolo específico de lista foi criado no runtime.
    for proibido in [
        "pinker_leque_anexar_lista",
        "pinker_leque_carga_lista",
        "pinker_leque_anexar_b",
        "pinker_leque_carga_v",
    ] {
        assert!(
            !asm.contains(proibido),
            "símbolo de runtime redundante '{proibido}' emitido: {asm}"
        );
    }
    // Os nomes internos do compilador não vazam para o assembly.
    for interno in enum_payload::ANEXAR_INTRINSICS
        .iter()
        .chain(enum_payload::CARGA_INTRINSICS.iter())
    {
        assert!(
            !asm.contains(interno),
            "intrínseca interna '{interno}' vazou para o assembly: {asm}"
        );
    }
}
// @pinker-nav:end evidencia.leques.carga-lista-abi-runtime
