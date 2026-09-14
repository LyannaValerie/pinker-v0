//! Cobertura corrente da cartografia (Trama Pinker — T1, #675).
//!
//! O catálogo `src/navigation.jsonl` prova consistência das regiões
//! publicadas; ele não prova que o código corrente está cartografado. Um
//! arquivo sem nenhum marcador é invisível para qualquer mecanismo que parta
//! das próprias regiões.
//!
//! Este módulo parte do universo físico de arquivos das raízes oficiais
//! ([`crate::nav::official_source_files`]), nunca dos marcadores, e responde
//! por arquivo: quantas regiões existem, quais intervalos estão cobertos,
//! quais intervalos relevantes não estão, qual a completude e qual a
//! disposição declarada. Escopo, exceções e disposições de chave vêm de uma
//! única autoridade versionada em
//! `.pinker/cartography/coverage-policy-v1.jsonl`; o catálogo derivado nunca é
//! autoridade de exceção.
//!
//! A obrigação tem dois níveis, e os dois são executados pelo mesmo gate:
//!
//! - **arquivo**: todo arquivo de raiz `required` precisa de ao menos uma
//!   região ou de uma exceção estreita aprovada;
//! - **intervalo**: nenhum arquivo de raiz `required` pode ter linha
//!   relevante fora de região. Não existe contagem tolerada, orçamento nem
//!   registro de dívida: a única rota de aceitação para código descoberto é
//!   cartografar a responsabilidade ou aprovar uma exceção estreita de arquivo
//!   inteiro. A origem histórica de uma lacuna é fato, não autorização. O
//!   próprio nível de obrigação de cada raiz é contrato do código: a
//!   autoridade declara as raízes para que o escopo seja auditável, e não pode
//!   rebaixar uma raiz de produção.
//!
//! ```text
//! CATALOG_CONSISTENT           != CURRENT_CODE_COVERAGE_COMPLETE
//! PARTIAL_REGION_INTERSECTION  != FULL_FILE_COVERAGE
//! ZERO_ANCHOR_FILE             != AUTOMATICALLY_EXCLUDED
//! DERIVED_CATALOG              != EXCEPTION_AUTHORITY
//! EXISTED_IN_BASELINE          != AUTHORIZED_TO_REMAIN
//! INVENTORIED_DEBT             != COVERAGE
//! DECLARED_SCOPE               != DECLARED_OBLIGATION_LEVEL
//! ```
//!
//! Zero dependências externas.

// @pinker-nav:start trama.cobertura.politica
// @pinker-nav:domain cartography-coverage
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav_coverage::CoveragePolicy|CoveragePolicy|rust-type|declaration
// @pinker-nav:symbol pinker_v0::nav_coverage::CoveragePolicy|CoveragePolicy|rust-type|implementation
// @pinker-nav:summary Autoridade unica e versionada de escopo, excecao e disposicao da cobertura de cartografia: le o JSONL revisavel, exige que os escopos declarem exatamente as raizes oficiais com a categoria e o nivel de obrigacao fixados pelo contrato do codigo, de modo que nenhuma politica rebaixe uma raiz de producao, recusa excecao ampla por construcao ao aceitar somente caminho de arquivo exato com razao e condicao de revisao, e nunca aceita o catalogo derivado como autoridade.
use crate::jsonl::{self, JsonObject};
use crate::nav::{
    self, official_scan_roots, official_source_files, relevant_source_lines, CodeIndex,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::Path;

/// Schema público do inventário de cobertura.
pub const COVERAGE_SCHEMA: u64 = 1;

/// Caminho repo-relativo da única autoridade versionada de escopo e exceções.
pub const POLICY_PATH: &str = ".pinker/cartography/coverage-policy-v1.jsonl";

/// Schema aceito nos registros da autoridade de política.
pub const POLICY_SCHEMA: i64 = 1;

/// Nível de exigência de cobertura aplicado a uma raiz oficial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Enforcement {
    /// A raiz é produção: todo arquivo precisa de cobertura ou de uma exceção
    /// estreita aprovada.
    Required,
    /// A raiz é inventariada e exposta, mas não é exigida nesta unidade. A
    /// existência e a disposição continuam explícitas — nunca invisíveis.
    Inventory,
}

impl Enforcement {
    pub fn as_str(self) -> &'static str {
        match self {
            Enforcement::Required => "required",
            Enforcement::Inventory => "inventory",
        }
    }

    fn parse(value: &str) -> Option<Enforcement> {
        match value {
            "required" => Some(Enforcement::Required),
            "inventory" => Some(Enforcement::Inventory),
            _ => None,
        }
    }
}

/// Contrato de raiz: categoria e nível de obrigação de cada raiz oficial,
/// fixados aqui, no código.
///
/// A autoridade versionada declara as raízes para que o escopo seja auditável
/// num arquivo revisável; ela NÃO decide o nível de obrigação. Rebaixar uma
/// raiz de produção para `inventory` na política aceitaria código descoberto
/// por registro de política — a mesma rota que a dívida era, só que por raiz
/// inteira em vez de por caminho exato. Uma raiz oficial que não aparecer aqui
/// falha fechado em vez de herdar um nível por omissão.
const ROOT_CONTRACT: &[(&str, &str, Enforcement)] = &[
    ("src", "production", Enforcement::Required),
    ("runtime/pinker_rt/src", "production", Enforcement::Required),
    ("tests", "evidence", Enforcement::Inventory),
    ("apps", "example", Enforcement::Inventory),
];

