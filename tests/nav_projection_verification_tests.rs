use pinker_v0::nav::CodeRegion;
use pinker_v0::nav_projection_recipe::{verify_composed, Library};
use pinker_v0::nav_projection_report::{
    render_verification_human, render_verification_json, verify_all, verify_one,
};
use pinker_v0::nav_projection_snapshot::{
    measure, render, HarnessFailure, Outcome, ProjectionSnapshot, SnapshotState, SNAPSHOT_SCHEMA,
};
use pinker_v0::nav_projection_store::ProjectionStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRepo(PathBuf);

impl TempRepo {
    fn new() -> TempRepo {
        let path = std::env::temp_dir().join(format!(
            "pinker_projection_verify_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join(".pinker/projections/recipes")).unwrap();
        TempRepo(path)
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

fn region(key: &str) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some("fixture".to_string()),
        layer: Some("verify".to_string()),
        phase: None,
        file: format!("src/{key}.rs"),
        start_marker: 1,
        content_start: 2,
        content_end: 3,
        end_marker: 4,
        summary: format!("Resumo {key}"),
        hash: format!("fnv1a64:{:016x}", key.len()),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

fn snapshot(
    id: &str,
    state: SnapshotState,
    catalog: &[CodeRegion],
    base: Option<&str>,
) -> ProjectionSnapshot {
    ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA,
        id: id.to_string(),
        state,
        predecessor: None,
        justification: Some("fixture".to_string()),
        measures: measure(catalog.iter()),
        expected_overrides: 0,
        expected_exclusions: 0,
        base_snapshot: base.map(str::to_string),
        recipes: Vec::new(),
        rules: Vec::new(),
    }
}

fn write(repo: &TempRepo, model: &ProjectionSnapshot) {
    fs::write(repo.path().join(model.relative_path()), render(model)).unwrap();
}

#[test]
fn verificar_um_ignora_invalido_independente_e_verificar_todos_o_reporta() {
    let repo = TempRepo::new();
    let catalog = vec![region("a"), region("b")];
    write(
        &repo,
        &snapshot("independente", SnapshotState::Frozen, &catalog, None),
    );
    fs::write(
        repo.path()
            .join(".pinker/projections/candidate-invalido.toml"),
        "schema = 3\nid = \"candidate-invalido\"\nstate = \"MEIO\"\n",
    )
    .unwrap();
    let store = ProjectionStore::load(repo.path()).unwrap();
    let one = verify_one(&store, "independente", &catalog).unwrap();
    assert_eq!(one.report.outcome, Outcome::Match);

    let all = verify_all(&store, &catalog);
    assert_eq!(all.outcome(), "HARNESS_FAILURE");
    assert_eq!(all.results.len(), 1);
    assert_eq!(all.errors.len(), 1);
}

#[test]
fn frozen_dependendo_de_candidate_e_harness_failure() {
    let catalog = vec![region("a")];
    let library = Library::new()
        .with_snapshot(snapshot(
            "candidate",
            SnapshotState::Candidate,
            &catalog,
            None,
        ))
        .unwrap()
        .with_snapshot(snapshot(
            "frozen",
            SnapshotState::Frozen,
            &catalog,
            Some("candidate"),
        ))
        .unwrap();
    let report = verify_composed(&library, "frozen", &catalog);
    assert!(matches!(
        report.outcome,
        Outcome::HarnessFailure(HarnessFailure::FrozenDependsOnCandidate { .. })
    ));
    assert!(report.observed.is_none());
}

#[test]
fn drift_da_base_agrupa_dependentes_bloqueados_com_mesmo_significado_em_json_e_humano() {
    let repo = TempRepo::new();
    let initial = vec![region("a")];
    write(
        &repo,
        &snapshot("base", SnapshotState::Frozen, &initial, None),
    );
    write(
        &repo,
        &snapshot("filho-a", SnapshotState::Frozen, &initial, Some("base")),
    );
    write(
        &repo,
        &snapshot("filho-b", SnapshotState::Frozen, &initial, Some("base")),
    );
    let current = vec![region("a"), region("nova")];
    let store = ProjectionStore::load(repo.path()).unwrap();
    let batch = verify_all(&store, &current);
    assert_eq!(batch.outcome(), "HARNESS_FAILURE");
    assert_eq!(batch.causes.len(), 1);
    assert_eq!(batch.causes[0].cause, "base");
    assert_eq!(batch.causes[0].blocked.len(), 2);
    assert_eq!(batch.results.len(), 1);
    assert!(matches!(batch.results[0].report.outcome, Outcome::Drift(_)));

    let human = render_verification_human(&batch);
    let json = render_verification_json(&batch);
    for marker in ["base", "filho-a", "filho-b", "bloqueado"] {
        assert!(human.contains(marker), "{marker}: {human}");
    }
    for marker in ["base", "filho-a", "filho-b", "blocked"] {
        assert!(json.contains(marker), "{marker}: {json}");
    }
    assert_eq!(json.lines().count(), 1);
    assert!(!json.contains(repo.path().to_str().unwrap()));
}
