//! Trama Pinker — contrato somente leitura dos snapshots históricos (#384).
//!
//! Cobre parser estrito, renderer canônico, reconstrução pura com consumo
//! exato de regras, classificação entre `MATCH`, `DRIFT` e `HARNESS_FAILURE`,
//! relatórios determinísticos e as fronteiras negativas do recorte: nenhuma
//! escrita, nenhuma rede, nenhuma operação Git e nenhuma dependência do root
//! absoluto.
//!
//! Todas as fixtures deste arquivo são sintéticas: ele exercita o contrato, não
//! o acervo. A autoridade histórica real materializada pela Issue #384 é coberta
//! por `nav_projection_authority_tests.rs`; aqui só verificamos que ela não
//! prolifera além dos artefatos autorizados. Este estágio segue sem expor
//! superfície de CLI.

// A #605 moveu implementação de `src/main.rs` para `src/pink_cli/`. Os
// oráculos abaixo leem o binário inteiro, não um arquivo só (#601, OG-1).
#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;

use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::RECIPES_DIR;
use pinker_v0::nav_projection_snapshot::{
    fnv1a64, human_report, json_report, measure, parse, reconstruct, render, stable_projection,
    verify, HarnessFailure, Measures, Outcome, ProjectionSnapshot, Rule, SnapshotState, FNV_PREFIX,
    SNAPSHOTS_DIR, SNAPSHOT_SCHEMA, SNAPSHOT_SCHEMA_V1,
};

// ---------------------------------------------------------------------------
// Fixtures sintéticas
// ---------------------------------------------------------------------------

fn region(key: &str, file: &str, hash: &str) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some("dominio".to_string()),
        layer: Some("camada".to_string()),
        phase: None,
        file: file.to_string(),
        start_marker: 1,
        content_start: 2,
        content_end: 3,
        end_marker: 4,
        summary: format!("Resumo de {key}."),
        hash: hash.to_string(),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

/// Catálogo corrente sintético: duas regiões históricas, uma região posterior e
/// um arquivo inteiro posterior (`apps/`).
fn catalogo_corrente() -> Vec<CodeRegion> {
    vec![
        region("a.b.um", "src/um.rs", "fnv1a64:0000000000000001"),
        region("a.b.dois", "src/dois.rs", "fnv1a64:0000000000000002"),
        region("posterior.novo", "src/novo.rs", "fnv1a64:0000000000000003"),
        region(
            "apps.x.y",
            "apps/x/principal.pink",
            "fnv1a64:0000000000000004",
        ),
        region(
            "apps.x.z",
            "apps/x/auxiliar.pink",
            "fnv1a64:0000000000000005",
        ),
    ]
}

/// Estado histórico esperado: as duas regiões antigas, com o hash de `a.b.um`
/// restaurado para o valor anterior.
fn estado_historico() -> Vec<CodeRegion> {
    vec![
        region("a.b.um", "src/um.rs", "fnv1a64:00000000000000ff"),
        region("a.b.dois", "src/dois.rs", "fnv1a64:0000000000000002"),
    ]
}

fn medidas_historicas() -> Measures {
    measure(estado_historico().iter())
}

/// Snapshot canônico que reconstrói `estado_historico` a partir de
/// `catalogo_corrente`.
fn snapshot_texto() -> String {
    let medidas = medidas_historicas();
    format!(
        concat!(
            "schema = 1\n",
            "id = \"historico-exemplo\"\n",
            "state = \"FROZEN\"\n",
            "predecessor = \"historico-anterior\"\n",
            "justification = \"fixture sintetica da campanha\"\n",
            "\n[reconstruction]\n",
            "expected_overrides = 1\n",
            "expected_exclusions = 2\n",
            "\n[measures]\n",
            "regions = {}\n",
            "length = {}\n",
            "fnv1a64 = \"{}\"\n",
            "\n[[rules]]\n",
            "op = \"exclude-key\"\n",
            "key = \"posterior.novo\"\n",
            "expected_matches = 1\n",
            "\n[[rules]]\n",
            "op = \"exclude-file-prefix\"\n",
            "prefix = \"apps/\"\n",
            "expected_matches = 2\n",
            "\n[[rules]]\n",
            "op = \"override-hash\"\n",
            "key = \"a.b.um\"\n",
            "from = \"fnv1a64:0000000000000001\"\n",
            "to = \"fnv1a64:00000000000000ff\"\n",
            "expect_file = \"src/um.rs\"\n",
            "expect_domain = \"dominio\"\n",
            "expect_layer = \"camada\"\n",
        ),
        medidas.regions,
        medidas.length,
        medidas.fnv1a64_canonical()
    )
}

fn snapshot() -> ProjectionSnapshot {
    parse(&snapshot_texto()).expect("fixture canônica é válida")
}

/// Substitui uma linha inteira da fixture, para os testes de sensibilidade.
fn com_linha(original: &str, substituta: &str) -> String {
    let texto = snapshot_texto();
    assert!(
        texto.contains(original),
        "linha ausente na fixture: {original}"
    );
    texto.replace(original, substituta)
}

