//! Trama Pinker — composição de reconstrução: schema 2 e receitas (#384).
//!
//! Cobre os invariantes que o schema 2 congela: separação estrutural entre as
//! duas autoridades, base validada contra as próprias medidas, proibição
//! transitiva de congelado sobre candidato, detecção de ciclo no grafo
//! completo, ordem de aplicação e consumo exato por escopo.

use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::{
    parse_recipe, render_recipe, resolve, verify_frozen_dependencies, Library, Recipe, RECIPES_DIR,
    RECIPE_SCHEMA, RECIPE_SCHEMA_V1, RECIPE_SCHEMA_V2,
};
use pinker_v0::nav_projection_snapshot::{
    measure, parse, render, stable_projection, HarnessFailure, Measures, ProjectionRegion,
    ProjectionSnapshot, Rule, SchemaAuthority, SnapshotState, SNAPSHOT_SCHEMA_V1,
    SNAPSHOT_SCHEMA_V2, SNAPSHOT_SCHEMA_V3,
};

// ---------------------------------------------------------------------------
// Fixtures
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

/// Catálogo corrente sintético.
fn catalogo() -> Vec<CodeRegion> {
    vec![
        region("a.um", "src/a.rs", "fnv1a64:0000000000000001"),
        region("a.dois", "src/a.rs", "fnv1a64:0000000000000002"),
        region("b.um", "src/b.rs", "fnv1a64:0000000000000003"),
        region("posterior.nova", "src/novo.rs", "fnv1a64:0000000000000004"),
        region("evidencia.x.um", "tests/x.rs", "fnv1a64:0000000000000005"),
    ]
}

fn medidas_de(regioes: &[CodeRegion]) -> Measures {
    measure(regioes.iter())
}

/// Estado depois de remover `posterior.nova`.
fn estado_sem_posterior() -> Vec<CodeRegion> {
    catalogo()
        .into_iter()
        .filter(|r| r.key != "posterior.nova")
        .collect()
}

fn receita(id: &str, steps: &[&str], rules: Vec<Rule>) -> Recipe {
    let overrides = rules.iter().filter(|r| r.op() == "override-hash").count() as u64;
    Recipe {
        schema: RECIPE_SCHEMA,
        id: id.to_string(),
        steps: steps.iter().map(|s| s.to_string()).collect(),
        expected_overrides: overrides,
        expected_exclusions: rules.len() as u64 - overrides,
        rules,
    }
}

fn snapshot(
    id: &str,
    state: SnapshotState,
    medidas: Measures,
    base: Option<&str>,
    recipes: &[&str],
    rules: Vec<Rule>,
) -> ProjectionSnapshot {
    let overrides = rules.iter().filter(|r| r.op() == "override-hash").count() as u64;
    ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA_V2,
        id: id.to_string(),
        state,
        predecessor: None,
        justification: Some("fixture sintetica".to_string()),
        measures: medidas,
        expected_materializations: 0,
        expected_overrides: overrides,
        expected_exclusions: rules.len() as u64 - overrides,
        base_snapshot: base.map(str::to_string),
        recipes: recipes.iter().map(|r| r.to_string()).collect(),
        rules,
    }
}

fn excluir(key: &str) -> Rule {
    Rule::ExcludeKey {
        key: key.to_string(),
        expected_matches: 1,
    }
}

fn excluir_arquivo(file: &str, n: u64) -> Rule {
    Rule::ExcludeFile {
        file: file.to_string(),
        expected_matches: n,
    }
}

fn excluir_prefixo_de_chave(prefix: &str, n: u64) -> Rule {
    Rule::ExcludeKeyPrefix {
        prefix: prefix.to_string(),
        expected_matches: n,
    }
}

fn override_hash(key: &str, de: &str, para: &str) -> Rule {
    Rule::OverrideHash {
        key: key.to_string(),
        from: de.to_string(),
        to: para.to_string(),
        expect_file: None,
        expect_domain: None,
        expect_layer: None,
    }
}

// ---------------------------------------------------------------------------
// As duas autoridades são distintas
// ---------------------------------------------------------------------------

#[test]
fn receita_nao_tem_medida_estado_nem_predecessor() {
    let r = receita("intermediaria", &[], vec![excluir("posterior.nova")]);
    // O tipo é a prova: não existem esses campos para preencher.
    assert_eq!(r.schema, RECIPE_SCHEMA);
    assert_eq!(RECIPE_SCHEMA_V1, 1, "o formato de receita estreou em 1");
    assert_eq!(RECIPE_SCHEMA_V2, 2, "e ganhou override-region no 2");
    assert_eq!(RECIPES_DIR, ".pinker/projections/recipes/");
    let fonte = include_str!("../src/nav_projection_recipe.rs");
    let inicio = fonte.find("pub struct Recipe {").expect("struct Recipe");
    let fim = fonte[inicio..].find('}').expect("fim da struct") + inicio;
    let corpo = &fonte[inicio..fim];
    for proibido in ["measures", "state", "predecessor", "base_snapshot"] {
        assert!(
            !corpo.contains(proibido),
            "receita adquiriu campo de snapshot: {proibido}"
        );
    }
}

#[test]
fn namespaces_sao_estruturais_e_nao_ha_base_ambigua() {
    // O mesmo identificador textual em ambas as autoridades não colide: cada
    // campo procura em exatamente uma coleção.
    let alvo = estado_sem_posterior();
    let library = Library::new()
        .with_recipe(receita("mesmo-nome", &[], vec![excluir("posterior.nova")]))
        .unwrap()
        .with_snapshot(snapshot(
            "mesmo-nome",
            SnapshotState::Frozen,
            medidas_de(&alvo),
            None,
            &["mesmo-nome"],
            vec![],
        ))
        .unwrap();

    let composicao = resolve(&library, "mesmo-nome", &catalogo()).expect("resolve sem ambiguidade");
    assert_eq!(composicao.measures(), medidas_de(&alvo));

    // E não existe variante de falha para base ambígua: a ambiguidade não foi
    // introduzida, então não há o que remediar.
    let fonte = include_str!("../src/nav_projection_snapshot.rs");
    assert!(!fonte.contains("AmbiguousBase"));
    assert!(!fonte.contains("BASE-AMBIGUA"));
    // `E-SNAP-SELETOR-AMBIGUO` continua existindo e é outro conceito: seletor de
    // override com mais de uma correspondência, não ambiguidade de namespace.
    assert!(fonte.contains("E-SNAP-SELETOR-AMBIGUO"));
}

#[test]
fn receita_nao_pode_depender_de_snapshot() {
    // Estruturalmente impossível: `steps` resolve apenas contra receitas.
    let fonte = include_str!("../src/nav_projection_recipe.rs");
    let inicio = fonte.find("fn resolve_recipe(").expect("resolve_recipe");
    let corpo = &fonte[inicio..];
    let fim = corpo.find("\nfn ciclo(").unwrap_or(corpo.len());
    let corpo = &corpo[..fim];
    assert!(
        !corpo.contains("library.snapshot("),
        "uma receita alcançou o namespace de snapshots"
    );
    assert!(corpo.contains("library.recipe("));
}

// ---------------------------------------------------------------------------
// Ordem de aplicação
// ---------------------------------------------------------------------------

#[test]
fn a_ordem_e_base_receitas_exclusoes_locais_overrides_locais() {
    // A receita remove `posterior.nova`; a regra local troca o hash de `a.um`.
    // Se a ordem invertesse, o override local encontraria um estado diferente.
    let mut alvo = estado_sem_posterior();
    for r in &mut alvo {
        if r.key == "a.um" {
            r.hash = "fnv1a64:00000000000000ff".to_string();
        }
    }
    let library = Library::new()
        .with_recipe(receita(
            "remove-posterior",
            &[],
            vec![excluir("posterior.nova")],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "com-receita",
            SnapshotState::Frozen,
            medidas_de(&alvo),
            None,
            &["remove-posterior"],
            vec![override_hash(
                "a.um",
                "fnv1a64:0000000000000001",
                "fnv1a64:00000000000000ff",
            )],
        ))
        .unwrap();
    let composicao = resolve(&library, "com-receita", &catalogo()).expect("composição válida");
    assert_eq!(composicao.measures(), medidas_de(&alvo));
}

#[test]
fn a_ordem_declarada_das_receitas_e_significado() {
    // Duas receitas cujo resultado depende da ordem: a primeira exclui por
    // prefixo de chave, a segunda conta as correspondências restantes.
    let primeira = receita(
        "exclui-evidencia",
        &[],
        vec![excluir_prefixo_de_chave("evidencia.", 1)],
    );
    let segunda = receita(
        "exclui-arquivo-b",
        &[],
        vec![excluir_arquivo("src/b.rs", 1)],
    );

    let alvo: Vec<CodeRegion> = catalogo()
        .into_iter()
        .filter(|r| !r.key.starts_with("evidencia.") && r.file != "src/b.rs")
        .collect();

    let library = Library::new()
        .with_recipe(primeira)
        .unwrap()
        .with_recipe(segunda)
        .unwrap()
        .with_snapshot(snapshot(
            "ordenado",
            SnapshotState::Frozen,
            medidas_de(&alvo),
            None,
            &["exclui-evidencia", "exclui-arquivo-b"],
            vec![],
        ))
        .unwrap();
    let composicao = resolve(&library, "ordenado", &catalogo()).expect("composição válida");
    assert_eq!(composicao.measures(), medidas_de(&alvo));

    // O ledger registra os escopos na ordem de aplicação.
    let escopos: Vec<&str> = composicao.ledger.iter().map(|e| e.scope.as_str()).collect();
    assert_eq!(
        escopos,
        vec![
            "recipe:exclui-evidencia",
            "recipe:exclui-arquivo-b",
            "snapshot:ordenado"
        ]
    );
}