/// Escopo declarado para uma raiz oficial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeRule {
    pub root: String,
    pub category: String,
    pub file_enforcement: Enforcement,
}

/// Exceção estreita: um caminho de arquivo exato, com razão e condição de
/// revisão. Padrões, diretórios e curingas são recusados na carga — a
/// estreiteza é estrutural, não uma promessa de quem escreve a política.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionRule {
    pub path: String,
    pub reason: String,
    pub review: String,
}

/// Destino declarado de uma chave de região que saiu do catálogo corrente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DispositionKind {
    Moved,
    Split,
    Merged,
    Retired,
}

impl DispositionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DispositionKind::Moved => "moved",
            DispositionKind::Split => "split",
            DispositionKind::Merged => "merged",
            DispositionKind::Retired => "retired",
        }
    }

    fn parse(value: &str) -> Option<DispositionKind> {
        match value {
            "moved" => Some(DispositionKind::Moved),
            "split" => Some(DispositionKind::Split),
            "merged" => Some(DispositionKind::Merged),
            "retired" => Some(DispositionKind::Retired),
            _ => None,
        }
    }

    /// `moved`, `split` e `merged` precisam nomear onde a responsabilidade
    /// passou a viver; `retired` precisa declarar que não há destino.
    fn requires_targets(self) -> bool {
        !matches!(self, DispositionKind::Retired)
    }
}

/// Disposição versionada de uma chave que existia na base e não existe mais no
/// candidato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispositionRule {
    pub key: String,
    pub kind: DispositionKind,
    pub targets: Vec<String>,
    pub reason: String,
    pub review: String,
}

/// A autoridade carregada e validada.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoveragePolicy {
    pub scopes: Vec<ScopeRule>,
    pub exceptions: Vec<ExceptionRule>,
    pub dispositions: Vec<DispositionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    Io {
        path: String,
        msg: String,
    },
    Malformed {
        line: usize,
        msg: String,
    },
    UnknownKind {
        line: usize,
        kind: String,
    },
    MissingField {
        line: usize,
        field: String,
    },
    BadSchema {
        line: usize,
        found: String,
    },
    BroadException {
        line: usize,
        path: String,
        detail: String,
    },
    DuplicateScope {
        root: String,
    },
    DuplicateException {
        path: String,
    },
    DuplicateDisposition {
        key: String,
    },
    ScopeRootsMismatch {
        declared: Vec<String>,
        official: Vec<String>,
    },
    /// A política declarou categoria ou nível de obrigação diferente do
    /// contrato fixado no código para aquela raiz.
    ScopeContractMismatch {
        root: String,
        declared_category: String,
        declared_enforcement: &'static str,
        expected_category: &'static str,
        expected_enforcement: &'static str,
    },
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyError::Io { path, msg } => {
                write!(f, "E-COVERAGE-POLICY-IO: {path}: {msg}")
            }
            PolicyError::Malformed { line, msg } => {
                write!(f, "E-COVERAGE-POLICY-MALFORMED: linha {line}: {msg}")
            }
            PolicyError::UnknownKind { line, kind } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-KIND: linha {line}: registro desconhecido '{kind}'"
                )
            }
            PolicyError::MissingField { line, field } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-FIELD: linha {line}: campo obrigatório '{field}' ausente ou inválido"
                )
            }
            PolicyError::BadSchema { line, found } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-SCHEMA: linha {line}: schema '{found}' não é {POLICY_SCHEMA}"
                )
            }
            PolicyError::BroadException { line, path, detail } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-BROAD: linha {line}: exceção '{path}' não é estreita ({detail})"
                )
            }
            PolicyError::DuplicateScope { root } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-DUP-SCOPE: raiz '{root}' declarada duas vezes"
                )
            }
            PolicyError::DuplicateException { path } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-DUP-EXCEPTION: exceção '{path}' declarada duas vezes"
                )
            }
            PolicyError::DuplicateDisposition { key } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-DUP-DISPOSITION: disposição '{key}' declarada duas vezes"
                )
            }
            PolicyError::ScopeContractMismatch {
                root,
                declared_category,
                declared_enforcement,
                expected_category,
                expected_enforcement,
            } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-SCOPE-CONTRACT: a raiz '{root}' é '{expected_category}'/'{expected_enforcement}' por contrato do código e a política declara '{declared_category}'/'{declared_enforcement}'; o nível de obrigação de uma raiz não é declarável na autoridade"
                )
            }
            PolicyError::ScopeRootsMismatch { declared, official } => {
                write!(
                    f,
                    "E-COVERAGE-POLICY-ROOTS: escopos declarados [{}] não cobrem exatamente as raízes oficiais [{}]",
                    declared.join(", "),
                    official.join(", ")
                )
            }
        }
    }
}