// ---------------------------------------------------------------------------
// Parser: aceitação
// ---------------------------------------------------------------------------

#[test]
fn parse_aceita_snapshot_valido() {
    let snapshot = snapshot();
    // A fixture declara `schema = 1` e continua significando o que significava:
    // lista plana, sem composição. `SNAPSHOT_SCHEMA` passou a apontar para a
    // versão mais nova, então o teste fixa a versão do arquivo, não a máxima.
    assert_eq!(snapshot.schema, SNAPSHOT_SCHEMA_V1);
    assert_eq!(snapshot.base_snapshot, None);
    assert!(snapshot.recipes.is_empty());
    assert_eq!(snapshot.id, "historico-exemplo");
    assert_eq!(snapshot.state, SnapshotState::Frozen);
    assert_eq!(snapshot.predecessor.as_deref(), Some("historico-anterior"));
    assert_eq!(
        snapshot.justification.as_deref(),
        Some("fixture sintetica da campanha")
    );
    assert_eq!(snapshot.expected_overrides, 1);
    assert_eq!(snapshot.expected_exclusions, 2);
    assert_eq!(snapshot.rules.len(), 3);
    assert_eq!(snapshot.measures, medidas_historicas());
    assert_eq!(
        snapshot.relative_path(),
        format!("{SNAPSHOTS_DIR}historico-exemplo.toml")
    );
}

#[test]
fn parse_aceita_candidato_como_modelo() {
    let texto = com_linha("state = \"FROZEN\"", "state = \"CANDIDATE\"");
    let snapshot = parse(&texto).expect("candidato é modelo válido");
    assert_eq!(snapshot.state, SnapshotState::Candidate);
    assert_eq!(snapshot.state.as_str(), "CANDIDATE");
    // O modelo aceita o estado, mas este estágio não cria nem aceita candidatos:
    // não há API de preparação nem de aceitação exposta.
    assert!(!render(&snapshot).contains("preparar"));
}

#[test]
fn justificativa_ausente_com_predecessor_e_rejeitada() {
    let texto = snapshot_texto().replace("justification = \"fixture sintetica da campanha\"\n", "");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::MissingField { .. })
    ));
}

#[test]
fn candidato_sem_justificativa_e_rejeitado() {
    let texto = snapshot_texto()
        .replace("justification = \"fixture sintetica da campanha\"\n", "")
        .replace("predecessor = \"historico-anterior\"\n", "")
        .replace("state = \"FROZEN\"", "state = \"CANDIDATE\"");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::MissingField { .. })
    ));
}

#[test]
fn ordem_canonica_das_regras_e_independente_da_ordem_textual() {
    let snapshot = snapshot();
    let ops: Vec<&str> = snapshot.rules.iter().map(Rule::op).collect();
    assert_eq!(
        ops,
        vec!["exclude-key", "exclude-file-prefix", "override-hash"]
    );
}

// ---------------------------------------------------------------------------
// Renderer canônico
// ---------------------------------------------------------------------------

#[test]
fn render_e_canonico_e_estavel() {
    let snapshot = snapshot();
    let uma_vez = render(&snapshot);
    let duas_vezes = render(&parse(&uma_vez).expect("render volta a interpretar"));
    assert_eq!(uma_vez, duas_vezes, "render não é idempotente");
    assert_eq!(
        parse(&uma_vez).expect("reparse"),
        snapshot,
        "render/parse não preserva o modelo"
    );
}

#[test]
fn render_nao_depende_da_ordem_de_declaracao_das_regras() {
    let direto = snapshot();
    let mut invertido = direto.clone();
    invertido.rules.reverse();
    assert_eq!(render(&direto), render(&invertido));
}

#[test]
fn render_nao_carrega_root_absoluto_nem_estado_de_ambiente() {
    let texto = render(&snapshot());
    assert!(!texto.contains('\u{1b}'), "renderer não emite ANSI");
    for proibido in ["/home/", "/tmp/", "/pinker/", "\\u{1b}"] {
        assert!(
            !texto.contains(proibido),
            "render vazou estado de ambiente: {proibido}"
        );
    }
    // Nenhum caminho absoluto: todo path do modelo é repo-relativo.
    for linha in texto.lines() {
        assert!(!linha.contains("= \"/"), "path absoluto no render: {linha}");
    }
}

#[test]
fn render_escapa_texto_e_o_parser_devolve_o_original() {
    let mut snapshot = snapshot();
    snapshot.justification = Some("aspas \" barra \\ tab \t nova\nlinha".to_string());
    let texto = render(&snapshot);
    assert!(texto.contains("\\\"") && texto.contains("\\\\") && texto.contains("\\t"));
    assert!(
        !texto.contains("linha\nlinha"),
        "quebra literal não pode vazar para o TOML"
    );
    let reparsed = parse(&texto).expect("escaping é reversível");
    assert_eq!(reparsed.justification, snapshot.justification);
}

