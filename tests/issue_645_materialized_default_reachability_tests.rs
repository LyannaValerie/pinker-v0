mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Prova comportamental da #645: o ALCANCE de relação de um corpo default materializado cross-unit é decidido contra a unidade que DECLAROU o trato, e ausência de contexto semântico não amplia o conjunto de relações candidatas. A matriz cobre o caso causal (a declarante não alcança a relação, e o programa passa a ser recusado com a origem correta), a contraprova positiva que separa esta correção de atribuir o contexto ao importador (a declarante alcança, o importador não, e o programa continua aceito, tanto pelo nível próprio quanto pelo subordinado da #577), o override explícito nas duas direções, o contrato nominal da #517 com homônimo da raiz, os controles que provam que a fixture chega à materialização, a forma qualificada, a ordem de imports, o programa de arquivo único sem composição e a paridade interpretador/nativo. O oráculo é o valor observado e a fonte renderizada, nunca a simples ausência de mensagem.

/// Um caso é um conjunto de fontes; a raiz é escrita por último.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, &str)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #645");
    for (modulo, fonte) in modulos {
        escrever(dir.path(), modulo, fonte);
    }
    let raiz = escrever(dir.path(), nome, raiz);
    Caso { dir, raiz }
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

fn checar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--check"], &caso.raiz)
}

fn executar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--run"], &caso.raiz)
}

fn codigo(saida: &std::process::Output) -> i32 {
    saida.status.code().expect("status com código")
}

fn stderr(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

// ---------------------------------------------------------------------------
// Fontes compartilhadas
//
// `tr2` declara o contrato `Medida`; `impl_m` declara a ÚNICA relação
// `Medida para bombom`, e devolve 77. Nenhum dos casos abaixo escreve essa
// relação em outro lugar: quando o programa executa 77, ele alcançou
// exatamente a relação de `impl_m`.
// ---------------------------------------------------------------------------

const TR2: (&str, &str) = (
    "m645_tr2",
    "pacote m645_tr2;\n\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n",
);

const IMPL_M: (&str, &str) = (
    "m645_impl",
    "pacote m645_impl;\ntrazer m645_tr2.Medida;\n\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 77; }\n}\n",
);

/// A unidade declarante do trato cujo corpo default é materializado alhures.
/// `imports` é o que ELA autorizou — a única variável que os casos abaixo mexem.
fn declarante(imports: &str) -> (String, String) {
    (
        "m645_tr".to_string(),
        format!(
            "pacote m645_tr;\n{imports}\ntrato Base {{\n    carinho rodar(valor: si) -> bombom {{ mimo valor.medir(); }}\n}}\n"
        ),
    )
}

/// O materializador: importa apenas o trato, declara a relação com bloco vazio
/// — e portanto recebe o corpo default copiado — e chama o método.
const USER: (&str, &str) = (
    "m645_user",
    "pacote m645_user;\ntrazer m645_tr.Base;\n\nimpl Base para bombom {}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
);

const RAIZ: &str = "pacote main;\ntrazer m645_tr.Base;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\ntrazer m645_user.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n";

fn fontes(declarante_imports: &str, user: (&str, &str)) -> Vec<(String, String)> {
    let (nome, fonte) = declarante(declarante_imports);
    vec![
        (TR2.0.to_string(), TR2.1.to_string()),
        (IMPL_M.0.to_string(), IMPL_M.1.to_string()),
        (nome, fonte),
        (user.0.to_string(), user.1.to_string()),
    ]
}

fn caso_composto(nome: &str, declarante_imports: &str, user: (&str, &str)) -> Caso {
    let fontes = fontes(declarante_imports, user);
    let emprestados: Vec<(&str, &str)> = fontes
        .iter()
        .map(|(modulo, fonte)| (modulo.as_str(), fonte.as_str()))
        .collect();
    caso(nome, RAIZ, &emprestados)
}

// ---------------------------------------------------------------------------
// X5 — o caso causal: a unidade DECLARANTE não alcança a relação
// ---------------------------------------------------------------------------

/// X5 da #644: `m645_tr` declara o default e não importa nem o contrato nem a
/// unidade que o implementa. O corpo materializado em `m645_user` alcançava
/// `Medida para bombom` de qualquer forma, porque o span copiado não reivindica
/// unidade-fonte nenhuma e o alcance concedia o nível próprio a quem não
/// reconhecia — `--check` saía 0 e o programa executava 77.
///
/// A recusa tem de vir pela CAUSA certa: a mensagem é de método não
/// implementado para o tipo, e a fonte renderizada é a unidade declarante, que
/// é quem escreveu a chamada.
#[test]
fn x5_default_materializado_nao_alcanca_relacao_que_a_declarante_nao_autoriza() {
    let c = caso_composto("x5_645", "", USER);
    let saida = checar(&c, "issue-645-x5-check");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("método 'medir' não implementado"),
        "a recusa tem de ser de alcance de método, não outra falha: {erro}"
    );
    assert!(
        erro.contains("m645_tr.pink"),
        "a chamada é da unidade declarante e o diagnóstico tem de dizer isso: {erro}"
    );
    let execucao = executar(&c, "issue-645-x5-run");
    assert_eq!(
        codigo(&execucao),
        1,
        "o programa recusado não pode executar 77: {}",
        stderr(&execucao)
    );
}