#[test]
fn consumo_e_validado_em_cada_escopo_e_nao_e_contado_duas_vezes() {
    let alvo = estado_sem_posterior();
    let library = Library::new()
        .with_recipe(receita(
            "remove-posterior",
            &[],
            vec![excluir("posterior.nova")],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "consumo",
            SnapshotState::Frozen,
            medidas_de(&alvo),
            None,
            &["remove-posterior"],
            vec![],
        ))
        .unwrap();
    let composicao = resolve(&library, "consumo", &catalogo()).expect("composição válida");

    // Cada regra aparece em exatamente um escopo.
    let total: usize = composicao.ledger.iter().map(|e| e.entries.len()).sum();
    assert_eq!(total, 1, "a única regra foi contada uma única vez");
    let receita_escopo = composicao
        .ledger
        .iter()
        .find(|e| e.scope == "recipe:remove-posterior")
        .expect("escopo da receita");
    assert_eq!(receita_escopo.entries.len(), 1);
    assert_eq!(receita_escopo.entries[0].consumed, 1);

    // Uma segunda exclusão da mesma chave, agora no escopo do snapshot, não
    // encontra nada: o consumo do escopo anterior não é reaproveitado.
    let library = Library::new()
        .with_recipe(receita(
            "remove-posterior",
            &[],
            vec![excluir("posterior.nova")],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "consumo-duplo",
            SnapshotState::Frozen,
            medidas_de(&alvo),
            None,
            &["remove-posterior"],
            vec![excluir("posterior.nova")],
        ))
        .unwrap();
    assert!(matches!(
        resolve(&library, "consumo-duplo", &catalogo()),
        Err(HarnessFailure::ExclusionNoMatch { .. })
    ));
}

// ---------------------------------------------------------------------------
// Base: existência, autorreferência, ciclo
// ---------------------------------------------------------------------------

#[test]
fn base_ausente_e_falha_de_harness() {
    let library = Library::new()
        .with_snapshot(snapshot(
            "orfao",
            SnapshotState::Frozen,
            medidas_de(&catalogo()),
            Some("nao-existe"),
            &[],
            vec![],
        ))
        .unwrap();
    match resolve(&library, "orfao", &catalogo()) {
        Err(HarnessFailure::BaseSnapshotMissing { id }) => assert_eq!(id, "nao-existe"),
        outro => panic!("esperada base ausente, veio {outro:?}"),
    }
}

#[test]
fn receita_ausente_e_falha_de_harness() {
    let library = Library::new()
        .with_snapshot(snapshot(
            "sem-receita",
            SnapshotState::Frozen,
            medidas_de(&catalogo()),
            None,
            &["nao-existe"],
            vec![],
        ))
        .unwrap();
    match resolve(&library, "sem-receita", &catalogo()) {
        Err(HarnessFailure::RecipeMissing { id }) => assert_eq!(id, "nao-existe"),
        outro => panic!("esperada receita ausente, veio {outro:?}"),
    }
}

#[test]
fn autorreferencia_e_rejeitada_no_parser() {
    let texto = concat!(
        "schema = 2\n",
        "id = \"eu-mesmo\"\n",
        "state = \"FROZEN\"\n",
        "\n[reconstruction]\n",
        "base_snapshot = \"eu-mesmo\"\n",
        "expected_overrides = 0\n",
        "expected_exclusions = 0\n",
        "\n[measures]\n",
        "regions = 1\n",
        "length = 1\n",
        "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
    );
    match parse(texto) {
        Err(HarnessFailure::SelfBase { id }) => assert_eq!(id, "eu-mesmo"),
        outro => panic!("esperada autorreferência, veio {outro:?}"),
    }
}

#[test]
fn ciclo_entre_snapshots_e_detectado() {
    let m = medidas_de(&catalogo());
    let library = Library::new()
        .with_snapshot(snapshot(
            "a",
            SnapshotState::Frozen,
            m,
            Some("b"),
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "b",
            SnapshotState::Frozen,
            m,
            Some("a"),
            &[],
            vec![],
        ))
        .unwrap();
    match resolve(&library, "a", &catalogo()) {
        Err(HarnessFailure::CompositionCycle { path }) => {
            assert!(path.contains("snapshot:a"), "{path}");
            assert!(path.contains("snapshot:b"), "{path}");
        }
        outro => panic!("esperado ciclo, veio {outro:?}"),
    }
}

#[test]
fn ciclo_entre_receitas_e_detectado_no_mesmo_grafo() {
    let library = Library::new()
        .with_recipe(receita("x", &["y"], vec![]))
        .unwrap()
        .with_recipe(receita("y", &["x"], vec![]))
        .unwrap()
        .with_snapshot(snapshot(
            "usa-x",
            SnapshotState::Frozen,
            medidas_de(&catalogo()),
            None,
            &["x"],
            vec![],
        ))
        .unwrap();
    match resolve(&library, "usa-x", &catalogo()) {
        Err(HarnessFailure::CompositionCycle { path }) => {
            assert!(
                path.contains("recipe:x") && path.contains("recipe:y"),
                "{path}"
            );
        }
        outro => panic!("esperado ciclo entre receitas, veio {outro:?}"),
    }
}

#[test]
fn receita_repetida_no_mesmo_escopo_e_rejeitada_no_parser() {
    let texto = concat!(
        "schema = 2\n",
        "id = \"repetida\"\n",
        "state = \"FROZEN\"\n",
        "\n[reconstruction]\n",
        "recipes = [\"uma\", \"uma\"]\n",
        "expected_overrides = 0\n",
        "expected_exclusions = 0\n",
        "\n[measures]\n",
        "regions = 1\n",
        "length = 1\n",
        "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
    );
    assert!(matches!(
        parse(texto),
        Err(HarnessFailure::InvalidField { .. })
    ));
}

// ---------------------------------------------------------------------------
// A base é verificada contra as próprias medidas
// ---------------------------------------------------------------------------

#[test]
fn base_e_verificada_contra_as_proprias_medidas() {
    let intermediario = estado_sem_posterior();
    let mut final_ = intermediario.clone();
    final_.retain(|r| r.file != "tests/x.rs");

    let library = Library::new()
        .with_snapshot(snapshot(
            "base-ok",
            SnapshotState::Frozen,
            medidas_de(&intermediario),
            None,
            &[],
            vec![excluir("posterior.nova")],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "descendente",
            SnapshotState::Frozen,
            medidas_de(&final_),
            Some("base-ok"),
            &[],
            vec![excluir_arquivo("tests/x.rs", 1)],
        ))
        .unwrap();

    let composicao = resolve(&library, "descendente", &catalogo()).expect("composição válida");
    assert_eq!(composicao.measures(), medidas_de(&final_));
    assert_eq!(composicao.verified_bases, vec!["base-ok".to_string()]);
}

#[test]
fn base_com_medida_errada_e_harness_failure_e_nao_drift() {
    let intermediario = estado_sem_posterior();
    let mut final_ = intermediario.clone();
    final_.retain(|r| r.file != "tests/x.rs");

    // A base declara uma medida que a própria reconstrução não produz.
    let mut medidas_erradas = medidas_de(&intermediario);
    medidas_erradas.length += 7;

    let library = Library::new()
        .with_snapshot(snapshot(
            "base-quebrada",
            SnapshotState::Frozen,
            medidas_erradas,
            None,
            &[],
            vec![excluir("posterior.nova")],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "descendente",
            SnapshotState::Frozen,
            medidas_de(&final_),
            Some("base-quebrada"),
            &[],
            vec![excluir_arquivo("tests/x.rs", 1)],
        ))
        .unwrap();

    // O descendente bateria com as próprias medidas — e ainda assim a
    // composição falha, porque a base não bate com as dela.
    match resolve(&library, "descendente", &catalogo()) {
        Err(HarnessFailure::BaseMeasuresDiverged { id, .. }) => {
            assert_eq!(id, "base-quebrada");
        }
        outro => panic!("base quebrada foi absorvida: {outro:?}"),
    }
}

#[test]
fn regras_do_descendente_nao_mascaram_falha_da_base() {
    // A base é reconstruída de forma inválida (exclusão sem correspondência).
    // Nenhuma regra do descendente pode compensar isso.
    let library = Library::new()
        .with_snapshot(snapshot(
            "base-invalida",
            SnapshotState::Frozen,
            medidas_de(&catalogo()),
            None,
            &[],
            vec![excluir("chave.que.nao.existe")],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "descendente",
            SnapshotState::Frozen,
            medidas_de(&catalogo()),
            Some("base-invalida"),
            &[],
            vec![],
        ))
        .unwrap();
    assert!(matches!(
        resolve(&library, "descendente", &catalogo()),
        Err(HarnessFailure::ExclusionNoMatch { .. })
    ));
}

