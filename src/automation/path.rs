//! Política **lexical** de paths e allowlist lógica em memória.
//!
//! Este estágio valida apenas a forma do path, sem tocar o filesystem. O
//! confinamento real — descoberta canônica do root, resolução, rejeição de
//! symlink no target e em ancestral — pertence ao estágio de apply, e depende de
//! syscalls que o núcleo puro não pode executar.
//!
//! A separação é deliberada: um path lexicalmente válido ainda pode ser
//! inseguro no disco, e prometer o contrário aqui seria enganoso.

use super::{PolicyCause, MAX_PATH_LEN};

// @pinker-nav:start automation.paths.politica-lexical
// @pinker-nav:domain paths
// @pinker-nav:layer automation
// @pinker-nav:summary Política lexical de paths repo-relativos (rejeita vazio, absoluto, travessia, componente degenerado, barra invertida, caractere de controle e excesso de comprimento) e allowlist lógica em memória, ordenada e sem duplicatas — sem qualquer acesso ao filesystem, cujo confinamento real pertence ao estágio de apply.

/// Um path repo-relativo já validado lexicalmente.
///
/// O tipo é a prova: não existe construção que não passe pela validação, então
/// nenhuma parte do núcleo precisa revalidar.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelativePath(String);

impl RelativePath {
    /// Valida e constrói. Puro: nenhuma consulta ao disco.
    pub fn new(raw: &str) -> Result<RelativePath, PolicyCause> {
        if raw.is_empty() {
            return Err(PolicyCause::PathEmpty);
        }
        if raw.len() > MAX_PATH_LEN {
            return Err(PolicyCause::PathTooLong {
                path: raw.to_string(),
                len: raw.len(),
            });
        }
        if raw.starts_with('/') {
            return Err(PolicyCause::PathAbsolute {
                path: raw.to_string(),
            });
        }
        if raw.contains('\\') {
            return Err(PolicyCause::PathBackslash {
                path: raw.to_string(),
            });
        }
        if raw.chars().any(|c| c.is_control()) {
            return Err(PolicyCause::PathControlChar {
                path: raw.to_string(),
            });
        }
        for component in raw.split('/') {
            if component == ".." {
                return Err(PolicyCause::PathTraversal {
                    path: raw.to_string(),
                });
            }
            if component.is_empty() || component == "." {
                return Err(PolicyCause::PathDegenerateComponent {
                    path: raw.to_string(),
                    component: component.to_string(),
                });
            }
        }
        Ok(RelativePath(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Allowlist lógica: o conjunto fechado de paths que um plano pode declarar.
///
/// É lógica e em memória de propósito. Ela não consulta o disco e não substitui
/// o confinamento do estágio de apply; ela responde apenas "este target foi
/// declarado?".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowlist {
    entries: Vec<RelativePath>,
}

impl Allowlist {
    /// Constrói a partir de paths crus, validando cada um.
    ///
    /// A ordem interna é canônica (ordenada, sem duplicatas), de modo que duas
    /// allowlists com os mesmos membros são iguais qualquer que seja a ordem de
    /// declaração.
    pub fn new(paths: &[&str]) -> Result<Allowlist, PolicyCause> {
        let mut entries = Vec::with_capacity(paths.len());
        for raw in paths {
            entries.push(RelativePath::new(raw)?);
        }
        entries.sort();
        entries.dedup();
        Ok(Allowlist { entries })
    }

    /// Verdadeiro se o path foi declarado.
    pub fn permits(&self, path: &RelativePath) -> bool {
        self.entries.binary_search(path).is_ok()
    }

    /// Membros em ordem canônica.
    pub fn entries(&self) -> &[RelativePath] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
// @pinker-nav:end automation.paths.politica-lexical

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_path_repo_relativo() {
        let path = RelativePath::new("docs/engine/state.md").expect("path válido");
        assert_eq!(path.as_str(), "docs/engine/state.md");
    }

    #[test]
    fn rejeita_formas_invalidas() {
        assert_eq!(RelativePath::new(""), Err(PolicyCause::PathEmpty));
        assert!(matches!(
            RelativePath::new("/etc/passwd"),
            Err(PolicyCause::PathAbsolute { .. })
        ));
        assert!(matches!(
            RelativePath::new("../fora.md"),
            Err(PolicyCause::PathTraversal { .. })
        ));
        assert!(matches!(
            RelativePath::new("docs//a.md"),
            Err(PolicyCause::PathDegenerateComponent { .. })
        ));
        assert!(matches!(
            RelativePath::new("docs\\a.md"),
            Err(PolicyCause::PathBackslash { .. })
        ));
        assert!(matches!(
            RelativePath::new("docs/\u{7}.md"),
            Err(PolicyCause::PathControlChar { .. })
        ));
    }

    #[test]
    fn allowlist_e_canonica_e_independe_da_ordem() {
        let a = Allowlist::new(&["b.md", "a.md", "b.md"]).expect("allowlist válida");
        let b = Allowlist::new(&["a.md", "b.md"]).expect("allowlist válida");
        assert_eq!(a, b);
        assert_eq!(a.entries().len(), 2);
        assert!(a.permits(&RelativePath::new("a.md").unwrap()));
        assert!(!a.permits(&RelativePath::new("c.md").unwrap()));
    }
}