/// Controle X5a: a MESMA chamada escrita diretamente pelo materializador, sem
/// trato nem default no meio. Ela já era recusada, e continua — é o que prova
/// que a fixture de X5 não estava medindo uma recusa que o importador sofreria
/// de todo jeito por outro motivo.
#[test]
fn x5a_controle_chamada_direta_do_materializador_continua_recusada() {
    let c = caso(
        "x5a_645",
        "pacote main;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\ntrazer m645_user.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &[
            TR2,
            IMPL_M,
            (
                "m645_user",
                "pacote m645_user;\n\ncarinho usar(x: bombom) -> bombom { mimo x.medir(); }\n",
            ),
        ],
    );
    let saida = checar(&c, "issue-645-x5a");
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
    assert!(
        stderr(&saida).contains("método 'medir' não implementado"),
        "{}",
        stderr(&saida)
    );
}

/// Controle X5c: o mesmo programa com o trato declarado LOCALMENTE no
/// materializador — portanto com unidade-fonte registrada e sem travessia de
/// prepass. Já era recusado antes da #645 e continua: a correção não inverteu
/// um caso que o mecanismo antigo acertava.
#[test]
fn x5c_controle_trato_local_continua_recusado() {
    let c = caso(
        "x5c_645",
        "pacote main;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\ntrazer m645_user.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &[
            TR2,
            IMPL_M,
            (
                "m645_user",
                "pacote m645_user;\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo valor.medir(); }\n}\n\nimpl Base para bombom {}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
            ),
        ],
    );
    let saida = checar(&c, "issue-645-x5c");
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// Contraprova positiva: o ambiente é o da DECLARANTE, não o do importador
// ---------------------------------------------------------------------------

/// A declarante importa o contrato E a unidade que o implementa; o
/// materializador importa apenas o trato. O corpo default continua aceito e
/// continua executando 77.
///
/// `COULD_THIS_TEST_STAY_GREEN_IF_CONTEXT_WERE_ASSIGNED_TO_THE_WRONG_UNIT?`
/// Não: com o contexto do importador, `m645_user` não autoriza nem `Medida` nem
/// `m645_impl`, e o programa seria recusado. É este caso que separa a correção
/// de `DEFAULT_CONTEXT = IMPORTER`.
#[test]
fn declarante_autorizada_e_importador_nao_continua_aceito() {
    let c = caso_composto(
        "x5p_645",
        "trazer m645_tr2.Medida;\ntrazer m645_impl;\n",
        USER,
    );
    let saida = checar(&c, "issue-645-x5p-check");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-x5p-run");
    assert_eq!(
        codigo(&execucao),
        77,
        "o default tem de alcançar a relação de m645_impl pelo ambiente da declarante: {}",
        stderr(&execucao)
    );
}