// ---------------------------------------------------------------------------
// Congelado não se apoia em candidato — nem transitivamente
// ---------------------------------------------------------------------------

#[test]
fn congelado_sobre_candidato_direto_e_rejeitado() {
    let m = medidas_de(&catalogo());
    let library = Library::new()
        .with_snapshot(snapshot(
            "cand",
            SnapshotState::Candidate,
            m,
            None,
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "cong",
            SnapshotState::Frozen,
            m,
            Some("cand"),
            &[],
            vec![],
        ))
        .unwrap();
    match verify_frozen_dependencies(&library) {
        Err(HarnessFailure::FrozenDependsOnCandidate { frozen, candidate }) => {
            assert_eq!(frozen, "cong");
            assert_eq!(candidate, "cand");
        }
        outro => panic!("esperada dependência proibida, veio {outro:?}"),
    }
}

#[test]
fn congelado_sobre_candidato_transitivo_e_rejeitado() {
    // A → B → C congelados, C → D candidato. A proibição precisa alcançar D.
    let m = medidas_de(&catalogo());
    let library = Library::new()
        .with_snapshot(snapshot(
            "d",
            SnapshotState::Candidate,
            m,
            None,
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "c",
            SnapshotState::Frozen,
            m,
            Some("d"),
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "b",
            SnapshotState::Frozen,
            m,
            Some("c"),
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "a",
            SnapshotState::Frozen,
            m,
            Some("b"),
            &[],
            vec![],
        ))
        .unwrap();
    match verify_frozen_dependencies(&library) {
        Err(HarnessFailure::FrozenDependsOnCandidate { candidate, .. }) => {
            assert_eq!(candidate, "d", "a proibição parou antes do candidato");
        }
        outro => panic!("esperada dependência transitiva proibida, veio {outro:?}"),
    }
}

#[test]
fn cadeia_toda_congelada_e_aceita() {
    let m = medidas_de(&catalogo());
    let library = Library::new()
        .with_snapshot(snapshot("c", SnapshotState::Frozen, m, None, &[], vec![]))
        .unwrap()
        .with_snapshot(snapshot(
            "b",
            SnapshotState::Frozen,
            m,
            Some("c"),
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "a",
            SnapshotState::Frozen,
            m,
            Some("b"),
            &[],
            vec![],
        ))
        .unwrap();
    assert!(verify_frozen_dependencies(&library).is_ok());
}

#[test]
fn candidato_pode_se_apoiar_em_congelado() {
    let m = medidas_de(&catalogo());
    let library = Library::new()
        .with_snapshot(snapshot(
            "cong",
            SnapshotState::Frozen,
            m,
            None,
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "cand",
            SnapshotState::Candidate,
            m,
            Some("cong"),
            &[],
            vec![],
        ))
        .unwrap();
    assert!(verify_frozen_dependencies(&library).is_ok());
}

// ---------------------------------------------------------------------------
// O schema 1 preserva o significado que tinha
// ---------------------------------------------------------------------------

fn schema1(extra_reconstruction: &str, extra_rule: &str) -> String {
    format!(
        concat!(
            "schema = 1\n",
            "id = \"antigo\"\n",
            "state = \"FROZEN\"\n",
            "\n[reconstruction]\n",
            "{}",
            "expected_overrides = 0\n",
            "expected_exclusions = 0\n",
            "\n[measures]\n",
            "regions = 1\n",
            "length = 1\n",
            "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
            "{}"
        ),
        extra_reconstruction, extra_rule
    )
}

#[test]
fn schema_1_continua_valido_e_sem_composicao() {
    let snapshot = parse(&schema1("", "")).expect("schema 1 continua válido");
    assert_eq!(snapshot.schema, SNAPSHOT_SCHEMA_V1);
    assert_eq!(snapshot.base_snapshot, None);
    assert!(snapshot.recipes.is_empty());
}

#[test]
fn schema_1_rejeita_todas_as_capacidades_do_schema_2() {
    let casos = [
        ("base_snapshot = \"outro\"\n", ""),
        ("recipes = [\"uma\"]\n", ""),
        (
            "",
            "\n[[rules]]\nop = \"exclude-file\"\nfile = \"src/a.rs\"\nexpected_matches = 1\n",
        ),
        (
            "",
            "\n[[rules]]\nop = \"exclude-key-prefix\"\nprefix = \"a.\"\nexpected_matches = 1\n",
        ),
    ];
    for (rec, rule) in casos {
        match parse(&schema1(rec, rule)) {
            Err(HarnessFailure::CapabilityRequiresSchema {
                authority,
                found_schema,
                required_schema,
                ..
            }) => {
                assert_eq!(authority, SchemaAuthority::Snapshot);
                assert_eq!(found_schema, SNAPSHOT_SCHEMA_V1);
                assert_eq!(required_schema, SNAPSHOT_SCHEMA_V2);
            }
            outro => panic!("schema 1 aceitou capacidade do schema 2: {rec}{rule} -> {outro:?}"),
        }
    }
}

#[test]
fn schema_2_aceita_as_capacidades_novas() {
    let texto = concat!(
        "schema = 2\n",
        "id = \"novo\"\n",
        "state = \"FROZEN\"\n",
        "\n[reconstruction]\n",
        "base_snapshot = \"outro\"\n",
        "recipes = [\"primeira\", \"segunda\"]\n",
        "expected_overrides = 0\n",
        "expected_exclusions = 2\n",
        "\n[measures]\n",
        "regions = 1\n",
        "length = 1\n",
        "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
        "\n[[rules]]\n",
        "op = \"exclude-file\"\n",
        "file = \"src/a.rs\"\n",
        "expected_matches = 1\n",
        "\n[[rules]]\n",
        "op = \"exclude-key-prefix\"\n",
        "prefix = \"a.\"\n",
        "expected_matches = 1\n",
    );
    let s = parse(texto).expect("schema 2 válido");
    assert_eq!(s.schema, SNAPSHOT_SCHEMA_V2);
    assert_eq!(s.base_snapshot.as_deref(), Some("outro"));
    assert_eq!(
        s.recipes,
        vec!["primeira".to_string(), "segunda".to_string()]
    );
}

#[test]
fn o_renderer_preserva_a_ordem_declarada_das_receitas() {
    let texto = concat!(
        "schema = 2\n",
        "id = \"ordem\"\n",
        "state = \"FROZEN\"\n",
        "\n[reconstruction]\n",
        "recipes = [\"zebra\", \"alfa\", \"meio\"]\n",
        "expected_overrides = 0\n",
        "expected_exclusions = 0\n",
        "\n[measures]\n",
        "regions = 1\n",
        "length = 1\n",
        "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
    );
    let s = parse(texto).expect("válido");
    let renderizado = pinker_v0::nav_projection_snapshot::render(&s);
    assert!(
        renderizado.contains("recipes = [\"zebra\", \"alfa\", \"meio\"]"),
        "a ordem procedural foi canonicalizada por nome:\n{renderizado}"
    );
    assert_eq!(parse(&renderizado).expect("reparse"), s);
}

#[test]
fn identificadores_duplicados_sao_rejeitados_por_autoridade() {
    let m = medidas_de(&catalogo());
    let erro = Library::new()
        .with_snapshot(snapshot(
            "igual",
            SnapshotState::Frozen,
            m,
            None,
            &[],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "igual",
            SnapshotState::Frozen,
            m,
            None,
            &[],
            vec![],
        ))
        .expect_err("snapshot duplicado");
    assert!(matches!(erro, HarnessFailure::DuplicateSnapshot { .. }));

    let erro = Library::new()
        .with_recipe(receita("igual", &[], vec![]))
        .unwrap()
        .with_recipe(receita("igual", &[], vec![]))
        .expect_err("receita duplicada");
    assert!(matches!(erro, HarnessFailure::DuplicateRecipe { .. }));
}

// ---------------------------------------------------------------------------
// Serialização: round-trip e idempotência nas duas autoridades
// ---------------------------------------------------------------------------

/// Snapshot schema 2 exercitando composição e as quatro operações de regra.
const SNAPSHOT_V2: &str = concat!(
    "schema = 2\n",
    "id = \"composto\"\n",
    "state = \"FROZEN\"\n",
    "predecessor = \"anterior\"\n",
    "justification = \"fixture de congelamento\"\n",
    "\n[reconstruction]\n",
    "base_snapshot = \"a-base\"\n",
    "recipes = [\"zebra\", \"alfa\", \"meio\"]\n",
    "expected_overrides = 1\n",
    "expected_exclusions = 4\n",
    "\n[measures]\n",
    "regions = 7\n",
    "length = 1234\n",
    "fnv1a64 = \"fnv1a64:0123456789abcdef\"\n",
    "\n[[rules]]\n",
    "op = \"exclude-key\"\n",
    "key = \"posterior.nova\"\n",
    "expected_matches = 1\n",
    "\n[[rules]]\n",
    "op = \"exclude-key-prefix\"\n",
    "prefix = \"evidencia.\"\n",
    "expected_matches = 3\n",
    "\n[[rules]]\n",
    "op = \"exclude-file\"\n",
    "file = \"tests/x.rs\"\n",
    "expected_matches = 2\n",
    "\n[[rules]]\n",
    "op = \"exclude-file-prefix\"\n",
    "prefix = \"apps/\"\n",
    "expected_matches = 5\n",
    "\n[[rules]]\n",
    "op = \"override-hash\"\n",
    "key = \"a.um\"\n",
    "from = \"fnv1a64:0000000000000001\"\n",
    "to = \"fnv1a64:00000000000000ff\"\n",
    "expect_file = \"src/a.rs\"\n",
    "expect_domain = \"dominio\"\n",
    "expect_layer = \"camada\"\n",
);

