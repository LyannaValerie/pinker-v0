use pinker_v0::automation::{Failure, RepoRoot};
use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_lifecycle::{
    apply_accept, apply_prepare, empty_recipe, plan_accept, plan_prepare, recipe_id, recipe_path,
    snapshot_path, ProjectionError,
};
use pinker_v0::nav_projection_recipe::{render_recipe, verify_composed, Recipe};
use pinker_v0::nav_projection_snapshot::{
    measure, parse, render, Measures, ProjectionSnapshot, Rule, SnapshotState, SNAPSHOT_SCHEMA,
};
use pinker_v0::nav_projection_store::ProjectionStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRepo(PathBuf);

impl TempRepo {
    fn new(label: &str) -> TempRepo {
        let path = std::env::temp_dir().join(format!(
            "pinker_projection_lifecycle_{}_{}_{}",
            label,
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".pinker/projections/recipes")).unwrap();
        fs::write(path.join(".pinker/doc.toml"), "# root\n").unwrap();
        TempRepo(path)
    }

    fn root(&self) -> RepoRoot {
        RepoRoot::at(&self.0).unwrap()
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn region(key: &str, hash: u64) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some("fixture".to_string()),
        layer: Some("tests".to_string()),
        phase: None,
        file: format!("src/{key}.rs"),
        start_marker: 1,
        content_start: 2,
        content_end: 3,
        end_marker: 4,
        summary: format!("Resumo {key}"),
        hash: format!("fnv1a64:{hash:016x}"),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

fn catalog() -> Vec<CodeRegion> {
    vec![region("a", 1), region("b", 2)]
}

fn frozen(id: &str, catalog: &[CodeRegion]) -> ProjectionSnapshot {
    ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA,
        id: id.to_string(),
        state: SnapshotState::Frozen,
        predecessor: None,
        justification: Some("marco predecessor".to_string()),
        measures: measure(catalog.iter()),
        expected_overrides: 0,
        expected_exclusions: 0,
        base_snapshot: None,
        recipes: Vec::new(),
        rules: Vec::new(),
    }
}

fn write_snapshot(repo: &TempRepo, snapshot: &ProjectionSnapshot) {
    fs::write(repo.path().join(snapshot.relative_path()), render(snapshot)).unwrap();
}

fn setup(label: &str) -> (TempRepo, Vec<CodeRegion>) {
    let repo = TempRepo::new(label);
    let catalog = catalog();
    write_snapshot(&repo, &frozen("marco-a", &catalog));
    (repo, catalog)
}

fn prepare(repo: &TempRepo, catalog: &[CodeRegion], id: &str) {
    let root = repo.root();
    let plan = plan_prepare(&root, catalog, id, "marco-a", "novo marco").unwrap();
    let applied = apply_prepare(&root, catalog, &plan, &plan.digest()).unwrap();
    assert_eq!(applied.outcome, "CANDIDATE_PREPARED");
}

#[test]
fn preparar_candidate_novo_planeja_dois_targets_sem_escrever_e_calcula_medidas() {
    let (repo, catalog) = setup("new");
    let root = repo.root();
    let plan = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    assert_eq!(plan.plan.targets().len(), 2);
    assert_eq!(plan.planned_outcome(), "CANDIDATE_PLANNED");
    assert_eq!(plan.desired_snapshot.state, SnapshotState::Candidate);
    assert_eq!(
        plan.desired_snapshot.predecessor.as_deref(),
        Some("marco-a")
    );
    assert_eq!(plan.desired_snapshot.base_snapshot, None);
    assert_eq!(plan.desired_snapshot.measures, measure(catalog.iter()));
    assert_eq!(plan.desired_snapshot.recipes, [recipe_id("marco-c")]);
    assert!(plan.desired_snapshot.rules.is_empty());
    assert!(!repo.path().join(snapshot_path("marco-c")).exists());
    assert!(!repo.path().join(recipe_path("marco-c")).exists());
}

#[test]
fn preparar_exige_predecessor_frozen_justificativa_e_recusa_colisao_frozen() {
    let (repo, catalog) = setup("policy");
    let root = repo.root();
    for (predecessor, justification, expected) in [
        ("", "ok", "predecessor"),
        ("ausente", "ok", "não existe"),
        ("marco-a", "", "justificativa"),
    ] {
        let error =
            plan_prepare(&root, &catalog, "marco-c", predecessor, justification).unwrap_err();
        assert!(matches!(error, ProjectionError::Policy { .. }));
        assert!(error.to_string().contains(expected), "{error}");
    }

    let candidate_predecessor = ProjectionSnapshot {
        state: SnapshotState::Candidate,
        id: "candidate-pai".to_string(),
        predecessor: Some("marco-a".to_string()),
        justification: Some("pai ainda candidato".to_string()),
        measures: measure(catalog.iter()),
        recipes: vec![recipe_id("candidate-pai")],
        ..frozen("candidate-pai", &catalog)
    };
    write_snapshot(&repo, &candidate_predecessor);
    let error = plan_prepare(&root, &catalog, "marco-c", "candidate-pai", "novo").unwrap_err();
    assert!(error.to_string().contains("deve ser FROZEN"));

    let error = plan_prepare(&root, &catalog, "marco-a", "marco-a", "colisão").unwrap_err();
    assert!(matches!(error, ProjectionError::Policy { .. }));
}

#[test]
fn reprepare_converge_e_substitui_candidate_quando_catalogo_muda() {
    let (repo, catalog) = setup("reprepare");
    prepare(&repo, &catalog, "marco-c");
    let root = repo.root();
    let same = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    assert_eq!(same.planned_outcome(), "NO_CHANGE");
    assert!(same
        .check
        .targets
        .iter()
        .all(|target| target.change.as_str() == "NO_CHANGE"));

    let mut grown = catalog.clone();
    grown.push(region("c", 3));
    let changed = plan_prepare(&root, &grown, "marco-c", "marco-a", "novo marco").unwrap();
    assert_eq!(changed.planned_outcome(), "CANDIDATE_PLANNED");
    assert_eq!(changed.desired_snapshot.measures, measure(grown.iter()));
    assert_eq!(
        changed
            .check
            .targets
            .iter()
            .find(|target| target.path == snapshot_path("marco-c"))
            .unwrap()
            .change
            .as_str(),
        "REPLACE"
    );
}

#[test]
fn recipe_vazia_canonica_e_preservada_e_recipe_nao_vazia_e_recusada() {
    let (repo, catalog) = setup("recipe-policy");
    prepare(&repo, &catalog, "marco-c");
    let recipe_file = repo.path().join(recipe_path("marco-c"));
    let before = fs::read(&recipe_file).unwrap();
    let root = repo.root();
    let plan = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    let applied = apply_prepare(&root, &catalog, &plan, &plan.digest()).unwrap();
    assert_eq!(applied.outcome, "NO_CHANGE");
    assert_eq!(fs::read(&recipe_file).unwrap(), before);

    let mut maintained = empty_recipe("marco-c");
    maintained.rules.push(Rule::ExcludeKey {
        key: "b".to_string(),
        expected_matches: 1,
    });
    maintained.expected_exclusions = 1;
    fs::write(&recipe_file, render_recipe(&maintained)).unwrap();
    let error = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap_err();
    assert!(matches!(error, ProjectionError::Policy { .. }));
    assert!(error
        .to_string()
        .contains("sobrescrita automática recusada"));
    assert_eq!(
        fs::read_to_string(recipe_file).unwrap(),
        render_recipe(&maintained)
    );
}

#[test]
fn digest_e_independente_do_root_absoluto_e_digest_errado_nao_escreve() {
    let (one, catalog) = setup("digest-one");
    let (two, _) = setup("digest-two-long-root");
    let a = plan_prepare(&one.root(), &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    let b = plan_prepare(&two.root(), &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    assert_eq!(a.digest(), b.digest());

    let error = apply_prepare(&one.root(), &catalog, &a, "0").unwrap_err();
    let ProjectionError::Apply(report) = error else {
        panic!("classificação inesperada")
    };
    assert!(matches!(report.failure, Some(Failure::PolicyViolation(_))));
    assert!(report.applied.is_empty());
    assert!(!one.path().join(snapshot_path("marco-c")).exists());
}

#[test]
fn stale_plan_e_distinto_de_digest_errado() {
    let (repo, catalog) = setup("stale");
    let root = repo.root();
    let plan = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    fs::write(
        repo.path().join(snapshot_path("marco-c")),
        "mudança concorrente\n",
    )
    .unwrap();
    let error = apply_prepare(&root, &catalog, &plan, &plan.digest()).unwrap_err();
    let ProjectionError::Apply(report) = error else {
        panic!("classificação inesperada")
    };
    assert!(matches!(report.failure, Some(Failure::StalePlan { .. })));
    assert!(report.applied.is_empty());
    assert_eq!(
        fs::read_to_string(repo.path().join(snapshot_path("marco-c"))).unwrap(),
        "mudança concorrente\n"
    );
}

#[cfg(unix)]
#[test]
fn apply_parcial_e_observavel_e_rerun_converge() {
    use std::os::unix::fs::PermissionsExt;

    let (repo, catalog) = setup("partial");
    let root = repo.root();
    let plan = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    let recipes = repo.path().join(".pinker/projections/recipes");
    fs::set_permissions(&recipes, fs::Permissions::from_mode(0o500)).unwrap();
    let error = apply_prepare(&root, &catalog, &plan, &plan.digest()).unwrap_err();
    fs::set_permissions(&recipes, fs::Permissions::from_mode(0o700)).unwrap();
    let ProjectionError::Apply(report) = error else {
        panic!("classificação inesperada")
    };
    assert_eq!(report.applied, [snapshot_path("marco-c")]);
    assert_eq!(
        report.failed.as_deref(),
        Some(recipe_path("marco-c").as_str())
    );
    assert!(!report.rollback_performed);
    assert!(repo.path().join(snapshot_path("marco-c")).is_file());

    let rerun = plan_prepare(&root, &catalog, "marco-c", "marco-a", "novo marco").unwrap();
    let applied = apply_prepare(&root, &catalog, &rerun, &rerun.digest()).unwrap();
    assert_eq!(applied.outcome, "CANDIDATE_PREPARED");
    assert_eq!(applied.report.applied, [recipe_path("marco-c")]);
}

#[test]
fn aceitar_planeja_um_target_muda_so_state_e_preserva_recipe() {
    let (repo, catalog) = setup("accept");
    prepare(&repo, &catalog, "marco-c");
    let candidate_bytes = fs::read(repo.path().join(snapshot_path("marco-c"))).unwrap();
    let recipe_bytes = fs::read(repo.path().join(recipe_path("marco-c"))).unwrap();
    let candidate = parse(std::str::from_utf8(&candidate_bytes).unwrap()).unwrap();
    let root = repo.root();
    let plan = plan_accept(&root, &catalog, "marco-c").unwrap();
    assert_eq!(plan.plan.targets().len(), 1);
    assert_eq!(plan.planned_outcome(), "FROZEN_PLANNED");
    assert_eq!(plan.desired_snapshot.state, SnapshotState::Frozen);
    let applied = apply_accept(&root, &catalog, &plan, &plan.digest()).unwrap();
    assert_eq!(applied.outcome, "FROZEN_ACCEPTED");
    let frozen =
        parse(&fs::read_to_string(repo.path().join(snapshot_path("marco-c"))).unwrap()).unwrap();
    let mut expected = candidate.clone();
    expected.state = SnapshotState::Frozen;
    assert_eq!(frozen, expected);
    assert_eq!(frozen.predecessor, candidate.predecessor);
    assert_eq!(frozen.justification, candidate.justification);
    assert_eq!(frozen.measures, candidate.measures);
    assert_eq!(frozen.rules, candidate.rules);
    assert_eq!(
        fs::read(repo.path().join(recipe_path("marco-c"))).unwrap(),
        recipe_bytes
    );

    let second = plan_accept(&root, &catalog, "marco-c").unwrap_err();
    assert!(matches!(second, ProjectionError::Policy { .. }));
}

#[test]
fn aceitar_recusa_drift_harness_e_candidate_invalido() {
    let (repo, catalog) = setup("accept-errors");
    prepare(&repo, &catalog, "marco-c");
    let mut grown = catalog.clone();
    grown.push(region("c", 3));
    let drift = plan_accept(&repo.root(), &grown, "marco-c").unwrap_err();
    assert!(matches!(drift, ProjectionError::Drift { .. }));

    // Um FROZEN que depende do candidate torna a biblioteca inválida para
    // aceitação, sem converter a falha de harness em drift.
    let candidate =
        parse(&fs::read_to_string(repo.path().join(snapshot_path("marco-c"))).unwrap()).unwrap();
    let blocked = ProjectionSnapshot {
        id: "blocked".to_string(),
        state: SnapshotState::Frozen,
        base_snapshot: Some(candidate.id.clone()),
        measures: Measures {
            regions: 0,
            length: 0,
            fnv1a64: 0,
        },
        ..frozen("blocked", &catalog)
    };
    write_snapshot(&repo, &blocked);
    let harness = plan_accept(&repo.root(), &catalog, "marco-c").unwrap_err();
    assert!(matches!(harness, ProjectionError::Harness { .. }));
}

#[test]
fn evolucao_futura_preserva_frozen_e_compõe_recipes_sem_fabricar_historia() {
    let repo = TempRepo::new("future-evolution");
    let s0 = catalog();
    let norm_b_id = "normalizacao-corrente-para-marco-b";
    let norm_b = Recipe {
        schema: 2,
        id: norm_b_id.to_string(),
        steps: Vec::new(),
        expected_overrides: 0,
        expected_exclusions: 0,
        rules: Vec::new(),
    };
    fs::write(
        repo.path().join(norm_b.relative_path()),
        render_recipe(&norm_b),
    )
    .unwrap();
    let mut marco_b = frozen("marco-b", &s0);
    marco_b.recipes = vec![norm_b.id.clone()];
    let s0_a: Vec<CodeRegion> = s0
        .iter()
        .filter(|region| region.key != "b")
        .cloned()
        .collect();
    let mut marco_a = frozen("marco-a", &s0_a);
    marco_a.base_snapshot = Some("marco-b".to_string());
    marco_a.expected_exclusions = 1;
    marco_a.rules = vec![Rule::ExcludeKey {
        key: "b".to_string(),
        expected_matches: 1,
    }];
    write_snapshot(&repo, &marco_a);
    write_snapshot(&repo, &marco_b);
    let frozen_a = fs::read(repo.path().join(marco_a.relative_path())).unwrap();
    let frozen_b = fs::read(repo.path().join(marco_b.relative_path())).unwrap();

    // S1: o catálogo cresce. A manutenção humana mínima da recipe de B
    // preserva A/B sem editar nenhum FROZEN.
    let mut s1 = s0.clone();
    s1.push(region("c", 3));
    let mut norm_b_s1 = norm_b.clone();
    norm_b_s1.expected_exclusions = 1;
    norm_b_s1.rules.push(Rule::ExcludeKey {
        key: "c".to_string(),
        expected_matches: 1,
    });
    fs::write(
        repo.path().join(norm_b_s1.relative_path()),
        render_recipe(&norm_b_s1),
    )
    .unwrap();
    let store = ProjectionStore::load(repo.path()).unwrap();
    let library = store.library().unwrap();
    assert_eq!(
        verify_composed(&library, "marco-a", &s1).outcome.as_str(),
        "MATCH"
    );
    assert_eq!(
        verify_composed(&library, "marco-b", &s1).outcome.as_str(),
        "MATCH"
    );

    // S2/S3: preparar e aceitar C como nova raiz, sem alterar recipes pelo
    // lifecycle. Depois a manutenção humana liga B à recipe de C por steps.
    prepare(&repo, &s1, "marco-c");
    let norm_c_id = recipe_id("marco-c");
    norm_b_s1.steps = vec![norm_c_id.clone()];
    fs::write(
        repo.path().join(norm_b_s1.relative_path()),
        render_recipe(&norm_b_s1),
    )
    .unwrap();
    let accept = plan_accept(&repo.root(), &s1, "marco-c").unwrap();
    apply_accept(&repo.root(), &s1, &accept, &accept.digest()).unwrap();

    // S4: região nova + mudança legítima de hash. A recipe própria de C
    // absorve ambas; A/B herdam por steps. Measures e bytes FROZEN ficam.
    let old_a_hash = s1
        .iter()
        .find(|region| region.key == "a")
        .unwrap()
        .hash
        .clone();
    let mut s2 = s1.clone();
    s2.push(region("d", 4));
    let changed_a_hash = "fnv1a64:00000000000000aa".to_string();
    s2.iter_mut()
        .find(|region| region.key == "a")
        .unwrap()
        .hash
        .clone_from(&changed_a_hash);
    let norm_c = Recipe {
        schema: 2,
        id: norm_c_id,
        steps: Vec::new(),
        expected_overrides: 1,
        expected_exclusions: 1,
        rules: vec![
            Rule::ExcludeKey {
                key: "d".to_string(),
                expected_matches: 1,
            },
            Rule::OverrideHash {
                key: "a".to_string(),
                from: changed_a_hash,
                to: old_a_hash,
                expect_file: None,
                expect_domain: None,
                expect_layer: None,
            },
        ],
    };
    fs::write(
        repo.path().join(norm_c.relative_path()),
        render_recipe(&norm_c),
    )
    .unwrap();
    let store = ProjectionStore::load(repo.path()).unwrap();
    let library = store.library().unwrap();
    for id in ["marco-a", "marco-b", "marco-c"] {
        assert_eq!(
            verify_composed(&library, id, &s2).outcome.as_str(),
            "MATCH",
            "{id}"
        );
    }
    assert_eq!(
        fs::read(repo.path().join(marco_a.relative_path())).unwrap(),
        frozen_a
    );
    assert_eq!(
        fs::read(repo.path().join(marco_b.relative_path())).unwrap(),
        frozen_b
    );

    // Excluir região que pertencia a C produz reconstrução válida mas quebra
    // as medidas; retirar região exigida pelo override quebra o harness. A
    // segunda condição nunca é reclassificada como drift.
    let mut destructive = norm_c.clone();
    destructive.expected_exclusions += 1;
    destructive.rules.push(Rule::ExcludeKey {
        key: "b".to_string(),
        expected_matches: 1,
    });
    fs::write(
        repo.path().join(destructive.relative_path()),
        render_recipe(&destructive),
    )
    .unwrap();
    let store = ProjectionStore::load(repo.path()).unwrap();
    assert_eq!(
        verify_composed(&store.library().unwrap(), "marco-c", &s2)
            .outcome
            .as_str(),
        "DRIFT"
    );

    fs::write(
        repo.path().join(norm_c.relative_path()),
        render_recipe(&norm_c),
    )
    .unwrap();
    let without_a: Vec<CodeRegion> = s2.into_iter().filter(|region| region.key != "a").collect();
    let store = ProjectionStore::load(repo.path()).unwrap();
    assert_eq!(
        verify_composed(&store.library().unwrap(), "marco-c", &without_a)
            .outcome
            .as_str(),
        "HARNESS_FAILURE"
    );
}
