//! #442/C5 — o fato "este corpo veio de um default de trato" tem uma dona só.
//!
//! Antes da consolidação a resposta vinha de três portadoras que se completavam
//! mal: `ImplFunctionFacts.generated_default`, que só existia no default
//! SELECIONADO e não dizia de qual trato; `FunctionDecl.default_body_trait`,
//! que só existia na dependência sintética; e o prefixo do nome — única
//! portadora do papel de CHECAGEM, que não tinha nenhum fato estruturado.
//! Quem precisava das três juntas as reunia parseando o nome sintético.
//!
//! Estes testes provam que o fato passou a nascer inteiro no ponto que
//! materializa o corpo (`FunctionDecl::trait_default_body`), com papel e
//! origem; que as fases posteriores apenas o leem; que nenhuma delas decide
//! origem de default pelo prefixo; e que o comportamento observável — seleção,
//! checagem, ambiente de resolução do corpo e precedência de C2 — não mudou.

mod common;

use common::rust_source::codigo_executavel;
use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::{Item, TraitDefaultBody, TraitDefaultBodyRole};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// O fato adulto, lido diretamente da AST
// ---------------------------------------------------------------------------

/// Papel e origem gravados para cada função do programa, em ordem estável.
fn fato_por_funcao(codigo: &str) -> Vec<(String, Option<(TraitDefaultBodyRole, String)>)> {
    let program = common::parse(codigo).expect("programa aceito");
    let mut out: Vec<_> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some((
                function.name.clone(),
                function.trait_default_body.as_ref().map(
                    |TraitDefaultBody {
                         role,
                         trait_spelling,
                     }| (*role, trait_spelling.clone()),
                ),
            )),
            _ => None,
        })
        .collect();
    out.sort_by(|esquerda, direita| esquerda.0.cmp(&direita.0));
    out
}

fn fato_de(codigo: &str, prefixo: &str) -> Option<(TraitDefaultBodyRole, String)> {
    fato_por_funcao(codigo)
        .into_iter()
        .find(|(nome, _)| nome.starts_with(prefixo))
        .unwrap_or_else(|| panic!("nenhuma função começa por `{prefixo}`"))
        .1
}

const DEFAULT_SEM_OVERRIDE: &str = "pacote main;\n\
    trato Marca { carinho marcar(valor: si) -> bombom { mimo 11; } }\n\
    impl Marca para bombom {}\n\
    carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n";

const DEFAULT_COM_OVERRIDE: &str = "pacote main;\n\
    trato Marca { carinho marcar(valor: si) -> bombom { mimo 11; } }\n\
    impl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo 23; } }\n\
    carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n";

const METODO_EXPLICITO_SEM_DEFAULT: &str = "pacote main;\n\
    trato Marca { carinho marcar(valor: si) -> bombom; }\n\
    impl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo 29; } }\n\
    carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n";

const DEFAULT_COM_CLOSURE: &str = "pacote main;\n\
    carinho apoio_c5() -> bombom { mimo 3; }\n\
    trato Marca {\n    \
        carinho marcar(valor: si) -> bombom {\n        \
            nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom \
                { mimo apoio_c5() + v; };\n        \
            mimo f(2);\n    \
        }\n\
    }\n\
    impl Marca para bombom {}\n\
    carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n";

const SINTETICO_SEM_TRATO: &str = "pacote main;\n\
    carinho principal() -> bombom {\n    \
        nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom { mimo v + 1; };\n    \
        mimo f(1);\n\
    }\n";

/// Papel A — nenhum override venceu, o corpo default É o método do `impl`.
#[test]
fn default_selecionado_carrega_papel_e_origem() {
    assert_eq!(
        fato_de(DEFAULT_SEM_OVERRIDE, "__impl_"),
        Some((TraitDefaultBodyRole::SelectedAsImpl, "Marca".to_string()))
    );
}