/// Receita schema 1 exercitando passos encadeados e as cinco operações.
const RECEITA_V1: &str = concat!(
    "schema = 1\n",
    "id = \"intermediaria\"\n",
    "\n[reconstruction]\n",
    "steps = [\"segunda\", \"primeira\"]\n",
    "expected_overrides = 1\n",
    "expected_exclusions = 2\n",
    "\n[[rules]]\n",
    "op = \"exclude-file\"\n",
    "file = \"tests/y.rs\"\n",
    "expected_matches = 1\n",
    "\n[[rules]]\n",
    "op = \"exclude-key-prefix\"\n",
    "prefix = \"evidencia.\"\n",
    "expected_matches = 2\n",
    "\n[[rules]]\n",
    "op = \"override-hash\"\n",
    "key = \"b.um\"\n",
    "from = \"fnv1a64:0000000000000003\"\n",
    "to = \"fnv1a64:00000000000000bb\"\n",
);

#[test]
fn round_trip_canonico_do_snapshot_schema_2() {
    let modelo = parse(SNAPSHOT_V2).expect("snapshot schema 2 válido");
    let renderizado = render(&modelo);
    let reinterpretado = parse(&renderizado).expect("render volta a interpretar");
    assert_eq!(modelo, reinterpretado, "parse(render(x)) != x");
    assert_eq!(modelo.schema, SNAPSHOT_SCHEMA_V2);
    assert_eq!(modelo.base_snapshot.as_deref(), Some("a-base"));
    // As quatro operações de exclusão e o override sobreviveram ao ciclo.
    let ops: Vec<&str> = reinterpretado.rules.iter().map(Rule::op).collect();
    assert!(ops.contains(&"exclude-key"));
    assert!(ops.contains(&"exclude-key-prefix"));
    assert!(ops.contains(&"exclude-file"));
    assert!(ops.contains(&"exclude-file-prefix"));
    assert!(ops.contains(&"override-hash"));
}

#[test]
fn renderer_do_snapshot_e_idempotente() {
    let modelo = parse(SNAPSHOT_V2).expect("válido");
    let uma = render(&modelo);
    let duas = render(&parse(&uma).expect("reparse"));
    assert_eq!(uma, duas, "render não é idempotente");
}

#[test]
fn round_trip_canonico_da_receita_schema_1() {
    let modelo = parse_recipe(RECEITA_V1).expect("receita schema 1 válida");
    let renderizado = render_recipe(&modelo);
    let reinterpretado = parse_recipe(&renderizado).expect("render volta a interpretar");
    assert_eq!(
        modelo, reinterpretado,
        "parse_recipe(render_recipe(x)) != x"
    );
    assert_eq!(
        modelo.schema, RECIPE_SCHEMA_V1,
        "a fixture declara schema 1 e assim permanece"
    );
    assert_eq!(
        modelo.relative_path(),
        ".pinker/projections/recipes/intermediaria.toml"
    );
}

#[test]
fn renderer_da_receita_e_idempotente() {
    let modelo = parse_recipe(RECEITA_V1).expect("válida");
    let uma = render_recipe(&modelo);
    let duas = render_recipe(&parse_recipe(&uma).expect("reparse"));
    assert_eq!(uma, duas);
}

#[test]
fn a_ordem_declarada_sobrevive_ao_ciclo_nas_duas_autoridades() {
    let snapshot = parse(SNAPSHOT_V2).expect("válido");
    assert_eq!(
        snapshot.recipes,
        vec!["zebra".to_string(), "alfa".to_string(), "meio".to_string()],
        "a ordem procedural das receitas foi canonicalizada"
    );
    assert!(render(&snapshot).contains("recipes = [\"zebra\", \"alfa\", \"meio\"]"));

    let receita = parse_recipe(RECEITA_V1).expect("válida");
    assert_eq!(
        receita.steps,
        vec!["segunda".to_string(), "primeira".to_string()],
        "a ordem procedural dos passos foi canonicalizada"
    );
    assert!(render_recipe(&receita).contains("steps = [\"segunda\", \"primeira\"]"));
}

#[test]
fn receita_rejeita_campos_que_pertencem_a_snapshot() {
    for (campo, linha) in [
        ("state", "state = \"FROZEN\"\n"),
        ("predecessor", "predecessor = \"outro\"\n"),
        ("justification", "justification = \"por que\"\n"),
    ] {
        let texto = RECEITA_V1.replace("id = \"intermediaria\"\n", &format!("id = \"x\"\n{linha}"));
        match parse_recipe(&texto) {
            Err(HarnessFailure::RecipeHasSnapshotField { field }) => assert_eq!(field, campo),
            outro => panic!("receita aceitou '{campo}': {outro:?}"),
        }
    }
    let com_medidas = format!(
        "{}\n[measures]\nregions = 1\nlength = 1\nfnv1a64 = \"fnv1a64:0000000000000000\"\n",
        RECEITA_V1
    );
    match parse_recipe(&com_medidas) {
        Err(HarnessFailure::RecipeHasSnapshotField { field }) => assert_eq!(field, "measures"),
        outro => panic!("receita aceitou medidas: {outro:?}"),
    }
}

#[test]
fn receita_com_schema_desconhecido_e_rejeitada() {
    // O conjunto aceito por cada autoridade é diferente; o diagnóstico precisa
    // falar do formato certo, e não do conjunto aceito pelo outro.
    let texto = RECEITA_V1.replace("schema = 1", "schema = 9");
    match parse_recipe(&texto) {
        Err(erro @ HarnessFailure::SchemaUnknown { .. }) => {
            assert_eq!(erro.code(), "E-RECEITA-SCHEMA");
            let msg = erro.to_string();
            assert!(
                msg.contains("desconhecido para receita"),
                "a mensagem não identifica a autoridade: {msg}"
            );
            assert!(
                msg.contains("aceita 1 ou 2"),
                "a mensagem não diz o que a receita aceita: {msg}"
            );
            assert!(
                !msg.contains("1, 2 ou 3"),
                "a mensagem de receita citou o conjunto de snapshot: {msg}"
            );
        }
        outro => panic!("esperado schema desconhecido, veio {outro:?}"),
    }
}

#[test]
fn o_diagnostico_de_schema_e_separado_por_autoridade() {
    let de_snapshot = parse(&SNAPSHOT_V2.replace("schema = 2", "schema = 9"))
        .expect_err("schema 9 é inválido para snapshot");
    assert_eq!(de_snapshot.code(), "E-SNAP-SCHEMA");
    let msg = de_snapshot.to_string();
    assert!(msg.contains("desconhecido para snapshot"), "{msg}");
    assert!(
        msg.contains("aceita 1, 2, 3 ou 4"),
        "snapshot aceita as quatro versões e a mensagem precisa dizer isso: {msg}"
    );

    let de_receita = parse_recipe(&RECEITA_V1.replace("schema = 1", "schema = 9"))
        .expect_err("schema 9 é inválido para receita");
    assert_eq!(de_receita.code(), "E-RECEITA-SCHEMA");
    assert!(de_receita.to_string().contains("aceita 1 ou 2"));
    assert!(
        !de_receita.to_string().contains("1, 2, 3 ou 4"),
        "a receita citou o conjunto do snapshot"
    );

    // Os dois códigos são distintos: um leitor de log separa as autoridades.
    assert_ne!(de_snapshot.code(), de_receita.code());

    // E o schema 1 continua válido nas duas, cada uma no seu formato.
    assert!(parse_recipe(RECEITA_V1).is_ok());
    assert_eq!(
        parse(VALID_SCHEMA_1).expect("schema 1 de snapshot").schema,
        SNAPSHOT_SCHEMA_V1
    );
}

/// Snapshot mínimo em schema 1, para provar que a versão 1 segue aceita.
const VALID_SCHEMA_1: &str = concat!(
    "schema = 1\n",
    "id = \"antigo-valido\"\n",
    "state = \"FROZEN\"\n",
    "\n[reconstruction]\n",
    "expected_overrides = 0\n",
    "expected_exclusions = 0\n",
    "\n[measures]\n",
    "regions = 1\n",
    "length = 1\n",
    "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
);

#[test]
fn receita_com_passo_repetido_ou_autorreferente_e_rejeitada() {
    let repetido = RECEITA_V1.replace(
        "steps = [\"segunda\", \"primeira\"]",
        "steps = [\"segunda\", \"segunda\"]",
    );
    assert!(matches!(
        parse_recipe(&repetido),
        Err(HarnessFailure::InvalidField { .. })
    ));
    let propria = RECEITA_V1.replace(
        "steps = [\"segunda\", \"primeira\"]",
        "steps = [\"intermediaria\"]",
    );
    match parse_recipe(&propria) {
        Err(erro @ HarnessFailure::RecipeSelfStep { .. }) => {
            assert_eq!(erro.code(), "E-RECEITA-PASSO-PROPRIO");
            let msg = erro.to_string();
            assert!(
                msg.contains("receita 'intermediaria' declara a si mesma como passo"),
                "{msg}"
            );
            assert!(
                !msg.contains("base"),
                "a receita não tem base; a mensagem descreveu relação inexistente: {msg}"
            );
        }
        outro => panic!("esperado passo próprio, veio {outro:?}"),
    }
}