// ---------------------------------------------------------------------------
// Parser: rejeições estruturais (sensibilidade)
// ---------------------------------------------------------------------------

#[test]
fn schema_ausente_e_rejeitado() {
    let texto = snapshot_texto().replace("schema = 1\n", "");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::SchemaUnknown { found: 0, .. })
    ));
}

#[test]
fn schema_desconhecido_e_rejeitado() {
    // `2` passou a ser válido quando a composição chegou; o exemplo de versão
    // desconhecida acompanha a versão máxima aceita.
    let texto = com_linha("schema = 1", &format!("schema = {}", SNAPSHOT_SCHEMA + 1));
    match parse(&texto) {
        Err(HarnessFailure::SchemaUnknown { found, .. }) => assert_eq!(found, SNAPSHOT_SCHEMA + 1),
        outro => panic!("esperado schema desconhecido, veio {outro:?}"),
    }
}

#[test]
fn chave_desconhecida_e_rejeitada() {
    let texto = com_linha(
        "id = \"historico-exemplo\"",
        "id = \"historico-exemplo\"\napelido = \"x\"",
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::InvalidField { .. })
    ));
}

#[test]
fn chave_duplicada_e_rejeitada() {
    let texto = com_linha(
        "id = \"historico-exemplo\"",
        "id = \"historico-exemplo\"\nid = \"historico-exemplo\"",
    );
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("duplicada"), "{err}"),
        other => panic!("esperada falha de chave duplicada, veio {other:?}"),
    }
}

#[test]
fn secao_duplicada_e_rejeitada() {
    let texto = com_linha("[measures]", "[measures]\n\n[measures]");
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("duplicada"), "{err}"),
        other => panic!("esperada falha de seção duplicada, veio {other:?}"),
    }
}

#[test]
fn secao_desconhecida_e_rejeitada() {
    let texto = format!("{}\n[extra]\nx = 1\n", snapshot_texto());
    assert!(matches!(parse(&texto), Err(HarnessFailure::Toml(_))));
}

#[test]
fn estado_desconhecido_e_rejeitado() {
    let texto = com_linha("state = \"FROZEN\"", "state = \"DRAFT\"");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::StateUnknown { .. })
    ));
}

#[test]
fn id_inseguro_e_rejeitado() {
    for inseguro in [
        "\"../fuga\"",
        "\"/absoluto\"",
        "\"Maiuscula\"",
        "\"com espaco\"",
        "\"ponto.\"",
        "\"\"",
    ] {
        let texto = com_linha("id = \"historico-exemplo\"", &format!("id = {inseguro}"));
        assert!(
            matches!(parse(&texto), Err(HarnessFailure::IdUnsafe { .. })),
            "id inseguro aceito: {inseguro}"
        );
    }
}

#[test]
fn hash_invalido_e_rejeitado() {
    for invalido in [
        "\"fnv1a64:XYZ\"",
        "\"fnv1a64:00000000000000\"",
        "\"fnv1a64:00000000000000FF\"",
        "\"0000000000000001\"",
    ] {
        let texto = com_linha(
            "from = \"fnv1a64:0000000000000001\"",
            &format!("from = {invalido}"),
        );
        assert!(
            matches!(parse(&texto), Err(HarnessFailure::HashInvalid { .. })),
            "hash inválido aceito: {invalido}"
        );
    }
}

#[test]
fn numero_negativo_e_rejeitado() {
    let texto = com_linha("expected_overrides = 1", "expected_overrides = -1");
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("negativo"), "{err}"),
        other => panic!("esperada rejeição de negativo, veio {other:?}"),
    }
}

#[test]
fn overflow_de_inteiro_e_rejeitado() {
    let texto = com_linha(
        "expected_overrides = 1",
        "expected_overrides = 18446744073709551616",
    );
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("overflow"), "{err}"),
        other => panic!("esperada rejeição de overflow, veio {other:?}"),
    }
}

#[test]
fn predecessor_igual_ao_proprio_id_e_rejeitado() {
    let texto = com_linha(
        "predecessor = \"historico-anterior\"",
        "predecessor = \"historico-exemplo\"",
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::PredecessorSelfReference { .. })
    ));
}

#[test]
fn path_absoluto_e_rejeitado() {
    let texto = com_linha("prefix = \"apps/\"", "prefix = \"/apps/\"");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::PathAbsolute { .. })
    ));
}

#[test]
fn travessia_de_path_e_rejeitada() {
    let texto = com_linha("prefix = \"apps/\"", "prefix = \"../apps/\"");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::PathTraversal { .. })
    ));
    let texto = com_linha(
        "expect_file = \"src/um.rs\"",
        "expect_file = \"../src/um.rs\"",
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::PathTraversal { .. })
    ));
}

#[test]
fn regra_sem_operacao_e_rejeitada() {
    let texto = snapshot_texto().replace("op = \"exclude-key\"\n", "");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::RuleWithoutOperation { .. })
    ));
}

