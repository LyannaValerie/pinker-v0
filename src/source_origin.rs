/// Proveniência canônica de uma entidade gerada a partir de fonte.
///
/// `Module` recebe a mesma chave textual que o loader usa para resolução,
/// ciclo, deduplicação e lookup. Caminho físico, cwd, worktree e ordem de
/// import não participam desta identidade.
// @pinker-nav:start identities.source-provenance
// @pinker-nav:domain identity
// @pinker-nav:layer compiler
// @pinker-nav:summary Minimal shared provenance value for generated identities that cross program assembly: it distinguishes builtin, root and module sources by the loader's canonical key, without reconstruction from a display name or a physical path.
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
// @pinker-nav:end identities.source-provenance
