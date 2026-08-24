//! Identidade de fonte anexada a localização de diagnóstico.
//!
//! `SOURCE_LOCATION = SOURCE_ID + SPAN`.
//!
//! Um `Span` sozinho é um par de posições e não diz a que texto pertence. Isso
//! basta enquanto existe um texto só; deixa de bastar assim que um módulo
//! contribui posições para o mesmo pipeline, porque nada impede que a linha 22
//! de um módulo seja renderizada contra a linha 22 da raiz.
//!
//! `SourceId` é a metade que faltava. Ele é atribuído por unidade-fonte no
//! momento em que ela é lida, viaja dentro do `Span` e é consultado na
//! renderização. `SourceMap` é o registro que resolve o id de volta ao texto.
//!
//! `SourceId::UNKNOWN` existe para spans sintéticos, que não reivindicam fonte
//! alguma. Não reivindicar fonte é diferente de reivindicar a raiz: o
//! renderizador trata `UNKNOWN` como "sem alegação" e cai no texto primário,
//! que é exatamente o comportamento histórico de programa de arquivo único.

use std::collections::HashMap;

// @pinker-nav:start diagnostico.fonte.identidade
// @pinker-nav:domain diagnostico
// @pinker-nav:layer compilador
// @pinker-nav:summary SourceId identifica a unidade-fonte de onde uma posição veio e viaja dentro do Span; SourceKey distingue raiz de módulo pela chave canônica do carregador; SourceMap registra texto e rótulo por id e é a única autoridade que resolve id de volta a texto na renderização de diagnóstico. UNKNOWN é ausência de alegação de fonte, não alegação de raiz.
/// Identidade da unidade-fonte à qual uma posição pertence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(u32);

impl SourceId {
    /// Span sintético: nenhuma fonte é reivindicada.
    pub const UNKNOWN: SourceId = SourceId(u32::MAX);

    /// A unidade-fonte primária do pipeline (o arquivo pedido na linha de
    /// comando). Sempre o primeiro id registrado num `SourceMap`.
    pub const ROOT: SourceId = SourceId(0);

    pub fn as_u32(self) -> u32 {
        self.0
    }

    pub fn is_unknown(self) -> bool {
        self == Self::UNKNOWN
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_unknown() {
            write!(f, "desconhecida")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Papel da unidade-fonte na composição.
///
/// `Module` recebe a mesma chave textual que o carregador usa para resolução,
/// ciclo e deduplicação — a mesma que `SourceOrigin::Module`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceKey {
    Root,
    Module(String),
}

impl SourceKey {
    pub fn module_key(&self) -> Option<&str> {
        match self {
            SourceKey::Root => None,
            SourceKey::Module(key) => Some(key.as_str()),
        }
    }
}

impl std::fmt::Display for SourceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceKey::Root => write!(f, "raiz"),
            SourceKey::Module(key) => write!(f, "módulo '{}'", key),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceRecord {
    pub id: SourceId,
    pub key: SourceKey,
    /// Rótulo legível — tipicamente o caminho do arquivo lido.
    pub display: String,
    pub text: String,
}

/// Registro de unidades-fonte de uma única compilação.
///
/// A ordem de registro é a ordem de descoberta e a raiz é sempre a primeira,
/// então `SourceId::ROOT` é estável sem precisar ser procurado.
#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    records: Vec<SourceRecord>,
    by_module: HashMap<String, SourceId>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra a unidade-fonte primária. Deve ser a primeira chamada.
    pub fn register_root(
        &mut self,
        display: impl Into<String>,
        text: impl Into<String>,
    ) -> SourceId {
        debug_assert!(self.records.is_empty(), "raiz precisa ser a primeira fonte");
        self.push(SourceKey::Root, display, text)
    }

    /// Registra um módulo pela chave canônica do carregador. Uma chave já
    /// registrada devolve o mesmo id: a mesma unidade-fonte nunca recebe duas
    /// identidades.
    pub fn register_module(
        &mut self,
        module_key: impl Into<String>,
        display: impl Into<String>,
        text: impl Into<String>,
    ) -> SourceId {
        let module_key = module_key.into();
        if let Some(existing) = self.by_module.get(&module_key) {
            return *existing;
        }
        let id = self.push(SourceKey::Module(module_key.clone()), display, text);
        self.by_module.insert(module_key, id);
        id
    }

    fn push(
        &mut self,
        key: SourceKey,
        display: impl Into<String>,
        text: impl Into<String>,
    ) -> SourceId {
        let id = SourceId(u32::try_from(self.records.len()).expect("fontes cabem em u32"));
        self.records.push(SourceRecord {
            id,
            key,
            display: display.into(),
            text: text.into(),
        });
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceRecord> {
        if id.is_unknown() {
            return None;
        }
        self.records.get(id.0 as usize)
    }

    pub fn module_id(&self, module_key: &str) -> Option<SourceId> {
        self.by_module.get(module_key).copied()
    }

    pub fn root(&self) -> Option<&SourceRecord> {
        self.records.first()
    }

    /// Texto ao qual um span pertence.
    ///
    /// `UNKNOWN` não reivindica fonte e cai na raiz — sem isso, todo span
    /// sintético perderia o trecho que sempre teve. Um id registrado devolve
    /// exclusivamente o seu próprio texto: é aqui que
    /// `SOURCE_LOCATION_INTEGRITY` deixa de depender de convenção.
    pub fn text_for(&self, id: SourceId) -> Option<&str> {
        if id.is_unknown() {
            return self.root().map(|record| record.text.as_str());
        }
        // Id registrado devolve o próprio texto. Id que este mapa não conhece
        // devolve NADA — cair na raiz aqui é precisamente o defeito C1: um
        // trecho plausível do arquivo errado é pior que trecho nenhum.
        self.get(id).map(|record| record.text.as_str())
    }

    /// Rótulo da fonte para uso em diagnóstico, quando ela não é a raiz.
    pub fn label_for(&self, id: SourceId) -> Option<&str> {
        self.get(id).map(|record| record.display.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn records(&self) -> &[SourceRecord] {
        &self.records
    }
}

/// `SourceMap` de um texto só, para os caminhos que não compõem módulos.
pub fn single(display: impl Into<String>, text: impl Into<String>) -> SourceMap {
    let mut map = SourceMap::new();
    map.register_root(display, text);
    map
}
// @pinker-nav:end diagnostico.fonte.identidade