impl CoveragePolicy {
    /// Carrega e valida a autoridade a partir da raiz do repositório.
    pub fn load(repo_root: &Path) -> Result<CoveragePolicy, PolicyError> {
        let path = repo_root.join(POLICY_PATH);
        let text = fs::read_to_string(path).map_err(|err| PolicyError::Io {
            path: POLICY_PATH.to_string(),
            msg: err.to_string(),
        })?;
        CoveragePolicy::parse(&text)
    }

    /// Interpreta o texto JSONL da autoridade. Toda recusa é explícita: não
    /// existe registro silenciosamente ignorado.
    pub fn parse(text: &str) -> Result<CoveragePolicy, PolicyError> {
        let mut policy = CoveragePolicy::default();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let object = jsonl::parse_object(trimmed)
                .map_err(|err| PolicyError::Malformed { line, msg: err.msg })?;
            let schema = object
                .get("schema")
                .and_then(|value| value.as_int())
                .ok_or(PolicyError::MissingField {
                    line,
                    field: "schema".to_string(),
                })?;
            if schema != POLICY_SCHEMA {
                return Err(PolicyError::BadSchema {
                    line,
                    found: schema.to_string(),
                });
            }
            let kind = required_str(&object, "kind", line)?;
            match kind.as_str() {
                "scope" => policy.scopes.push(parse_scope(&object, line)?),
                "exception" => policy.exceptions.push(parse_exception(&object, line)?),
                "disposition" => policy.dispositions.push(parse_disposition(&object, line)?),
                other => {
                    return Err(PolicyError::UnknownKind {
                        line,
                        kind: other.to_string(),
                    })
                }
            }
        }
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), PolicyError> {
        let mut roots = BTreeSet::new();
        for scope in &self.scopes {
            if !roots.insert(scope.root.clone()) {
                return Err(PolicyError::DuplicateScope {
                    root: scope.root.clone(),
                });
            }
        }
        let official: BTreeSet<String> = official_scan_roots()
            .into_iter()
            .map(|root| root.relative_path)
            .collect();
        if roots != official {
            return Err(PolicyError::ScopeRootsMismatch {
                declared: roots.into_iter().collect(),
                official: official.into_iter().collect(),
            });
        }
        for scope in &self.scopes {
            let contract = ROOT_CONTRACT
                .iter()
                .find(|(root, _, _)| *root == scope.root);
            let (expected_category, expected_enforcement) = match contract {
                Some((_, category, enforcement)) => (*category, *enforcement),
                // Raiz oficial sem contrato declarado no código: falha fechado.
                None => ("", Enforcement::Required),
            };
            if scope.category != expected_category || scope.file_enforcement != expected_enforcement
            {
                return Err(PolicyError::ScopeContractMismatch {
                    root: scope.root.clone(),
                    declared_category: scope.category.clone(),
                    declared_enforcement: scope.file_enforcement.as_str(),
                    expected_category,
                    expected_enforcement: expected_enforcement.as_str(),
                });
            }
        }
        let mut paths = BTreeSet::new();
        for exception in &self.exceptions {
            if !paths.insert(exception.path.clone()) {
                return Err(PolicyError::DuplicateException {
                    path: exception.path.clone(),
                });
            }
        }
        let mut keys = BTreeSet::new();
        for disposition in &self.dispositions {
            if !keys.insert(disposition.key.clone()) {
                return Err(PolicyError::DuplicateDisposition {
                    key: disposition.key.clone(),
                });
            }
        }
        Ok(())
    }

    /// Escopo declarado para uma raiz oficial.
    pub fn scope(&self, root: &str) -> Option<&ScopeRule> {
        self.scopes.iter().find(|scope| scope.root == root)
    }

    /// Exceção estreita declarada para um caminho exato.
    pub fn exception(&self, path: &str) -> Option<&ExceptionRule> {
        self.exceptions
            .iter()
            .find(|exception| exception.path == path)
    }

    /// Disposição declarada para uma chave de região.
    pub fn disposition(&self, key: &str) -> Option<&DispositionRule> {
        self.dispositions
            .iter()
            .find(|disposition| disposition.key == key)
    }
}

fn required_str(object: &JsonObject, field: &str, line: usize) -> Result<String, PolicyError> {
    object
        .get(field)
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or(PolicyError::MissingField {
            line,
            field: field.to_string(),
        })
}

fn parse_scope(object: &JsonObject, line: usize) -> Result<ScopeRule, PolicyError> {
    let root = required_str(object, "root", line)?;
    let category = required_str(object, "category", line)?;
    let raw = required_str(object, "file_enforcement", line)?;
    let file_enforcement = Enforcement::parse(&raw).ok_or(PolicyError::MissingField {
        line,
        field: "file_enforcement".to_string(),
    })?;
    Ok(ScopeRule {
        root,
        category,
        file_enforcement,
    })
}