/// O mesmo, pelo nível SUBORDINADO da #577: a declarante não nomeia `Medida`,
/// só importa a unidade que a implementa. O alcance por unidade importada
/// também é perguntado à declarante, e não ao materializador.
#[test]
fn declarante_alcanca_pelo_nivel_subordinado_da_577() {
    let c = caso_composto("sub577_645", "trazer m645_impl;\n", USER);
    let saida = checar(&c, "issue-645-sub577-check");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-sub577-run");
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

/// Ordem de import não decide alcance: as mesmas fontes, com a declarante
/// listando os imports na ordem inversa e a raiz também, produzem o mesmo
/// resultado.
#[test]
fn ordem_de_import_nao_muda_o_alcance_do_default_materializado() {
    let fontes = fontes("trazer m645_impl;\ntrazer m645_tr2.Medida;\n", USER);
    let emprestados: Vec<(&str, &str)> = fontes
        .iter()
        .map(|(modulo, fonte)| (modulo.as_str(), fonte.as_str()))
        .collect();
    let c = caso(
        "ordem_645",
        "pacote main;\ntrazer m645_user.usar;\ntrazer m645_impl;\ntrazer m645_tr2.Medida;\ntrazer m645_tr.Base;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &emprestados,
    );
    let saida = checar(&c, "issue-645-ordem-check");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-ordem-run");
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

// ---------------------------------------------------------------------------
// Override explícito: ambiente e proveniência PRÓPRIOS
// ---------------------------------------------------------------------------

/// O override é escrito pelo materializador e responde pelo ambiente DELE,
/// mesmo quando o default omitido teria alcançado a relação. A declarante
/// autoriza `Medida` e `m645_impl`; o override, não — e é recusado, com a fonte
/// do override.
///
/// É o caso que falha se o override herdar o contexto do default.
#[test]
fn override_nao_herda_o_ambiente_do_default_omitido() {
    let c = caso_composto(
        "ovr_fecha_645",
        "trazer m645_tr2.Medida;\ntrazer m645_impl;\n",
        (
            "m645_user",
            "pacote m645_user;\ntrazer m645_tr.Base;\n\nimpl Base para bombom {\n    carinho rodar(valor: bombom) -> bombom { mimo valor.medir(); }\n}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
        ),
    );
    let saida = checar(&c, "issue-645-override-fechado");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("m645_user.pink"),
        "o corpo é do override e a proveniência tem de ser dele: {erro}"
    );
}

/// A direção oposta: o override alcança pelo PRÓPRIO ambiente uma relação que a
/// declarante não alcança, e o programa é aceito e executa 77.
///
/// O default desta declarante é auto-contido de propósito: o corpo dele continua
/// devendo checagem ao contrato mesmo vencido pelo override (#592), e um default
/// que dependesse de relação inalcançável recusaria o programa por conta própria
/// — o que é o caso `default_inalcancavel_recusa_mesmo_com_override_presente`
/// abaixo, não este.
///
/// `COULD_THIS_TEST_STAY_GREEN_IF_CONTEXT_WERE_ASSIGNED_TO_THE_WRONG_UNIT?`
/// Não: se o override herdasse o ambiente da declarante, `m645_tr` não autoriza
/// `Medida` nem `m645_impl`, e a chamada do override seria recusada.
#[test]
fn override_alcanca_pelo_proprio_ambiente() {
    let c = caso(
        "ovr_abre_645",
        RAIZ,
        &[
            TR2,
            IMPL_M,
            (
                "m645_tr",
                "pacote m645_tr;\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo 1; }\n}\n",
            ),
            (
                "m645_user",
                "pacote m645_user;\ntrazer m645_tr.Base;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\n\nimpl Base para bombom {\n    carinho rodar(valor: bombom) -> bombom { mimo valor.medir(); }\n}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
            ),
        ],
    );
    let saida = checar(&c, "issue-645-override-aberto");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-override-run");
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

/// O corpo default vencido por override continua devendo checagem, e agora a
/// deve contra a unidade que o escreveu: a declarante não alcança a relação, e o
/// programa é recusado ainda que ninguém vá executar aquele corpo.
///
/// Isto NÃO é regra nova da #645: o programa equivalente com o trato declarado
/// LOCALMENTE — coberto por `default_local_vencido_por_override_ja_era_recusado`
/// — já era recusado antes desta Task. A #645 alinhou o caso cross-unit ao
/// local, em vez de criar uma exceção para ele.
#[test]
fn default_inalcancavel_recusa_mesmo_com_override_presente() {
    let c = caso_composto(
        "ovr_default_645",
        "",
        (
            "m645_user",
            "pacote m645_user;\ntrazer m645_tr.Base;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\n\nimpl Base para bombom {\n    carinho rodar(valor: bombom) -> bombom { mimo 5; }\n}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
        ),
    );
    let saida = checar(&c, "issue-645-override-default-devedor");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("m645_tr.pink"),
        "quem deve a checagem é o corpo da declarante: {erro}"
    );
}

/// Controle de uniformidade do caso acima, com o trato declarado LOCALMENTE: o
/// default inalcançável vencido por override já era recusado, e continua. É a
/// prova de que a recusa cross-unit não é uma regra inventada pela #645.
#[test]
fn default_local_vencido_por_override_ja_era_recusado() {
    let c = caso(
        "ovr_local_645",
        "pacote main;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\ntrazer m645_user.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &[
            TR2,
            IMPL_M,
            (
                "m645_user",
                "pacote m645_user;\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo valor.medir(); }\n}\n\nimpl Base para bombom {\n    carinho rodar(valor: bombom) -> bombom { mimo 5; }\n}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
            ),
        ],
    );
    let saida = checar(&c, "issue-645-override-local");
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// #517 — o contrato nominal continua com dentes
// ---------------------------------------------------------------------------

/// Regressão de preservação da #517, na forma exata que a #645 recebeu como
/// contrato: a raiz declara um homônimo do auxiliar da declarante. O corpo
/// default materializado tem de continuar resolvendo o nome contra a unidade
/// que o escreveu — 11, nunca 99.
///
/// Ele NÃO é defeito de alcance: nome e alcance são perguntas distintas, e a
/// #645 alinha a segunda à primeira em vez de afrouxar a primeira.
#[test]
fn homonimo_da_raiz_nao_captura_o_nome_do_corpo_default_materializado() {
    let c = caso(
        "hom_645",
        "pacote main;\ntrazer m645_nome.Base;\n\ncarinho auxiliar() -> bombom { mimo 99; }\n\nimpl Base para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 0;\n    mimo x.rodar();\n}\n",
        &[(
            "m645_nome",
            "pacote m645_nome;\n\ncarinho auxiliar() -> bombom { mimo 11; }\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo auxiliar(); }\n}\n",
        )],
    );
    let saida = executar(&c, "issue-645-homonimo-517");
    assert_eq!(
        codigo(&saida),
        11,
        "o corpo default resolve o nome na unidade declarante (11), nunca no homônimo da raiz (99): {}",
        stderr(&saida)
    );
}

/// A forma QUALIFICADA dentro do corpo default materializado continua sendo
/// resolução de identidade pelo ambiente da declarante — o caso Y1 da #644, que
/// a closure daquela Task classificou como contrato #517, não como defeito.
/// A #645 não muda essa pergunta.
#[test]
fn chamada_qualificada_no_corpo_default_segue_a_declarante() {
    let c = caso(
        "y1_645",
        RAIZ,
        &[
            TR2,
            IMPL_M,
            (
                "m645_tr",
                "pacote m645_tr;\ntrazer m645_tr2.Medida;\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo Medida.medir(valor); }\n}\n",
            ),
            USER,
        ],
    );
    let saida = checar(&c, "issue-645-y1-check");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-y1-run");
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

/// Controle da qualificada: a MESMA chamada escrita pelo materializador, que
/// não pode nomear `Medida`, continua recusada. É o que impede ler o caso acima
/// como "a qualificada escapou do ambiente".
#[test]
fn controle_chamada_qualificada_escrita_pelo_materializador_e_recusada() {
    let c = caso(
        "y1c_645",
        RAIZ,
        &[
            TR2,
            IMPL_M,
            (
                "m645_tr",
                "pacote m645_tr;\ntrazer m645_tr2.Medida;\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo Medida.medir(valor); }\n}\n",
            ),
            (
                "m645_user",
                "pacote m645_user;\ntrazer m645_tr.Base;\n\nimpl Base para bombom {}\n\ncarinho usar(x: bombom) -> bombom { mimo Medida.medir(x); }\n",
            ),
        ],
    );
    let saida = checar(&c, "issue-645-y1c");
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// Fluxos legítimos sem composição não podem falhar fechado
// ---------------------------------------------------------------------------

/// Arquivo único: trato, relação e default no mesmo lugar. Não há índice de
/// alcance porque não há a quem restringir, e o programa continua aceito.
#[test]
fn arquivo_unico_sem_composicao_continua_aceito() {
    let c = caso(
        "single_645",
        "pacote main;\n\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo valor.medir(); }\n}\n\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 77; }\n}\n\nimpl Base para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 5;\n    mimo x.rodar();\n}\n",
        &[],
    );
    let saida = checar(&c, "issue-645-single-check");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-single-run");
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

/// Default declarado e materializado na MESMA unidade de um programa composto:
/// o corpo nunca atravessou o prepass, a unidade-fonte dele sempre foi
/// registrada, e nada muda.
#[test]
fn default_local_em_programa_composto_continua_aceito() {
    let c = caso(
        "local_645",
        "pacote main;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\ntrazer m645_user.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &[
            TR2,
            IMPL_M,
            (
                "m645_user",
                "pacote m645_user;\ntrazer m645_tr2.Medida;\ntrazer m645_impl;\n\ntrato Base {\n    carinho rodar(valor: si) -> bombom { mimo valor.medir(); }\n}\n\nimpl Base para bombom {}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
            ),
        ],
    );
    let saida = checar(&c, "issue-645-local-check");
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    let execucao = executar(&c, "issue-645-local-run");
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

// ---------------------------------------------------------------------------
// Paridade interpretador × nativo
// ---------------------------------------------------------------------------

/// A decisão de alcance é uma só: o ELF nativo observa o mesmo resultado do
/// interpretador no caso positivo, e o programa recusado não chega a gerar
/// binário.
#[test]
fn paridade_interpretador_e_nativo_do_default_materializado() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-645-paridade", true)
    else {
        return;
    };
    let c = caso_composto(
        "paridade_645",
        "trazer m645_tr2.Medida;\ntrazer m645_impl;\n",
        (
            "m645_user",
            "pacote m645_user;\ntrazer m645_tr.Base;\n\nimpl Base para bombom {}\n\ncarinho usar(x: bombom) -> bombom { mimo x.rodar(); }\n",
        ),
    );
    let interpretado = executar(&c, "issue-645-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 77, "{}", stderr(&interpretado));

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("issue-645-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #645");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_645"))
        .logical_case("issue-645-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #645");
    assert_eq!(
        nativo.status.code(),
        interpretado.status.code(),
        "interpretador e nativo têm de consumir a mesma decisão de alcance"
    );
}

// Lei da #645 exercitada diretamente sobre a autoridade de alcance: com índice de composição NÃO vazio, um span cuja unidade-fonte o índice não reconhece não alcança relação nenhuma, enquanto o índice vazio — programa legítimo sem composição — continua concedendo o nível próprio. Prova o invariante `MISSING_SEMANTIC_CONTEXT -X-> EXPANDED_REACHABILITY` no ponto de decisão, sem depender de existir hoje um caminho de produto que perca contexto.

use pinker_v0::lexer::Lexer;
use pinker_v0::module_graph::ModuleGraph;
use pinker_v0::module_resolve::{
    nivel_de_despacho, resolver_grafo, tratos_visiveis_por_fonte, NivelDeDespacho,
};
use pinker_v0::parser::Parser;
use pinker_v0::source_map::{SourceId, SourceMap};
use pinker_v0::token::{Position, Span};

fn programa(fonte: &str, source: SourceId) -> pinker_v0::ast::Program {
    let tokens = Lexer::com_fonte(fonte, source)
        .tokenize()
        .expect("lexar fonte do caso");
    Parser::new(tokens).parse().expect("parsear fonte do caso")
}

const RAIZ_LEI: &str =
    "pacote main;\ntrazer m645_lei.Marca;\n\ncarinho principal() -> bombom { mimo 0; }\n";
const MODULO_LEI: &str =
    "pacote m645_lei;\n\ntrato Marca {\n    carinho marcar(valor: si) -> bombom { mimo 1; }\n}\n";

/// Grafo mínimo COM composição: a raiz, que importa `Marca` por nome, e o módulo
/// que declara o trato. A terceira fonte é registrada e deliberadamente deixada
/// FORA do grafo: ela é a unidade-fonte que existe sem ser unidade do programa.
fn grafo_com_composicao() -> (ModuleGraph, SourceId) {
    let mut sources = SourceMap::new();
    let raiz = sources.register_root("raiz.pink", RAIZ_LEI.to_string());
    let modulo = sources.register_module("m645_lei", "m645_lei.pink", MODULO_LEI.to_string());
    let fora = sources.register_module(
        "m645_fora",
        "m645_fora.pink",
        "pacote m645_fora;\n".to_string(),
    );
    let mut graph = ModuleGraph::new();
    graph.insert_root(raiz, "raiz.pink", programa(RAIZ_LEI, raiz));
    graph.insert_module(
        "m645_lei",
        modulo,
        "m645_lei.pink",
        programa(MODULO_LEI, modulo),
    );
    // O índice de alcance é derivado do grafo JÁ resolvido — é ali que a
    // declaração do módulo passa a se chamar pelo nome canônico, e é o nome
    // canônico que o import do chamador autoriza.
    let resolvido = resolver_grafo(&graph).expect("resolver o grafo do caso");
    (resolvido, fora)
}

fn span_de(source: SourceId) -> Span {
    Span::em(source, Position::new(1, 1), Position::new(1, 2))
}

/// A raiz importou `Marca` por nome: ela alcança o trato no nível próprio.
/// É o controle positivo que prova que o índice deste caso não está vazio nem
/// recusando tudo — sem ele, o caso negativo abaixo passaria por vacuidade.
#[test]
fn lei_fonte_reconhecida_alcanca_o_que_autorizou() {
    let (graph, _fora) = grafo_com_composicao();
    let por_fonte = tratos_visiveis_por_fonte(&graph);
    assert!(!por_fonte.is_empty(), "o caso precisa ter composição");
    let raiz = graph.root().source_id;
    assert_eq!(
        nivel_de_despacho(&por_fonte, span_de(raiz), "m645_lei.Marca", None),
        Some(NivelDeDespacho::Proprio)
    );
}

/// A mesma pergunta, com spans cuja unidade-fonte NENHUMA unidade do grafo
/// reivindica — exatamente o que um corpo copiado sem proveniência produzia.
///
/// Antes da #645 isto devolvia `Proprio`, e a ausência de contexto virava a
/// autorização mais ampla que existe: alcance a toda relação carregada.
#[test]
fn lei_fonte_desconhecida_nao_alcanca_nada() {
    let (graph, fora) = grafo_com_composicao();
    let por_fonte = tratos_visiveis_por_fonte(&graph);
    for desconhecida in [SourceId::UNKNOWN, fora] {
        assert_eq!(
            nivel_de_despacho(&por_fonte, span_de(desconhecida), "m645_lei.Marca", None),
            None,
            "fonte {desconhecida} não é unidade deste grafo e não pode alcançar relação alguma"
        );
    }
}

/// E o fluxo legítimo sem composição continua concedendo o nível próprio: índice
/// vazio não é contexto perdido, é ausência de quem restringir.
#[test]
fn lei_indice_vazio_continua_permissivo() {
    let vazio = std::collections::HashMap::new();
    assert_eq!(
        nivel_de_despacho(&vazio, span_de(SourceId::UNKNOWN), "Marca", None),
        Some(NivelDeDespacho::Proprio)
    );
}
