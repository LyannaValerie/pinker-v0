//! Índice derivado de símbolos para `pink nav localizar`.
//!
//! Não existe manifesto de símbolos: este módulo agrega exclusivamente os
//! vínculos explícitos já serializados no catálogo da Trama e resolve IDs
//! documentais contra o catálogo documental vigente.

// @pinker-nav:start trama.symbols.model
// @pinker-nav:domain symbols
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::symbol_index::LocateReport|LocateReport|rust-type|declaration
// @pinker-nav:symbol-doc pinker_v0::symbol_index::LocateReport|development.symbol-index
// @pinker-nav:summary Single, public, versioned lookup model: same-named candidates stay separated by identity; relations carry the status KNOWN, UNKNOWN or UNAVAILABLE, repo-relative paths and the explicit authority that produced each link.
use crate::doc_index::{DocCatalog, DocDocument, DocSection};
use crate::nav::{CodeCatalog, CodeRegion, SymbolKind, SymbolRole};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Schema público próprio de `pink nav localizar`.
pub const SYMBOL_LOCATION_SCHEMA: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationStatus {
    Known,
    Unknown,
    Unavailable,
}

impl RelationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RelationStatus::Known => "KNOWN",
            RelationStatus::Unknown => "UNKNOWN",
            RelationStatus::Unavailable => "UNAVAILABLE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationAuthority {
    pub catalog: String,
    pub path: String,
    pub region: Option<String>,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CodeLocation {
    pub path: String,
    pub region: String,
    pub start: usize,
    pub end: usize,
    pub authority: RelationAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionRelation {
    pub id: String,
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub authority: RelationAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentationRelation {
    pub id: String,
    pub document: String,
    pub path: String,
    pub title: String,
    pub authority: RelationAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestRelation {
    pub region: String,
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub authority: RelationAuthority,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation<T> {
    pub status: RelationStatus,
    pub reason: Option<String>,
    pub items: Vec<T>,
}

impl<T> Relation<T> {
    fn known(items: Vec<T>) -> Relation<T> {
        Relation {
            status: RelationStatus::Known,
            reason: None,
            items,
        }
    }

    fn unknown(reason: &str) -> Relation<T> {
        Relation {
            status: RelationStatus::Unknown,
            reason: Some(reason.to_string()),
            items: Vec::new(),
        }
    }

    fn unavailable(reason: &str) -> Relation<T> {
        Relation {
            status: RelationStatus::Unavailable,
            reason: Some(reason.to_string()),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolCandidate {
    pub identity: String,
    pub name: String,
    pub kind: SymbolKind,
    pub stability: String,
    pub declaration: Relation<CodeLocation>,
    pub implementation: Relation<CodeLocation>,
    pub regions: Relation<RegionRelation>,
    pub documentation: Relation<DocumentationRelation>,
    pub tests: Relation<TestRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocateReport {
    pub schema: u64,
    pub query: String,
    /// Identidades explícitas sob o contrato de metadados já existente.
    pub candidates: Vec<SymbolCandidate>,
    /// Declarações observadas lexicalmente na fonte corrente. Não são
    /// identidade semântica e nunca carregam vínculos explícitos.
    pub extracted_candidates: Vec<ExtractedCandidate>,
    /// Ocorrências textuais delimitadas, fora da extração estrutural.
    pub textual_occurrences: Vec<TextualOccurrence>,
    /// Limitações declaradas da observação, como dados estáveis em inglês.
    pub limitations: Vec<String>,
    /// Arquivos cuja fonte mudou durante a leitura e não puderam ser
    /// observados de forma estável. Nunca viram resultado silencioso.
    pub unstable_sources: Vec<String>,
    /// Total de resultados das três classes antes da janela de orçamento.
    pub total: usize,
    /// Início da janela efetivamente devolvida.
    pub offset: usize,
    pub truncated: bool,
    pub continuation: Option<usize>,
}

/// Uma declaração reconhecida na fonte corrente. Deliberadamente não é
/// identidade semântica: o nome foi observado lexicalmente, não resolvido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedCandidate {
    pub path: String,
    pub kind: String,
    pub name: String,
    /// Primeira linha do intervalo de declaração observado (1-based).
    pub start: usize,
    /// Última linha do intervalo de declaração observado (1-based).
    pub end: usize,
    /// Contexto estrutural reconhecível (`mod`, `impl`, `trait`) quando houver.
    pub context: Option<String>,
    /// Havia atributo imediatamente antes da declaração. Presença observada,
    /// nunca avaliada.
    pub attributes_present: bool,
    /// Algum desses atributos mencionava `cfg`. Presença observada, nunca
    /// avaliada.
    pub cfg_present: bool,
    /// Texto exatamente correspondente a `start..=end`, possivelmente cortado.
    pub snippet: String,
    pub snippet_truncated: bool,
}

/// Ocorrência textual delimitada: o nome apareceu em código que o extrator não
/// reconheceu como declaração suportada. Não é declaração nem identidade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextualOccurrence {
    pub path: String,
    pub line: usize,
    pub snippet: String,
    pub snippet_truncated: bool,
    /// Motivo estável, em inglês, pelo qual a ocorrência não foi promovida.
    pub limitation: String,
}

impl LocateReport {
    pub fn found(&self) -> bool {
        !self.candidates.is_empty()
            || !self.extracted_candidates.is_empty()
            || !self.textual_occurrences.is_empty()
    }
}
// @pinker-nav:end trama.symbols.model

// @pinker-nav:start trama.symbols.derivation
// @pinker-nav:domain symbols
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::symbol_index::locate|locate|rust-function|declaration
// @pinker-nav:symbol pinker_v0::symbol_index::locate|locate|rust-function|implementation
// @pinker-nav:symbol-doc pinker_v0::symbol_index::locate|development.symbol-index
// @pinker-nav:summary Derives the index entirely in memory from CodeCatalog and DocCatalog: it matches only exact names or identities, validates destinations and consistency, preserves same-named items and never reads sources, runs grep, writes, calls Git, the network or subprocesses.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolIndexError {
    SourceScan(String),
    ConflictingIdentity {
        identity: String,
    },
    MissingTarget {
        region: String,
        field: String,
        identity: String,
    },
    InvalidTestRegion {
        region: String,
        identity: String,
    },
    MissingDocumentTarget {
        region: String,
        identity: String,
        document: String,
    },
    AmbiguousDocumentTarget {
        region: String,
        identity: String,
        document: String,
    },
}

impl fmt::Display for SymbolIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolIndexError::SourceScan(message) => write!(f, "E-NAV-SOURCE\n{message}"),
            SymbolIndexError::ConflictingIdentity { identity } => write!(
                f,
                "E-NAV-SYMBOL-INDEX\nIdentidade de símbolo '{}' possui nome ou categoria conflitante.",
                identity
            ),
            SymbolIndexError::MissingTarget {
                region,
                field,
                identity,
            } => write!(
                f,
                "E-NAV-SYMBOL-INDEX\nRegião '{}' referencia símbolo inexistente '{}' em '{}'.",
                region, identity, field
            ),
            SymbolIndexError::InvalidTestRegion { region, identity } => write!(
                f,
                "E-NAV-SYMBOL-INDEX\nVínculo de teste para '{}' deve partir de região na camada 'evidencia'; encontrado '{}'.",
                identity, region
            ),
            SymbolIndexError::MissingDocumentTarget {
                region,
                identity,
                document,
            } => write!(
                f,
                "E-NAV-SYMBOL-INDEX\nRegião '{}' vincula '{}' ao destino documental inexistente '{}'.",
                region, identity, document
            ),
            SymbolIndexError::AmbiguousDocumentTarget {
                region,
                identity,
                document,
            } => write!(
                f,
                "E-NAV-SYMBOL-INDEX\nRegião '{}' vincula '{}' ao destino documental ambíguo '{}'.",
                region, identity, document
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct CandidateBuilder {
    identity: String,
    name: String,
    kind: SymbolKind,
    declarations: BTreeSet<CodeLocation>,
    implementations: BTreeSet<CodeLocation>,
    regions: BTreeSet<RegionRelation>,
    docs: BTreeSet<DocumentationRelation>,
    tests: BTreeSet<TestRelation>,
}

impl CandidateBuilder {
    fn new(identity: &str, name: &str, kind: SymbolKind) -> CandidateBuilder {
        CandidateBuilder {
            identity: identity.to_string(),
            name: name.to_string(),
            kind,
            declarations: BTreeSet::new(),
            implementations: BTreeSet::new(),
            regions: BTreeSet::new(),
            docs: BTreeSet::new(),
            tests: BTreeSet::new(),
        }
    }
}

pub fn locate(
    code: &CodeCatalog,
    docs: Option<&DocCatalog>,
    query: &str,
) -> Result<LocateReport, SymbolIndexError> {
    let mut builders: BTreeMap<String, CandidateBuilder> = BTreeMap::new();

    for region in &code.regions {
        for binding in &region.symbols {
            let entry = builders.entry(binding.identity.clone()).or_insert_with(|| {
                CandidateBuilder::new(&binding.identity, &binding.name, binding.kind)
            });
            if entry.name != binding.name || entry.kind != binding.kind {
                return Err(SymbolIndexError::ConflictingIdentity {
                    identity: binding.identity.clone(),
                });
            }
            let location = code_location(region, binding.role.as_str());
            match binding.role {
                SymbolRole::Declaration => {
                    entry.declarations.insert(location);
                }
                SymbolRole::Implementation => {
                    entry.implementations.insert(location);
                }
            }
            entry.regions.insert(region_relation(region, "symbols"));
        }
    }

    for region in &code.regions {
        for identity in &region.related_symbols {
            let Some(entry) = builders.get_mut(identity) else {
                return Err(missing_target(region, "related-symbol", identity));
            };
            entry
                .regions
                .insert(region_relation(region, "related_symbols"));
        }
        for identity in &region.test_for {
            let Some(entry) = builders.get_mut(identity) else {
                return Err(missing_target(region, "test-for", identity));
            };
            if region.layer.as_deref() != Some("evidence") {
                return Err(SymbolIndexError::InvalidTestRegion {
                    region: region.key.clone(),
                    identity: identity.clone(),
                });
            }
            entry.tests.insert(test_relation(region));
            entry.regions.insert(region_relation(region, "test_for"));
        }
        for link in &region.symbol_docs {
            let Some(entry) = builders.get_mut(&link.identity) else {
                return Err(missing_target(region, "symbol-doc", &link.identity));
            };
            if let Some(catalog) = docs {
                let document = resolve_document(catalog, &link.document).map_err(|()| {
                    SymbolIndexError::AmbiguousDocumentTarget {
                        region: region.key.clone(),
                        identity: link.identity.clone(),
                        document: link.document.clone(),
                    }
                })?;
                let Some(document) = document else {
                    return Err(SymbolIndexError::MissingDocumentTarget {
                        region: region.key.clone(),
                        identity: link.identity.clone(),
                        document: link.document.clone(),
                    });
                };
                entry.docs.insert(document_relation(
                    region,
                    &link.document,
                    document.document,
                    document.section,
                ));
            }
        }
    }

    let mut candidates = builders
        .into_values()
        .filter(|candidate| candidate.identity == query || candidate.name == query)
        .map(|candidate| finish_candidate(candidate, docs.is_some()))
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        a.identity
            .cmp(&b.identity)
            .then(a.kind.cmp(&b.kind))
            .then(a.name.cmp(&b.name))
    });
    let total = candidates.len();

    Ok(LocateReport {
        schema: SYMBOL_LOCATION_SCHEMA,
        query: query.to_string(),
        candidates,
        extracted_candidates: Vec::new(),
        textual_occurrences: Vec::new(),
        limitations: Vec::new(),
        unstable_sources: Vec::new(),
        total,
        offset: 0,
        truncated: false,
        continuation: None,
    })
}

/// Extends explicit navigation with bounded lexical observations from the
/// current worktree. Source is read directly, so dirty and untracked files are
/// part of the observed universe; no cache participates in this result.
fn missing_target(region: &CodeRegion, field: &str, identity: &str) -> SymbolIndexError {
    SymbolIndexError::MissingTarget {
        region: region.key.clone(),
        field: field.to_string(),
        identity: identity.to_string(),
    }
}

fn code_location(region: &CodeRegion, field: &str) -> CodeLocation {
    CodeLocation {
        path: region.file.clone(),
        region: region.key.clone(),
        start: region.content_start,
        end: region.content_end,
        authority: code_authority(region, field),
    }
}

fn region_relation(region: &CodeRegion, field: &str) -> RegionRelation {
    RegionRelation {
        id: region.key.clone(),
        path: region.file.clone(),
        start: region.content_start,
        end: region.content_end,
        authority: code_authority(region, field),
    }
}

fn test_relation(region: &CodeRegion) -> TestRelation {
    TestRelation {
        region: region.key.clone(),
        path: region.file.clone(),
        start: region.content_start,
        end: region.content_end,
        authority: code_authority(region, "test_for"),
    }
}

fn code_authority(region: &CodeRegion, field: &str) -> RelationAuthority {
    RelationAuthority {
        catalog: "src/navigation.jsonl".to_string(),
        path: region.file.clone(),
        region: Some(region.key.clone()),
        field: field.to_string(),
    }
}

struct ResolvedDocument<'a> {
    document: &'a DocDocument,
    section: Option<&'a DocSection>,
}

fn resolve_document<'a>(
    catalog: &'a DocCatalog,
    target: &str,
) -> Result<Option<ResolvedDocument<'a>>, ()> {
    let documents = catalog
        .documents
        .iter()
        .filter(|document| document.id == target)
        .collect::<Vec<_>>();
    if documents.len() > 1 {
        return Err(());
    }
    if let Some(document) = documents.first() {
        return Ok(Some(ResolvedDocument {
            document,
            section: None,
        }));
    }
    let sections = catalog
        .sections
        .iter()
        .filter(|section| section.id == target)
        .collect::<Vec<_>>();
    if sections.len() > 1 {
        return Err(());
    }
    if let Some(section) = sections.first() {
        return Ok(catalog
            .document(&section.document)
            .map(|document| ResolvedDocument {
                document,
                section: Some(section),
            }));
    }
    let authorities = catalog
        .documents
        .iter()
        .filter(|document| {
            document
                .canonical_for
                .iter()
                .any(|concept| concept == target)
        })
        .collect::<Vec<_>>();
    if authorities.len() > 1 {
        return Err(());
    }
    Ok(authorities.first().map(|document| ResolvedDocument {
        document,
        section: None,
    }))
}

fn document_relation(
    region: &CodeRegion,
    target: &str,
    document: &DocDocument,
    section: Option<&DocSection>,
) -> DocumentationRelation {
    let (id, title) = section.map_or_else(
        || (target.to_string(), document.title.clone()),
        |section| (section.id.clone(), section.title.clone()),
    );
    DocumentationRelation {
        id,
        document: document.id.clone(),
        path: repo_doc_path(&document.file),
        title,
        authority: RelationAuthority {
            catalog: "docs/navigation.jsonl".to_string(),
            path: repo_doc_path(&document.file),
            region: Some(region.key.clone()),
            field: "symbol_docs+document_id".to_string(),
        },
    }
}

fn repo_doc_path(path: &str) -> String {
    if path == "docs" || path.starts_with("docs/") {
        path.to_string()
    } else {
        format!("docs/{path}")
    }
}

fn finish_candidate(builder: CandidateBuilder, docs_available: bool) -> SymbolCandidate {
    let declaration = relation_or_unknown(
        builder.declarations.into_iter().collect(),
        "nenhuma declaração explícita publicada pela Trama",
    );
    let implementation = relation_or_unknown(
        builder.implementations.into_iter().collect(),
        "nenhuma implementação explícita publicada pela Trama",
    );
    let regions = relation_or_unknown(
        builder.regions.into_iter().collect(),
        "nenhuma região explicitamente relacionada",
    );
    let documentation = if docs_available {
        relation_or_unknown(
            builder.docs.into_iter().collect(),
            "nenhum vínculo documental explícito publicado",
        )
    } else {
        Relation::unavailable("catálogo docs/navigation.jsonl ausente")
    };
    let tests = relation_or_unknown(
        builder.tests.into_iter().collect(),
        "nenhum vínculo de teste explícito publicado",
    );
    SymbolCandidate {
        identity: builder.identity,
        name: builder.name,
        kind: builder.kind,
        stability: "explicit-navigation-metadata-v1".to_string(),
        declaration,
        implementation,
        regions,
        documentation,
        tests,
    }
}

fn relation_or_unknown<T>(items: Vec<T>, reason: &str) -> Relation<T> {
    if items.is_empty() {
        Relation::unknown(reason)
    } else {
        Relation::known(items)
    }
}
// @pinker-nav:end trama.symbols.derivation

// @pinker-nav:start trama.symbols.rendering
// @pinker-nav:domain symbols
// @pinker-nav:layer reports
// @pinker-nav:summary Renders human and JSON output exclusively from LocateReport, preserving the same material information, its own schema 1, fixed order, explicit absence and no absolute path, ANSI or incidental data.

pub fn render_json(report: &LocateReport) -> String {
    let candidates = report
        .candidates
        .iter()
        .map(candidate_json)
        .collect::<Vec<_>>()
        .join(",");
    let extracted = report
        .extracted_candidates
        .iter()
        .map(extracted_json)
        .collect::<Vec<_>>()
        .join(",");
    let textual = report
        .textual_occurrences
        .iter()
        .map(textual_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"query\":{},\"candidates\":[{}],\"extracted_candidates\":[{}],\"textual_occurrences\":[{}],\"limitations\":{},\"unstable_sources\":{},\"total\":{},\"offset\":{},\"truncated\":{},\"continuation\":{}}}",
        report.schema,
        json_string(&report.query),
        candidates,
        extracted,
        textual,
        json_string_list(&report.limitations),
        json_string_list(&report.unstable_sources),
        report.total,
        report.offset,
        report.truncated,
        report
            .continuation
            .map_or_else(|| "null".to_string(), |value| value.to_string())
    )
}

fn json_string_list(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub fn render_human(report: &LocateReport) -> String {
    // Fonte instável não pode sair como "nada encontrado": não observar é
    // diferente de observar ausência.
    if !report.found() && report.unstable_sources.is_empty() {
        return format!(
            "Nenhum símbolo estruturado encontrado para: {}\n",
            report.query
        );
    }
    let mut out = String::new();
    out.push_str(&format!("Símbolos para '{}'\n", report.query));
    for candidate in &report.candidates {
        out.push_str(&format!(
            "\n{} [{}]\n  classificação: EXPLICIT_SYMBOL\n  nome: {}\n  estabilidade: {}\n",
            candidate.identity,
            candidate.kind.as_str(),
            candidate.name,
            candidate.stability
        ));
        append_code_relation(&mut out, "declaração", &candidate.declaration);
        append_code_relation(&mut out, "implementação", &candidate.implementation);
        append_region_relation(&mut out, &candidate.regions);
        append_document_relation(&mut out, &candidate.documentation);
        append_test_relation(&mut out, &candidate.tests);
    }
    for candidate in &report.extracted_candidates {
        out.push_str(&format!(
            "\n{} [{}]\n  classificação: EXTRACTED_CANDIDATE\n  local: {}:{}-{}\n  contexto: {}\n  atributos: {}\n  cfg: {}\n  trecho: {}{}\n",
            candidate.name,
            candidate.kind,
            candidate.path,
            candidate.start,
            candidate.end,
            candidate.context.as_deref().unwrap_or("UNKNOWN"),
            candidate.attributes_present,
            candidate.cfg_present,
            candidate.snippet,
            if candidate.snippet_truncated {
                " [SNIPPET_TRUNCATED]"
            } else {
                ""
            }
        ));
    }
    for occurrence in &report.textual_occurrences {
        out.push_str(&format!(
            "\n{}\n  classificação: TEXTUAL_OCCURRENCE\n  local: {}:{}\n  trecho: {}{}\n  limitação: {}\n",
            report.query,
            occurrence.path,
            occurrence.line,
            occurrence.snippet,
            if occurrence.snippet_truncated {
                " [SNIPPET_TRUNCATED]"
            } else {
                ""
            },
            occurrence.limitation
        ));
    }
    if !report.limitations.is_empty() {
        out.push_str("\nLimitações declaradas:\n");
        for limitation in &report.limitations {
            out.push_str(&format!("  - {limitation}\n"));
        }
    }
    if !report.unstable_sources.is_empty() {
        out.push_str("\nUNSTABLE_SOURCE = TRUE\n");
        for path in &report.unstable_sources {
            out.push_str(&format!("  - {path}\n"));
        }
    }
    if report.truncated {
        out.push_str(&format!(
            "\nTRUNCATED = TRUE; total {}; continue com --desde {}\n",
            report.total,
            report.continuation.unwrap_or_default()
        ));
    }
    out
}

fn extracted_json(item: &ExtractedCandidate) -> String {
    format!(
        "{{\"classification\":\"EXTRACTED_CANDIDATE\",\"path\":{},\"kind\":{},\"name\":{},\"start\":{},\"end\":{},\"context\":{},\"attributes_present\":{},\"cfg_present\":{},\"snippet\":{},\"snippet_truncated\":{}}}",
        json_string(&item.path),
        json_string(&item.kind),
        json_string(&item.name),
        item.start,
        item.end,
        option_string(item.context.as_deref()),
        item.attributes_present,
        item.cfg_present,
        json_string(&item.snippet),
        item.snippet_truncated
    )
}

fn textual_json(item: &TextualOccurrence) -> String {
    format!(
        "{{\"classification\":\"TEXTUAL_OCCURRENCE\",\"path\":{},\"line\":{},\"snippet\":{},\"snippet_truncated\":{},\"limitation\":{}}}",
        json_string(&item.path),
        item.line,
        json_string(&item.snippet),
        item.snippet_truncated,
        json_string(&item.limitation)
    )
}

fn append_relation_status<T>(out: &mut String, label: &str, relation: &Relation<T>) -> bool {
    out.push_str(&format!("  {label}: {}", relation.status.as_str()));
    if let Some(reason) = &relation.reason {
        out.push_str(&format!(" ({reason})"));
    }
    out.push('\n');
    relation.status == RelationStatus::Known
}

fn append_code_relation(out: &mut String, label: &str, relation: &Relation<CodeLocation>) {
    if append_relation_status(out, label, relation) {
        for item in &relation.items {
            out.push_str(&format!(
                "    - {}:{}-{} (região {}; autoridade {}:{}:{})\n",
                item.path,
                item.start,
                item.end,
                item.region,
                item.authority.catalog,
                item.authority.region.as_deref().unwrap_or("-"),
                item.authority.field
            ));
        }
    }
}

fn append_region_relation(out: &mut String, relation: &Relation<RegionRelation>) {
    if append_relation_status(out, "regiões", relation) {
        for item in &relation.items {
            out.push_str(&format!(
                "    - {} em {}:{}-{} (autoridade {}:{})\n",
                item.id,
                item.path,
                item.start,
                item.end,
                item.authority.catalog,
                item.authority.field
            ));
        }
    }
}

fn append_document_relation(out: &mut String, relation: &Relation<DocumentationRelation>) {
    if append_relation_status(out, "documentação", relation) {
        for item in &relation.items {
            out.push_str(&format!(
                "    - {} [{}], documento {}, em {} (autoridade {}:{}:{})\n",
                item.id,
                item.title,
                item.document,
                item.path,
                item.authority.catalog,
                item.authority.region.as_deref().unwrap_or("-"),
                item.authority.field
            ));
        }
    }
}

fn append_test_relation(out: &mut String, relation: &Relation<TestRelation>) {
    if append_relation_status(out, "testes", relation) {
        for item in &relation.items {
            out.push_str(&format!(
                "    - {} em {}:{}-{} (autoridade {}:{})\n",
                item.region,
                item.path,
                item.start,
                item.end,
                item.authority.catalog,
                item.authority.field
            ));
        }
    }
}

fn candidate_json(candidate: &SymbolCandidate) -> String {
    format!(
        "{{\"classification\":\"EXPLICIT_SYMBOL\",\"identity\":{},\"name\":{},\"kind\":{},\"stability\":{},\"declaration\":{},\"implementation\":{},\"regions\":{},\"documentation\":{},\"tests\":{}}}",
        json_string(&candidate.identity),
        json_string(&candidate.name),
        json_string(candidate.kind.as_str()),
        json_string(&candidate.stability),
        relation_json(&candidate.declaration, code_location_json),
        relation_json(&candidate.implementation, code_location_json),
        relation_json(&candidate.regions, region_json),
        relation_json(&candidate.documentation, documentation_json),
        relation_json(&candidate.tests, test_json),
    )
}

fn relation_json<T>(relation: &Relation<T>, render: fn(&T) -> String) -> String {
    let items = relation
        .items
        .iter()
        .map(render)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"status\":{},\"reason\":{},\"items\":[{}]}}",
        json_string(relation.status.as_str()),
        option_string(relation.reason.as_deref()),
        items
    )
}

fn code_location_json(item: &CodeLocation) -> String {
    format!(
        "{{\"path\":{},\"region\":{},\"start\":{},\"end\":{},\"authority\":{}}}",
        json_string(&item.path),
        json_string(&item.region),
        item.start,
        item.end,
        authority_json(&item.authority)
    )
}

fn region_json(item: &RegionRelation) -> String {
    format!(
        "{{\"id\":{},\"path\":{},\"start\":{},\"end\":{},\"authority\":{}}}",
        json_string(&item.id),
        json_string(&item.path),
        item.start,
        item.end,
        authority_json(&item.authority)
    )
}

fn documentation_json(item: &DocumentationRelation) -> String {
    format!(
        "{{\"id\":{},\"document\":{},\"path\":{},\"title\":{},\"authority\":{}}}",
        json_string(&item.id),
        json_string(&item.document),
        json_string(&item.path),
        json_string(&item.title),
        authority_json(&item.authority)
    )
}

fn test_json(item: &TestRelation) -> String {
    format!(
        "{{\"region\":{},\"path\":{},\"start\":{},\"end\":{},\"authority\":{}}}",
        json_string(&item.region),
        json_string(&item.path),
        item.start,
        item.end,
        authority_json(&item.authority)
    )
}

fn authority_json(authority: &RelationAuthority) -> String {
    format!(
        "{{\"catalog\":{},\"path\":{},\"region\":{},\"field\":{}}}",
        json_string(&authority.catalog),
        json_string(&authority.path),
        option_string(authority.region.as_deref()),
        json_string(&authority.field)
    )
}

fn option_string(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_string(), json_string)
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
// @pinker-nav:end trama.symbols.rendering
// @pinker-nav:start evidence.symbols.derivation-and-render
// @pinker-nav:domain symbols
// @pinker-nav:layer evidence
// @pinker-nav:summary Proofs of the symbol index: the explicit relations are derived without heuristics and absence is published as absence, same-named items are preserved in identity order, a nonexistent destination and a fabricated test outside evidence are refused, and the renderers consume the same model and are deterministic.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc_index::DocCatalog;
    use crate::nav::CodeCatalog;

    const DOCS: &str = "{\"schema\":2,\"record\":\"document\",\"id\":\"development.symbol-index\",\"domain\":\"development\",\"kind\":\"reference\",\"status\":\"active\",\"parent\":\"development\",\"file\":\"development/symbol-index.md\",\"title\":\"Índice\",\"audience\":[\"human\"]}\n";

    fn line(key: &str, file: &str, layer: &str, extra: &str) -> String {
        format!(
            "{{\"schema\":1,\"key\":\"{key}\",\"kind\":\"region\",\"layer\":\"{layer}\",\"file\":\"{file}\",\"start_marker\":1,\"content_start\":2,\"content_end\":3,\"end_marker\":4,\"hash\":\"fnv1a64:00\",\"status\":\"active\"{extra}}}\n"
        )
    }

    #[test]
    fn deriva_relacoes_explicitas_e_ausencias_sem_heuristica() {
        let code = CodeCatalog::parse(
            &(line(
                "codigo.alvo",
                "src/a.rs",
                "core",
                ",\"symbols\":[\"pkg::alvo|alvo|rust-function|declaration\"],\"symbol_docs\":[\"pkg::alvo|development.symbol-index\"]",
            ) + &line(
                "evidence.alvo",
                "tests/a.rs",
                "evidence",
                ",\"test_for\":[\"pkg::alvo\"]",
            )),
            "src/navigation.jsonl",
        )
        .unwrap();
        let docs = DocCatalog::parse(DOCS, "docs/navigation.jsonl").unwrap();
        let report = locate(&code, Some(&docs), "alvo").unwrap();
        let candidate = &report.candidates[0];
        assert_eq!(candidate.declaration.status, RelationStatus::Known);
        assert_eq!(candidate.implementation.status, RelationStatus::Unknown);
        assert_eq!(candidate.documentation.status, RelationStatus::Known);
        assert_eq!(candidate.tests.status, RelationStatus::Known);
        assert_eq!(
            candidate.documentation.items[0].path,
            "docs/development/symbol-index.md"
        );
    }

    #[test]
    fn homonimos_sao_preservados_em_ordem_de_identidade() {
        let code = CodeCatalog::parse(
            &(line(
                "b",
                "src/b.rs",
                "core",
                ",\"symbols\":[\"pkg_b::igual|igual|rust-type|declaration\"]",
            ) + &line(
                "a",
                "src/a.rs",
                "core",
                ",\"symbols\":[\"pkg_a::igual|igual|rust-function|declaration\"]",
            )),
            "catalog",
        )
        .unwrap();
        let report = locate(&code, None, "igual").unwrap();
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].identity, "pkg_a::igual");
        assert_eq!(report.candidates[1].identity, "pkg_b::igual");
        assert!(report
            .candidates
            .iter()
            .all(|candidate| candidate.documentation.status == RelationStatus::Unavailable));
    }

    #[test]
    fn rejeita_destinos_inexistentes_e_teste_fabricado_fora_de_evidencia() {
        let missing = CodeCatalog::parse(
            &line(
                "x",
                "src/x.rs",
                "core",
                ",\"related_symbols\":[\"pkg::ausente\"]",
            ),
            "catalog",
        )
        .unwrap();
        assert!(matches!(
            locate(&missing, None, "ausente"),
            Err(SymbolIndexError::MissingTarget { .. })
        ));

        let fabricated = CodeCatalog::parse(
            &(line(
                "declaracao",
                "src/a.rs",
                "core",
                ",\"symbols\":[\"pkg::alvo|alvo|rust-type|declaration\"]",
            ) + &line(
                "nao-teste",
                "src/b.rs",
                "core",
                ",\"test_for\":[\"pkg::alvo\"]",
            )),
            "catalog",
        )
        .unwrap();
        assert!(matches!(
            locate(&fabricated, None, "alvo"),
            Err(SymbolIndexError::InvalidTestRegion { .. })
        ));

        let linked = CodeCatalog::parse(
            &line(
                "documentado",
                "src/a.rs",
                "core",
                ",\"symbols\":[\"pkg::alvo|alvo|rust-type|declaration\"],\"symbol_docs\":[\"pkg::alvo|conceito.ambiguo\"]",
            ),
            "catalog",
        )
        .unwrap();
        let docs = DocCatalog::parse(
            "{\"schema\":2,\"record\":\"document\",\"id\":\"a\",\"domain\":\"development\",\"kind\":\"reference\",\"status\":\"active\",\"file\":\"a.md\",\"canonical_for\":[\"conceito.ambiguo\"]}\n{\"schema\":2,\"record\":\"document\",\"id\":\"b\",\"domain\":\"development\",\"kind\":\"reference\",\"status\":\"active\",\"file\":\"b.md\",\"canonical_for\":[\"conceito.ambiguo\"]}\n",
            "docs/navigation.jsonl",
        )
        .unwrap();
        assert!(matches!(
            locate(&linked, Some(&docs), "alvo"),
            Err(SymbolIndexError::AmbiguousDocumentTarget { .. })
        ));
    }

    #[test]
    fn renderizadores_consumem_o_mesmo_modelo_e_sao_deterministicos() {
        let code = CodeCatalog::parse(
            &line(
                "codigo",
                "src/a.rs",
                "core",
                ",\"symbols\":[\"pkg::alvo|alvo|UNKNOWN|implementation\"]",
            ),
            "catalog",
        )
        .unwrap();
        let report = locate(&code, None, "alvo").unwrap();
        assert_eq!(render_json(&report), render_json(&report));
        assert_eq!(render_human(&report), render_human(&report));
        assert!(render_json(&report).contains("\"kind\":\"UNKNOWN\""));
        assert!(render_json(&report).contains("\"status\":\"UNKNOWN\""));
        assert!(!render_json(&report).contains("/tmp/"));
    }
}
// @pinker-nav:end evidence.symbols.derivation-and-render
