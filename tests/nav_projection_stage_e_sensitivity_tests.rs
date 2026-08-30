//! Sensibilidade permanente do Stage E. Cada caso muta uma fixture; o controle
//! positivo intacto precisa passar antes que os negativos tenham significado.

use pinker_v0::automation::RepoRoot;
use pinker_v0::automation::{Allowlist, PlanBuilder};
use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_lifecycle::{plan_prepare, recipe_path, snapshot_path};
use pinker_v0::nav_projection_recipe::{render_recipe, verify_composed, Recipe};
use pinker_v0::nav_projection_snapshot::{
    json_report, measure, render, Outcome, ProjectionSnapshot, Rule, SnapshotState, VerifyReport,
    SNAPSHOT_REPORT_SCHEMA, SNAPSHOT_SCHEMA,
};
use pinker_v0::nav_projection_store::ProjectionStore;
use std::fs;
use std::path::{Path, PathBuf};

struct Repo(PathBuf);

impl Repo {
    fn new() -> Repo {
        let path =
            std::env::temp_dir().join(format!("pinker_stage_e_sensitivity_{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".pinker/projections/recipes")).unwrap();
        fs::write(path.join(".pinker/doc.toml"), "# root\n").unwrap();
        Repo(path)
    }

    fn root(&self) -> RepoRoot {
        RepoRoot::at(&self.0).unwrap()
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn region(key: &str) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some("sensitivity".to_string()),
        layer: Some("tests".to_string()),
        phase: None,
        file: format!("src/{key}.rs"),
        start_marker: 1,
        content_start: 2,
        content_end: 3,
        end_marker: 4,
        summary: key.to_string(),
        hash: "fnv1a64:0000000000000001".to_string(),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

fn frozen(id: &str, catalog: &[CodeRegion]) -> ProjectionSnapshot {
    ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA,
        id: id.to_string(),
        state: SnapshotState::Frozen,
        predecessor: None,
        justification: Some("controle".to_string()),
        measures: measure(catalog.iter()),
        expected_overrides: 0,
        expected_exclusions: 0,
        expected_materializations: 0,
        base_snapshot: None,
        recipes: Vec::new(),
        rules: Vec::new(),
    }
}

#[test]
fn controle_positivo_e_matriz_de_regressoes() {
    let repo = Repo::new();
    let catalog = vec![region("a")];
    let model = frozen("marco-a", &catalog);
    fs::write(repo.path().join(model.relative_path()), render(&model)).unwrap();

    // Controle positivo intacto.
    let store = ProjectionStore::load(repo.path()).unwrap();
    assert!(store.errors().is_empty());
    assert_eq!(
        verify_composed(&store.library().unwrap(), "marco-a", &catalog).outcome,
        Outcome::Match
    );

    // Remover filename == id precisa ser detectado.
    fs::write(
        repo.path().join(".pinker/projections/nome-errado.toml"),
        render(&frozen("id-interno", &catalog)),
    )
    .unwrap();
    assert!(ProjectionStore::load(repo.path())
        .unwrap()
        .errors()
        .iter()
        .any(|error| error.message.contains("não corresponde")));
    fs::remove_file(repo.path().join(".pinker/projections/nome-errado.toml")).unwrap();

    // Sobrescrever FROZEN e usar predecessor CANDIDATE são recusados.
    assert!(plan_prepare(&repo.root(), &catalog, "marco-a", "marco-a", "x").is_err());
    let mut candidate_parent = frozen("candidate-parent", &catalog);
    candidate_parent.state = SnapshotState::Candidate;
    candidate_parent.predecessor = Some("marco-a".to_string());
    candidate_parent.recipes = vec!["normalizacao-corrente-para-candidate-parent".to_string()];
    fs::write(
        repo.path().join(candidate_parent.relative_path()),
        render(&candidate_parent),
    )
    .unwrap();
    assert!(plan_prepare(&repo.root(), &catalog, "marco-c", "candidate-parent", "x").is_err());

    // Recipe não vazia nunca é sobrescrita.
    let mut recipe = Recipe {
        schema: 2,
        id: "normalizacao-corrente-para-marco-c".to_string(),
        steps: Vec::new(),
        expected_overrides: 0,
        expected_exclusions: 1,
        rules: vec![Rule::ExcludeKey {
            key: "a".to_string(),
            expected_matches: 1,
        }],
    };
    fs::write(
        repo.path().join(recipe_path("marco-c")),
        render_recipe(&recipe),
    )
    .unwrap();
    assert!(plan_prepare(&repo.root(), &catalog, "marco-c", "marco-a", "x").is_err());

    // Allowlist estreita rejeita terceiro target.
    let snapshot = snapshot_path("marco-c");
    let own_recipe = recipe_path("marco-c");
    let allowlist = Allowlist::new(&[snapshot.as_str(), own_recipe.as_str()]).unwrap();
    assert!(PlanBuilder::new("sensitivity", allowlist)
        .desire("src/main.rs", Vec::new())
        .is_err());

    // Schema de relatório não deriva do schema do artefato.
    assert_eq!(SNAPSHOT_REPORT_SCHEMA, 1);
    assert_ne!(SNAPSHOT_REPORT_SCHEMA, SNAPSHOT_SCHEMA);
    let report = VerifyReport {
        snapshot_id: "x".to_string(),
        state: SnapshotState::Frozen,
        predecessor: None,
        expected: model.measures,
        observed: Some(model.measures),
        outcome: Outcome::Match,
        ledger: Vec::new(),
    };
    assert!(json_report(&report).starts_with("{\"schema\":1,"));

    // Harness permanece separado de drift quando uma regra deixa de consumir.
    recipe.expected_overrides = 1;
    recipe.expected_exclusions = 0;
    recipe.rules = vec![Rule::OverrideHash {
        key: "ausente".to_string(),
        from: "fnv1a64:0000000000000001".to_string(),
        to: "fnv1a64:0000000000000002".to_string(),
        expect_file: None,
        expect_domain: None,
        expect_layer: None,
    }];
    let mut target = frozen("com-recipe", &catalog);
    target.recipes = vec![recipe.id.clone()];
    let library = pinker_v0::nav_projection_recipe::Library::new()
        .with_recipe(recipe)
        .unwrap()
        .with_snapshot(target)
        .unwrap();
    assert!(matches!(
        verify_composed(&library, "com-recipe", &catalog).outcome,
        Outcome::HarnessFailure(_)
    ));
}