fn parse_exception(object: &JsonObject, line: usize) -> Result<ExceptionRule, PolicyError> {
    let path = required_str(object, "path", line)?;
    if let Some(detail) = broad_exception_detail(&path) {
        return Err(PolicyError::BroadException {
            line,
            path,
            detail: detail.to_string(),
        });
    }
    Ok(ExceptionRule {
        path,
        reason: required_str(object, "reason", line)?,
        review: required_str(object, "review", line)?,
    })
}

/// Uma exceção só pode nomear um arquivo exato. Esta função é o que impede,
/// por construção, que uma exceção ampla esconda dívida não relacionada.
fn broad_exception_detail(path: &str) -> Option<&'static str> {
    if path.contains('*') || path.contains('?') || path.contains('[') {
        return Some("curinga não é aceito: declare um arquivo por vez");
    }
    if path.ends_with('/') {
        return Some("diretório não é aceito: declare um arquivo por vez");
    }
    if path.starts_with('/') {
        return Some("caminho absoluto não é aceito");
    }
    if path.contains('\\') {
        return Some("separador inválido");
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Some("componente de caminho degenerado");
    }
    None
}

fn parse_disposition(object: &JsonObject, line: usize) -> Result<DispositionRule, PolicyError> {
    let key = required_str(object, "key", line)?;
    let raw = required_str(object, "disposition", line)?;
    let kind = DispositionKind::parse(&raw).ok_or(PolicyError::MissingField {
        line,
        field: "disposition".to_string(),
    })?;
    let targets = object
        .get("to")
        .and_then(|value| value.as_str_array())
        .unwrap_or_default();
    if kind.requires_targets() && targets.is_empty() {
        return Err(PolicyError::MissingField {
            line,
            field: "to".to_string(),
        });
    }
    if !kind.requires_targets() && !targets.is_empty() {
        return Err(PolicyError::MissingField {
            line,
            field: "to".to_string(),
        });
    }
    Ok(DispositionRule {
        key,
        kind,
        targets,
        reason: required_str(object, "reason", line)?,
        review: required_str(object, "review", line)?,
    })
}
// @pinker-nav:end trama.cobertura.politica

// @pinker-nav:start trama.cobertura.inventario
// @pinker-nav:domain cartography-coverage
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav_coverage::inventory|inventory|rust-function|declaration
// @pinker-nav:symbol pinker_v0::nav_coverage::inventory|inventory|rust-function|implementation
// @pinker-nav:summary Inventario corrente por arquivo derivado do universo fisico das raizes oficiais e nunca dos marcadores: publica regioes encontradas, intervalos cobertos, intervalos relevantes descobertos, completude e disposicao, preservando arquivo zero-ancora visivel e recusando tratar intersecao parcial como completude.

/// Intervalo de linhas 1-indexado e inclusivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineInterval {
    pub start: usize,
    pub end: usize,
}

impl LineInterval {
    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start) + 1
    }

    pub fn is_empty(self) -> bool {
        self.end < self.start
    }
}

/// Completude de um arquivo perante a cartografia corrente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Completeness {
    /// Toda linha relevante do arquivo cai dentro de alguma região publicada.
    Complete,
    /// Existe ao menos uma região e ao menos um intervalo relevante fora dela.
    /// Interseção parcial nunca vira completude.
    Partial,
    /// Nenhuma região publicada intersecta o arquivo.
    Absent,
}

impl Completeness {
    pub fn as_str(self) -> &'static str {
        match self {
            Completeness::Complete => "COMPLETE",
            Completeness::Partial => "PARTIAL",
            Completeness::Absent => "NONE",
        }
    }
}

/// Disposição observada de um arquivo elegível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileDisposition {
    /// Possui ao menos uma região publicada.
    Covered,
    /// Não possui região, mas possui exceção estreita aprovada.
    Exception { reason: String, review: String },
    /// Raiz inventariada sem exigência de cobertura nesta unidade.
    InventoryOnly,
    /// Lacuna de produção: sem região e sem exceção.
    Gap,
}

impl FileDisposition {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileDisposition::Covered => "COVERED",
            FileDisposition::Exception { .. } => "EXCEPTION",
            FileDisposition::InventoryOnly => "INVENTORY_ONLY",
            FileDisposition::Gap => "GAP",
        }
    }
}

/// Fatos determinísticos de um arquivo elegível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCoverage {
    pub path: String,
    pub root: String,
    pub category: String,
    pub file_enforcement: Enforcement,
    pub regions: Vec<String>,
    pub covered_intervals: Vec<LineInterval>,
    pub uncovered_relevant_intervals: Vec<LineInterval>,
    pub relevant_lines: usize,
    pub covered_relevant_lines: usize,
    pub uncovered_relevant_lines: usize,
    pub completeness: Completeness,
    pub disposition: FileDisposition,
}

/// Inventário completo das raízes oficiais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageInventory {
    pub schema: u64,
    pub policy_path: String,
    pub files: Vec<FileCoverage>,
}

#[derive(Debug)]
pub enum InventoryError {
    Scan(nav::ScanError),
    Policy(PolicyError),
    Io { path: String, msg: String },
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InventoryError::Scan(error) => write!(f, "{error}"),
            InventoryError::Policy(error) => write!(f, "{error}"),
            InventoryError::Io { path, msg } => {
                write!(f, "E-COVERAGE-IO: {path}: {msg}")
            }
        }
    }
}

