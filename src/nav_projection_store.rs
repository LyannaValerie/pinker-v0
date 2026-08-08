//! Leitura isolada e inventário determinístico da autoridade de projeções.
//!
//! O store é deliberadamente somente leitura. Ele transforma cada artefato em
//! um resultado próprio, de modo que um TOML inválido não torne snapshots
//! independentes invisíveis. Escritas do lifecycle pertencem exclusivamente ao
//! automation core.

use crate::nav_projection_recipe::{parse_recipe, Library, Recipe, RECIPES_DIR};
use crate::nav_projection_snapshot::{
    parse as parse_snapshot, HarnessFailure, ProjectionSnapshot, SNAPSHOTS_DIR,
};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

// @pinker-nav:start trama.projecoes.store
// @pinker-nav:domain projecoes
// @pinker-nav:layer store
// @pinker-nav:summary Store somente leitura da autoridade de projeções: enumera snapshots e recipes em ordem determinística, valida filename contra id interno, preserva bytes e paths repo-relativos e isola falhas estruturais por artefato sem ocultar os demais.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Snapshot,
    Recipe,
    Unknown,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::Snapshot => "snapshot",
            ArtifactKind::Recipe => "recipe",
            ArtifactKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactError {
    pub path: String,
    pub kind: ArtifactKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSnapshot {
    pub path: String,
    pub snapshot: ProjectionSnapshot,
    pub bytes: Vec<u8>,
    pub canonical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecipe {
    pub path: String,
    pub recipe: Recipe,
    pub bytes: Vec<u8>,
    pub canonical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreFailure {
    AuthorityIo { path: String, msg: String },
}

impl fmt::Display for StoreFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreFailure::AuthorityIo { path, msg } => {
                write!(f, "autoridade de projeções ilegível em '{}': {}", path, msg)
            }
        }
    }
}

impl std::error::Error for StoreFailure {}

#[derive(Debug, Clone, Default)]
pub struct ProjectionStore {
    snapshots: BTreeMap<String, StoredSnapshot>,
    recipes: BTreeMap<String, StoredRecipe>,
    errors: Vec<ArtifactError>,
}

impl ProjectionStore {
    pub fn load(root: &Path) -> Result<ProjectionStore, StoreFailure> {
        let snapshot_entries = read_authority(root, SNAPSHOTS_DIR)?;
        let recipe_entries = read_authority(root, RECIPES_DIR)?;
        let mut store = ProjectionStore::default();

        for path in recipe_entries {
            store.load_recipe(root, &path);
        }
        for path in snapshot_entries {
            if path.file_name().and_then(|name| name.to_str()) == Some("recipes") && path.is_dir() {
                continue;
            }
            store.load_snapshot(root, &path);
        }
        store.errors.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(store)
    }

    pub fn snapshots(&self) -> impl Iterator<Item = &StoredSnapshot> {
        self.snapshots.values()
    }

    pub fn recipes(&self) -> impl Iterator<Item = &StoredRecipe> {
        self.recipes.values()
    }

    pub fn snapshot(&self, id: &str) -> Option<&StoredSnapshot> {
        self.snapshots.get(id)
    }

    pub fn recipe(&self, id: &str) -> Option<&StoredRecipe> {
        self.recipes.get(id)
    }

    pub fn errors(&self) -> &[ArtifactError] {
        &self.errors
    }

    pub fn snapshot_error(&self, id: &str) -> Option<&ArtifactError> {
        let path = format!("{SNAPSHOTS_DIR}{id}.toml");
        self.errors.iter().find(|error| error.path == path)
    }

    pub fn recipe_error(&self, id: &str) -> Option<&ArtifactError> {
        let path = format!("{RECIPES_DIR}{id}.toml");
        self.errors.iter().find(|error| error.path == path)
    }

    /// Constrói a biblioteca a partir de todos os artefatos válidos. Erros já
    /// permanecem disponíveis separadamente em [`ProjectionStore::errors`].
    pub fn library(&self) -> Result<Library, HarnessFailure> {
        let mut library = Library::new();
        for stored in self.recipes.values() {
            library = library.with_recipe(stored.recipe.clone())?;
        }
        for stored in self.snapshots.values() {
            library = library.with_snapshot(stored.snapshot.clone())?;
        }
        Ok(library)
    }