#[test]
fn regra_com_operacao_desconhecida_e_rejeitada() {
    let texto = com_linha("op = \"exclude-key\"", "op = \"rename-key\"");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::RuleOperationUnknown { .. })
    ));
}

#[test]
fn regra_sem_seletor_e_rejeitada() {
    let texto = snapshot_texto().replace("key = \"posterior.novo\"\n", "");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::RuleWithoutSelector { .. })
    ));
    let texto = snapshot_texto().replace("prefix = \"apps/\"\n", "");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::RuleWithoutSelector { .. })
    ));
}

#[test]
fn dado_residual_apos_o_valor_e_rejeitado() {
    let texto = com_linha(
        "id = \"historico-exemplo\"",
        "id = \"historico-exemplo\" lixo",
    );
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("residual"), "{err}"),
        other => panic!("esperada rejeição de dado residual, veio {other:?}"),
    }
}

#[test]
fn string_incompleta_e_rejeitada() {
    let texto = com_linha("id = \"historico-exemplo\"", "id = \"historico-exemplo");
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("incompleta"), "{err}"),
        other => panic!("esperada rejeição de string incompleta, veio {other:?}"),
    }
}

#[test]
fn escape_nao_suportado_e_rejeitado() {
    let texto = com_linha(
        "justification = \"fixture sintetica da campanha\"",
        "justification = \"fixture \\u0041 sintetica\"",
    );
    match parse(&texto) {
        Err(HarnessFailure::Toml(err)) => assert!(err.msg.contains("escape"), "{err}"),
        other => panic!("esperada rejeição de escape, veio {other:?}"),
    }
}

#[test]
fn comentario_e_linha_em_branco_sao_ignorados() {
    let texto = format!("# comentário de topo\n\n{}", snapshot_texto());
    assert_eq!(parse(&texto).expect("comentário aceito"), snapshot());
}

// ---------------------------------------------------------------------------
// Consumo de regras (sensibilidade)
// ---------------------------------------------------------------------------

#[test]
fn override_ausente_e_rejeitado() {
    let texto = com_linha("expected_overrides = 1", "expected_overrides = 2");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::OverrideMissing {
            declared: 2,
            found: 1
        })
    ));
}

#[test]
fn override_excedente_e_rejeitado() {
    let texto = com_linha("expected_overrides = 1", "expected_overrides = 0");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::OverrideExcess {
            declared: 0,
            found: 1
        })
    ));
}

#[test]
fn override_repetido_e_rejeitado() {
    let texto = format!(
        "{}\n[[rules]]\nop = \"override-hash\"\nkey = \"a.b.um\"\nfrom = \"fnv1a64:0000000000000001\"\nto = \"fnv1a64:00000000000000ff\"\n",
        com_linha("expected_overrides = 1", "expected_overrides = 2")
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::OverrideRepeated { .. })
    ));
}

#[test]
fn exclusao_ausente_ou_excedente_e_rejeitada() {
    let texto = com_linha("expected_exclusions = 2", "expected_exclusions = 3");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::ExclusionMissing { .. })
    ));
    let texto = com_linha("expected_exclusions = 2", "expected_exclusions = 1");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::ExclusionExcess { .. })
    ));
}

#[test]
fn exclusao_repetida_e_rejeitada() {
    let texto = format!(
        "{}\n[[rules]]\nop = \"exclude-key\"\nkey = \"posterior.novo\"\nexpected_matches = 1\n",
        com_linha("expected_exclusions = 2", "expected_exclusions = 3")
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::ExclusionRepeated { .. })
    ));
}

#[test]
fn exclusao_sem_correspondencia_e_falha_de_harness() {
    let texto = com_linha(
        "key = \"posterior.novo\"",
        "key = \"posterior.inexistente\"",
    );
    let snapshot = parse(&texto).expect("estruturalmente válido");
    assert!(matches!(
        reconstruct(&catalogo_corrente(), &snapshot),
        Err(HarnessFailure::ExclusionNoMatch { .. })
    ));
}

#[test]
fn exclusao_parcialmente_consumida_e_falha_de_harness() {
    let texto = com_linha(
        "prefix = \"apps/\"\nexpected_matches = 2",
        "prefix = \"apps/\"\nexpected_matches = 1",
    );
    let snapshot = parse(&texto).expect("estruturalmente válido");
    match reconstruct(&catalogo_corrente(), &snapshot) {
        Err(HarnessFailure::ExclusionPartiallyConsumed {
            expected, consumed, ..
        }) => {
            assert_eq!((expected, consumed), (1, 2));
        }
        other => panic!("esperada exclusão parcial, veio {other:?}"),
    }
}

#[test]
fn exclusao_com_orcamento_zero_e_rejeitada() {
    let texto = com_linha(
        "key = \"posterior.novo\"\nexpected_matches = 1",
        "key = \"posterior.novo\"\nexpected_matches = 0",
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::InvalidField { .. })
    ));
}