/// Constrói o inventário corrente. `index` traz as regiões já varridas; a
/// enumeração de arquivos é independente dele, então um arquivo sem marcador
/// nenhum continua presente no resultado.
pub fn inventory(
    repo_root: &Path,
    index: &CodeIndex,
    policy: &CoveragePolicy,
) -> Result<CoverageInventory, InventoryError> {
    let sources = official_source_files(repo_root).map_err(InventoryError::Scan)?;
    let mut by_file: BTreeMap<&str, Vec<&nav::CodeRegion>> = BTreeMap::new();
    for region in &index.regions {
        by_file
            .entry(region.file.as_str())
            .or_default()
            .push(region);
    }

    let mut files = Vec::with_capacity(sources.len());
    for source in sources {
        let absolute = repo_root.join(&source.path);
        let text = fs::read_to_string(&absolute).map_err(|err| InventoryError::Io {
            path: source.path.clone(),
            msg: err.to_string(),
        })?;
        let relevant = relevant_source_lines(&text, source.dialect);
        let regions = by_file
            .get(source.path.as_str())
            .cloned()
            .unwrap_or_default();
        let covered_intervals = merge_intervals(
            regions
                .iter()
                .map(|region| LineInterval {
                    start: region.start_marker,
                    end: region.end_marker,
                })
                .collect(),
        );
        let uncovered_relevant_intervals = uncovered_relevant(&relevant, &covered_intervals);
        let relevant_lines = relevant.iter().filter(|flag| **flag).count();
        let uncovered_relevant_lines: usize = uncovered_relevant_intervals
            .iter()
            .map(|interval| relevant_within(&relevant, *interval))
            .sum();
        let covered_relevant_lines = relevant_lines - uncovered_relevant_lines;
        let uncovered_relevant_line_count = uncovered_relevant_lines;
        let mut keys: Vec<String> = regions.iter().map(|region| region.key.clone()).collect();
        keys.sort();

        let completeness = if keys.is_empty() {
            Completeness::Absent
        } else if uncovered_relevant_intervals.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial
        };

        let scope = policy.scope(&source.root);
        let category = scope
            .map(|scope| scope.category.clone())
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let file_enforcement = scope
            .map(|scope| scope.file_enforcement)
            .unwrap_or(Enforcement::Required);

        let disposition = if !keys.is_empty() {
            FileDisposition::Covered
        } else if let Some(exception) = policy.exception(&source.path) {
            FileDisposition::Exception {
                reason: exception.reason.clone(),
                review: exception.review.clone(),
            }
        } else if file_enforcement == Enforcement::Inventory {
            FileDisposition::InventoryOnly
        } else {
            FileDisposition::Gap
        };

        files.push(FileCoverage {
            path: source.path,
            root: source.root,
            category,
            file_enforcement,
            regions: keys,
            covered_intervals,
            uncovered_relevant_intervals,
            relevant_lines,
            covered_relevant_lines,
            uncovered_relevant_lines: uncovered_relevant_line_count,
            completeness,
            disposition,
        });
    }

    Ok(CoverageInventory {
        schema: COVERAGE_SCHEMA,
        policy_path: POLICY_PATH.to_string(),
        files,
    })
}

/// Une intervalos sobrepostos ou adjacentes numa cobertura canônica ordenada.
fn merge_intervals(mut intervals: Vec<LineInterval>) -> Vec<LineInterval> {
    intervals.retain(|interval| !interval.is_empty() && interval.start > 0);
    intervals.sort();
    let mut merged: Vec<LineInterval> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        match merged.last_mut() {
            Some(last) if interval.start <= last.end.saturating_add(1) => {
                last.end = last.end.max(interval.end);
            }
            _ => merged.push(interval),
        }
    }
    merged
}

/// Corridas maximais de linhas relevantes que nenhum intervalo coberto alcança.
fn uncovered_relevant(relevant: &[bool], covered: &[LineInterval]) -> Vec<LineInterval> {
    let mut out: Vec<LineInterval> = Vec::new();
    let mut open: Option<LineInterval> = None;
    for (index, is_relevant) in relevant.iter().enumerate() {
        let line = index + 1;
        let inside = covered
            .iter()
            .any(|interval| interval.start <= line && line <= interval.end);
        if *is_relevant && !inside {
            match open.as_mut() {
                Some(current) if current.end + 1 == line => current.end = line,
                Some(_) => {
                    out.push(open.take().expect("intervalo aberto"));
                    open = Some(LineInterval {
                        start: line,
                        end: line,
                    });
                }
                None => {
                    open = Some(LineInterval {
                        start: line,
                        end: line,
                    })
                }
            }
        }
    }
    if let Some(current) = open {
        out.push(current);
    }
    out
}

fn relevant_within(relevant: &[bool], interval: LineInterval) -> usize {
    (interval.start..=interval.end)
        .filter(|line| relevant.get(line - 1).copied().unwrap_or(false))
        .count()
}
// @pinker-nav:end trama.cobertura.inventario