    fn load_snapshot(&mut self, root: &Path, absolute: &Path) {
        let relative = relative_path(root, absolute);
        if !is_toml_file(absolute) {
            self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Unknown,
                message: "entrada não reconhecida; esperado arquivo .toml de snapshot".to_string(),
            });
            return;
        }
        let Some(stem) = file_stem(absolute) else {
            self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Snapshot,
                message: "nome de arquivo não é UTF-8".to_string(),
            });
            return;
        };
        let bytes = match fs::read(absolute) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.errors.push(ArtifactError {
                    path: relative,
                    kind: ArtifactKind::Snapshot,
                    message: error.to_string(),
                });
                return;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(error) => {
                self.errors.push(ArtifactError {
                    path: relative,
                    kind: ArtifactKind::Snapshot,
                    message: format!("conteúdo não é UTF-8: {error}"),
                });
                return;
            }
        };
        match parse_snapshot(text) {
            Err(error) => self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Snapshot,
                message: error.to_string().replace('\n', " "),
            }),
            Ok(snapshot) if snapshot.id != stem => self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Snapshot,
                message: format!(
                    "id interno '{}' não corresponde ao nome do arquivo '{}'",
                    snapshot.id, stem
                ),
            }),
            Ok(snapshot) => {
                let canonical = crate::nav_projection_snapshot::render(&snapshot).as_bytes()
                    == bytes.as_slice();
                self.snapshots.insert(
                    snapshot.id.clone(),
                    StoredSnapshot {
                        path: relative,
                        snapshot,
                        bytes,
                        canonical,
                    },
                );
            }
        }
    }

    fn load_recipe(&mut self, root: &Path, absolute: &Path) {
        let relative = relative_path(root, absolute);
        if !is_toml_file(absolute) {
            self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Unknown,
                message: "entrada não reconhecida; esperado arquivo .toml de recipe".to_string(),
            });
            return;
        }
        let Some(stem) = file_stem(absolute) else {
            self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Recipe,
                message: "nome de arquivo não é UTF-8".to_string(),
            });
            return;
        };
        let bytes = match fs::read(absolute) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.errors.push(ArtifactError {
                    path: relative,
                    kind: ArtifactKind::Recipe,
                    message: error.to_string(),
                });
                return;
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(error) => {
                self.errors.push(ArtifactError {
                    path: relative,
                    kind: ArtifactKind::Recipe,
                    message: format!("conteúdo não é UTF-8: {error}"),
                });
                return;
            }
        };
        match parse_recipe(text) {
            Err(error) => self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Recipe,
                message: error.to_string().replace('\n', " "),
            }),
            Ok(recipe) if recipe.id != stem => self.errors.push(ArtifactError {
                path: relative,
                kind: ArtifactKind::Recipe,
                message: format!(
                    "id interno '{}' não corresponde ao nome do arquivo '{}'",
                    recipe.id, stem
                ),
            }),
            Ok(recipe) => {
                let canonical = crate::nav_projection_recipe::render_recipe(&recipe).as_bytes()
                    == bytes.as_slice();
                self.recipes.insert(
                    recipe.id.clone(),
                    StoredRecipe {
                        path: relative,
                        recipe,
                        bytes,
                        canonical,
                    },
                );
            }
        }
    }
}

fn read_authority(root: &Path, relative: &str) -> Result<Vec<PathBuf>, StoreFailure> {
    let mut entries = Vec::new();
    let directory = root.join(relative);
    let iterator = fs::read_dir(directory).map_err(|error| StoreFailure::AuthorityIo {
        path: relative.trim_end_matches('/').to_string(),
        msg: if error.kind() == ErrorKind::NotFound {
            "diretório ausente".to_string()
        } else {
            error.to_string()
        },
    })?;
    for entry in iterator {
        match entry {
            Ok(entry) => entries.push(entry.path()),
            Err(error) => {
                return Err(StoreFailure::AuthorityIo {
                    path: relative.trim_end_matches('/').to_string(),
                    msg: error.to_string(),
                })
            }
        }
    }
    entries.sort();
    Ok(entries)
}

fn is_toml_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|extension| extension.to_str()) == Some("toml")
}

fn file_stem(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(str::to_string)
}

fn relative_path(root: &Path, absolute: &Path) -> String {
    absolute
        .strip_prefix(root)
        .unwrap_or(absolute)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

// @pinker-nav:end trama.projecoes.store