/// Papel B — um override venceu e o corpo default continua devendo checagem.
///
/// É o papel que antes NÃO tinha nenhum fato estruturado: `impl_facts` era
/// `None` e a única coisa que dizia de onde ele veio era o prefixo do nome.
#[test]
fn default_check_only_carrega_o_mesmo_fato_com_papel_distinto() {
    assert_eq!(
        fato_de(DEFAULT_COM_OVERRIDE, "__trait_default_check_"),
        Some((TraitDefaultBodyRole::CheckOnly, "Marca".to_string()))
    );
}

/// O método explícito do bloco `impl` não é materialização de default.
#[test]
fn override_explicito_nao_e_default() {
    assert_eq!(fato_de(DEFAULT_COM_OVERRIDE, "__impl_"), None);
    assert_eq!(fato_de(METODO_EXPLICITO_SEM_DEFAULT, "__impl_"), None);
}

/// Papel C — a dependência sintética que o corpo default cita.
#[test]
fn dependencia_de_default_carrega_papel_e_origem() {
    assert_eq!(
        fato_de(DEFAULT_COM_CLOSURE, "__anon_carinho_"),
        Some((TraitDefaultBodyRole::Dependency, "Marca".to_string()))
    );
}

/// Função sintética que nada tem com `trato` não pode ser capturada pelo fato.
#[test]
fn sintetico_nao_relacionado_a_trato_nao_e_default() {
    assert_eq!(fato_de(SINTETICO_SEM_TRATO, "__anon_carinho_"), None);
}