// @pinker-nav:start trama.cobertura.verificacao
// @pinker-nav:domain cartography-coverage
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav_coverage::verify|verify|rust-function|declaration
// @pinker-nav:symbol pinker_v0::nav_coverage::verify|verify|rust-function|implementation
// @pinker-nav:summary Propriedade executavel da cobertura corrente sobre o inventario e a autoridade: arquivo de producao sem regiao e sem excecao falha, qualquer linha relevante fora de regiao falha sem rota de declaracao que a aceite, excecao obsoleta ou desnecessaria falha em vez de virar folga silenciosa, e chave com disposicao declarada nao pode ressuscitar no catalogo corrente.

/// Violação da propriedade de cobertura corrente.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoverageViolation {
    /// Arquivo de produção sem nenhuma região e sem exceção aprovada.
    UncoveredProductionFile { path: String },
    /// Exceção que aponta para um caminho que não existe mais nas raízes.
    StaleException { path: String },
    /// Exceção mantida sobre arquivo que já possui cobertura: folga silenciosa.
    UnnecessaryException { path: String },
    /// Chave declarada como movida/dividida/fundida/aposentada que voltou a
    /// existir no catálogo corrente sem retirar a disposição.
    ResurrectedDisposedKey {
        key: String,
        disposition: &'static str,
    },
    /// Disposição cujo destino declarado não existe no catálogo corrente.
    MissingDispositionTarget { key: String, target: String },
    /// Arquivo sob enforcement com linha relevante fora de qualquer região.
    ///
    /// A origem histórica da lacuna não entra aqui: não existe rota pela qual
    /// declarar a lacuna na autoridade a transforme em aprovação. Ou a
    /// responsabilidade é cartografada, ou o arquivo inteiro recebe uma
    /// exceção estreita aprovada. Nem rebaixar a raiz serve: o nível de
    /// obrigação de cada raiz oficial é contrato do código (`ROOT_CONTRACT`),
    /// e a política que o contradiz é recusada na carga.
    UncoveredRelevantInterval {
        path: String,
        uncovered_lines: usize,
        intervals: usize,
        first: LineInterval,
    },
}

impl fmt::Display for CoverageViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoverageViolation::UncoveredProductionFile { path } => write!(
                f,
                "E-COVERAGE-UNCOVERED: {path} não possui nenhuma região publicada nem exceção aprovada"
            ),
            CoverageViolation::StaleException { path } => write!(
                f,
                "E-COVERAGE-EXCEPTION-STALE: exceção declara {path}, que não existe nas raízes oficiais"
            ),
            CoverageViolation::UnnecessaryException { path } => write!(
                f,
                "E-COVERAGE-EXCEPTION-UNNECESSARY: {path} já possui cobertura; a exceção deve ser retirada"
            ),
            CoverageViolation::ResurrectedDisposedKey { key, disposition } => write!(
                f,
                "E-COVERAGE-DISPOSITION-RESURRECTED: chave {key} está declarada como '{disposition}' e voltou ao catálogo corrente"
            ),
            CoverageViolation::MissingDispositionTarget { key, target } => write!(
                f,
                "E-COVERAGE-DISPOSITION-TARGET: destino {target} da disposição {key} não existe no catálogo corrente"
            ),
            CoverageViolation::UncoveredRelevantInterval {
                path,
                uncovered_lines,
                intervals,
                first,
            } => write!(
                f,
                "E-COVERAGE-UNCOVERED-INTERVAL: {path} tem {uncovered_lines} linha(s) relevante(s) em {intervals} intervalo(s) fora de qualquer região (a partir da linha {}); cartografe a responsabilidade",
                first.start
            ),
        }
    }
}

/// Avalia a propriedade sobre o inventário corrente. Somente leitura.
pub fn verify(
    inventory: &CoverageInventory,
    index: &CodeIndex,
    policy: &CoveragePolicy,
) -> Vec<CoverageViolation> {
    let mut violations = Vec::new();

    for file in &inventory.files {
        if file.disposition == FileDisposition::Gap {
            violations.push(CoverageViolation::UncoveredProductionFile {
                path: file.path.clone(),
            });
        }
        if file.file_enforcement != Enforcement::Required {
            continue;
        }
        // Exceção estreita aprovada é a única rota que retira um arquivo da
        // obrigação, e ela vale para o arquivo inteiro. Não há rota por
        // intervalo: declarar a lacuna não a autoriza.
        if matches!(file.disposition, FileDisposition::Exception { .. }) {
            continue;
        }
        if let Some(first) = file.uncovered_relevant_intervals.first() {
            violations.push(CoverageViolation::UncoveredRelevantInterval {
                path: file.path.clone(),
                uncovered_lines: file.uncovered_relevant_lines,
                intervals: file.uncovered_relevant_intervals.len(),
                first: *first,
            });
        }
    }

    let inventoried: BTreeMap<&str, &FileCoverage> = inventory
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();
    for exception in &policy.exceptions {
        match inventoried.get(exception.path.as_str()) {
            None => violations.push(CoverageViolation::StaleException {
                path: exception.path.clone(),
            }),
            Some(file) if !file.regions.is_empty() => {
                violations.push(CoverageViolation::UnnecessaryException {
                    path: exception.path.clone(),
                })
            }
            Some(_) => {}
        }
    }

    let current: BTreeSet<&str> = index
        .regions
        .iter()
        .map(|region| region.key.as_str())
        .collect();
    for disposition in &policy.dispositions {
        if current.contains(disposition.key.as_str()) {
            violations.push(CoverageViolation::ResurrectedDisposedKey {
                key: disposition.key.clone(),
                disposition: disposition.kind.as_str(),
            });
        }
        for target in &disposition.targets {
            if !current.contains(target.as_str()) {
                violations.push(CoverageViolation::MissingDispositionTarget {
                    key: disposition.key.clone(),
                    target: target.clone(),
                });
            }
        }
    }

    violations.sort();
    violations
}
// @pinker-nav:end trama.cobertura.verificacao

