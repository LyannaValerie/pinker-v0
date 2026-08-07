//! Trama Pinker — composição de reconstrução: schema 2 e receitas (#384).
//!
//! Cobre os invariantes que o schema 2 congela: separação estrutural entre as
//! duas autoridades, base validada contra as próprias medidas, proibição
//! transitiva de congelado sobre candidato, detecção de ciclo no grafo
//! completo, ordem de aplicação e consumo exato por escopo.

use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::{
    parse_recipe, render_recipe, resolve, verify_frozen_dependencies, Library, Recipe, RECIPES_DIR,
    RECIPE_SCHEMA,
};
use pinker_v0::nav_projection_snapshot::{
    measure, parse, render, stable_projection, HarnessFailure, Measures, ProjectionSnapshot, Rule,
    SnapshotState, SNAPSHOT_SCHEMA_V1, SNAPSHOT_SCHEMA_V2,
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
    assert_eq!(RECIPE_SCHEMA, 1, "o formato de receita estreia em 1");
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
            Err(HarnessFailure::CapabilityRequiresSchema2 { found, .. }) => {
                assert_eq!(found, SNAPSHOT_SCHEMA_V1);
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
    assert_eq!(modelo.schema, RECIPE_SCHEMA);
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
    // `2` é válido para snapshot e inválido para receita: o diagnóstico precisa
    // falar do formato certo, e não do conjunto aceito pelo outro.
    let texto = RECEITA_V1.replace("schema = 1", "schema = 2");
    match parse_recipe(&texto) {
        Err(erro @ HarnessFailure::SchemaUnknown { .. }) => {
            assert_eq!(erro.code(), "E-RECEITA-SCHEMA");
            let msg = erro.to_string();
            assert!(
                msg.contains("desconhecido para receita"),
                "a mensagem não identifica a autoridade: {msg}"
            );
            assert!(
                msg.contains("aceita somente 1"),
                "a mensagem não diz o que a receita aceita: {msg}"
            );
            assert!(
                !msg.contains("1 ou 2"),
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
        msg.contains("aceita 1 ou 2"),
        "snapshot aceita as duas versões e a mensagem precisa dizer isso: {msg}"
    );
    assert!(
        !msg.contains("somente"),
        "a mensagem afirmou que snapshot aceita uma versão só: {msg}"
    );

    let de_receita = parse_recipe(&RECEITA_V1.replace("schema = 1", "schema = 9"))
        .expect_err("schema 9 é inválido para receita");
    assert_eq!(de_receita.code(), "E-RECEITA-SCHEMA");
    assert!(de_receita.to_string().contains("aceita somente 1"));

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
    let um = relativizar(
        "/pinker/work/amara/clone-a",
        &["/pinker/work/amara/clone-a/tests/x.rs"],
    );
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