#[test]
fn regiao_removida_e_falha_de_harness() {
    let mut catalogo = catalogo_corrente();
    catalogo.retain(|r| r.key != "a.b.um");
    match reconstruct(&catalogo, &snapshot()) {
        Err(HarnessFailure::RegionRemoved { key }) => assert_eq!(key, "a.b.um"),
        other => panic!("esperada região removida, veio {other:?}"),
    }
}

#[test]
fn key_alterada_e_falha_de_harness() {
    let mut catalogo = catalogo_corrente();
    for regiao in &mut catalogo {
        if regiao.key == "a.b.um" {
            regiao.key = "a.b.renomeada".to_string();
        }
    }
    match reconstruct(&catalogo, &snapshot()) {
        Err(HarnessFailure::KeyChanged { expected, found }) => {
            assert_eq!(expected, "a.b.um");
            assert_eq!(found, "a.b.renomeada");
        }
        other => panic!("esperada key alterada, veio {other:?}"),
    }
}

#[test]
fn path_alterado_e_falha_de_harness() {
    let mut catalogo = catalogo_corrente();
    for regiao in &mut catalogo {
        if regiao.key == "a.b.um" {
            regiao.file = "src/movido.rs".to_string();
        }
    }
    match reconstruct(&catalogo, &snapshot()) {
        Err(HarnessFailure::PathChanged {
            expected, found, ..
        }) => {
            assert_eq!(expected, "src/um.rs");
            assert_eq!(found, "src/movido.rs");
        }
        other => panic!("esperado path alterado, veio {other:?}"),
    }
}

#[test]
fn metadata_alterada_e_falha_de_harness() {
    let mut catalogo = catalogo_corrente();
    for regiao in &mut catalogo {
        if regiao.key == "a.b.um" {
            regiao.layer = Some("outra".to_string());
        }
    }
    match reconstruct(&catalogo, &snapshot()) {
        Err(HarnessFailure::MetadataChanged { field, found, .. }) => {
            assert_eq!(field, "layer");
            assert_eq!(found, "outra");
        }
        other => panic!("esperada metadata alterada, veio {other:?}"),
    }
}

#[test]
fn seletor_ambiguo_e_falha_de_harness() {
    let mut catalogo = catalogo_corrente();
    catalogo.push(region(
        "a.b.um",
        "src/duplicado.rs",
        "fnv1a64:0000000000000001",
    ));
    match reconstruct(&catalogo, &snapshot()) {
        Err(HarnessFailure::SelectorAmbiguous { key, matches }) => {
            assert_eq!(key, "a.b.um");
            assert_eq!(matches, 2);
        }
        other => panic!("esperado seletor ambíguo, veio {other:?}"),
    }
}

#[test]
fn base_divergente_do_override_e_falha_de_harness() {
    let mut catalogo = catalogo_corrente();
    for regiao in &mut catalogo {
        if regiao.key == "a.b.um" {
            regiao.hash = "fnv1a64:00000000000000aa".to_string();
        }
    }
    assert!(matches!(
        reconstruct(&catalogo, &snapshot()),
        Err(HarnessFailure::OverrideStaleBase { .. })
    ));
}

// ---------------------------------------------------------------------------
// Reconstrução e consumo integral
// ---------------------------------------------------------------------------

#[test]
fn reconstrucao_produz_o_estado_historico_exato() {
    let reconstruction =
        reconstruct(&catalogo_corrente(), &snapshot()).expect("reconstrução válida");
    let mut obtido: Vec<String> = reconstruction
        .regions
        .iter()
        .map(|r| format!("{}|{}", r.key, r.hash))
        .collect();
    obtido.sort();
    let mut esperado: Vec<String> = estado_historico()
        .iter()
        .map(|r| format!("{}|{}", r.key, r.hash))
        .collect();
    esperado.sort();
    assert_eq!(obtido, esperado);
    assert_eq!(reconstruction.measures(), medidas_historicas());
}

#[test]
fn todas_as_regras_sao_consumidas_integralmente() {
    let reconstruction =
        reconstruct(&catalogo_corrente(), &snapshot()).expect("reconstrução válida");
    assert_eq!(reconstruction.ledger.len(), 3);
    for entrada in &reconstruction.ledger {
        assert_eq!(
            entrada.consumed, entrada.expected,
            "regra {} {} consumida fora do orçamento",
            entrada.op, entrada.selector
        );
    }
    let total: u64 = reconstruction.ledger.iter().map(|e| e.consumed).sum();
    assert_eq!(total, 4, "1 override + 1 chave + 2 arquivos de apps/");
}

#[test]
fn reconstrucao_nao_altera_a_entrada() {
    let base = catalogo_corrente();
    let copia = base.clone();
    let _ = reconstruct(&base, &snapshot()).expect("reconstrução válida");
    assert_eq!(base, copia, "a reconstrução mutou o catálogo de entrada");
}

// ---------------------------------------------------------------------------
// Classificação: MATCH, DRIFT e HARNESS_FAILURE
// ---------------------------------------------------------------------------

