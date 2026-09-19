//! Descoberta canônica da raiz do repositório.
//!
//! O núcleo puro do estágio anterior só conhece paths repo-relativos. Para
//! observar e aplicar é preciso uma raiz, e ela precisa ser **canônica**: dois
//! processos que alcancem o mesmo repositório por caminhos diferentes — um por
//! link simbólico, outro por caminho relativo — têm de convergir para a mesma
//! raiz, ou o confinamento vira ficção.

// @pinker-nav:start automation.root.discovery
// @pinker-nav:domain root
// @pinker-nav:layer automation
// @pinker-nav:summary Canonical discovery of the repository root by walking up from the starting directory until the `.pinker/doc.toml` marker is found, with canonicalization that resolves symbolic links and relative components, so that distinct paths to the same repository converge to the same absolute root.
use super::{Failure, HarnessCause};
use std::path::{Component, Path, PathBuf};

/// Marcador que identifica a raiz do repositório.
///
/// Reutiliza a autoridade existente: `.pinker/doc.toml` é a configuração
/// canônica da Trama, declarada por [`crate::doc::CONFIG_RELATIVE_PATH`]. Não se
/// inventa um segundo marcador.
pub const ROOT_MARKER: &str = crate::doc::CONFIG_RELATIVE_PATH;

/// Uma raiz de repositório já canonicalizada.
///
/// O tipo é a prova: não existe construção que não passe pela canonicalização,
/// então nenhuma parte do confinamento precisa reconferir a forma da raiz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRoot {
    path: PathBuf,
}

impl RepoRoot {
    /// Descobre a raiz subindo a partir de `start` até achar o marcador.
    ///
    /// `start` é canonicalizado antes da subida, então links simbólicos no
    /// caminho de partida não produzem raízes diferentes para o mesmo
    /// repositório.
    pub fn discover(start: &Path) -> Result<RepoRoot, Failure> {
        let canonical = start.canonicalize().map_err(|err| {
            Failure::HarnessFailure(HarnessCause::RootNotFound {
                start: start.display().to_string(),
                msg: err.to_string(),
            })
        })?;
        let mut current = canonical.as_path();
        loop {
            if current.join(ROOT_MARKER).is_file() {
                return Ok(RepoRoot {
                    path: current.to_path_buf(),
                });
            }
            match current.parent() {
                Some(parent) => current = parent,
                None => {
                    return Err(Failure::HarnessFailure(HarnessCause::RootNotFound {
                        start: canonical.display().to_string(),
                        msg: format!("nenhum ancestral contém o marcador '{}'", ROOT_MARKER),
                    }))
                }
            }
        }
    }

    /// Aceita uma raiz declarada explicitamente, canonicalizando-a e exigindo o
    /// marcador. Não sobe: quem declara a raiz declara exatamente qual é.
    pub fn at(path: &Path) -> Result<RepoRoot, Failure> {
        let canonical = path.canonicalize().map_err(|err| {
            Failure::HarnessFailure(HarnessCause::RootNotFound {
                start: path.display().to_string(),
                msg: err.to_string(),
            })
        })?;
        if !canonical.join(ROOT_MARKER).is_file() {
            return Err(Failure::HarnessFailure(HarnessCause::RootNotFound {
                start: canonical.display().to_string(),
                msg: format!("marcador '{}' ausente na raiz declarada", ROOT_MARKER),
            }));
        }
        Ok(RepoRoot { path: canonical })
    }

    /// Raiz absoluta e canônica.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Verdadeiro quando o path absoluto está sob esta raiz.
    ///
    /// Comparação por componentes, não por prefixo textual: `/repo-de-outro`
    /// tem `/repo` como prefixo de string e não está sob ele.
    pub fn contains(&self, absolute: &Path) -> bool {
        if !absolute.is_absolute() {
            return false;
        }
        let mut root_components = self.path.components();
        let mut candidate = absolute.components();
        loop {
            match (root_components.next(), candidate.next()) {
                (None, _) => return true,
                (Some(_), None) => return false,
                (Some(a), Some(b)) if a == b => continue,
                _ => return false,
            }
        }
    }

    /// Junta um path repo-relativo à raiz, sem tocar o filesystem.
    ///
    /// A validação lexical já aconteceu no tipo `RelativePath`; aqui só se
    /// verifica, por construção, que nenhum componente estranho entrou.
    pub(crate) fn join_relative(&self, relative: &str) -> PathBuf {
        let mut out = self.path.clone();
        for component in Path::new(relative).components() {
            match component {
                Component::Normal(part) => out.push(part),
                // Inalcançável: `RelativePath` já rejeitou absolutos, `.` e `..`.
                _ => continue,
            }
        }
        out
    }
}
// @pinker-nav:end automation.root.discovery

// @pinker-nav:start evidence.automation.root
// @pinker-nav:domain automation
// @pinker-nav:layer evidence
// @pinker-nav:summary Proofs of confinement by the root: the containment question compares by component and not by textual prefix, and the relative join discards foreign components instead of escaping the root.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_compara_por_componente_e_nao_por_prefixo_textual() {
        let root = RepoRoot {
            path: PathBuf::from("/repo"),
        };
        assert!(root.contains(Path::new("/repo")));
        assert!(root.contains(Path::new("/repo/docs/a.md")));
        assert!(!root.contains(Path::new("/repo-de-outro/a.md")));
        assert!(!root.contains(Path::new("/outro/repo/a.md")));
        assert!(!root.contains(Path::new("docs/a.md")));
    }

    #[test]
    fn join_relative_descarta_componentes_estranhos() {
        let root = RepoRoot {
            path: PathBuf::from("/repo"),
        };
        assert_eq!(
            root.join_relative("docs/a.md"),
            PathBuf::from("/repo/docs/a.md")
        );
    }
}
// @pinker-nav:end evidence.automation.root
