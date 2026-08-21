/// Proveniência canônica de uma entidade gerada a partir de fonte.
///
/// `Module` recebe a mesma chave textual que o loader usa para resolução,
/// ciclo, deduplicação e lookup. Caminho físico, cwd, worktree e ordem de
/// import não participam desta identidade.
// @pinker-nav:start identidades.proveniencia-fonte
// @pinker-nav:domain identidade
// @pinker-nav:layer compilador
// @pinker-nav:summary Valor mínimo compartilhado de proveniência para identidades geradas que atravessam montagem de programa: distingue fonte builtin, raiz e módulo pela chave canônica do loader, sem reconstrução por display name ou caminho físico.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceOrigin {
    Builtin,
    Root,
    Module(String),
}

impl SourceOrigin {
    pub fn module(module_key: impl Into<String>) -> Self {
        Self::Module(module_key.into())
    }
}
// @pinker-nav:end identidades.proveniencia-fonte