#[test]
fn snapshot_congelado_intacto_produz_match() {
    let report = verify(&snapshot(), &catalogo_corrente());
    assert_eq!(report.outcome, Outcome::Match);
    assert_eq!(report.state, SnapshotState::Frozen);
    assert_eq!(report.observed, Some(medidas_historicas()));
}

#[test]
fn drift_por_quantidade_de_regioes() {
    let mut snapshot = snapshot();
    snapshot.measures.regions += 1;
    let report = verify(&snapshot, &catalogo_corrente());
    match &report.outcome {
        Outcome::Drift(divergencias) => {
            assert_eq!(divergencias.len(), 1);
            assert_eq!(divergencias[0].measure, "regions");
        }
        other => panic!("esperado drift de regiões, veio {other:?}"),
    }
}

#[test]
fn drift_por_comprimento() {
    let mut snapshot = snapshot();
    snapshot.measures.length += 1;
    match &verify(&snapshot, &catalogo_corrente()).outcome {
        Outcome::Drift(divergencias) => {
            assert_eq!(divergencias.len(), 1);
            assert_eq!(divergencias[0].measure, "length");
        }
        other => panic!("esperado drift de comprimento, veio {other:?}"),
    }
}

#[test]
fn drift_por_fnv() {
    let mut snapshot = snapshot();
    snapshot.measures.fnv1a64 ^= 1;
    match &verify(&snapshot, &catalogo_corrente()).outcome {
        Outcome::Drift(divergencias) => {
            assert_eq!(divergencias.len(), 1);
            assert_eq!(divergencias[0].measure, "fnv1a64");
            assert!(divergencias[0].expected.starts_with(FNV_PREFIX));
        }
        other => panic!("esperado drift de FNV, veio {other:?}"),
    }
}

#[test]
fn drift_real_do_catalogo_e_detectado() {
    // Uma região histórica muda de conteúdo: o hash corrente difere e a medida
    // reconstruída deixa de bater com o snapshot congelado.
    let mut catalogo = catalogo_corrente();
    for regiao in &mut catalogo {
        if regiao.key == "a.b.dois" {
            regiao.hash = "fnv1a64:00000000000000bb".to_string();
        }
    }
    match &verify(&snapshot(), &catalogo).outcome {
        Outcome::Drift(divergencias) => {
            assert!(divergencias.iter().any(|d| d.measure == "fnv1a64"));
        }
        other => panic!("esperado drift real, veio {other:?}"),
    }
}

#[test]
fn falha_de_harness_nunca_vira_drift() {
    let mut catalogo = catalogo_corrente();
    catalogo.retain(|r| r.key != "posterior.novo");
    let report = verify(&snapshot(), &catalogo);
    assert!(matches!(report.outcome, Outcome::HarnessFailure(_)));
    assert_eq!(report.outcome.as_str(), "HARNESS_FAILURE");
    assert!(
        report.observed.is_none(),
        "sem reconstrução válida não pode existir medida observada"
    );
    assert!(report.ledger.is_empty());
}

#[test]
fn sensibilidade_da_classificacao_separa_causa_de_resultado() {
    // Mesmo catálogo, três snapshots: um íntegro, um com medida adulterada e um
    // com regra inconsistente. Os três resultados são distintos.
    let catalogo = catalogo_corrente();

    let integro = verify(&snapshot(), &catalogo);
    let mut adulterado = snapshot();
    adulterado.measures.length += 7;
    let com_drift = verify(&adulterado, &catalogo);
    let texto = com_linha("key = \"a.b.um\"", "key = \"a.b.ausente\"");
    let com_falha = verify(&parse(&texto).expect("estruturalmente válido"), &catalogo);

    assert_eq!(integro.outcome.as_str(), "MATCH");
    assert_eq!(com_drift.outcome.as_str(), "DRIFT");
    assert_eq!(com_falha.outcome.as_str(), "HARNESS_FAILURE");
}

// ---------------------------------------------------------------------------
// Independência de root absoluto
// ---------------------------------------------------------------------------

#[test]
fn medidas_sao_identicas_entre_roots_absolutos_diferentes() {
    // O catálogo guarda paths repo-relativos. Simula o mesmo repositório
    // materializado em dois roots absolutos distintos: as regiões são derivadas
    // relativizando cada root, e as medidas precisam coincidir byte a byte.
    fn relativizar(root: &str, absolutos: &[&str]) -> Vec<CodeRegion> {
        absolutos
            .iter()
            .enumerate()
            .map(|(indice, absoluto)| {
                let relativo = absoluto
                    .strip_prefix(root)
                    .expect("path pertence ao root")
                    .trim_start_matches('/');
                region(
                    &format!("k.{indice}"),
                    relativo,
                    &format!("fnv1a64:{:016x}", indice as u64 + 1),
                )
            })
            .collect()
    }

    let um = relativizar(
        "/var/tmp/clone-a",
        &["/var/tmp/clone-a/src/um.rs", "/var/tmp/clone-a/src/dois.rs"],
    );
    let outro = relativizar(
        "/var/tmp/outro-root-bem-mais-longo",
        &[
            "/var/tmp/outro-root-bem-mais-longo/src/um.rs",
            "/var/tmp/outro-root-bem-mais-longo/src/dois.rs",
        ],
    );

    assert_eq!(
        stable_projection(um.iter()),
        stable_projection(outro.iter())
    );
    assert_eq!(measure(um.iter()), measure(outro.iter()));

    let esperado = measure(um.iter());
    assert!(
        !stable_projection(um.iter()).contains("/pinker/"),
        "a projeção não pode conter root absoluto"
    );
    assert_eq!(esperado, measure(outro.iter()));
}