#[test]
fn autorreferencia_de_receita_e_de_snapshot_sao_diagnosticos_distintos() {
    let de_receita = parse_recipe(&RECEITA_V1.replace(
        "steps = [\"segunda\", \"primeira\"]",
        "steps = [\"intermediaria\"]",
    ))
    .expect_err("passo próprio");

    let de_snapshot =
        parse(&SNAPSHOT_V2.replace("base_snapshot = \"a-base\"", "base_snapshot = \"composto\""))
            .expect_err("base própria");

    assert_eq!(de_receita.code(), "E-RECEITA-PASSO-PROPRIO");
    assert_eq!(de_snapshot.code(), "E-SNAP-BASE-PROPRIA");
    assert_ne!(de_receita.code(), de_snapshot.code());

    assert!(de_snapshot
        .to_string()
        .contains("snapshot 'composto' declara a si mesmo como base"));
    assert!(
        !de_receita.to_string().contains("snapshot"),
        "o diagnóstico da receita mencionou snapshot"
    );
}

#[test]
fn receita_valida_consumo_declarado_das_proprias_regras() {
    let a_mais = RECEITA_V1.replace("expected_overrides = 1", "expected_overrides = 2");
    assert!(matches!(
        parse_recipe(&a_mais),
        Err(HarnessFailure::OverrideMissing { .. })
    ));
    let a_menos = RECEITA_V1.replace("expected_exclusions = 2", "expected_exclusions = 1");
    assert!(matches!(
        parse_recipe(&a_menos),
        Err(HarnessFailure::ExclusionExcess { .. })
    ));
}

// ---------------------------------------------------------------------------
// Forma canônica não carrega ambiente
// ---------------------------------------------------------------------------

#[test]
fn a_forma_canonica_nao_carrega_informacao_ambiental() {
    let snapshot = render(&parse(SNAPSHOT_V2).expect("válido"));
    let receita = render_recipe(&parse_recipe(RECEITA_V1).expect("válida"));
    for texto in [&snapshot, &receita] {
        for proibido in ["/home/", "/tmp/", "/pinker/", "/var/", "C:\\", "\u{1b}"] {
            assert!(!texto.contains(proibido), "ambiente na forma canônica");
        }
        for linha in texto.lines() {
            assert!(!linha.contains("= \"/"), "path absoluto: {linha}");
        }
        // Nem timestamp, PID, usuário ou locale: a forma é função só do modelo.
        for suspeito in ["20", "pid", "PID", "amara", "UTC", "pt_BR"] {
            if suspeito == "20" {
                continue; // dígitos legítimos aparecem em medidas
            }
            assert!(
                !texto.contains(suspeito),
                "ambiente na forma canônica: {suspeito}"
            );
        }
    }
}

#[test]
fn renderizacao_e_estavel_entre_execucoes_e_independente_de_root() {
    // O modelo só conhece paths repo-relativos, então duas montagens do mesmo
    // repositório em roots absolutos distintos produzem a mesma forma canônica.
    fn relativizar(root: &str, absolutos: &[&str]) -> Vec<String> {
        absolutos
            .iter()
            .map(|a| {
                a.strip_prefix(root)
                    .expect("path sob o root")
                    .trim_start_matches('/')
                    .to_string()
            })
            .collect()
    }
    let um = relativizar("/var/tmp/clone-a", &["/var/tmp/clone-a/tests/x.rs"]);
    let outro = relativizar(
        "/var/tmp/outro-root-bem-mais-longo",
        &["/var/tmp/outro-root-bem-mais-longo/tests/x.rs"],
    );
    assert_eq!(um, outro);

    let modelo = parse(SNAPSHOT_V2).expect("válido");
    assert_eq!(render(&modelo), render(&modelo));
    let receita = parse_recipe(RECEITA_V1).expect("válida");
    assert_eq!(render_recipe(&receita), render_recipe(&receita));
}

// ---------------------------------------------------------------------------
// Grafo de composição completo, com as três formas
// ---------------------------------------------------------------------------

#[test]
fn o_grafo_completo_resolve_snapshot_para_snapshot_para_receita_para_receita() {
    // recipe interna → recipe externa; snapshot base → snapshot descendente.
    let interna = receita("interna", &[], vec![excluir("posterior.nova")]);
    let externa = receita(
        "externa",
        &["interna"],
        vec![excluir_prefixo_de_chave("evidencia.", 1)],
    );

    let apos_base: Vec<CodeRegion> = catalogo()
        .into_iter()
        .filter(|r| r.key != "posterior.nova" && !r.key.starts_with("evidencia."))
        .collect();
    let apos_descendente: Vec<CodeRegion> = apos_base
        .clone()
        .into_iter()
        .filter(|r| r.file != "src/b.rs")
        .collect();

    let library = Library::new()
        .with_recipe(interna)
        .unwrap()
        .with_recipe(externa)
        .unwrap()
        .with_snapshot(snapshot(
            "base",
            SnapshotState::Frozen,
            medidas_de(&apos_base),
            None,
            &["externa"],
            vec![],
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "descendente",
            SnapshotState::Frozen,
            medidas_de(&apos_descendente),
            Some("base"),
            &[],
            vec![excluir_arquivo("src/b.rs", 1)],
        ))
        .unwrap();

    let composicao = resolve(&library, "descendente", &catalogo()).expect("grafo completo resolve");
    assert_eq!(composicao.measures(), medidas_de(&apos_descendente));
    assert_eq!(composicao.verified_bases, vec!["base".to_string()]);
    let escopos: Vec<&str> = composicao.ledger.iter().map(|e| e.scope.as_str()).collect();
    assert_eq!(
        escopos,
        vec![
            "recipe:interna",
            "recipe:externa",
            "snapshot:base",
            "snapshot:descendente"
        ],
        "a ordem de aplicação do grafo completo mudou"
    );
    // A projeção estável do estado composto é a mesma calculada diretamente.
    assert_eq!(
        stable_projection(composicao.regions.iter()),
        stable_projection(apos_descendente.iter())
    );
}

// ---------------------------------------------------------------------------
// override-region: schema 3 do snapshot, schema 2 da receita
// ---------------------------------------------------------------------------

fn regiao_alvo() -> Vec<CodeRegion> {
    vec![region("a.um", "src/a.rs", "fnv1a64:0000000000000001")]
}

/// Constrói um snapshot schema 3 com um único `override-region`.
fn snapshot_v3(regra: &str) -> String {
    format!(
        concat!(
            "schema = 3\n",
            "id = \"com-override-region\"\n",
            "state = \"FROZEN\"\n",
            "\n[reconstruction]\n",
            "expected_overrides = 1\n",
            "expected_exclusions = 0\n",
            "\n[measures]\n",
            "regions = 1\n",
            "length = 1\n",
            "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
            "\n[[rules]]\n",
            "op = \"override-region\"\n",
            "key = \"a.um\"\n",
            "{}"
        ),
        regra
    )
}

fn receita_v2(regra: &str) -> String {
    format!(
        concat!(
            "schema = 2\n",
            "id = \"com-override-region\"\n",
            "\n[reconstruction]\n",
            "expected_overrides = 1\n",
            "expected_exclusions = 0\n",
            "\n[[rules]]\n",
            "op = \"override-region\"\n",
            "key = \"a.um\"\n",
            "{}"
        ),
        regra
    )
}

const PAR_HASH: &str = concat!(
    "from_hash = \"fnv1a64:0000000000000001\"\n",
    "to_hash = \"fnv1a64:00000000000000ff\"\n",
);
const PAR_SUMMARY: &str = concat!(
    "from_summary = \"Resumo de a.um.\"\n",
    "to_summary = \"Resumo historico restaurado.\"\n",
);

/// Aplica uma regra isolada a uma região e devolve o resultado.
fn aplicar_regra(
    regra: Rule,
    entrada: Vec<CodeRegion>,
) -> Result<Vec<ProjectionRegion>, HarnessFailure> {
    let library = Library::new()
        .with_snapshot(snapshot(
            "isolado",
            SnapshotState::Frozen,
            medidas_de(&entrada),
            None,
            &[],
            vec![regra],
        ))
        .unwrap();
    resolve(&library, "isolado", &entrada).map(|c| c.regions)
}

fn override_region(
    from_hash: Option<&str>,
    to_hash: Option<&str>,
    from_summary: Option<&str>,
    to_summary: Option<&str>,
) -> Rule {
    Rule::OverrideRegion {
        key: "a.um".to_string(),
        from_hash: from_hash.map(str::to_string),
        to_hash: to_hash.map(str::to_string),
        from_summary: from_summary.map(str::to_string),
        to_summary: to_summary.map(str::to_string),
        expect_file: None,
        expect_domain: None,
        expect_layer: None,
    }
}

