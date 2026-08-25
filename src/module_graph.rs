//! Unidade modular preservada e o grafo que a compõe.
//!
//! A composição modular anterior reduzia cada `Program(module)` a itens
//! selecionados e clonados dentro de `root_program.items` no instante do
//! carregamento. `package`, `freestanding`, `imports` e `impls` do módulo
//! deixavam de existir antes que qualquer regra semântica pudesse observá-los,
//! e os itens sobreviventes chegavam à resolução carregando apenas a própria
//! grafia.
//!
//! `ModuleUnit` é a unidade que sobrevive. Ela guarda os cinco campos do
//! `Program` mais a identidade da unidade: qual módulo é, de que fonte veio.
//! `ModuleGraph` guarda todas as unidades de uma compilação, com a raiz sempre
//! em `ModuleId::ROOT`.
//!
//! Nada aqui decide semântica de `package` nem de `freestanding`. Os dois
//! campos são transportados como DADO, exatamente como foram escritos. Preservar
//! um dado não é atribuir significado a ele, e atribuir significado aos dois
//! exige decisão que esta camada não possui.

use std::collections::HashMap;

use crate::ast::{ImplDecl, ImportDecl, Item, PackageDecl, Program};
use crate::source_map::SourceId;
use crate::source_origin::SourceOrigin;
use crate::token::Span;

// @pinker-nav:start modulos.unidade.preservacao
// @pinker-nav:domain modulos
// @pinker-nav:layer compilador
// @pinker-nav:summary ModuleId/ModuleKey identificam a unidade modular; ModuleUnit preserva os cinco campos do Program (package, freestanding, imports, impls, items) junto da identidade de módulo e da fonte de origem, de modo que nenhuma validação dependente desses dados possa perdê-los antes de rodar; ModuleGraph reúne as unidades de uma compilação com a raiz em ModuleId::ROOT e oferece a ordem de dependência já resolvida. package e freestanding trafegam como dado, sem contrato semântico novo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(u32);

impl ModuleId {
    pub const ROOT: ModuleId = ModuleId(0);

    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn is_root(self) -> bool {
        self == Self::ROOT
    }
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identidade efetiva da unidade na composição.
///
/// Para módulo, é a chave de import — a mesma que o carregador usa para
/// resolver, deduplicar e detectar ciclo, e a mesma que `SourceOrigin::Module`
/// e `GenericOrigin::Module` já carregavam. A declaração `pacote` do arquivo
/// NÃO participa: hoje ela é inerte na composição, e torná-la identidade seria
/// decidir o contrato `package`, que esta camada não decide.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleKey {
    Root,
    Module(String),
}

impl ModuleKey {
    pub fn module_key(&self) -> Option<&str> {
        match self {
            ModuleKey::Root => None,
            ModuleKey::Module(key) => Some(key.as_str()),
        }
    }

    pub fn source_origin(&self) -> SourceOrigin {
        match self {
            ModuleKey::Root => SourceOrigin::Root,
            ModuleKey::Module(key) => SourceOrigin::module(key.as_str()),
        }
    }

    /// Nome canônico de uma entidade de topo desta unidade.
    ///
    /// A raiz preserva a grafia: ela é o programa que executa, `principal`
    /// continua sendo `principal`, e nenhum símbolo de runtime muda de nome
    /// porque a identidade de frontend passou a existir. Um módulo qualifica
    /// pela própria chave, que é a forma já usada pelos tipos qualificados
    /// (`<módulo>.<Tipo>`) desde antes desta camada.
    ///
    /// `SOURCE SPELLING != SYMBOL IDENTITY`: duas unidades podem escrever
    /// `auxiliar` e obter identidades distintas, que é exatamente o que
    /// `helper` homônimo em módulos independentes precisava e não tinha.
    pub fn canonical(&self, name: &str) -> String {
        match self {
            ModuleKey::Root => name.to_string(),
            ModuleKey::Module(key) => format!("{}.{}", key, name),
        }
    }
}

impl std::fmt::Display for ModuleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleKey::Root => write!(f, "raiz"),
            ModuleKey::Module(key) => write!(f, "{}", key),
        }
    }
}