#[test]
fn relatorios_sao_identicos_entre_execucoes_e_sem_root_absoluto() {
    let report = verify(&snapshot(), &catalogo_corrente());
    let json_a = json_report(&report);
    let json_b = json_report(&verify(&snapshot(), &catalogo_corrente()));
    assert_eq!(json_a, json_b);
    assert!(!json_a.contains('\u{1b}'), "JSON não pode conter ANSI");
    for proibido in ["/home/", "/tmp/", "/pinker/", "/var/"] {
        assert!(!json_a.contains(proibido), "root absoluto no JSON");
    }
}

// ---------------------------------------------------------------------------
// Relatórios
// ---------------------------------------------------------------------------

#[test]
fn json_e_de_uma_linha_com_ordem_fixa() {
    let json = json_report(&verify(&snapshot(), &catalogo_corrente()));
    assert!(!json.contains('\n'), "JSON precisa ser de uma linha");
    let ordem = [
        "\"schema\":",
        "\"snapshot\":",
        "\"state\":",
        "\"predecessor\":",
        "\"outcome\":",
        "\"expected\":",
        "\"observed\":",
        "\"divergences\":",
        "\"failure\":",
        "\"consumption\":",
    ];
    let mut anterior = 0usize;
    for campo in ordem {
        let posicao = json
            .find(campo)
            .unwrap_or_else(|| panic!("campo ausente: {campo}"));
        assert!(posicao >= anterior, "ordem de chaves instável em {campo}");
        anterior = posicao;
    }
    assert!(json.contains("\"outcome\":\"MATCH\""));
    assert!(json.contains("\"failure\":null"));
}

#[test]
fn json_de_falha_carrega_codigo_estavel() {
    let mut catalogo = catalogo_corrente();
    catalogo.retain(|r| r.key != "a.b.um");
    let json = json_report(&verify(&snapshot(), &catalogo));
    assert!(json.contains("\"outcome\":\"HARNESS_FAILURE\""));
    assert!(json.contains("\"code\":\"E-SNAP-REGIAO-REMOVIDA\""));
    assert!(json.contains("\"observed\":null"));
    assert!(!json.contains('\n'));
}

#[test]
fn json_escapa_caracteres_de_controle_e_aspas() {
    let mut snapshot = snapshot();
    snapshot.id = "historico-exemplo".to_string();
    let mut report = verify(&snapshot, &catalogo_corrente());
    report.snapshot_id = "aspas \" barra \\ tab \t".to_string();
    let json = json_report(&report);
    assert!(json.contains("\\\"") && json.contains("\\\\") && json.contains("\\t"));
    assert!(
        !json.contains('\t'),
        "tab literal não pode vazar para o JSON"
    );
}

#[test]
fn relatorio_humano_e_deterministico_e_sem_ansi() {
    let report = verify(&snapshot(), &catalogo_corrente());
    let a = human_report(&report);
    let b = human_report(&report);
    assert_eq!(a, b);
    assert!(!a.contains('\u{1b}'));
    assert!(a.starts_with("snapshot historico-exemplo [FROZEN] MATCH\n"));
    assert!(a.contains("consumo override-hash a.b.um: 1/1"));
}

#[test]
fn relatorio_humano_de_drift_lista_cada_medida() {
    let mut snapshot = snapshot();
    snapshot.measures.regions += 1;
    snapshot.measures.length += 1;
    let texto = human_report(&verify(&snapshot, &catalogo_corrente()));
    assert!(texto.contains("drift regions:"));
    assert!(texto.contains("drift length:"));
    assert!(!texto.contains("drift fnv1a64:"));
}

// ---------------------------------------------------------------------------
// FNV e projeção estável
// ---------------------------------------------------------------------------