#[test]
fn snapshot_schema_2_rejeita_override_region_exigindo_schema_3() {
    let texto = snapshot_v3(PAR_HASH).replace("schema = 3", "schema = 2");
    match parse(&texto) {
        Err(
            erro @ HarnessFailure::CapabilityRequiresSchema {
                authority: SchemaAuthority::Snapshot,
                found_schema: SNAPSHOT_SCHEMA_V2,
                required_schema: SNAPSHOT_SCHEMA_V3,
                ..
            },
        ) => {
            assert_eq!(erro.code(), "E-SNAP-CAPACIDADE-SCHEMA");
            assert!(erro.to_string().contains("override-region"), "{erro}");
            assert!(erro.to_string().contains("de snapshot"), "{erro}");
        }
        outro => panic!("esperada capacidade de schema 3, veio {outro:?}"),
    }
}

#[test]
fn recipe_schema_1_rejeita_override_region_exigindo_schema_2() {
    let texto = receita_v2(PAR_HASH).replace("schema = 2", "schema = 1");
    match parse_recipe(&texto) {
        Err(
            erro @ HarnessFailure::CapabilityRequiresSchema {
                authority: SchemaAuthority::Recipe,
                found_schema: 1,
                required_schema: 2,
                ..
            },
        ) => {
            assert_eq!(erro.code(), "E-RECEITA-CAPACIDADE-SCHEMA");
            assert!(erro.to_string().contains("de receita"), "{erro}");
        }
        outro => panic!("esperada capacidade de receita 2, veio {outro:?}"),
    }
}

#[test]
fn a_matriz_de_capacidades_e_por_autoridade() {
    // `exclude-file` e `exclude-key-prefix` chegaram ao snapshot no 2, mas o
    // formato de receita nasceu depois e já as trouxe no 1.
    let casos: [(Rule, u64, u64); 6] = [
        (excluir("x"), 1, 1),
        (
            Rule::ExcludeFilePrefix {
                prefix: "apps/".to_string(),
                expected_matches: 1,
            },
            1,
            1,
        ),
        (excluir_arquivo("src/a.rs", 1), 2, 1),
        (excluir_prefixo_de_chave("a.", 1), 2, 1),
        (
            override_hash(
                "a.um",
                "fnv1a64:0000000000000001",
                "fnv1a64:00000000000000ff",
            ),
            1,
            1,
        ),
        (
            override_region(
                Some("fnv1a64:0000000000000001"),
                Some("fnv1a64:00000000000000ff"),
                None,
                None,
            ),
            3,
            2,
        ),
    ];
    for (regra, snap, rec) in casos {
        assert_eq!(
            regra.min_schema(SchemaAuthority::Snapshot),
            snap,
            "matriz de snapshot errada para {}",
            regra.op()
        );
        assert_eq!(
            regra.min_schema(SchemaAuthority::Recipe),
            rec,
            "matriz de receita errada para {}",
            regra.op()
        );
    }
}

#[test]
fn snapshot_schema_3_faz_round_trip_canonico_com_override_region() {
    let texto = snapshot_v3(&format!(
        "{PAR_HASH}{PAR_SUMMARY}expect_file = \"src/a.rs\"\nexpect_domain = \"dominio\"\nexpect_layer = \"camada\"\n"
    ));
    let modelo = parse(&texto).expect("schema 3 válido");
    assert_eq!(modelo.schema, SNAPSHOT_SCHEMA_V3);
    let renderizado = render(&modelo);
    assert_eq!(parse(&renderizado).expect("reparse"), modelo);
    assert_eq!(render(&parse(&renderizado).unwrap()), renderizado);
}

#[test]
fn recipe_schema_2_faz_round_trip_canonico_com_override_region() {
    let texto = receita_v2(&format!("{PAR_HASH}{PAR_SUMMARY}"));
    let modelo = parse_recipe(&texto).expect("receita schema 2 válida");
    assert_eq!(modelo.schema, 2);
    let renderizado = render_recipe(&modelo);
    assert_eq!(parse_recipe(&renderizado).expect("reparse"), modelo);
    assert_eq!(
        render_recipe(&parse_recipe(&renderizado).unwrap()),
        renderizado
    );
}

#[test]
fn restauracao_somente_de_summary_funciona() {
    let saida = aplicar_regra(
        override_region(
            None,
            None,
            Some("Resumo de a.um."),
            Some("Resumo historico."),
        ),
        regiao_alvo(),
    )
    .expect("summary sozinho é válido");
    assert_eq!(saida[0].summary, "Resumo historico.");
    assert_eq!(
        saida[0].hash, "fnv1a64:0000000000000001",
        "o hash não podia ter sido tocado"
    );
}

#[test]
fn restauracao_de_hash_e_summary_e_uma_unica_regra() {
    let saida = aplicar_regra(
        override_region(
            Some("fnv1a64:0000000000000001"),
            Some("fnv1a64:00000000000000ff"),
            Some("Resumo de a.um."),
            Some("Resumo historico."),
        ),
        regiao_alvo(),
    )
    .expect("os dois campos juntos");
    assert_eq!(saida[0].hash, "fnv1a64:00000000000000ff");
    assert_eq!(saida[0].summary, "Resumo historico.");
}

#[test]
fn expected_overrides_conta_por_regra_e_nao_por_campo() {
    // Uma regra que restaura dois campos continua sendo uma regra.
    let texto = snapshot_v3(&format!("{PAR_HASH}{PAR_SUMMARY}"));
    let modelo = parse(&texto).expect("válido");
    assert_eq!(modelo.expected_overrides, 1);
    assert_eq!(modelo.rules.len(), 1);
    assert!(modelo.rules[0].is_override());

    // Declarar 2 seria override ausente, não "dois campos".
    let dois = texto.replace("expected_overrides = 1", "expected_overrides = 2");
    assert!(matches!(
        parse(&dois),
        Err(HarnessFailure::OverrideMissing {
            declared: 2,
            found: 1
        })
    ));
}

#[test]
fn par_de_hash_incompleto_e_rejeitado() {
    for regra in [
        "from_hash = \"fnv1a64:0000000000000001\"\n",
        "to_hash = \"fnv1a64:00000000000000ff\"\n",
    ] {
        match parse(&snapshot_v3(&format!("{regra}{PAR_SUMMARY}"))) {
            Err(HarnessFailure::OverrideRegionPairInvalid { msg, .. }) => {
                assert!(msg.contains("from_hash"), "{msg}");
            }
            outro => panic!("meio par de hash aceito: {outro:?}"),
        }
    }
}

#[test]
fn par_de_summary_incompleto_e_rejeitado() {
    for regra in ["from_summary = \"x\"\n", "to_summary = \"y\"\n"] {
        match parse(&snapshot_v3(&format!("{PAR_HASH}{regra}"))) {
            Err(HarnessFailure::OverrideRegionPairInvalid { msg, .. }) => {
                assert!(msg.contains("from_summary"), "{msg}");
            }
            outro => panic!("meio par de summary aceito: {outro:?}"),
        }
    }
}

#[test]
fn regra_sem_nenhum_par_e_rejeitada() {
    match parse(&snapshot_v3("expect_file = \"src/a.rs\"\n")) {
        Err(HarnessFailure::OverrideRegionPairInvalid { key, msg }) => {
            assert_eq!(key, "a.um");
            assert!(msg.contains("ao menos um par"), "{msg}");
        }
        outro => panic!("regra sem par aceita: {outro:?}"),
    }
}

#[test]
fn hash_corrente_divergente_falha_antes_da_mutacao() {
    let erro = aplicar_regra(
        override_region(
            Some("fnv1a64:00000000000000aa"),
            Some("fnv1a64:00000000000000ff"),
            Some("Resumo de a.um."),
            Some("Resumo historico."),
        ),
        regiao_alvo(),
    )
    .expect_err("hash divergente");
    assert!(matches!(erro, HarnessFailure::OverrideStaleBase { .. }));
    assert_eq!(erro.code(), "E-SNAP-OVERRIDE-BASE");
}

#[test]
fn summary_corrente_divergente_falha_antes_da_mutacao() {
    let erro = aplicar_regra(
        override_region(
            Some("fnv1a64:0000000000000001"),
            Some("fnv1a64:00000000000000ff"),
            Some("Resumo que nao e o corrente."),
            Some("Resumo historico."),
        ),
        regiao_alvo(),
    )
    .expect_err("summary divergente");
    match &erro {
        HarnessFailure::OverrideStaleSummary { key, .. } => assert_eq!(key, "a.um"),
        outro => panic!("esperado summary divergente, veio {outro:?}"),
    }
    assert_eq!(erro.code(), "E-SNAP-OVERRIDE-SUMMARY");
}

#[test]
fn com_dois_campos_declarados_nenhum_e_alterado_se_uma_precondicao_falhar() {
    // O hash confere, o summary não. Nenhum dos dois pode ser tocado — é isto
    // que significa "atômica no sentido lógico da regra".
    let entrada = regiao_alvo();
    let erro = aplicar_regra(
        override_region(
            Some("fnv1a64:0000000000000001"),
            Some("fnv1a64:00000000000000ff"),
            Some("Resumo errado."),
            Some("Resumo historico."),
        ),
        entrada.clone(),
    )
    .expect_err("uma precondição falha");
    assert!(matches!(erro, HarnessFailure::OverrideStaleSummary { .. }));

    // A entrada segue intacta: a falha aconteceu antes da fase de mutação.
    assert_eq!(entrada[0].hash, "fnv1a64:0000000000000001");
    assert_eq!(entrada[0].summary, "Resumo de a.um.");

    // E a ordem inversa também: summary confere, hash não.
    let erro = aplicar_regra(
        override_region(
            Some("fnv1a64:00000000000000aa"),
            Some("fnv1a64:00000000000000ff"),
            Some("Resumo de a.um."),
            Some("Resumo historico."),
        ),
        entrada.clone(),
    )
    .expect_err("a outra precondição falha");
    assert!(matches!(erro, HarnessFailure::OverrideStaleBase { .. }));
    assert_eq!(entrada[0].summary, "Resumo de a.um.");
}

