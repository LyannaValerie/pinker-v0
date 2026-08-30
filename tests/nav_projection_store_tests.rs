use pinker_v0::nav_projection_lifecycle::empty_recipe;
use pinker_v0::nav_projection_recipe::{render_recipe, RECIPE_SCHEMA};
use pinker_v0::nav_projection_snapshot::{
    render, Measures, ProjectionSnapshot, SnapshotState, SNAPSHOT_SCHEMA,
};
use pinker_v0::nav_projection_store::{ArtifactKind, ProjectionStore, StoreFailure};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct TempRepo(PathBuf);

impl TempRepo {
    fn new(label: &str) -> TempRepo {
        let path = std::env::temp_dir().join(format!(
            "pinker_projection_store_{}_{}_{}",
            label,
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

fn snapshot(id: &str, state: SnapshotState) -> ProjectionSnapshot {
    ProjectionSnapshot {
        schema: SNAPSHOT_SCHEMA,
        id: id.to_string(),
        state,
        predecessor: None,
        justification: Some("fixture".to_string()),
        measures: Measures {
            regions: 0,
            length: 0,
            fnv1a64: 0,
        },
        expected_overrides: 0,
        expected_exclusions: 0,
        expected_materializations: 0,
        base_snapshot: None,
        recipes: Vec::new(),
        rules: Vec::new(),
    }
}

fn write_snapshot(root: &Path, file: &str, model: &ProjectionSnapshot) {
    fs::write(
        root.join(format!(".pinker/projections/{file}.toml")),
        render(model),
    )
    .unwrap();
}

#[test]
fn inventario_vazio_e_valido() {
    let repo = TempRepo::new("empty");
    let store = ProjectionStore::load(repo.path()).unwrap();
    assert_eq!(store.snapshots().count(), 0);
    assert_eq!(store.recipes().count(), 0);
    assert!(store.errors().is_empty());
    assert!(store.library().is_ok());
}

#[test]
fn inventario_valido_ordena_e_reporta_paths_relativos() {
    let repo = TempRepo::new("ordered");
    write_snapshot(
        repo.path(),
        "zeta",
        &snapshot("zeta", SnapshotState::Frozen),
    );
    write_snapshot(
        repo.path(),
        "alfa",
        &snapshot("alfa", SnapshotState::Candidate),
    );
    let recipe = empty_recipe("alfa");
    assert_eq!(recipe.schema, RECIPE_SCHEMA);
    fs::write(
        repo.path().join(recipe.relative_path()),
        render_recipe(&recipe),
    )
    .unwrap();

    let store = ProjectionStore::load(repo.path()).unwrap();
    let ids: Vec<&str> = store
        .snapshots()
        .map(|stored| stored.snapshot.id.as_str())
        .collect();
    assert_eq!(ids, ["alfa", "zeta"]);
    assert_eq!(
        store.snapshot("alfa").unwrap().path,
        ".pinker/projections/alfa.toml"
    );
    assert_eq!(
        store.recipe(&recipe.id).unwrap().path,
        ".pinker/projections/recipes/normalizacao-corrente-para-alfa.toml"
    );
    assert!(!store
        .snapshot("alfa")
        .unwrap()
        .path
        .contains(repo.path().to_str().unwrap()));
}

#[test]
fn filename_divergente_snapshot_e_recipe_sao_isolados() {
    let repo = TempRepo::new("filename");
    write_snapshot(
        repo.path(),
        "nome-externo",
        &snapshot("nome-interno", SnapshotState::Frozen),
    );
    let recipe = empty_recipe("interno");
    fs::write(
        repo.path()
            .join(".pinker/projections/recipes/normalizacao-corrente-para-externo.toml"),
        render_recipe(&recipe),
    )
    .unwrap();
    write_snapshot(
        repo.path(),
        "valido",
        &snapshot("valido", SnapshotState::Frozen),
    );

    let store = ProjectionStore::load(repo.path()).unwrap();
    assert!(store.snapshot("valido").is_some());
    assert!(store.snapshot("nome-interno").is_none());
    assert_eq!(store.errors().len(), 2);
    assert!(store
        .errors()
        .iter()
        .all(|error| error.message.contains("não corresponde")));
}

#[test]
fn snapshot_candidate_e_recipe_invalidos_nao_ocultam_artefatos_validos() {
    let repo = TempRepo::new("invalid");
    write_snapshot(
        repo.path(),
        "frozen-ok",
        &snapshot("frozen-ok", SnapshotState::Frozen),
    );
    fs::write(
        repo.path().join(".pinker/projections/quebrado.toml"),
        "schema = 3\nid = \"quebrado\"\nstate = \"MEIO\"\n",
    )
    .unwrap();
    fs::write(
        repo.path()
            .join(".pinker/projections/candidate-quebrado.toml"),
        "schema = 3\nid = \"candidate-quebrado\"\nstate = \"CANDIDATE\"\n",
    )
    .unwrap();
    fs::write(
        repo.path()
            .join(".pinker/projections/recipes/quebrada.toml"),
        "schema = 999\nid = \"quebrada\"\n",
    )
    .unwrap();

    let store = ProjectionStore::load(repo.path()).unwrap();
    assert!(store.snapshot("frozen-ok").is_some());
    assert_eq!(store.errors().len(), 3);
    assert!(store.library().unwrap().snapshot("frozen-ok").is_some());
}

#[test]
fn arquivo_extra_nao_reconhecido_aparece_como_erro_escopado() {
    let repo = TempRepo::new("extra");
    fs::write(repo.path().join(".pinker/projections/README.txt"), "extra").unwrap();
    let store = ProjectionStore::load(repo.path()).unwrap();
    assert_eq!(store.errors().len(), 1);
    assert_eq!(store.errors()[0].kind, ArtifactKind::Unknown);
    assert_eq!(store.errors()[0].path, ".pinker/projections/README.txt");
}

#[test]
fn diretorio_ausente_e_falha_de_autoridade() {
    let repo = TempRepo::new("missing");
    fs::remove_dir_all(repo.path().join(".pinker/projections/recipes")).unwrap();
    let error = ProjectionStore::load(repo.path()).unwrap_err();
    assert!(matches!(error, StoreFailure::AuthorityIo { .. }));
    assert!(error.to_string().contains("diretório ausente"));
}

#[test]
fn roots_absolutos_distintos_produzem_o_mesmo_inventario_relativo() {
    let one = TempRepo::new("root-one");
    let two = TempRepo::new("root-two-with-long-name");
    for repo in [&one, &two] {
        write_snapshot(
            repo.path(),
            "marco",
            &snapshot("marco", SnapshotState::Frozen),
        );
    }
    let a = ProjectionStore::load(one.path()).unwrap();
    let b = ProjectionStore::load(two.path()).unwrap();
    assert_eq!(
        a.snapshot("marco").unwrap().path,
        b.snapshot("marco").unwrap().path
    );
    assert_eq!(
        a.snapshot("marco").unwrap().bytes,
        b.snapshot("marco").unwrap().bytes
    );
}