// @pinker-nav:start trama.cobertura.renderizacao
// @pinker-nav:domain cartography-coverage
// @pinker-nav:layer trama
// @pinker-nav:summary Renderizadores humano e JSON deterministicos do inventario de cobertura, com ordem fixa, intervalos explicitos e caminhos repo-relativos, para que cobertura, descoberta e completude sejam auditaveis sem reexecutar a varredura.

/// Resumo agregado por raiz oficial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSummary {
    pub root: String,
    pub category: String,
    pub file_enforcement: Enforcement,
    pub files: usize,
    pub complete: usize,
    pub partial: usize,
    pub absent: usize,
    pub exceptions: usize,
    pub gaps: usize,
}

pub fn summarize(inventory: &CoverageInventory) -> Vec<RootSummary> {
    let mut by_root: BTreeMap<&str, RootSummary> = BTreeMap::new();
    for file in &inventory.files {
        let entry = by_root
            .entry(file.root.as_str())
            .or_insert_with(|| RootSummary {
                root: file.root.clone(),
                category: file.category.clone(),
                file_enforcement: file.file_enforcement,
                files: 0,
                complete: 0,
                partial: 0,
                absent: 0,
                exceptions: 0,
                gaps: 0,
            });
        entry.files += 1;
        match file.completeness {
            Completeness::Complete => entry.complete += 1,
            Completeness::Partial => entry.partial += 1,
            Completeness::Absent => entry.absent += 1,
        }
        match file.disposition {
            FileDisposition::Exception { .. } => entry.exceptions += 1,
            FileDisposition::Gap => entry.gaps += 1,
            _ => {}
        }
    }
    by_root.into_values().collect()
}

pub fn render_text(inventory: &CoverageInventory) -> String {
    let mut out = String::new();
    out.push_str("cobertura corrente da cartografia\n");
    out.push_str(&format!("autoridade: {}\n", inventory.policy_path));
    for summary in summarize(inventory) {
        out.push_str(&format!(
            "- {} [{}/{}] arquivos={} completos={} parciais={} sem-regiao={} excecoes={} lacunas={}\n",
            summary.root,
            summary.category,
            summary.file_enforcement.as_str(),
            summary.files,
            summary.complete,
            summary.partial,
            summary.absent,
            summary.exceptions,
            summary.gaps,
        ));
    }
    for file in &inventory.files {
        out.push_str(&format!(
            "{} {} regioes={} relevantes={} cobertas={} disposicao={}\n",
            file.path,
            file.completeness.as_str(),
            file.regions.len(),
            file.relevant_lines,
            file.covered_relevant_lines,
            file.disposition.as_str(),
        ));
        if !file.uncovered_relevant_intervals.is_empty() {
            let intervals = file
                .uncovered_relevant_intervals
                .iter()
                .map(|interval| format!("{}-{}", interval.start, interval.end))
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!("  descobertos: {intervals}\n"));
        }
    }
    out
}

pub fn render_json(inventory: &CoverageInventory) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{{\"schema\":{},\"policy\":{},\"files\":[",
        inventory.schema,
        nav::json_string(&inventory.policy_path)
    ));
    for (index, file) in inventory.files.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"path\":{},\"root\":{},\"category\":{},\"file_enforcement\":{},\"regions\":{},\"covered_intervals\":{},\"uncovered_relevant_intervals\":{},\"relevant_lines\":{},\"covered_relevant_lines\":{},\"uncovered_relevant_lines\":{},\"completeness\":{},\"disposition\":{}}}",
            nav::json_string(&file.path),
            nav::json_string(&file.root),
            nav::json_string(&file.category),
            nav::json_string(file.file_enforcement.as_str()),
            nav::json_string_array(&file.regions),
            render_intervals(&file.covered_intervals),
            render_intervals(&file.uncovered_relevant_intervals),
            file.relevant_lines,
            file.covered_relevant_lines,
            file.uncovered_relevant_lines,
            nav::json_string(file.completeness.as_str()),
            nav::json_string(file.disposition.as_str()),
        ));
    }
    out.push_str("]}");
    out
}