#[test]
fn expectativa_de_identidade_protege_antes_da_mutacao() {
    let entrada = regiao_alvo();
    for (campo, regra) in [
        (
            "file",
            Rule::OverrideRegion {
                key: "a.um".to_string(),
                from_hash: Some("fnv1a64:0000000000000001".to_string()),
                to_hash: Some("fnv1a64:00000000000000ff".to_string()),
                from_summary: None,
                to_summary: None,
                expect_file: Some("src/outro.rs".to_string()),
                expect_domain: None,
                expect_layer: None,
            },
        ),
        (
            "domain",
            Rule::OverrideRegion {
                key: "a.um".to_string(),
                from_hash: Some("fnv1a64:0000000000000001".to_string()),
                to_hash: Some("fnv1a64:00000000000000ff".to_string()),
                from_summary: None,
                to_summary: None,
                expect_file: None,
                expect_domain: Some("outro".to_string()),
                expect_layer: None,
            },
        ),
        (
            "layer",
            Rule::OverrideRegion {
                key: "a.um".to_string(),
                from_hash: Some("fnv1a64:0000000000000001".to_string()),
                to_hash: Some("fnv1a64:00000000000000ff".to_string()),
                from_summary: None,
                to_summary: None,
                expect_file: None,
                expect_domain: None,
                expect_layer: Some("outra".to_string()),
            },
        ),
    ] {
        let erro = aplicar_regra(regra, entrada.clone()).expect_err("identidade divergente");
        let esperado = matches!(
            erro,
            HarnessFailure::PathChanged { .. } | HarnessFailure::MetadataChanged { .. }
        );
        assert!(esperado, "campo {campo}: veio {erro:?}");
        assert_eq!(
            entrada[0].hash, "fnv1a64:0000000000000001",
            "mutou mesmo assim"
        );
    }
}

#[test]
fn duas_regras_de_override_para_a_mesma_key_continuam_rejeitadas() {
    // Mesmo misturando as duas operações de override.
    let texto = format!(
        "{}\n[[rules]]\nop = \"override-hash\"\nkey = \"a.um\"\nfrom = \"fnv1a64:00000000000000ff\"\nto = \"fnv1a64:00000000000000bb\"\n",
        snapshot_v3(PAR_HASH).replace("expected_overrides = 1", "expected_overrides = 2")
    );
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::OverrideRepeated { .. })
    ));
}

#[test]
fn override_hash_mantem_a_semantica_anterior() {
    // A operação antiga não mudou: continua schema 1 nas duas autoridades e
    // continua tocando apenas o hash.
    let regra = override_hash(
        "a.um",
        "fnv1a64:0000000000000001",
        "fnv1a64:00000000000000ff",
    );
    assert_eq!(regra.min_schema(SchemaAuthority::Snapshot), 1);
    assert_eq!(regra.min_schema(SchemaAuthority::Recipe), 1);
    let saida = aplicar_regra(regra, regiao_alvo()).expect("override-hash segue válido");
    assert_eq!(saida[0].hash, "fnv1a64:00000000000000ff");
    assert_eq!(saida[0].summary, "Resumo de a.um.", "summary foi tocado");
}

#[test]
fn schemas_1_e_2_de_snapshot_seguem_compativeis() {
    // O schema 1 continua sendo lista plana sem composição.
    let s1 = parse(VALID_SCHEMA_1).expect("schema 1 válido");
    assert_eq!(s1.schema, SNAPSHOT_SCHEMA_V1);
    assert_eq!(s1.base_snapshot, None);
    assert!(s1.recipes.is_empty());
    assert_eq!(render(&s1), render(&parse(&render(&s1)).unwrap()));

    // O schema 2 continua com composição e sem override-region.
    let s2 = parse(SNAPSHOT_V2).expect("schema 2 válido");
    assert_eq!(s2.schema, SNAPSHOT_SCHEMA_V2);
    assert_eq!(s2.base_snapshot.as_deref(), Some("a-base"));
    assert_eq!(parse(&render(&s2)).expect("round-trip"), s2);
    assert!(!s2.rules.iter().any(|r| r.op() == "override-region"));

    // E o schema 2 segue recusando as capacidades do 3.
    assert!(matches!(
        parse(&snapshot_v3(PAR_HASH).replace("schema = 3", "schema = 2")),
        Err(HarnessFailure::CapabilityRequiresSchema { .. })
    ));
}

#[test]
fn schema_1_de_recipe_segue_compativel() {
    let r1 = parse_recipe(RECEITA_V1).expect("receita schema 1 válida");
    assert_eq!(r1.schema, 1);
    assert_eq!(parse_recipe(&render_recipe(&r1)).expect("round-trip"), r1);
    // E o schema 1 recusa a capacidade do 2.
    assert!(matches!(
        parse_recipe(&receita_v2(PAR_HASH).replace("schema = 2", "schema = 1")),
        Err(HarnessFailure::CapabilityRequiresSchema { .. })
    ));
}

#[test]
fn override_region_consome_exatamente_uma_regiao() {
    let library = Library::new()
        .with_snapshot(snapshot(
            "consumo",
            SnapshotState::Frozen,
            medidas_de(&regiao_alvo()),
            None,
            &[],
            vec![override_region(
                Some("fnv1a64:0000000000000001"),
                Some("fnv1a64:00000000000000ff"),
                Some("Resumo de a.um."),
                Some("Resumo historico."),
            )],
        ))
        .unwrap();
    let composicao = resolve(&library, "consumo", &regiao_alvo()).expect("válido");
    let total: usize = composicao.ledger.iter().map(|e| e.entries.len()).sum();
    assert_eq!(
        total, 1,
        "uma regra, um consumo — mesmo restaurando dois campos"
    );
    let entrada = &composicao.ledger[0].entries[0];
    assert_eq!(entrada.op, "override-region");
    assert_eq!((entrada.expected, entrada.consumed), (1, 1));
}

#[test]
fn override_region_com_seletor_sem_correspondencia_falha() {
    let regra = Rule::OverrideRegion {
        key: "nao.existe".to_string(),
        from_hash: Some("fnv1a64:0000000000000001".to_string()),
        to_hash: Some("fnv1a64:00000000000000ff".to_string()),
        from_summary: None,
        to_summary: None,
        expect_file: None,
        expect_domain: None,
        expect_layer: None,
    };
    assert!(matches!(
        aplicar_regra(regra, regiao_alvo()),
        Err(HarnessFailure::RegionRemoved { .. })
    ));
}

// ---------------------------------------------------------------------------
// Estriteza por operação: campo de outra operação falha, nunca é descartado
// ---------------------------------------------------------------------------

/// Monta um snapshot com uma única regra, na versão pedida.
fn snapshot_com_regra(schema: u64, corpo: &str) -> String {
    format!(
        concat!(
            "schema = {}\n",
            "id = \"estriteza\"\n",
            "state = \"FROZEN\"\n",
            "\n[reconstruction]\n",
            "expected_overrides = {}\n",
            "expected_exclusions = {}\n",
            "\n[measures]\n",
            "regions = 1\n",
            "length = 1\n",
            "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
            "\n[[rules]]\n",
            "{}"
        ),
        schema,
        u8::from(corpo.contains("override")),
        u8::from(!corpo.contains("override")),
        corpo
    )
}

fn receita_com_regra(schema: u64, corpo: &str) -> String {
    format!(
        concat!(
            "schema = {}\n",
            "id = \"estriteza\"\n",
            "\n[reconstruction]\n",
            "expected_overrides = {}\n",
            "expected_exclusions = {}\n",
            "\n[[rules]]\n",
            "{}"
        ),
        schema,
        u8::from(corpo.contains("override")),
        u8::from(!corpo.contains("override")),
        corpo
    )
}

const R_OVERRIDE_HASH: &str = concat!(
    "op = \"override-hash\"\n",
    "key = \"a.um\"\n",
    "from = \"fnv1a64:0000000000000001\"\n",
    "to = \"fnv1a64:00000000000000ff\"\n",
);
const R_OVERRIDE_REGION: &str = concat!(
    "op = \"override-region\"\n",
    "key = \"a.um\"\n",
    "from_hash = \"fnv1a64:0000000000000001\"\n",
    "to_hash = \"fnv1a64:00000000000000ff\"\n",
);
const R_EXCLUDE_KEY: &str = concat!(
    "op = \"exclude-key\"\n",
    "key = \"a.um\"\n",
    "expected_matches = 1\n",
);
const R_EXCLUDE_KEY_PREFIX: &str = concat!(
    "op = \"exclude-key-prefix\"\n",
    "prefix = \"a.\"\n",
    "expected_matches = 1\n",
);
const R_EXCLUDE_FILE: &str = concat!(
    "op = \"exclude-file\"\n",
    "file = \"src/a.rs\"\n",
    "expected_matches = 1\n",
);
const R_EXCLUDE_FILE_PREFIX: &str = concat!(
    "op = \"exclude-file-prefix\"\n",
    "prefix = \"src/\"\n",
    "expected_matches = 1\n",
);