/// Unidade-fonte preservada como unidade semântica.
#[derive(Debug, Clone)]
pub struct ModuleUnit {
    pub id: ModuleId,
    pub key: ModuleKey,
    pub source_id: SourceId,
    /// Caminho/rótulo do arquivo lido, para diagnóstico.
    pub display: String,
    /// Dado preservado. Sem contrato semântico novo nesta camada.
    pub package: Option<PackageDecl>,
    /// Dado preservado. Sem contrato semântico novo nesta camada.
    pub freestanding: Option<Span>,
    pub imports: Vec<ImportDecl>,
    pub impls: Vec<ImplDecl>,
    pub items: Vec<Item>,
}

impl ModuleUnit {
    pub fn origin(&self) -> SourceOrigin {
        self.key.source_origin()
    }

    pub fn canonical(&self, name: &str) -> String {
        self.key.canonical(name)
    }

    pub fn is_root(&self) -> bool {
        self.id.is_root()
    }

    /// Reconstrói o `Program` desta unidade, com todos os cinco campos.
    ///
    /// É a forma que a validação modular consome: validar a unidade exige o
    /// `Program` que ela de fato é, não a projeção do que dela sobrou.
    pub fn to_program(&self) -> Program {
        Program {
            package: self.package.clone(),
            freestanding: self.freestanding,
            imports: self.imports.clone(),
            impls: self.impls.clone(),
            items: self.items.clone(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModuleGraph {
    units: Vec<ModuleUnit>,
    by_key: HashMap<String, ModuleId>,
}

impl ModuleGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insere a unidade raiz. Deve ser a primeira.
    pub fn insert_root(
        &mut self,
        source_id: SourceId,
        display: impl Into<String>,
        program: Program,
    ) -> ModuleId {
        debug_assert!(self.units.is_empty(), "raiz precisa ser a primeira unidade");
        self.push(ModuleKey::Root, source_id, display, program)
    }

    /// Insere um módulo pela chave canônica do carregador. Uma chave já
    /// presente devolve o id existente: uma unidade-fonte nunca recebe duas
    /// identidades modulares.
    pub fn insert_module(
        &mut self,
        module_key: impl Into<String>,
        source_id: SourceId,
        display: impl Into<String>,
        program: Program,
    ) -> ModuleId {
        let module_key = module_key.into();
        if let Some(existing) = self.by_key.get(&module_key) {
            return *existing;
        }
        let id = self.push(
            ModuleKey::Module(module_key.clone()),
            source_id,
            display,
            program,
        );
        self.by_key.insert(module_key, id);
        id
    }

    fn push(
        &mut self,
        key: ModuleKey,
        source_id: SourceId,
        display: impl Into<String>,
        program: Program,
    ) -> ModuleId {
        let id = ModuleId(u32::try_from(self.units.len()).expect("unidades cabem em u32"));
        self.units.push(ModuleUnit {
            id,
            key,
            source_id,
            display: display.into(),
            package: program.package,
            freestanding: program.freestanding,
            imports: program.imports,
            impls: program.impls,
            items: program.items,
        });
        id
    }

    pub fn root(&self) -> &ModuleUnit {
        self.units.first().expect("grafo possui raiz")
    }

    pub fn unit(&self, id: ModuleId) -> &ModuleUnit {
        &self.units[id.0 as usize]
    }

    pub fn unit_mut(&mut self, id: ModuleId) -> &mut ModuleUnit {
        &mut self.units[id.0 as usize]
    }

    pub fn module_id(&self, module_key: &str) -> Option<ModuleId> {
        self.by_key.get(module_key).copied()
    }

    pub fn module(&self, module_key: &str) -> Option<&ModuleUnit> {
        self.module_id(module_key).map(|id| self.unit(id))
    }

    pub fn units(&self) -> &[ModuleUnit] {
        &self.units
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn has_modules(&self) -> bool {
        self.units.len() > 1
    }

    /// Ids em ordem de dependência: cada unidade aparece depois daquelas de
    /// que depende.
    ///
    /// A raiz ocupa o índice 0 porque é a primeira a ser lida, mas é a última a
    /// ser resolvida — ela depende de todo módulo que importa. Os módulos, por
    /// sua vez, já entram no grafo em ordem de dependência: o carregador só
    /// insere um módulo depois de ter recursado nos imports dele, e ciclo é
    /// recusado no carregamento, então essa ordem existe sempre.
    pub fn dependency_order(&self) -> Vec<ModuleId> {
        let mut order: Vec<ModuleId> = self.units.iter().skip(1).map(|unit| unit.id).collect();
        if !self.units.is_empty() {
            order.push(ModuleId::ROOT);
        }
        order
    }
}
// @pinker-nav:end modulos.unidade.preservacao