pub(crate) fn render_intervals(intervals: &[LineInterval]) -> String {
    let mut out = String::from("[");
    for (index, interval) in intervals.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"start\":{},\"end\":{}}}",
            interval.start, interval.end
        ));
    }
    out.push(']');
    out
}
// @pinker-nav:end trama.cobertura.renderizacao
// @pinker-nav:start evidencia.cobertura.autoridade-e-inventario
// @pinker-nav:domain cartography-coverage
// @pinker-nav:layer evidencia
// @pinker-nav:summary Provas da autoridade e do inventario de cobertura: os escopos precisam declarar exatamente as raizes oficiais, excecao ampla e recusada na carga, excecao estreita exige razao e condicao de revisao, disposicao exige destino coerente com o tipo, registro de kind desconhecido nao e ignorado em silencio, intervalo descoberto e corrida maximal de linhas relevantes e intervalos cobertos sobrepostos sao unidos.

#[cfg(test)]
mod tests {
    use super::*;

    const SCOPES: &str = concat!(
        r#"{"schema":1,"kind":"scope","root":"src","category":"production","file_enforcement":"required"}"#,
        "\n",
        r#"{"schema":1,"kind":"scope","root":"runtime/pinker_rt/src","category":"production","file_enforcement":"required"}"#,
        "\n",
        r#"{"schema":1,"kind":"scope","root":"tests","category":"evidence","file_enforcement":"inventory"}"#,
        "\n",
        r#"{"schema":1,"kind":"scope","root":"apps","category":"example","file_enforcement":"inventory"}"#,
        "\n",
    );

    #[test]
    fn politica_exige_exatamente_as_raizes_oficiais() {
        let faltando = SCOPES.lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(matches!(
            CoveragePolicy::parse(&faltando),
            Err(PolicyError::ScopeRootsMismatch { .. })
        ));
        assert!(CoveragePolicy::parse(SCOPES).is_ok());
    }

    #[test]
    fn excecao_ampla_e_recusada_na_carga() {
        for amplo in [
            "src/bin/*.rs",
            "src/bin/",
            "/src/bin/a.rs",
            "src/../src/a.rs",
        ] {
            let text = format!(
                "{SCOPES}{{\"schema\":1,\"kind\":\"exception\",\"path\":\"{amplo}\",\"reason\":\"r\",\"review\":\"v\"}}\n"
            );
            assert!(
                matches!(
                    CoveragePolicy::parse(&text),
                    Err(PolicyError::BroadException { .. })
                ),
                "aceitou exceção ampla: {amplo}"
            );
        }
    }

    #[test]
    fn excecao_estreita_exige_razao_e_condicao_de_revisao() {
        let sem_revisao = format!(
            "{SCOPES}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/a.rs","reason":"r"}"#
        );
        assert!(matches!(
            CoveragePolicy::parse(&sem_revisao),
            Err(PolicyError::MissingField { .. })
        ));
        let completa = format!(
            "{SCOPES}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/a.rs","reason":"r","review":"v"}"#
        );
        let policy = CoveragePolicy::parse(&completa).expect("política válida");
        assert_eq!(policy.exceptions.len(), 1);
    }

    #[test]
    fn disposicao_exige_destino_coerente_com_o_tipo() {
        let movida_sem_destino = format!(
            "{SCOPES}{}\n",
            r#"{"schema":1,"kind":"disposition","key":"a.b","disposition":"moved","reason":"r","review":"v"}"#
        );
        assert!(matches!(
            CoveragePolicy::parse(&movida_sem_destino),
            Err(PolicyError::MissingField { .. })
        ));
        let aposentada_com_destino = format!(
            "{SCOPES}{}\n",
            r#"{"schema":1,"kind":"disposition","key":"a.b","disposition":"retired","to":["c.d"],"reason":"r","review":"v"}"#
        );
        assert!(matches!(
            CoveragePolicy::parse(&aposentada_com_destino),
            Err(PolicyError::MissingField { .. })
        ));
    }

    #[test]
    fn registro_desconhecido_nao_e_ignorado_em_silencio() {
        let text = format!("{SCOPES}{}\n", r#"{"schema":1,"kind":"ignorar-tudo"}"#);
        assert!(matches!(
            CoveragePolicy::parse(&text),
            Err(PolicyError::UnknownKind { .. })
        ));
    }

    #[test]
    fn intervalos_descobertos_sao_corridas_maximais_de_linhas_relevantes() {
        let relevant = [true, true, false, true, true, true, false, true];
        let covered = [LineInterval { start: 4, end: 5 }];
        assert_eq!(
            uncovered_relevant(&relevant, &covered),
            vec![
                LineInterval { start: 1, end: 2 },
                LineInterval { start: 6, end: 6 },
                LineInterval { start: 8, end: 8 },
            ]
        );
    }

    #[test]
    fn intervalos_cobertos_sobrepostos_sao_unidos() {
        let merged = merge_intervals(vec![
            LineInterval { start: 10, end: 20 },
            LineInterval { start: 1, end: 5 },
            LineInterval { start: 6, end: 9 },
        ]);
        assert_eq!(merged, vec![LineInterval { start: 1, end: 20 }]);
    }
}
// @pinker-nav:end evidencia.cobertura.autoridade-e-inventario