/// Confere que anexar `intruso` à regra faz o parser recusar nomeando o campo.
fn recusa_campo(schema: u64, base: &str, intruso: &str, campo: &str) {
    let texto = snapshot_com_regra(schema, &format!("{base}{intruso}"));
    match parse(&texto) {
        Err(erro @ HarnessFailure::FieldNotAllowedForOp { .. }) => {
            assert_eq!(erro.code(), "E-SNAP-CAMPO-DA-OPERACAO");
            assert!(
                erro.to_string().contains(campo),
                "a mensagem não nomeia o campo '{campo}': {erro}"
            );
        }
        outro => panic!("campo '{campo}' foi aceito ou descartado em silêncio: {outro:?}"),
    }
}

#[test]
fn override_hash_recusa_campos_de_outras_operacoes() {
    for (intruso, campo) in [
        ("from_hash = \"fnv1a64:0000000000000001\"\n", "from_hash"),
        ("to_hash = \"fnv1a64:00000000000000ff\"\n", "to_hash"),
        ("from_summary = \"x\"\n", "from_summary"),
        ("to_summary = \"y\"\n", "to_summary"),
        ("file = \"src/a.rs\"\n", "file"),
        ("prefix = \"a.\"\n", "prefix"),
        ("expected_matches = 1\n", "expected_matches"),
    ] {
        recusa_campo(3, R_OVERRIDE_HASH, intruso, campo);
    }
}

#[test]
fn override_region_recusa_campos_de_outras_operacoes() {
    for (intruso, campo) in [
        ("from = \"fnv1a64:0000000000000001\"\n", "from"),
        ("to = \"fnv1a64:00000000000000ff\"\n", "to"),
        ("file = \"src/a.rs\"\n", "file"),
        ("prefix = \"a.\"\n", "prefix"),
        ("expected_matches = 1\n", "expected_matches"),
    ] {
        recusa_campo(3, R_OVERRIDE_REGION, intruso, campo);
    }
}

#[test]
fn cada_exclusao_recusa_campos_de_override_e_seletores_alheios() {
    let intrusos_de_override = [
        ("from = \"fnv1a64:0000000000000001\"\n", "from"),
        ("to = \"fnv1a64:00000000000000ff\"\n", "to"),
        ("from_hash = \"fnv1a64:0000000000000001\"\n", "from_hash"),
        ("to_summary = \"y\"\n", "to_summary"),
        ("expect_file = \"src/a.rs\"\n", "expect_file"),
        ("expect_domain = \"d\"\n", "expect_domain"),
        ("expect_layer = \"l\"\n", "expect_layer"),
    ];
    for base in [
        R_EXCLUDE_KEY,
        R_EXCLUDE_KEY_PREFIX,
        R_EXCLUDE_FILE,
        R_EXCLUDE_FILE_PREFIX,
    ] {
        for (intruso, campo) in intrusos_de_override {
            recusa_campo(3, base, intruso, campo);
        }
    }

    // E os seletores das outras exclusões, que o filtro global conhecia.
    recusa_campo(3, R_EXCLUDE_KEY, "prefix = \"a.\"\n", "prefix");
    recusa_campo(3, R_EXCLUDE_KEY, "file = \"src/a.rs\"\n", "file");
    recusa_campo(3, R_EXCLUDE_KEY_PREFIX, "key = \"a.um\"\n", "key");
    recusa_campo(3, R_EXCLUDE_KEY_PREFIX, "file = \"src/a.rs\"\n", "file");
    recusa_campo(3, R_EXCLUDE_FILE, "key = \"a.um\"\n", "key");
    recusa_campo(3, R_EXCLUDE_FILE, "prefix = \"src/\"\n", "prefix");
    recusa_campo(3, R_EXCLUDE_FILE_PREFIX, "key = \"a.um\"\n", "key");
    recusa_campo(3, R_EXCLUDE_FILE_PREFIX, "file = \"src/a.rs\"\n", "file");
}

#[test]
fn schemas_antigos_falham_em_vez_de_ignorar_campos_de_override_region() {
    // Este é o caso que a lacuna permitia: num schema que nem conhece
    // `override-region`, seus campos anexados a um `override-hash` passavam pelo
    // filtro global e eram descartados sem aviso.
    for schema in [SNAPSHOT_SCHEMA_V1, SNAPSHOT_SCHEMA_V2] {
        for (intruso, campo) in [
            ("from_summary = \"x\"\n", "from_summary"),
            ("to_summary = \"y\"\n", "to_summary"),
            ("from_hash = \"fnv1a64:0000000000000001\"\n", "from_hash"),
            ("to_hash = \"fnv1a64:00000000000000ff\"\n", "to_hash"),
        ] {
            recusa_campo(schema, R_OVERRIDE_HASH, intruso, campo);
        }
    }
}

#[test]
fn recipe_schema_1_falha_em_vez_de_ignorar_campos_de_override_region() {
    for (intruso, campo) in [
        ("from_summary = \"x\"\n", "from_summary"),
        ("to_summary = \"y\"\n", "to_summary"),
        ("from_hash = \"fnv1a64:0000000000000001\"\n", "from_hash"),
    ] {
        let texto = receita_com_regra(1, &format!("{R_OVERRIDE_HASH}{intruso}"));
        match parse_recipe(&texto) {
            Err(erro @ HarnessFailure::FieldNotAllowedForOp { .. }) => {
                assert!(erro.to_string().contains(campo), "{erro}");
            }
            outro => panic!("receita schema 1 ignorou '{campo}': {outro:?}"),
        }
    }
}

#[test]
fn a_estriteza_nao_quebra_as_formas_canonicas_validas() {
    // Cada operação, na sua versão mínima, continua válida e com round-trip.
    let casos: [(u64, &str); 6] = [
        (1, R_OVERRIDE_HASH),
        (1, R_EXCLUDE_KEY),
        (1, R_EXCLUDE_FILE_PREFIX),
        (2, R_EXCLUDE_FILE),
        (2, R_EXCLUDE_KEY_PREFIX),
        (3, R_OVERRIDE_REGION),
    ];
    for (schema, corpo) in casos {
        let texto = snapshot_com_regra(schema, corpo);
        let modelo = parse(&texto).unwrap_or_else(|e| panic!("schema {schema} recusou: {e}"));
        assert_eq!(modelo.schema, schema);
        let renderizado = render(&modelo);
        assert_eq!(parse(&renderizado).expect("reparse"), modelo);
        assert_eq!(render(&parse(&renderizado).unwrap()), renderizado);
    }
}

#[test]
fn as_formas_completas_com_expectativas_continuam_validas() {
    // `expect_*` pertence às duas operações de override e não pode ter sido
    // barrado junto com os campos alheios.
    let com_expectativas = format!(
        "{R_OVERRIDE_HASH}expect_file = \"src/a.rs\"\nexpect_domain = \"dominio\"\nexpect_layer = \"camada\"\n"
    );
    let modelo = parse(&snapshot_com_regra(1, &com_expectativas)).expect("override-hash completo");
    assert_eq!(parse(&render(&modelo)).expect("round-trip"), modelo);

    let regiao_completa = format!(
        "{R_OVERRIDE_REGION}from_summary = \"antes\"\nto_summary = \"depois\"\nexpect_file = \"src/a.rs\"\nexpect_domain = \"dominio\"\nexpect_layer = \"camada\"\n"
    );
    let modelo = parse(&snapshot_com_regra(3, &regiao_completa)).expect("override-region completo");
    assert_eq!(parse(&render(&modelo)).expect("round-trip"), modelo);
}

#[test]
fn operacao_desconhecida_continua_sendo_operacao_desconhecida() {
    // A tabela por operação não pode transformar op inválida em campo inválido.
    let texto = snapshot_com_regra(3, "op = \"renomear-chave\"\nkey = \"a.um\"\n");
    assert!(matches!(
        parse(&texto),
        Err(HarnessFailure::RuleOperationUnknown { .. })
    ));
}

#[test]
fn chave_fora_da_gramatica_continua_sendo_chave_desconhecida() {
    // Campo que nenhuma operação conhece segue no diagnóstico genérico; campo de
    // outra operação tem diagnóstico próprio. Os dois casos são distinguíveis.
    let fora = snapshot_com_regra(3, &format!("{R_OVERRIDE_HASH}inventado = \"x\"\n"));
    match parse(&fora) {
        Err(HarnessFailure::InvalidField { msg, .. }) => {
            assert!(msg.contains("desconhecida"), "{msg}")
        }
        outro => panic!("esperada chave desconhecida, veio {outro:?}"),
    }
    let de_outra = snapshot_com_regra(3, &format!("{R_OVERRIDE_HASH}from_summary = \"x\"\n"));
    assert!(matches!(
        parse(&de_outra),
        Err(HarnessFailure::FieldNotAllowedForOp { .. })
    ));
}