/// Nenhuma função comum recebe o fato.
#[test]
fn funcao_comum_nao_recebe_o_fato() {
    for codigo in [
        DEFAULT_SEM_OVERRIDE,
        DEFAULT_COM_OVERRIDE,
        DEFAULT_COM_CLOSURE,
        SINTETICO_SEM_TRATO,
    ] {
        for (nome, fato) in fato_por_funcao(codigo) {
            if nome == "principal" || nome == "apoio_c5" {
                assert_eq!(fato, None, "`{nome}` foi classificada como default");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Comportamento observável
// ---------------------------------------------------------------------------

struct Caso {
    _dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(modulos: &[(&str, &str)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso C5");
    let mut raiz = None;
    for (nome, fonte) in modulos {
        let caminho = escrever(dir.path(), nome, fonte);
        if *nome == "main" {
            raiz = Some(caminho);
        }
    }
    Caso {
        raiz: raiz.expect("caso tem raiz `main`"),
        _dir: dir,
    }
}

fn escrever(dir: &Path, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.join(format!("{nome}.pink"));
    fs::write(&caminho, fonte)
        .unwrap_or_else(|erro| panic!("gravar {}: {erro}", caminho.display()));
    caminho
}

fn pink(caso_logico: &str, args: &[&str], alvo: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg(alvo)
        .logical_case(caso_logico)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar pink")
}

fn codigo_de_saida(saida: &std::process::Output) -> i32 {
    saida.status.code().expect("status com código")
}

fn stderr(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

/// A: o default selecionado continua sendo o método executado.
#[test]
fn default_selecionado_continua_executando() {
    let caso = caso(&[("main", DEFAULT_SEM_OVERRIDE)]);
    let saida = pink("c5-a-default", &["--run"], &caso.raiz);
    assert_eq!(codigo_de_saida(&saida), 11, "{}", stderr(&saida));
}

/// B: o override explícito continua vencendo o default materializado.
#[test]
fn override_explicito_continua_vencendo() {
    let caso = caso(&[("main", DEFAULT_COM_OVERRIDE)]);
    let saida = pink("c5-b-override", &["--run"], &caso.raiz);
    assert_eq!(codigo_de_saida(&saida), 23, "{}", stderr(&saida));
}

/// C: o corpo default vencido por override continua sendo checado.
///
/// O oráculo é o diagnóstico do CORPO DEFAULT, que ninguém executa: se o papel
/// `CheckOnly` deixasse de existir, o programa passaria em `--check`.
#[test]
fn default_check_only_continua_sendo_checado() {
    let caso = caso(&[(
        "main",
        "pacote main;\n\
         trato Marca { carinho marcar(valor: si) -> bombom { mimo ausente_c5(); } }\n\
         impl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo 7; } }\n\
         carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n",
    )]);
    let saida = pink("c5-c-checagem", &["--check"], &caso.raiz);
    assert_eq!(
        codigo_de_saida(&saida),
        1,
        "o corpo default deixou de ser checado"
    );
    assert!(
        stderr(&saida).contains("'ausente_c5' não declarada"),
        "diagnóstico inesperado: {}",
        stderr(&saida)
    );
}

/// O trato declara `Marca` cujo default soma o auxiliar da PRÓPRIA unidade; a
/// raiz importa o trato, implementa sem override e tem um homônimo do auxiliar.
///
/// O oráculo é o valor: `40` é da origem, `1` é do importador.
const TRATO_COM_AUXILIAR: &str = "pacote c5t;\n\n\
    carinho apoio_c5() -> bombom { mimo 40; }\n\n\
    trato Marca {\n    \
        carinho marcar(valor: si) -> bombom {\n        \
            nova base: bombom = 5;\n        \
            nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom \
                { mimo apoio_c5() + base + v; };\n        \
            mimo f(2);\n    \
        }\n\
    }\n";

const RAIZ_COM_HOMONIMO: &str = "pacote main;\ntrazer c5t.Marca;\n\n\
    carinho apoio_c5() -> bombom { mimo 1; }\n\n\
    impl Marca para bombom {}\n\n\
    carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n";

/// D + E: o default importado e a dependência sintética dele continuam sendo
/// resolvidos no ambiente da unidade que DECLAROU o trato.
#[test]
fn default_importado_resolve_no_ambiente_da_unidade_declarante() {
    let caso = caso(&[("c5t", TRATO_COM_AUXILIAR), ("main", RAIZ_COM_HOMONIMO)]);
    let saida = pink("c5-de-origem", &["--run"], &caso.raiz);
    assert_eq!(
        codigo_de_saida(&saida),
        47,
        "o corpo default deixou de significar o que a origem escreveu: {}",
        stderr(&saida)
    );
}

/// C cross-unit: o corpo default vencido por override é materializado numa
/// unidade que NÃO declarou o trato, e mesmo assim precisa ser checado contra o
/// ambiente da unidade declarante.
///
/// O oráculo é o programa passar: `apoio_x_c5` só existe em `c5x`, e o corpo
/// que o cita foi copiado para `c5y`. Se o papel `CheckOnly` perdesse a origem,
/// a checagem procuraria o auxiliar no importador e o programa seria recusado.
#[test]
fn default_check_only_importado_e_checado_no_ambiente_da_origem() {
    let caso = caso(&[
        (
            "c5x",
            "pacote c5x;\n\
             carinho apoio_x_c5() -> bombom { mimo 40; }\n\
             trato Marca { carinho marcar(valor: si) -> bombom { mimo apoio_x_c5(); } }\n",
        ),
        (
            "c5y",
            "pacote c5y;\ntrazer c5x.Marca;\n\
             impl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo 5; } }\n\
             carinho via(x: bombom) -> bombom { mimo x.marcar(); }\n",
        ),
        (
            "main",
            "pacote main;\ntrazer c5y.via;\n\
             carinho principal() -> bombom { nova x: bombom = 0; mimo via(x); }\n",
        ),
    ]);
    let saida = pink("c5-c-checagem-importada", &["--run"], &caso.raiz);
    assert_eq!(
        codigo_de_saida(&saida),
        5,
        "o corpo default só para checagem perdeu o ambiente da origem: {}",
        stderr(&saida)
    );
}

/// D sem indireção: o corpo default cita o auxiliar da origem DIRETAMENTE.
///
/// Separado do caso com closure de propósito: ali a origem sobrevive pelo fato
/// da dependência (#567); aqui só o fato do próprio corpo materializado pode
/// responder, e é ele que esta prova ataca.
#[test]
fn default_importado_sem_closure_resolve_no_ambiente_da_origem() {
    let caso = caso(&[
        (
            "c5d",
            "pacote c5d;\n\n\
             carinho apoio_direto_c5() -> bombom { mimo 40; }\n\n\
             trato Marca { carinho marcar(valor: si) -> bombom { mimo apoio_direto_c5(); } }\n",
        ),
        (
            "main",
            "pacote main;\ntrazer c5d.Marca;\n\n\
             carinho apoio_direto_c5() -> bombom { mimo 1; }\n\n\
             impl Marca para bombom {}\n\n\
             carinho principal() -> bombom { nova x: bombom = 0; mimo x.marcar(); }\n",
        ),
    ]);
    let saida = pink("c5-d-direto", &["--run"], &caso.raiz);
    assert_eq!(
        codigo_de_saida(&saida),
        40,
        "o corpo default passou a ser resolvido no ambiente do importador: {}",
        stderr(&saida)
    );
}

/// F + I: dois tratos homônimos de módulos distintos permanecem isolados, e a
/// precedência explícito > default de C2 continua valendo em cada um.
#[test]
fn tratos_homonimos_permanecem_isolados_com_a_precedencia_de_c2() {
    let caso = caso(&[
        (
            "c5ga",
            "pacote c5ga;\ntrato Marca { carinho marcar(valor: si) -> bombom { mimo 10; } }\n",
        ),
        (
            "c5gb",
            "pacote c5gb;\ntrato Marca { carinho marcar(valor: si) -> bombom { mimo 20; } }\n",
        ),
        (
            "c5gia",
            "pacote c5gia;\ntrazer c5ga.Marca;\n\
             impl Marca para bombom {}\n\
             carinho via_a(x: bombom) -> bombom { mimo x.marcar(); }\n",
        ),
        (
            "c5gib",
            "pacote c5gib;\ntrazer c5gb.Marca;\n\
             impl Marca para bombom { carinho marcar(valor: bombom) -> bombom { mimo 21; } }\n\
             carinho via_b(x: bombom) -> bombom { mimo x.marcar(); }\n",
        ),
        (
            "main",
            "pacote main;\ntrazer c5gia.via_a;\ntrazer c5gib.via_b;\n\
             carinho principal() -> bombom { nova x: bombom = 0; mimo via_a(x) + via_b(x); }\n",
        ),
    ]);
    let saida = pink("c5-f-homonimos", &["--run"], &caso.raiz);
    assert_eq!(
        codigo_de_saida(&saida),
        31,
        "10 é o default de c5ga e 21 é o override de c5gb: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// Oráculos estruturais: quem pode responder pela origem
// ---------------------------------------------------------------------------

fn fonte(caminho: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(caminho))
        .unwrap_or_else(|erro| panic!("ler {caminho}: {erro}"))
}

/// Aplica `predicado` ao código executável de cada `.rs` de `src/`.
fn varredura_de_src(predicado: impl Fn(&str) -> bool) -> Vec<String> {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut encontrados = Vec::new();
    let mut pendentes = vec![raiz.clone()];
    while let Some(dir) = pendentes.pop() {
        for entrada in fs::read_dir(&dir).expect("diretório de fontes legível") {
            let caminho = entrada.expect("entrada legível").path();
            if caminho.is_dir() {
                pendentes.push(caminho);
                continue;
            }
            if !caminho.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let codigo = codigo_executavel(&fs::read_to_string(&caminho).expect("fonte legível"));
            if predicado(&codigo) {
                encontrados.push(
                    caminho
                        .strip_prefix(&raiz)
                        .expect("fonte dentro de src/")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    encontrados.sort();
    encontrados
}

/// O prefixo é transporte. Só o codec pode nomeá-lo.
///
/// Uma fase que volte a decidir origem de default por `starts_with`/
/// `strip_prefix` precisa nomear o prefixo — literalmente ou pela constante —
/// e aparece aqui. Teto declarado: um prefixo remontado por concatenação em
/// tempo de execução escapa a qualquer oráculo textual; o que o guarda são os
/// casos comportamentais e a inexistência de segunda fonte do fato.
#[test]
fn so_o_codec_nomeia_os_prefixos_sinteticos_de_trato() {
    for termo in ["IMPL_PREFIX", "TRAIT_DEFAULT_CHECK_PREFIX"] {
        assert_eq!(
            varredura_de_src(|codigo| codigo.contains(termo)),
            vec!["method_identity.rs".to_string()],
            "`{termo}` passou a ser nomeado fora do codec"
        );
    }
    // A grafia crua só existe como literal — valor da constante no codec e
    // forma reservada em `native_symbol` —, nunca em código executável.
    assert_eq!(
        varredura_de_src(|codigo| codigo.contains("__trait_default_check_")),
        Vec::<String>::new(),
        "alguma camada voltou a comparar a grafia crua do prefixo"
    );
}

/// A grafia crua do prefixo, escrita como LITERAL, tem lista fechada de donos.
///
/// O oráculo de código executável não alcança literal — `codigo_executavel` o
/// remove de propósito, para que uma menção em comentário não acuse. É
/// exatamente por isso que uma fase poderia voltar a decidir origem com
/// `starts_with("__impl_")` sem aparecer lá. Este teste fecha essa porta pelo
/// texto bruto, com a lista dos donos legítimos:
///
/// ```text
/// method_identity.rs  valor das constantes do codec
/// native_symbol.rs    forma reservada na fronteira léxica
/// backend_s.rs        recorte externo: o primeiro parâmetro de um método de
///                     `impl` é receiver. Pergunta de IDENTIDADE de método,
///                     não de origem de default: não distingue default de
///                     explícito e não nomeia trato. Fora do escopo de C5.
/// interpreter.rs      símbolos literais em teste de unidade
/// ```
///
/// `parser`, `semantic`, `ir` e `module_resolve` ficam de fora: são as fases
/// que a #592 proíbe de responder pela origem do corpo default.
///
/// Teto declarado: a lista é por ARQUIVO, não por ocorrência. Um quarto
/// `starts_with("__impl_")` acrescentado dentro de um arquivo já autorizado não
/// aparece aqui. Isso é aceito porque os quatro donos respondem perguntas que
/// não são a de C5 — valor de constante, reserva léxica, receiver no recorte
/// externo e literal de teste —, e o que guarda a fronteira dentro deles é a
/// matriz comportamental, não este oráculo.
#[test]
fn a_grafia_crua_do_prefixo_tem_donos_declarados() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut donos = Vec::new();
    let mut pendentes = vec![raiz.clone()];
    while let Some(dir) = pendentes.pop() {
        for entrada in fs::read_dir(&dir).expect("diretório de fontes legível") {
            let caminho = entrada.expect("entrada legível").path();
            if caminho.is_dir() {
                pendentes.push(caminho);
                continue;
            }
            if !caminho.extension().is_some_and(|ext| ext == "rs") {
                continue;
            }
            let bruto = fs::read_to_string(&caminho).expect("fonte legível");
            if bruto.contains("\"__impl_") || bruto.contains("\"__trait_default_check_") {
                donos.push(
                    caminho
                        .strip_prefix(&raiz)
                        .expect("fonte dentro de src/")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    donos.sort();
    assert_eq!(
        donos,
        vec![
            "backend_s.rs".to_string(),
            "interpreter.rs".to_string(),
            "method_identity.rs".to_string(),
            "native_symbol.rs".to_string(),
        ],
        "a grafia crua do prefixo apareceu fora dos donos declarados"
    );
}

/// O codec de nome sintético continua existindo e continua tendo consumidor
/// legítimo: recompor a identidade provisória com as identidades canônicas.
///
/// A pergunta permitida é "que identidade este símbolo codifica?". A proibida é
/// "de que trato veio este corpo?" — e essa não tem mais como ser feita ao
/// nome, porque `parse_trait_default_check_function_name`, cujo único consumidor
/// era exatamente ela, deixou de existir.
#[test]
fn o_codec_de_nome_sintetico_sobrevive_como_identidade() {
    assert_eq!(
        varredura_de_src(|codigo| codigo.contains("parse_synthetic_trait_body_name")),
        vec![
            "method_identity.rs".to_string(),
            "module_resolve.rs".to_string()
        ],
    );
    let codigo = codigo_executavel(&fonte("src/module_resolve.rs"));
    assert_eq!(
        codigo.matches("parse_synthetic_trait_body_name(").count(),
        1,
        "a resolução modular voltou a consultar o codec em mais de um lugar"
    );
    assert!(
        !codigo_executavel(&fonte("src/method_identity.rs"))
            .contains("parse_trait_default_check_function_name"),
        "o parser de origem por prefixo voltou a existir"
    );
}

/// A dona do fato é o parser, no ponto que materializa o corpo.
#[test]
fn so_o_parser_escreve_o_fato() {
    assert_eq!(
        varredura_de_src(|codigo| codigo.contains("TraitDefaultBody {")),
        vec!["ast.rs".to_string(), "parser/mod.rs".to_string()],
        "outra camada passou a construir o fato"
    );
}

/// `semantic` e `ir` consomem o fato pela leitura derivada e não reintroduzem
/// regra paralela: nenhuma das duas nomeia o campo, o papel ou o prefixo.
#[test]
fn semantic_e_ir_apenas_leem_o_fato() {
    for caminho in ["src/semantic.rs", "src/ir.rs"] {
        let codigo = codigo_executavel(&fonte(caminho));
        assert!(
            codigo.contains("e_default_selecionado()"),
            "{caminho} deixou de consumir a leitura do fato adulto"
        );
        for termo in [
            "trait_default_body",
            "TraitDefaultBodyRole",
            "TraitDefaultBody",
        ] {
            assert!(
                !codigo.contains(termo),
                "{caminho} passou a reconstruir o fato por conta própria com `{termo}`"
            );
        }
    }
}

/// C2 preservada: a autoridade de seleção continua recebendo um predicado e
/// não passa a conhecer a AST.
#[test]
fn method_dispatch_continua_sem_conhecer_o_fato() {
    let codigo = codigo_executavel(&fonte("src/method_dispatch.rs"));
    for termo in [
        "trait_default_body",
        "TraitDefaultBody",
        "TraitDefaultBodyRole",
        "crate::ast",
    ] {
        assert!(
            !codigo.contains(termo),
            "a autoridade de seleção passou a conhecer `{termo}`"
        );
    }
}

/// O `bool` antigo do `impl` não sobreviveu em lugar nenhum como campo da AST.
#[test]
fn o_bool_antigo_de_default_gerado_nao_sobrevive_na_ast() {
    assert!(
        !codigo_executavel(&fonte("src/ast.rs")).contains("generated_default"),
        "a AST voltou a carregar uma segunda codificação do fato"
    );
}

// ---------------------------------------------------------------------------
// Controles negativos do oráculo
// ---------------------------------------------------------------------------

/// Comentário e literal citando os nomes antigos não podem acusar.
#[test]
fn comentario_e_literal_nao_acusam() {
    let amostra = "// __trait_default_check_ e IMPL_PREFIX citados em comentário\n\
         let s = \"__trait_default_check_5_Marca\";\n\
         /* TraitDefaultBody { } em bloco */\n\
         let t = 1;";
    let codigo = codigo_executavel(amostra);
    for termo in [
        "__trait_default_check_",
        "IMPL_PREFIX",
        "TraitDefaultBody {",
    ] {
        assert!(!codigo.contains(termo), "`{termo}` sobreviveu à limpeza");
    }
    assert!(
        codigo.contains("let t = 1;"),
        "código executável foi perdido"
    );
}