#[test]
fn fnv1a64_reproduz_os_vetores_canonicos() {
    assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn projecao_estavel_ignora_posicao_de_linha() {
    let mut deslocada = estado_historico();
    for regiao in &mut deslocada {
        regiao.start_marker += 100;
        regiao.content_start += 100;
        regiao.content_end += 100;
        regiao.end_marker += 100;
    }
    assert_eq!(
        stable_projection(estado_historico().iter()),
        stable_projection(deslocada.iter())
    );
}

#[test]
fn projecao_estavel_e_sensivel_a_cada_campo_medido() {
    let referencia = stable_projection(estado_historico().iter());
    for mutacao in 0..5 {
        let mut regioes = estado_historico();
        match mutacao {
            0 => regioes[0].key.push('x'),
            1 => regioes[0].kind.push('x'),
            2 => regioes[0].file.push('x'),
            3 => regioes[0].summary.push('x'),
            _ => regioes[0].status.push('x'),
        }
        assert_ne!(
            referencia,
            stable_projection(regioes.iter()),
            "mutação {mutacao} não alterou a projeção"
        );
    }
}

// ---------------------------------------------------------------------------
// Fronteiras negativas do recorte
// ---------------------------------------------------------------------------

const FONTE: &str = include_str!("../src/nav_projection_snapshot.rs");

#[test]
fn o_modulo_nao_escreve_nem_le_o_filesystem() {
    for proibido in [
        "std::fs",
        "fs::write",
        "fs::read",
        "File::create",
        "File::open",
        "OpenOptions",
        "create_new",
        "rename(",
        "remove_file",
        "TempDir",
        "temp_dir",
    ] {
        assert!(
            !FONTE.contains(proibido),
            "o núcleo somente leitura tocou filesystem: {proibido}"
        );
    }
}

#[test]
fn o_modulo_nao_usa_rede_processos_nem_git() {
    for proibido in [
        "TcpStream",
        "TcpListener",
        "UdpSocket",
        "std::net",
        "Command::new",
        "std::process",
        "gh pr",
        "git rev-parse",
        "https://",
    ] {
        assert!(
            !FONTE.contains(proibido),
            "o núcleo somente leitura alcançou o mundo externo: {proibido}"
        );
    }
}

#[test]
fn o_modulo_nao_depende_de_estado_nao_deterministico() {
    for proibido in [
        "SystemTime",
        "Instant::",
        "std::time",
        "HashMap<",
        "HashSet<",
        "std::env",
        "random",
        "as *const",
    ] {
        assert!(
            !FONTE.contains(proibido),
            "o núcleo determinístico admitiu estado instável: {proibido}"
        );
    }
}

#[test]
fn o_nucleo_de_snapshot_nao_expoe_lifecycle_mutavel() {
    for proibido in [
        "pub fn prepare",
        "pub fn preparar",
        "pub fn accept",
        "pub fn aceitar",
        "pub fn write",
        "pub fn save",
        "pub fn load",
    ] {
        assert!(
            !FONTE.contains(proibido),
            "superfície mutável exposta antes do estágio devido: {proibido}"
        );
    }
    let cli = fonte_de_modulo::pink_cli();
    // Stage E pode classificar Outcome na borda, mas parsing, renderização,
    // medidas e reconstrução continuam fora do adaptador CLI.
    for proibido in [
        "parse_snapshot(",
        "render_snapshot(",
        "measure(",
        "apply_rules(",
    ] {
        assert!(
            !cli.contains(proibido),
            "a CLI assumiu autoridade do núcleo de snapshot: {proibido}"
        );
    }
}

/// A guarda de proliferação da autoridade histórica.
///
/// No estágio somente leitura ela exigia que `.pinker/projections/` **não
/// existisse**. A materialização da Issue #384 criou a autoridade autorizada, e
/// a guarda passou a exigir o que a substitui: exatamente os artefatos
/// aprovados, nem um a mais. O propósito não mudou — impedir que snapshots
/// apareçam sem decisão humana; mudou o que conta como estado correto.
#[test]
fn a_autoridade_historica_tem_exatamente_os_arquivos_autorizados() {
    let raiz = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let diretorio = raiz.join(SNAPSHOTS_DIR);
    assert!(
        diretorio.exists(),
        "a autoridade histórica materializada vive em {SNAPSHOTS_DIR}"
    );

    let mut snapshots: Vec<String> = std::fs::read_dir(&diretorio)
        .expect("diretório de snapshots legível")
        .map(|entrada| entrada.expect("entrada").path())
        .filter(|caminho| caminho.is_file())
        .map(|caminho| {
            caminho
                .file_name()
                .expect("nome")
                .to_str()
                .expect("utf-8")
                .to_string()
        })
        .collect();
    snapshots.sort();
    assert_eq!(
        snapshots.len(),
        13,
        "exatamente treze snapshots históricos: {snapshots:?}"
    );
    assert!(
        snapshots.iter().all(|nome| nome.ends_with(".toml")),
        "somente TOML na raiz da autoridade: {snapshots:?}"
    );

    let receitas = raiz.join(RECIPES_DIR);
    let mut nomes: Vec<String> = std::fs::read_dir(receitas)
        .expect("diretório de receitas legível")
        .map(|entrada| entrada.expect("entrada").path())
        .filter(|caminho| caminho.is_file())
        .map(|caminho| {
            caminho
                .file_name()
                .expect("nome")
                .to_str()
                .expect("utf-8")
                .to_string()
        })
        .collect();
    nomes.sort();
    assert_eq!(
        nomes,
        vec!["normalizacao-corrente-para-historico.toml".to_string()],
        "uma única receita técnica de normalização"
    );
}
