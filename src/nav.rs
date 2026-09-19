//! Trama Pinker — Etapa 3 (Navegação semântica do código).
//!
//! Varre um conjunto de raízes de código controladas do repositório (Onda 8A:
//! `src/`, `runtime/pinker_rt/src/` e `tests/`, todas obrigatórias no fluxo oficial) em
//! busca dos marcadores `@pinker-nav:start/end` e gera o catálogo derivado
//! `src/navigation.jsonl` (especificação, seções 10, 11, 12-17 e 22). O
//! agente que altera o código mantém os marcadores; o script nunca decide
//! semanticamente onde inseri-los. Zero dependências externas.

// @pinker-nav:start trama.code.symbol-index-model
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav::CodeIndex|CodeIndex|rust-type|declaration
// @pinker-nav:symbol-doc pinker_v0::nav::CodeIndex|development.symbol-index
// @pinker-nav:summary Declares the in-memory model of code navigation: the catalogued region and its explicit symbol metadata, the link dialect (category and role), the documentation link and the CodeIndex that aggregates them as the scan authority. It is also the module's prelude — the uses the whole file shares. The implementation remains in the trama.code.catalog region.
use crate::jsonl;
use crate::nav_coverage;
use crate::text_norm;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

const START: &str = "@pinker-nav:start";
const END: &str = "@pinker-nav:end";
const FIELD_PREFIX: &str = "@pinker-nav:";

/// Uma região de código catalogada (par de marcadores).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRegion {
    pub key: String,
    pub kind: String,
    pub domain: Option<String>,
    pub layer: Option<String>,
    pub phase: Option<u64>,
    pub file: String,
    pub start_marker: usize,
    pub content_start: usize,
    pub content_end: usize,
    pub end_marker: usize,
    pub summary: String,
    pub hash: String,
    pub status: String,
    pub symbols: Vec<RegionSymbol>,
    pub related_symbols: Vec<String>,
    pub test_for: Vec<String>,
    pub symbol_docs: Vec<SymbolDocLink>,
}

/// Categoria estrutural publicada explicitamente por uma região da Trama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind {
    RustFunction,
    RustType,
    PinkerFunction,
    Unknown,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolKind::RustFunction => "rust-function",
            SymbolKind::RustType => "rust-type",
            SymbolKind::PinkerFunction => "pinker-function",
            SymbolKind::Unknown => "UNKNOWN",
        }
    }
}

/// Papel da região em relação a um símbolo explicitamente identificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolRole {
    Declaration,
    Implementation,
}

impl SymbolRole {
    pub fn as_str(self) -> &'static str {
        match self {
            SymbolRole::Declaration => "declaration",
            SymbolRole::Implementation => "implementation",
        }
    }
}

/// Binding canônico `identidade | nome | categoria | papel` mantido no próprio
/// marcador da região; o catálogo apenas deriva e serializa este valor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionSymbol {
    pub identity: String,
    pub name: String,
    pub kind: SymbolKind,
    pub role: SymbolRole,
}

/// Vínculo documental explícito `identidade | id documental`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolDocLink {
    pub identity: String,
    pub document: String,
}

/// Índice de código em memória.
#[derive(Debug, Clone, Default)]
pub struct CodeIndex {
    pub regions: Vec<CodeRegion>,
    pub scan_problems: Vec<NavVerifyError>,
}
// @pinker-nav:end trama.code.symbol-index-model
// @pinker-nav:start trama.code.diagnostics
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Code navigation errors and their stable messages: ScanError covers the root and read failure before any scan, and NavVerifyError covers the failure of an already scanned cartography (orphan marker, invalid key, overlap, malformed metadata, conflicting symbol identity, missing destination and catalog drift). The text of each variant is an observable CLI and test surface.

#[derive(Debug)]
pub enum ScanError {
    Io {
        path: String,
        msg: String,
    },
    /// Raiz de código obrigatória ausente (§15). Falha antes de qualquer
    /// escrita do catálogo; não gera índice parcial.
    RootMissing {
        path: String,
    },
    /// Caminho da raiz existe mas não é um diretório (§15).
    RootNotDirectory {
        path: String,
    },
    /// Raiz recusada por ser um link simbólico (política explícita de
    /// segurança contra fuga/ciclo via symlink; §14/§20).
    RootSymlinkRefused {
        path: String,
    },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io { path, msg } => {
                write!(f, "E-NAV-SCAN\nFalha ao ler '{}': {}", path, msg)
            }
            ScanError::RootMissing { path } => {
                write!(
                    f,
                    "E-NAV-SCAN\nRaiz de código obrigatória ausente: '{}'.",
                    path
                )
            }
            ScanError::RootNotDirectory { path } => {
                write!(
                    f,
                    "E-NAV-SCAN\nRaiz de código não é um diretório: '{}'.",
                    path
                )
            }
            ScanError::RootSymlinkRefused { path } => {
                write!(
                    f,
                    "E-NAV-SCAN\nRaiz de código recusada por ser um link simbólico: '{}'.",
                    path
                )
            }
        }
    }
}

/// Divergências de validação do código (§22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavVerifyError {
    DuplicateKey {
        key: String,
        files: Vec<String>,
    },
    StartWithoutEnd {
        key: String,
        file: String,
        line: usize,
    },
    EndWithoutStart {
        key: String,
        file: String,
        line: usize,
    },
    KeyMismatch {
        start: String,
        end: String,
        file: String,
    },
    EmptyRange {
        key: String,
        file: String,
    },
    InvalidKey {
        key: String,
        file: String,
        line: usize,
    },
    Overlap {
        outer: String,
        inner: String,
        file: String,
    },
    MalformedMeta {
        key: String,
        file: String,
        field: String,
    },
    ConflictingSymbolIdentity {
        identity: String,
        regions: Vec<String>,
    },
    MissingSymbolTarget {
        region: String,
        field: String,
        identity: String,
    },
    InvalidTestLink {
        region: String,
        identity: String,
        layer: Option<String>,
    },
    IndexOutOfDate {
        path: String,
    },
}

impl fmt::Display for NavVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NavVerifyError::DuplicateKey { key, files } => {
                write!(f, "chave duplicada '{}' em: {}", key, files.join(", "))
            }
            NavVerifyError::StartWithoutEnd { key, file, line } => {
                write!(f, "marcador '{}' aberto sem fim em {}:{}", key, file, line)
            }
            NavVerifyError::EndWithoutStart { key, file, line } => {
                write!(
                    f,
                    "marcador '{}' fechado sem início em {}:{}",
                    key, file, line
                )
            }
            NavVerifyError::KeyMismatch { start, end, file } => write!(
                f,
                "par de marcador divergente em {}: início '{}' vs fim '{}'",
                file, start, end
            ),
            NavVerifyError::EmptyRange { key, file } => {
                write!(f, "região '{}' vazia em {}", key, file)
            }
            NavVerifyError::InvalidKey { key, file, line } => write!(
                f,
                "chave inválida '{}' em {}:{} (formato [a-z0-9]+([._-][a-z0-9]+)*)",
                key, file, line
            ),
            NavVerifyError::Overlap { outer, inner, file } => write!(
                f,
                "sobreposição de regiões em {}: '{}' dentro de '{}'",
                file, inner, outer
            ),
            NavVerifyError::MalformedMeta { key, file, field } => write!(
                f,
                "metadado malformado na região '{}' em {}: campo '{}'",
                key, file, field
            ),
            NavVerifyError::ConflictingSymbolIdentity { identity, regions } => write!(
                f,
                "identidade de símbolo '{}' possui nome ou categoria conflitante em: {}",
                identity,
                regions.join(", ")
            ),
            NavVerifyError::MissingSymbolTarget {
                region,
                field,
                identity,
            } => write!(
                f,
                "região '{}' referencia símbolo inexistente '{}' em '{}'",
                region, identity, field
            ),
            NavVerifyError::InvalidTestLink {
                region,
                identity,
                layer,
            } => write!(
                f,
                "vínculo de teste da região '{}' para '{}' exige layer evidence; encontrado {}",
                region,
                identity,
                layer.as_deref().unwrap_or("sem layer")
            ),
            NavVerifyError::IndexOutOfDate { path } => write!(
                f,
                "catálogo '{}' dessincronizado; rode `pink nav sincronizar`",
                path
            ),
        }
    }
}

// @pinker-nav:end trama.code.diagnostics
// @pinker-nav:start trama.code.catalog
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav::CodeIndex|CodeIndex|rust-type|implementation
// @pinker-nav:summary Generates the navigation catalog from the controlled roots and the @pinker-nav markers: it assembles regions and explicit links, computes hashes, renders deterministic JSONL and validates keys, markers, overlap, identity consistency, destinations and the tests' layer before writing.
impl CodeIndex {
    /// Varre uma única raiz (uso em fixtures/testes; compatibilidade
    /// histórica). Delega à varredura multi-raiz (§ trama.code.roots)
    /// tratando `src_root` como a raiz recebida diretamente: o caminho
    /// resultante é relativo a `src_root`, sem prefixo fabricado.
    pub fn scan(src_root: &Path) -> Result<CodeIndex, ScanError> {
        let root = ScanRoot::new("", &["rs"]);
        scan_roots(src_root, std::slice::from_ref(&root))
    }

    /// Varre o repositório real usando as raízes de código oficiais e
    /// obrigatórias (§15): `src/`, `runtime/pinker_rt/src/` e `tests/`. Usado pelo
    /// fluxo oficial `pink nav sincronizar`/`verificar`.
    pub fn scan_repo(repo_root: &Path) -> Result<CodeIndex, ScanError> {
        scan_roots(repo_root, &official_scan_roots())
    }

    /// Serializa as regiões em JSONL determinístico (ordenado por `key`).
    pub fn render_jsonl(&self) -> String {
        let mut sorted = self.regions.clone();
        sorted.sort_by(|a, b| a.key.cmp(&b.key).then(a.file.cmp(&b.file)));
        let mut out = String::new();
        for region in &sorted {
            out.push_str(&render_region_json(region));
            out.push('\n');
        }
        out
    }

    /// Validações do código (§22, subconjunto verificável sem histórico).
    pub fn verify(&self) -> Vec<NavVerifyError> {
        let mut errors = self.scan_problems.clone();

        let mut by_key: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for region in &self.regions {
            by_key
                .entry(&region.key)
                .or_default()
                .push(region.file.clone());
        }
        for (key, files) in by_key {
            if files.len() > 1 {
                errors.push(NavVerifyError::DuplicateKey {
                    key: key.to_string(),
                    files,
                });
            }
        }
        let mut identities: BTreeMap<&str, (&str, SymbolKind, Vec<String>)> = BTreeMap::new();
        for region in &self.regions {
            for symbol in &region.symbols {
                match identities.get_mut(symbol.identity.as_str()) {
                    Some((name, kind, regions)) => {
                        regions.push(region.key.clone());
                        if *name != symbol.name || *kind != symbol.kind {
                            errors.push(NavVerifyError::ConflictingSymbolIdentity {
                                identity: symbol.identity.clone(),
                                regions: regions.clone(),
                            });
                        }
                    }
                    None => {
                        identities.insert(
                            &symbol.identity,
                            (&symbol.name, symbol.kind, vec![region.key.clone()]),
                        );
                    }
                }
            }
        }
        for region in &self.regions {
            for (field, identity) in region
                .related_symbols
                .iter()
                .map(|identity| ("related-symbol", identity))
                .chain(
                    region
                        .test_for
                        .iter()
                        .map(|identity| ("test-for", identity)),
                )
                .chain(
                    region
                        .symbol_docs
                        .iter()
                        .map(|link| ("symbol-doc", &link.identity)),
                )
            {
                if !identities.contains_key(identity.as_str()) {
                    errors.push(NavVerifyError::MissingSymbolTarget {
                        region: region.key.clone(),
                        field: field.to_string(),
                        identity: identity.clone(),
                    });
                }
            }
            if region.layer.as_deref() != Some("evidence") {
                for identity in &region.test_for {
                    errors.push(NavVerifyError::InvalidTestLink {
                        region: region.key.clone(),
                        identity: identity.clone(),
                        layer: region.layer.clone(),
                    });
                }
            }
        }
        errors
    }

    pub fn region(&self, key: &str) -> Option<&CodeRegion> {
        self.regions.iter().find(|r| r.key == key)
    }

    /// Busca por chave, domínio, camada, resumo e caminho (prioridade §7.3).
    pub fn search(&self, query: &str) -> Vec<&CodeRegion> {
        score_regions(&self.regions, query)
            .into_iter()
            .map(|hit| hit.region)
            .collect()
    }

    /// Lista regiões de uma camada (layer) ou domínio (domain).
    pub fn list(&self, selector: &str) -> Vec<&CodeRegion> {
        self.regions
            .iter()
            .filter(|r| {
                r.layer.as_deref() == Some(selector) || r.domain.as_deref() == Some(selector)
            })
            .collect()
    }
}
// @pinker-nav:end trama.code.catalog

// @pinker-nav:start trama.code.reusable-verification
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Read-only model shared by pink nav verificar and internal consumers: it rescans the official roots, validates the regions, compares the rendered catalog with the versioned file and evaluates the current coverage of the cartography through the versioned authority, at both levels (a file without a region and a relevant interval outside a region), failing closed when that authority cannot be established.

/// Resultado da avaliação de cobertura corrente da cartografia. A autoridade
/// de escopo/exceções é externa e versionada; quando ela não pode ser
/// estabelecida, o estado é `PolicyUnavailable` e conta como erro — ausência
/// de autoridade nunca vira PASS silencioso.
#[derive(Debug)]
pub enum CoverageOutcome {
    Checked {
        inventory: nav_coverage::CoverageInventory,
        violations: Vec<nav_coverage::CoverageViolation>,
    },
    PolicyUnavailable {
        reason: String,
    },
}

impl CoverageOutcome {
    pub fn error_count(&self) -> usize {
        match self {
            CoverageOutcome::Checked { violations, .. } => violations.len(),
            CoverageOutcome::PolicyUnavailable { .. } => 1,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error_count() == 0
    }
}

/// Estado observacional produzido pela mesma autoridade de `pink nav
/// verificar`, sem impressão e sem escrita.
#[derive(Debug)]
pub struct RepositoryVerification {
    pub index: CodeIndex,
    pub source_errors: Vec<NavVerifyError>,
    pub catalog_out_of_date: bool,
    pub coverage: CoverageOutcome,
}

impl RepositoryVerification {
    pub fn is_ok(&self) -> bool {
        self.source_errors.is_empty() && !self.catalog_out_of_date && self.coverage.is_ok()
    }

    pub fn total_errors(&self) -> usize {
        self.source_errors.len()
            + usize::from(self.catalog_out_of_date)
            + self.coverage.error_count()
    }
}

/// Executa integralmente a verificação do catálogo de código em memória,
/// incluindo a propriedade de cobertura corrente das raízes oficiais.
pub fn verify_repository(
    repo_root: &Path,
    catalog_relative_path: &str,
) -> Result<RepositoryVerification, ScanError> {
    let index = CodeIndex::scan_repo(repo_root)?;
    let source_errors = index.verify();
    let rendered = index.render_jsonl();
    let catalog_out_of_date =
        fs::read_to_string(repo_root.join(catalog_relative_path)).unwrap_or_default() != rendered;
    let coverage = verify_coverage(repo_root, &index);
    Ok(RepositoryVerification {
        index,
        source_errors,
        catalog_out_of_date,
        coverage,
    })
}

/// Carrega a autoridade versionada de cobertura, monta o inventário corrente e
/// avalia a propriedade. Qualquer falha em estabelecer a autoridade ou o
/// inventário é reportada como indisponibilidade, jamais como sucesso.
fn verify_coverage(repo_root: &Path, index: &CodeIndex) -> CoverageOutcome {
    let policy = match nav_coverage::CoveragePolicy::load(repo_root) {
        Ok(policy) => policy,
        Err(error) => {
            return CoverageOutcome::PolicyUnavailable {
                reason: error.to_string(),
            }
        }
    };
    match nav_coverage::inventory(repo_root, index, &policy) {
        Ok(inventory) => {
            let violations = nav_coverage::verify(&inventory, index, &policy);
            CoverageOutcome::Checked {
                inventory,
                violations,
            }
        }
        Err(error) => CoverageOutcome::PolicyUnavailable {
            reason: error.to_string(),
        },
    }
}

// @pinker-nav:end trama.code.reusable-verification
// @pinker-nav:start trama.code.query-ranking
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Observed relevance of a region for a textual query: per-field weights, direct-access signals by key, term weight by document frequency and the evidence of the terms that matched. The score orders results and never decides correctness; querying a stable key remains deterministic retrieval above any lexical evidence.

/// Pesos por campo da relação de relevância por termo (§7.3 revisada — #672).
/// São uma hipótese calibrada no conjunto de desenvolvimento da Task, não um
/// contrato histórico: mudá-los muda apenas a ordenação, nunca a integridade.
const FIELD_WEIGHT_KEY: u32 = 6;
const FIELD_WEIGHT_DOMAIN_LAYER: u32 = 4;
const FIELD_WEIGHT_SUMMARY: u32 = 3;
const FIELD_WEIGHT_FILE: u32 = 2;

/// Sinais de acesso direto. Ficam acima de qualquer evidência textual para que
/// consultar uma chave estável continue sendo recuperação determinística e não
/// competição de relevância.
const SCORE_KEY_EXACT: u32 = 100_000;
const SCORE_KEY_CONTAINS_QUERY: u32 = 2_000;
const SCORE_DOMAIN_OR_LAYER_EQUALS_QUERY: u32 = 800;

/// Relevância observada de uma região para uma consulta, com a evidência que a
/// produziu. A pontuação é heurística de ordenação — não é probabilidade.
#[derive(Debug, Clone)]
pub struct RegionMatch<'a> {
    pub region: &'a CodeRegion,
    pub score: u32,
    /// Termos com poder discriminante que a região cobre.
    pub coverage: usize,
    /// Quantos termos com poder discriminante a consulta tinha.
    pub terms_considered: usize,
    pub matched_terms: Vec<String>,
}

/// Campos indexados de uma região para uma consulta.
struct RegionFields {
    key_norm: String,
    domain_norm: String,
    layer_norm: String,
    key: Vec<String>,
    domain_layer: Vec<String>,
    summary: Vec<String>,
    file: Vec<String>,
}

impl RegionFields {
    fn index(region: &CodeRegion) -> Self {
        let key_norm = text_norm::normalize(&region.key);
        let domain_norm = region
            .domain
            .as_deref()
            .map(text_norm::normalize)
            .unwrap_or_default();
        let layer_norm = region
            .layer
            .as_deref()
            .map(text_norm::normalize)
            .unwrap_or_default();
        let mut domain_layer = tokens(&domain_norm);
        domain_layer.extend(tokens(&layer_norm));
        RegionFields {
            key: tokens(&key_norm),
            domain_layer,
            summary: tokens(&text_norm::normalize(&region.summary)),
            file: tokens(&text_norm::normalize(&region.file)),
            key_norm,
            domain_norm,
            layer_norm,
        }
    }

    fn contains(&self, term: &str) -> bool {
        has(&self.key, term)
            || has(&self.domain_layer, term)
            || has(&self.summary, term)
            || has(&self.file, term)
    }

    /// Soma dos pesos dos campos em que o termo aparece como palavra inteira.
    fn field_weight(&self, term: &str) -> u32 {
        let mut weight = 0;
        if has(&self.key, term) {
            weight += FIELD_WEIGHT_KEY;
        }
        if has(&self.domain_layer, term) {
            weight += FIELD_WEIGHT_DOMAIN_LAYER;
        }
        if has(&self.summary, term) {
            weight += FIELD_WEIGHT_SUMMARY;
        }
        if has(&self.file, term) {
            weight += FIELD_WEIGHT_FILE;
        }
        weight
    }
}

fn tokens(normalized: &str) -> Vec<String> {
    normalized
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn has(words: &[String], term: &str) -> bool {
    words.iter().any(|w| w == term)
}

/// Peso de raridade inteiro e determinístico (sem ponto flutuante): um termo
/// presente na maioria das regiões não discrimina nada e vale zero; quanto mais
/// raro, maior o peso, com teto natural em `log2(total)`.
fn term_weight(regions_total: usize, document_frequency: usize) -> u32 {
    if document_frequency == 0 || regions_total == 0 || document_frequency * 2 > regions_total {
        return 0;
    }
    1 + (regions_total / document_frequency).ilog2()
}

/// Pontuação de código (§7.3 revisada — #672). Relação por termo e por campo,
/// ponderada por raridade, preservando a prioridade de acesso direto por chave.
/// Devolve as regiões ordenadas por (pontuação, cobertura, chave).
fn score_regions<'a>(regions: &'a [CodeRegion], query: &str) -> Vec<RegionMatch<'a>> {
    let q_norm = text_norm::normalize(query);
    if q_norm.is_empty() {
        return Vec::new();
    }
    // Termos distintos na ordem de aparição: repetir um termo não amplifica
    // relevância.
    let mut terms: Vec<String> = Vec::new();
    for term in text_norm::terms(query) {
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    // A forma canônica da consulta também ignora a repetição, para que
    // `foo foo` continue encontrando a chave `foo` por acesso direto.
    let q_canonical = terms.join(" ");

    let indexed: Vec<RegionFields> = regions.iter().map(RegionFields::index).collect();
    let mut document_frequency = vec![0usize; terms.len()];
    for fields in &indexed {
        for (i, term) in terms.iter().enumerate() {
            if fields.contains(term) {
                document_frequency[i] += 1;
            }
        }
    }
    let mut weights: Vec<u32> = document_frequency
        .iter()
        .map(|df| term_weight(regions.len(), *df))
        .collect();
    // A regra de raridade descarta termo sem poder discriminante, nunca toda a
    // evidência: quando nenhum termo da consulta discrimina — o caso de um
    // catálogo pequeno, em que qualquer termo está na maioria das regiões —
    // todos os termos presentes voltam a valer o mesmo peso mínimo.
    if weights.iter().all(|w| *w == 0) {
        weights = document_frequency
            .iter()
            .map(|df| u32::from(*df > 0))
            .collect();
    }
    let discriminating = weights.iter().filter(|w| **w > 0).count();

    let mut hits: Vec<RegionMatch<'a>> = Vec::new();
    for (region, fields) in regions.iter().zip(indexed.iter()) {
        let mut textual = 0u32;
        let mut matched_terms: Vec<String> = Vec::new();
        for (i, term) in terms.iter().enumerate() {
            if weights[i] == 0 {
                continue;
            }
            let field_weight = fields.field_weight(term);
            if field_weight == 0 {
                continue;
            }
            textual += weights[i] * field_weight;
            matched_terms.push(term.clone());
        }
        let coverage = matched_terms.len();
        let mut score = textual;
        // Cobrir todos os termos discriminantes é evidência melhor do que somar
        // muito em um termo só.
        if discriminating > 1 && coverage == discriminating {
            score += textual / 2;
        }
        if fields.key_norm == q_canonical {
            score += SCORE_KEY_EXACT;
        } else if fields.key_norm.contains(&q_canonical) {
            score += SCORE_KEY_CONTAINS_QUERY;
        }
        if fields.domain_norm == q_canonical || fields.layer_norm == q_canonical {
            score += SCORE_DOMAIN_OR_LAYER_EQUALS_QUERY;
        }

        if score > 0 {
            hits.push(RegionMatch {
                region,
                score,
                coverage,
                terms_considered: discriminating,
                matched_terms,
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.coverage.cmp(&a.coverage))
            .then(a.region.key.cmp(&b.region.key))
    });
    hits
}

// ---------------------------------------------------------------------------
// Raízes controladas de código (Onda 6D — especificação §12 a 17 e 20).
// ---------------------------------------------------------------------------

// @pinker-nav:end trama.code.query-ranking
// @pinker-nav:start trama.code.roots
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Defines the repository's controlled code roots (`src/`, `runtime/pinker_rt/src/` and `tests/`, all mandatory), validates each root before scanning — absence, a path that is not a directory or a symbolic link fail with `E-NAV-SCAN` before any writing —, collects `.rs` files without following symbolic links and normalizes each path into the repo-relative form with `/`, with no fabricated prefix, guaranteeing that each file is scanned at most once, regardless of the order of the roots.
/// Dialeto léxico usado para distinguir um marcador real do texto que apenas se
/// parece com um (§ Onda 9). Cada raiz declara o seu; o reconhecimento do
/// marcador `//` é idêntico entre dialetos — só muda o rastreamento dos
/// delimitadores (strings, comentários de bloco) que podem esconder um `//`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerDialect {
    /// Fontes `.rs`: strings normais/byte, raw strings, literais de caractere.
    Rust,
    /// Fontes `.pink`: strings normais, triplas (`"""`) e interpoladas (`$"`).
    Pinker,
}

/// Uma raiz de código controlada, relativa à raiz do repositório (§12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoot {
    /// Caminho repo-relativo da raiz (ex.: `"src"`, `"runtime/pinker_rt/src"`).
    /// Vazio significa "a própria raiz recebida": usado apenas pelo wrapper
    /// de compatibilidade [`CodeIndex::scan`] para fixtures de teste de
    /// raiz única, onde nenhum prefixo deve ser fabricado.
    pub relative_path: String,
    /// Extensões aceitas nesta raiz, sem ponto (ex.: `"rs"`).
    pub extensions: Vec<String>,
    /// Dialeto léxico aplicado a cada arquivo desta raiz.
    pub dialect: MarkerDialect,
}

impl ScanRoot {
    /// Cria uma raiz com o dialeto padrão [`MarkerDialect::Rust`]. Preserva a
    /// assinatura usada por todos os chamadores existentes.
    pub fn new(relative_path: impl Into<String>, extensions: &[&str]) -> ScanRoot {
        ScanRoot::with_dialect(relative_path, extensions, MarkerDialect::Rust)
    }

    /// Cria uma raiz declarando explicitamente o dialeto léxico.
    pub fn with_dialect(
        relative_path: impl Into<String>,
        extensions: &[&str],
        dialect: MarkerDialect,
    ) -> ScanRoot {
        ScanRoot {
            relative_path: relative_path.into(),
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
            dialect,
        }
    }
}

/// As raízes oficiais varridas pelo fluxo `pink nav sincronizar`/`verificar`
/// (§15): todas obrigatórias. Fonte única da política de raízes — a CLI
/// oficial e os testes que exercitam o caminho oficial usam esta mesma lista;
/// a API genérica `scan_roots` (via [`CodeIndex::scan`]) aceita listas
/// menores para fixtures.
pub fn official_scan_roots() -> Vec<ScanRoot> {
    vec![
        ScanRoot::new("src", &["rs"]),
        ScanRoot::new("runtime/pinker_rt/src", &["rs"]),
        ScanRoot::new("tests", &["rs"]),
        ScanRoot::with_dialect("apps", &["pink"], MarkerDialect::Pinker),
    ]
}

/// Varre múltiplas raízes controladas relativas a `repo_root` (§12-17) em
/// duas fases estritamente sequenciais. Fase 1 (`resolve_and_validate_roots`):
/// resolve e valida *todas* as raízes — ausente, não-diretório ou link
/// simbólico falham imediatamente com `ScanError`, sem índice parcial. Fase 2:
/// só então cada raiz já validada é percorrida; nenhuma chamada a
/// `collect_source_files` ocorre antes de a fase 1 terminar com sucesso para
/// todas as raízes. Os arquivos de todas as raízes são combinados,
/// deduplicados por caminho repo-relativo e ordenados antes de varrer, então
/// cada arquivo é lido e catalogado no máximo uma vez, independente da ordem
/// em que as raízes foram declaradas (§17). A chave de região continua
/// global: nenhuma raiz vira namespace.
fn scan_roots(repo_root: &Path, roots: &[ScanRoot]) -> Result<CodeIndex, ScanError> {
    // Fase 1: resolve e valida todas as raízes antes de qualquer coleta.
    let validated = resolve_and_validate_roots(repo_root, roots)?;

    // Fase 2: apenas raízes já validadas são percorridas.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut files: Vec<(String, PathBuf, MarkerDialect)> = Vec::new();
    for (root, abs_root) in &validated {
        let mut collected = Vec::new();
        collect_source_files(abs_root, abs_root, &root.extensions, &mut collected)?;
        for rel in collected {
            let display = compose_display_path(&root.relative_path, &rel);
            if seen.insert(display.clone()) {
                files.push((display, abs_root.join(&rel), root.dialect));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut index = CodeIndex::default();
    for (display, abs_path, dialect) in files {
        let text = fs::read_to_string(&abs_path).map_err(|err| ScanError::Io {
            path: abs_path.display().to_string(),
            msg: err.to_string(),
        })?;
        scan_file(&display, &text, dialect, &mut index);
    }
    index
        .regions
        .sort_by(|a, b| a.key.cmp(&b.key).then(a.file.cmp(&b.file)));
    Ok(index)
}

/// Resolve o caminho absoluto de cada raiz e a valida (§15), sem coletar
/// nenhum arquivo. Falha na primeira raiz inválida, na ordem declarada, sem
/// ter percorrido nenhuma raiz — a coleta só começa depois que todas as
/// raízes retornadas aqui já estão validadas (fase 1 de `scan_roots`).
fn resolve_and_validate_roots<'a>(
    repo_root: &Path,
    roots: &'a [ScanRoot],
) -> Result<Vec<(&'a ScanRoot, PathBuf)>, ScanError> {
    let mut validated = Vec::with_capacity(roots.len());
    for root in roots {
        let abs_root = if root.relative_path.is_empty() {
            repo_root.to_path_buf()
        } else {
            repo_root.join(&root.relative_path)
        };
        validate_root(&abs_root)?;
        validated.push((root, abs_root));
    }
    Ok(validated)
}

/// Valida uma raiz antes de varrê-la (§15): deve existir, ser diretório e não
/// ser um link simbólico (política explícita de segurança contra fuga/ciclo
/// via symlink de raiz; §14). Falha antes de qualquer leitura de arquivo.
/// Distingue ausência (`NotFound`) de outras falhas de metadados (ex.:
/// permissão negada, componente do caminho que não é diretório): apenas
/// `NotFound` vira `RootMissing`; qualquer outro erro de I/O vira
/// `ScanError::Io`, sem ser classificado como raiz ausente.
fn validate_root(path: &Path) -> Result<(), ScanError> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(ScanError::RootMissing {
                path: path.display().to_string(),
            });
        }
        Err(err) => {
            return Err(ScanError::Io {
                path: path.display().to_string(),
                msg: err.to_string(),
            });
        }
    };
    if meta.file_type().is_symlink() {
        return Err(ScanError::RootSymlinkRefused {
            path: path.display().to_string(),
        });
    }
    if !meta.is_dir() {
        return Err(ScanError::RootNotDirectory {
            path: path.display().to_string(),
        });
    }
    Ok(())
}

/// Caminha recursivamente a partir de `dir` (dentro de `root`) coletando
/// arquivos cuja extensão está em `extensions`, relativos a `root`. Nunca
/// segue links simbólicos — nem de diretório (evita ciclos e fuga da raiz)
/// nem de arquivo — para que nenhum symlink seja catalogado (§14/§20).
fn collect_source_files(
    root: &Path,
    dir: &Path,
    extensions: &[String],
    out: &mut Vec<PathBuf>,
) -> Result<(), ScanError> {
    let read_dir = fs::read_dir(dir).map_err(|err| ScanError::Io {
        path: dir.display().to_string(),
        msg: err.to_string(),
    })?;
    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|err| ScanError::Io {
            path: dir.display().to_string(),
            msg: err.to_string(),
        })?;
        entries.push(entry);
    }
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| ScanError::Io {
            path: path.display().to_string(),
            msg: err.to_string(),
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_source_files(root, &path, extensions, out)?;
        } else if file_type.is_file() {
            let has_ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| extensions.iter().any(|allowed| allowed == ext))
                .unwrap_or(false);
            if has_ext {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_path_buf());
                }
            }
        }
    }
    Ok(())
}

/// Compõe o caminho repo-relativo exibido no catálogo: `relative_path` da
/// raiz + caminho do arquivo relativo a ela, sempre com `/` (§14). Uma
/// `relative_path` vazia (usada só pelo wrapper de raiz única) não injeta
/// nenhum prefixo — o caminho fica relativo apenas à raiz recebida.
fn compose_display_path(relative_path: &str, file_relative: &Path) -> String {
    let file_str = file_relative.to_string_lossy().replace('\\', "/");
    if relative_path.is_empty() {
        file_str
    } else {
        format!("{relative_path}/{file_str}")
    }
}
// @pinker-nav:end trama.code.roots

// @pinker-nav:start trama.code.official-files
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav::official_source_files|official_source_files|rust-function|declaration
// @pinker-nav:symbol pinker_v0::nav::official_source_files|official_source_files|rust-function|implementation
// @pinker-nav:summary Enumerates the source files of the official roots regardless of whether they have markers, delivering the eligible universe that cartography coverage audits; the same root validation and the same symlink-free traversal of the scan flow decide the set.

/// Um arquivo-fonte pertencente a uma raiz oficial, com o dialeto léxico
/// daquela raiz. A lista existe para que a cobertura da cartografia parta do
/// universo físico de arquivos, e nunca dos próprios marcadores publicados:
/// um arquivo sem nenhuma região continua aparecendo aqui.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialSourceFile {
    /// Caminho repo-relativo exibido, idêntico ao usado pelo catálogo.
    pub path: String,
    /// Raiz oficial que contém o arquivo (ex.: `"src"`).
    pub root: String,
    /// Dialeto léxico declarado pela raiz.
    pub dialect: MarkerDialect,
}

/// Enumera todos os arquivos-fonte das raízes oficiais (§15), na mesma ordem
/// determinística do catálogo e com a mesma política de validação de raiz e de
/// recusa a symlink. Não lê o conteúdo dos arquivos e não depende de nenhum
/// marcador: a ausência de `@pinker-nav` não remove um arquivo desta lista.
pub fn official_source_files(repo_root: &Path) -> Result<Vec<OfficialSourceFile>, ScanError> {
    let roots = official_scan_roots();
    let validated = resolve_and_validate_roots(repo_root, &roots)?;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut files: Vec<OfficialSourceFile> = Vec::new();
    for (root, abs_root) in &validated {
        let mut collected = Vec::new();
        collect_source_files(abs_root, abs_root, &root.extensions, &mut collected)?;
        for rel in collected {
            let display = compose_display_path(&root.relative_path, &rel);
            if seen.insert(display.clone()) {
                files.push(OfficialSourceFile {
                    path: display,
                    root: root.relative_path.clone(),
                    dialect: root.dialect,
                });
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}
// @pinker-nav:end trama.code.official-files

// @pinker-nav:start trama.code.lexical-relevance
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav::relevant_source_lines|relevant_source_lines|rust-function|declaration
// @pinker-nav:symbol pinker_v0::nav::relevant_source_lines|relevant_source_lines|rust-function|implementation
// @pinker-nav:summary Conservative classification of a relevant line reusing the markers' lexical tracker: only an empty line and a line that is entirely a comment leave the coverage obligation, and every doubt remains relevant instead of becoming a silent PASS.

/// Decide, linha a linha, se uma linha do arquivo carrega obrigação de
/// cobertura. A classificação é **conservadora por desenho**: ela remove da
/// obrigação apenas o que é demonstravelmente vazio ou demonstravelmente
/// comentário integral, e mantém como relevante tudo que não souber decidir.
///
/// Reutiliza o mesmo rastreador léxico que distingue marcador real de texto
/// parecido com marcador, então `//` dentro de string não esconde código e
/// `/*` dentro de string não abre comentário.
///
/// Limitações declaradas, todas na direção segura (conservam obrigação):
///
/// - uma linha que abre e fecha `/* ... */` sem código permanece relevante;
/// - uma linha que fecha um comentário de bloco permanece relevante;
/// - linha de conteúdo de string multilinha permanece relevante.
///
/// O índice 0 do vetor corresponde à linha 1 do arquivo, e o comprimento do
/// vetor é o número de linhas de `text` segundo [`str::lines`].
pub fn relevant_source_lines(text: &str, dialect: MarkerDialect) -> Vec<bool> {
    match dialect {
        MarkerDialect::Rust => {
            let mut state = LexicalState::Code;
            text.lines()
                .map(|line| {
                    let opened_inside_block = matches!(state, LexicalState::BlockComment(_));
                    let whole_line_comment = marker_comment(line, &mut state).is_some();
                    let still_inside_block = matches!(state, LexicalState::BlockComment(_));
                    line_is_relevant(
                        line,
                        opened_inside_block,
                        still_inside_block,
                        whole_line_comment,
                    )
                })
                .collect()
        }
        MarkerDialect::Pinker => {
            let mut state = PinkerState::Code;
            text.lines()
                .map(|line| {
                    let opened_inside_block = matches!(state, PinkerState::BlockComment(_));
                    let whole_line_comment = marker_comment_pinker(line, &mut state).is_some();
                    let still_inside_block = matches!(state, PinkerState::BlockComment(_));
                    line_is_relevant(
                        line,
                        opened_inside_block,
                        still_inside_block,
                        whole_line_comment,
                    )
                })
                .collect()
        }
    }
}

/// Regra única compartilhada pelos dois dialetos. Somente três situações
/// retiram a obrigação de cobertura de uma linha: ela é vazia/branca, ela
/// começou e terminou dentro de um comentário de bloco, ou seu primeiro token
/// não-branco abre um comentário de linha a partir de estado de código.
fn line_is_relevant(
    line: &str,
    opened_inside_block: bool,
    still_inside_block: bool,
    whole_line_comment: bool,
) -> bool {
    if line.trim().is_empty() {
        return false;
    }
    if opened_inside_block && still_inside_block {
        return false;
    }
    !whole_line_comment
}
// @pinker-nav:end trama.code.lexical-relevance
// @pinker-nav:start trama.code.file-scan
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary A file's scan loop: it keeps the current open region, recognizes start/end by the root's dialect, refuses nesting and records an orphan or unclosed marker as a scan problem instead of discarding it silently.

struct OpenRegion {
    key: String,
    start_marker: usize,
    invalid_key: bool,
    domain: Option<String>,
    layer: Option<String>,
    phase: Option<u64>,
    kind: String,
    status: String,
    summary: String,
    content_start: Option<usize>,
    content_end: usize,
    content_lines: Vec<String>,
    in_meta: bool,
    symbols: Vec<RegionSymbol>,
    related_symbols: Vec<String>,
    test_for: Vec<String>,
    symbol_docs: Vec<SymbolDocLink>,
}

fn scan_file(rel_path: &str, text: &str, dialect: MarkerDialect, index: &mut CodeIndex) {
    let lines: Vec<&str> = text.lines().collect();
    let mut stack: Vec<OpenRegion> = Vec::new();
    let mut lexical_state = DialectState::new(dialect);

    for (i, raw) in lines.iter().enumerate() {
        let line_no = i + 1;
        let trimmed = raw.trim();
        let marker_line = lexical_state.next_marker(raw);

        if let Some(key) = marker_line.and_then(|comment| parse_marker(comment, START)) {
            if let Some(outer) = stack.last() {
                index.scan_problems.push(NavVerifyError::Overlap {
                    outer: outer.key.clone(),
                    inner: key.clone(),
                    file: rel_path.to_string(),
                });
            }
            let invalid_key = !valid_key(&key);
            if invalid_key {
                index.scan_problems.push(NavVerifyError::InvalidKey {
                    key: key.clone(),
                    file: rel_path.to_string(),
                    line: line_no,
                });
            }
            stack.push(OpenRegion {
                key,
                start_marker: line_no,
                invalid_key,
                domain: None,
                layer: None,
                phase: None,
                kind: "region".to_string(),
                status: "active".to_string(),
                summary: String::new(),
                content_start: None,
                content_end: line_no,
                content_lines: Vec::new(),
                in_meta: true,
                symbols: Vec::new(),
                related_symbols: Vec::new(),
                test_for: Vec::new(),
                symbol_docs: Vec::new(),
            });
            continue;
        }

        if let Some(end_key) = marker_line.and_then(|comment| parse_marker(comment, END)) {
            match stack.pop() {
                Some(open) => finish_region(rel_path, open, end_key, line_no, index),
                None => index.scan_problems.push(NavVerifyError::EndWithoutStart {
                    key: end_key,
                    file: rel_path.to_string(),
                    line: line_no,
                }),
            }
            continue;
        }

        // Linha comum ou de metadado dentro de uma região aberta.
        if let Some(open) = stack.last_mut() {
            if open.in_meta {
                if let Some((field, value)) = marker_line.and_then(parse_meta) {
                    apply_meta(open, rel_path, &field, &value, index);
                    continue;
                }
                open.in_meta = false;
            }
            if !trimmed.is_empty() {
                if open.content_start.is_none() {
                    open.content_start = Some(line_no);
                }
                open.content_end = line_no;
            }
            open.content_lines.push((*raw).to_string());
        }
    }

    for leftover in stack.into_iter().rev() {
        index.scan_problems.push(NavVerifyError::StartWithoutEnd {
            key: leftover.key,
            file: rel_path.to_string(),
            line: leftover.start_marker,
        });
    }
}
// @pinker-nav:end trama.code.file-scan
// @pinker-nav:start trama.code.real-comment
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Lexical recognition of a true comment per dialect, so that a marker written inside a text literal, a character literal, a raw string or a nested block comment never becomes a region. The lexical state is per file and advances line by line; only the recognizer changes between Rust and Pinker, never the key namespace.

/// Estado léxico ativo de um arquivo, escolhido pelo dialeto da raiz. O laço de
/// varredura é único e independente do dialeto: apenas o reconhecedor de
/// marcador (`next_marker`) difere, pois cada linguagem esconde `//` atrás de
/// delimitadores próprios. A chave de região permanece global — o dialeto só
/// afeta o reconhecimento léxico, nunca o namespace.
enum DialectState {
    Rust(LexicalState),
    Pinker(PinkerState),
}

impl DialectState {
    fn new(dialect: MarkerDialect) -> DialectState {
        match dialect {
            MarkerDialect::Rust => DialectState::Rust(LexicalState::Code),
            MarkerDialect::Pinker => DialectState::Pinker(PinkerState::Code),
        }
    }

    /// Devolve o comentário candidato desta linha e avança o estado léxico para
    /// a próxima, delegando ao reconhecedor do dialeto ativo.
    fn next_marker<'a>(&mut self, line: &'a str) -> Option<&'a str> {
        match self {
            DialectState::Rust(state) => marker_comment(line, state),
            DialectState::Pinker(state) => marker_comment_pinker(line, state),
        }
    }
}

/// Estado léxico mínimo para distinguir comentários reais de texto que apenas
/// se parece com marcador. Não pretende analisar Rust completo: só acompanha
/// os delimitadores que podem atravessar linhas e esconder `//`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexicalState {
    Code,
    NormalString,
    ByteString,
    RawString(usize),
    RawByteString(usize),
    BlockComment(usize),
}

/// Devolve o comentário candidato somente quando `//` é o primeiro token não
/// branco de uma linha iniciada em código. Ao mesmo tempo avança o estado
/// léxico para a linha seguinte.
fn marker_comment<'a>(line: &'a str, state: &mut LexicalState) -> Option<&'a str> {
    let bytes = line.as_bytes();
    let first_non_whitespace = bytes.iter().position(|byte| !byte.is_ascii_whitespace());
    let mut i = 0;

    while i < bytes.len() {
        match *state {
            LexicalState::Code => {
                if bytes[i..].starts_with(b"//") {
                    if Some(i) == first_non_whitespace {
                        return Some(&line[i..]);
                    }
                    return None;
                }
                if bytes[i..].starts_with(b"/*") {
                    *state = LexicalState::BlockComment(1);
                    i += 2;
                } else if let Some((raw_len, hashes, is_byte)) = raw_string_start(&bytes[i..]) {
                    *state = if is_byte {
                        LexicalState::RawByteString(hashes)
                    } else {
                        LexicalState::RawString(hashes)
                    };
                    i += raw_len;
                } else if bytes[i..].starts_with(b"b\"") {
                    *state = LexicalState::ByteString;
                    i += 2;
                } else if let Some(char_len) = char_literal_len(&bytes[i..]) {
                    i += char_len;
                } else if bytes[i] == b'\"' {
                    *state = LexicalState::NormalString;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            LexicalState::NormalString | LexicalState::ByteString => {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == b'\"' {
                    *state = LexicalState::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            LexicalState::RawString(hashes) | LexicalState::RawByteString(hashes) => {
                if bytes[i] == b'\"' && bytes[i + 1..].starts_with(&vec![b'#'; hashes]) {
                    *state = LexicalState::Code;
                    i += hashes + 1;
                } else {
                    i += 1;
                }
            }
            LexicalState::BlockComment(depth) => {
                if bytes[i..].starts_with(b"/*") {
                    *state = LexicalState::BlockComment(depth + 1);
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    if depth == 1 {
                        *state = LexicalState::Code;
                    } else {
                        *state = LexicalState::BlockComment(depth - 1);
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
    None
}

/// Reconhece o prefixo de uma raw string Rust e informa seu comprimento, a
/// quantidade de `#` e se é uma raw byte string (`br`).
fn raw_string_start(bytes: &[u8]) -> Option<(usize, usize, bool)> {
    let (prefix_len, is_byte) = if bytes.starts_with(b"br") {
        (2, true)
    } else if bytes.starts_with(b"r") {
        (1, false)
    } else {
        return None;
    };
    let mut pos = prefix_len;
    while bytes.get(pos) == Some(&b'#') {
        pos += 1;
    }
    (bytes.get(pos) == Some(&b'\"')).then_some((pos + 1, pos - prefix_len, is_byte))
}

/// Literais de caractere não atravessam linhas. Só os consome quando o
/// apóstrofo de fechamento vem imediatamente após um único caractere (ou
/// escape), para não confundir lifetimes como `'a` com um literal aberto.
fn char_literal_len(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'\'') {
        return None;
    }

    if bytes.get(1) == Some(&b'\\') {
        return (bytes.get(3) == Some(&b'\'')).then_some(4);
    }

    let character = std::str::from_utf8(bytes.get(1..)?).ok()?.chars().next()?;
    let closing_quote = 1 + character.len_utf8();
    (bytes.get(closing_quote) == Some(&b'\'')).then_some(closing_quote + 1)
}

/// Produz uma visão de Rust com comentários e literais substituídos por
/// espaços. Mantém novas linhas e offsets para consumidores lexicais que não
/// podem tratar texto escondido como código.
pub fn rust_code_mask(text: &str) -> String {
    mask_rust(text).0
}

/// Intervalos de bytes ocupados pelos comentários de documentação (`///`,
/// `//!`, `/** */`, `/*! */`). A máscara os apaga junto com os comentários
/// comuns, mas em Rust um comentário de documentação é atributo, não espaço em
/// branco: um consumidor léxico que separe tokens precisa saber distinguir os
/// dois em vez de tratar todo texto apagado como separador.
pub fn rust_doc_comment_spans(text: &str) -> Vec<Range<usize>> {
    mask_rust(text).1
}

/// Núcleo único da visão mascarada: um só percurso produz a máscara e os
/// intervalos de comentário de documentação, para que as duas respostas nunca
/// divirjam sobre o mesmo texto.
fn mask_rust(text: &str) -> (String, Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    let mut state = LexicalState::Code;
    let mut out = bytes.to_vec();
    let mut doc_comments: Vec<Range<usize>> = Vec::new();
    let mut doc_block_start: Option<usize> = None;
    let mut i = 0;
    while i < bytes.len() {
        match state {
            LexicalState::Code => {
                if bytes[i..].starts_with(b"//") {
                    let end = bytes[i..]
                        .iter()
                        .position(|b| *b == b'\n')
                        .map(|n| i + n)
                        .unwrap_or(bytes.len());
                    if is_doc_line_comment(&bytes[i..]) {
                        doc_comments.push(i..end);
                    }
                    out[i..end].fill(b' ');
                    i = end;
                } else if bytes[i..].starts_with(b"/*") {
                    if is_doc_block_comment(&bytes[i..]) {
                        doc_block_start = Some(i);
                    }
                    out[i..i + 2].fill(b' ');
                    state = LexicalState::BlockComment(1);
                    i += 2;
                } else if let Some((len, hashes, byte)) = raw_string_start(&bytes[i..]) {
                    out[i..i + len].fill(b' ');
                    state = if byte {
                        LexicalState::RawByteString(hashes)
                    } else {
                        LexicalState::RawString(hashes)
                    };
                    i += len;
                } else if bytes[i..].starts_with(b"b\"") {
                    out[i..i + 2].fill(b' ');
                    state = LexicalState::ByteString;
                    i += 2;
                } else if let Some(len) = char_literal_len(&bytes[i..]) {
                    out[i..i + len].fill(b' ');
                    i += len;
                } else if bytes[i] == b'\"' {
                    out[i] = b' ';
                    state = LexicalState::NormalString;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            LexicalState::NormalString | LexicalState::ByteString => {
                out[i] = if bytes[i] == b'\n' { b'\n' } else { b' ' };
                if bytes[i] == b'\\' {
                    if i + 1 < bytes.len() {
                        out[i + 1] = if bytes[i + 1] == b'\n' { b'\n' } else { b' ' };
                    }
                    i += 2;
                } else if bytes[i] == b'\"' {
                    state = LexicalState::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            LexicalState::RawString(hashes) | LexicalState::RawByteString(hashes) => {
                out[i] = if bytes[i] == b'\n' { b'\n' } else { b' ' };
                if bytes[i] == b'\"' && bytes[i + 1..].starts_with(&vec![b'#'; hashes]) {
                    for slot in &mut out[i + 1..i + 1 + hashes] {
                        *slot = b' ';
                    }
                    state = LexicalState::Code;
                    i += hashes + 1;
                } else {
                    i += 1;
                }
            }
            LexicalState::BlockComment(depth) => {
                out[i] = if bytes[i] == b'\n' { b'\n' } else { b' ' };
                if bytes[i..].starts_with(b"/*") {
                    if i + 1 < out.len() {
                        out[i + 1] = b' ';
                    }
                    state = LexicalState::BlockComment(depth + 1);
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    if i + 1 < out.len() {
                        out[i + 1] = b' ';
                    }
                    state = if depth == 1 {
                        if let Some(start) = doc_block_start.take() {
                            doc_comments.push(start..i + 2);
                        }
                        LexicalState::Code
                    } else {
                        LexicalState::BlockComment(depth - 1)
                    };
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
    // Um comentário de documentação de bloco que nunca fecha esconde o resto do
    // arquivo; ele é reportado até o fim em vez de desaparecer.
    if let Some(start) = doc_block_start {
        doc_comments.push(start..bytes.len());
    }
    let masked = String::from_utf8(out).expect("Rust source remains UTF-8 after masking");
    (masked, doc_comments)
}

/// `///` e `//!` são documentação; `////` é apenas um separador visual comum.
fn is_doc_line_comment(bytes: &[u8]) -> bool {
    (bytes.starts_with(b"///") && !bytes.starts_with(b"////")) || bytes.starts_with(b"//!")
}

/// `/** */` e `/*! */` são documentação; `/***` é ornamento comum e `/**/` é o
/// comentário de bloco vazio.
fn is_doc_block_comment(bytes: &[u8]) -> bool {
    (bytes.starts_with(b"/**") && !bytes.starts_with(b"/***") && !bytes.starts_with(b"/**/"))
        || bytes.starts_with(b"/*!")
}

/// Estado léxico mínimo do dialeto Pinker (§ Onda 9). Como no rastreador Rust,
/// só acompanha os delimitadores capazes de esconder um `//`: strings normais,
/// strings triplas (`"""`, atravessam linhas) e strings interpoladas (`$"`),
/// além de comentários de bloco aninhados (`/* */`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PinkerState {
    Code,
    NormalString,
    InterpolatedString,
    TripleString,
    BlockComment(usize),
}

/// Reconhecedor de marcador do dialeto Pinker. Devolve o comentário candidato
/// somente quando `//` é o primeiro token não branco de uma linha iniciada em
/// código (`PinkerState::Code`); avança o estado léxico para a linha seguinte.
/// Ignora `//` dentro de string normal, tripla, interpolada ou comentário de
/// bloco, e ignora `//` que venha após código executável na mesma linha.
/// Strings normais e interpoladas não atravessam linhas (voltam a `Code` no fim
/// da linha); strings triplas e comentários de bloco atravessam.
fn marker_comment_pinker<'a>(line: &'a str, state: &mut PinkerState) -> Option<&'a str> {
    let bytes = line.as_bytes();
    let first_non_whitespace = bytes.iter().position(|byte| !byte.is_ascii_whitespace());
    let mut i = 0;

    while i < bytes.len() {
        match *state {
            PinkerState::Code => {
                if bytes[i..].starts_with(b"//") {
                    // O comentário de linha consome o resto da linha; o estado da
                    // próxima linha permanece `Code`.
                    return (Some(i) == first_non_whitespace).then_some(&line[i..]);
                }
                if bytes[i..].starts_with(b"/*") {
                    *state = PinkerState::BlockComment(1);
                    i += 2;
                } else if bytes[i..].starts_with(b"\"\"\"") {
                    *state = PinkerState::TripleString;
                    i += 3;
                } else if bytes[i..].starts_with(b"$\"") {
                    *state = PinkerState::InterpolatedString;
                    i += 2;
                } else if bytes[i] == b'\"' {
                    *state = PinkerState::NormalString;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            PinkerState::NormalString | PinkerState::InterpolatedString => {
                if bytes[i] == b'\\' {
                    i += 2;
                } else if bytes[i] == b'\"' {
                    *state = PinkerState::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            PinkerState::TripleString => {
                if bytes[i..].starts_with(b"\"\"\"") {
                    *state = PinkerState::Code;
                    i += 3;
                } else {
                    i += 1;
                }
            }
            PinkerState::BlockComment(depth) => {
                if bytes[i..].starts_with(b"/*") {
                    *state = PinkerState::BlockComment(depth + 1);
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    *state = if depth == 1 {
                        PinkerState::Code
                    } else {
                        PinkerState::BlockComment(depth - 1)
                    };
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }

    // Strings normais e interpoladas não atravessam linhas: uma que fique aberta
    // no fim da linha é encerrada aqui, para não engolir um `//` da linha
    // seguinte. Strings triplas e comentários de bloco permanecem abertos.
    if matches!(
        *state,
        PinkerState::NormalString | PinkerState::InterpolatedString
    ) {
        *state = PinkerState::Code;
    }

    None
}

// @pinker-nav:end trama.code.real-comment
// @pinker-nav:start trama.code.region-closure
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Closing of an open region: it checks that the end's key matches the start's, computes the content bounds and the body's hash, applies the accumulated metadata and publishes the region in the index, converting a divergence into a named scan problem.
fn finish_region(
    rel_path: &str,
    open: OpenRegion,
    end_key: String,
    end_line: usize,
    index: &mut CodeIndex,
) {
    if open.key != end_key {
        index.scan_problems.push(NavVerifyError::KeyMismatch {
            start: open.key.clone(),
            end: end_key.clone(),
            file: rel_path.to_string(),
        });
    }
    let Some(content_start) = open.content_start else {
        index.scan_problems.push(NavVerifyError::EmptyRange {
            key: open.key.clone(),
            file: rel_path.to_string(),
        });
        return;
    };
    if open.invalid_key {
        return; // já registrado; não cataloga chave inválida
    }
    // Conteúdo da região = linhas não-vazias entre content_start e content_end.
    let body: Vec<&String> = open
        .content_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let hash = fnv1a64(
        &body
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );

    index.regions.push(CodeRegion {
        key: open.key,
        kind: open.kind,
        domain: open.domain,
        layer: open.layer,
        phase: open.phase,
        file: rel_path.to_string(),
        start_marker: open.start_marker,
        content_start,
        content_end: open.content_end,
        end_marker: end_line,
        summary: open.summary,
        hash,
        status: open.status,
        symbols: open.symbols,
        related_symbols: open.related_symbols,
        test_for: open.test_for,
        symbol_docs: open.symbol_docs,
    });
}

// @pinker-nav:end trama.code.region-closure
// @pinker-nav:start trama.code.symbol-metadata
// @pinker-nav:domain symbols
// @pinker-nav:layer trama
// @pinker-nav:summary Interprets and validates the symbol, related-symbol, test-for and symbol-doc bindings inside the @pinker-nav authority, with closed categories and roles, qualified identity, canonical destinations and duplication refused before the derived serialization.
fn apply_meta(
    open: &mut OpenRegion,
    rel_path: &str,
    field: &str,
    value: &str,
    index: &mut CodeIndex,
) {
    match field {
        "domain" => open.domain = Some(value.to_string()),
        "layer" => open.layer = Some(value.to_string()),
        "summary" => open.summary = value.to_string(),
        "kind" => open.kind = value.to_string(),
        "status" => open.status = value.to_string(),
        "phase" => match value.parse::<u64>() {
            Ok(phase) => open.phase = Some(phase),
            Err(_) => index.scan_problems.push(NavVerifyError::MalformedMeta {
                key: open.key.clone(),
                file: rel_path.to_string(),
                field: "phase".to_string(),
            }),
        },
        "symbol" => match parse_symbol(value) {
            Some(symbol) if !open.symbols.contains(&symbol) => open.symbols.push(symbol),
            _ => malformed_meta(open, rel_path, "symbol", index),
        },
        "related-symbol" => match parse_symbol_identity(value) {
            Some(identity) if !open.related_symbols.contains(&identity) => {
                open.related_symbols.push(identity)
            }
            _ => malformed_meta(open, rel_path, "related-symbol", index),
        },
        "test-for" => match parse_symbol_identity(value) {
            Some(identity) if !open.test_for.contains(&identity) => open.test_for.push(identity),
            _ => malformed_meta(open, rel_path, "test-for", index),
        },
        "symbol-doc" => match parse_symbol_doc(value) {
            Some(link) if !open.symbol_docs.contains(&link) => open.symbol_docs.push(link),
            _ => malformed_meta(open, rel_path, "symbol-doc", index),
        },
        other => index.scan_problems.push(NavVerifyError::MalformedMeta {
            key: open.key.clone(),
            file: rel_path.to_string(),
            field: other.to_string(),
        }),
    }
}

fn malformed_meta(open: &OpenRegion, rel_path: &str, field: &str, index: &mut CodeIndex) {
    index.scan_problems.push(NavVerifyError::MalformedMeta {
        key: open.key.clone(),
        file: rel_path.to_string(),
        field: field.to_string(),
    });
}

fn parse_symbol(value: &str) -> Option<RegionSymbol> {
    let parts = value.split('|').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 4 {
        return None;
    }
    let identity = parse_symbol_identity(parts[0])?;
    let name = parse_symbol_name(parts[1])?;
    let kind = match parts[2] {
        "rust-function" => SymbolKind::RustFunction,
        "rust-type" => SymbolKind::RustType,
        "pinker-function" => SymbolKind::PinkerFunction,
        "UNKNOWN" => SymbolKind::Unknown,
        _ => return None,
    };
    let role = match parts[3] {
        "declaration" => SymbolRole::Declaration,
        "implementation" => SymbolRole::Implementation,
        _ => return None,
    };
    Some(RegionSymbol {
        identity,
        name,
        kind,
        role,
    })
}

fn parse_symbol_doc(value: &str) -> Option<SymbolDocLink> {
    let parts = value.split('|').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 2 || !valid_key(parts[1]) {
        return None;
    }
    Some(SymbolDocLink {
        identity: parse_symbol_identity(parts[0])?,
        document: parts[1].to_string(),
    })
}

fn parse_symbol_identity(value: &str) -> Option<String> {
    let segments = value.split("::").collect::<Vec<_>>();
    if segments.len() < 2
        || segments
            .iter()
            .any(|segment| parse_symbol_name(segment).is_none())
    {
        return None;
    }
    Some(value.to_string())
}

fn parse_symbol_name(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_')
        || !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some(value.to_string())
}
// @pinker-nav:end trama.code.symbol-metadata
// @pinker-nav:start trama.code.marker-syntax
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Textual syntax of the @pinker-nav authority: it extracts a marker's key requiring a real line comment and a strict prefix, separates the metadata's field and value, and validates the closed key format. This is where a doc comment and an approximate prefix stop being metadata.

/// Extrai a chave após um marcador (`@pinker-nav:start`/`:end`) em uma linha
/// que deve ser um comentário `//`.
fn parse_marker(trimmed: &str, marker: &str) -> Option<String> {
    let comment = trimmed.strip_prefix("//")?;
    if comment.starts_with('/') || comment.starts_with('!') {
        return None;
    }
    let marker_text = comment.trim_start();
    let rest = marker_text.strip_prefix(marker)?;
    // Evita casar `@pinker-nav:start` quando o buscado é `@pinker-nav:end`
    // (ambos contêm `@pinker-nav:`), garantindo limite após o marcador.
    let after = rest;
    if !after.starts_with(char::is_whitespace) && !after.is_empty() {
        return None;
    }
    let key = after.trim();
    if key.is_empty() {
        None
    } else {
        Some(key.to_string())
    }
}

fn parse_meta(trimmed: &str) -> Option<(String, String)> {
    let comment = trimmed.strip_prefix("//")?;
    if comment.starts_with('/') || comment.starts_with('!') {
        return None;
    }
    let marker_text = comment.trim_start();
    let rest = marker_text.strip_prefix(FIELD_PREFIX)?;
    let mut parts = rest.splitn(2, char::is_whitespace);
    let field = parts.next()?.trim().to_string();
    if field == "start" || field == "end" {
        return None;
    }
    let value = parts.next().unwrap_or("").trim().to_string();
    // Campos desconhecidos também são consumidos como metadado e depois
    // sinalizados como malformados por `apply_meta`.
    Some((field, value))
}

fn valid_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }
    let bytes = key.as_bytes();
    let is_alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let is_sep = |c: u8| c == b'.' || c == b'_' || c == b'-';
    if !is_alnum(bytes[0]) || !is_alnum(bytes[bytes.len() - 1]) {
        return false;
    }
    let mut prev_sep = false;
    for &c in bytes {
        if is_alnum(c) {
            prev_sep = false;
        } else if is_sep(c) {
            if prev_sep {
                return false;
            }
            prev_sep = true;
        } else {
            return false;
        }
    }
    true
}
// @pinker-nav:end trama.code.marker-syntax
// @pinker-nav:start trama.code.json-serialization
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:summary Deterministic serialization of the derived catalog: fnv1a64 hash of the regions' bodies, JSON render of region, symbol and documentation link in a fixed field order, and text escaping. Determinism here is what makes catalog drift detectable by literal comparison.

pub(crate) fn fnv1a64(data: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{:016x}", hash)
}

fn render_region_json(r: &CodeRegion) -> String {
    let mut out = String::new();
    out.push_str("{\"schema\":1");
    out.push_str(&format!(",\"key\":{}", json_string(&r.key)));
    out.push_str(&format!(",\"kind\":{}", json_string(&r.kind)));
    if let Some(domain) = &r.domain {
        out.push_str(&format!(",\"domain\":{}", json_string(domain)));
    }
    if let Some(layer) = &r.layer {
        out.push_str(&format!(",\"layer\":{}", json_string(layer)));
    }
    if let Some(phase) = r.phase {
        out.push_str(&format!(",\"phase\":{}", phase));
    }
    out.push_str(&format!(",\"file\":{}", json_string(&r.file)));
    out.push_str(&format!(",\"start_marker\":{}", r.start_marker));
    out.push_str(&format!(",\"content_start\":{}", r.content_start));
    out.push_str(&format!(",\"content_end\":{}", r.content_end));
    out.push_str(&format!(",\"end_marker\":{}", r.end_marker));
    if !r.summary.is_empty() {
        out.push_str(&format!(",\"summary\":{}", json_string(&r.summary)));
    }
    out.push_str(&format!(",\"hash\":{}", json_string(&r.hash)));
    out.push_str(&format!(",\"status\":{}", json_string(&r.status)));
    if !r.symbols.is_empty() {
        let values = r.symbols.iter().map(render_symbol).collect::<Vec<_>>();
        out.push_str(&format!(",\"symbols\":{}", json_string_array(&values)));
    }
    if !r.related_symbols.is_empty() {
        out.push_str(&format!(
            ",\"related_symbols\":{}",
            json_string_array(&r.related_symbols)
        ));
    }
    if !r.test_for.is_empty() {
        out.push_str(&format!(",\"test_for\":{}", json_string_array(&r.test_for)));
    }
    if !r.symbol_docs.is_empty() {
        let values = r
            .symbol_docs
            .iter()
            .map(render_symbol_doc)
            .collect::<Vec<_>>();
        out.push_str(&format!(",\"symbol_docs\":{}", json_string_array(&values)));
    }
    out.push('}');
    out
}

fn render_symbol(symbol: &RegionSymbol) -> String {
    format!(
        "{}|{}|{}|{}",
        symbol.identity,
        symbol.name,
        symbol.kind.as_str(),
        symbol.role.as_str()
    )
}

fn render_symbol_doc(link: &SymbolDocLink) -> String {
    format!("{}|{}", link.identity, link.document)
}

pub(crate) fn json_string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(crate) fn json_string(value: &str) -> String {
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

// ---------------------------------------------------------------------------
// Catálogo carregado do JSONL (superfície de consulta — §5).
// ---------------------------------------------------------------------------

// @pinker-nav:end trama.code.json-serialization
// @pinker-nav:start trama.code.query
// @pinker-nav:domain navigation
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::nav::CatalogError|CatalogError|rust-type|declaration
// @pinker-nav:symbol pinker_v0::nav::CatalogError|CatalogError|rust-type|implementation
// @pinker-nav:symbol-doc pinker_v0::nav::CatalogError|development.symbol-index
// @pinker-nav:summary Rebuilds from the versioned JSONL the regions and their explicit symbol, documentation and test links; it serves mostrar/buscar/listar/mapa/localizar without rescanning the roots and, when extracting a region, it validates markers and hash, refusing drift.
/// Falha ao carregar o catálogo de código versionado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    Missing {
        path: String,
    },
    Invalid {
        path: String,
        line: usize,
        msg: String,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::Missing { path } => write!(
                f,
                "E-NAV-CATALOG\nCatálogo de código ausente: '{}'. Rode `pink nav sincronizar`.",
                path
            ),
            CatalogError::Invalid { path, line, msg } => write!(
                f,
                "E-NAV-CATALOG\nCatálogo de código inválido em '{}' (linha {}): {}. Rode `pink nav sincronizar`.",
                path, line, msg
            ),
        }
    }
}

/// Resultado da validação de uma região ao extrair conteúdo (§5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionCheck {
    Ok,
    /// Marcadores não delimitam mais o intervalo com a chave esperada.
    AnchorDrift,
    /// O conteúdo mudou: hash do catálogo diverge do hash recalculado.
    HashMismatch {
        expected: String,
        found: String,
    },
}

/// Catálogo de código em memória, reconstruído do JSONL versionado. As
/// consultas (`mostrar`, `listar`, `buscar`, `mapa`) usam esta superfície e não
/// revarrem as raízes de código controladas (§5).
#[derive(Debug, Clone, Default)]
pub struct CodeCatalog {
    pub regions: Vec<CodeRegion>,
}

impl CodeCatalog {
    pub fn load(path: &Path) -> Result<CodeCatalog, CatalogError> {
        let text = fs::read_to_string(path).map_err(|_| CatalogError::Missing {
            path: path.display().to_string(),
        })?;
        Self::parse(&text, &path.display().to_string())
    }

    pub fn parse(text: &str, path: &str) -> Result<CodeCatalog, CatalogError> {
        let mut catalog = CodeCatalog::default();
        for (idx, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let obj = jsonl::parse_object(line).map_err(|err| CatalogError::Invalid {
                path: path.to_string(),
                line: idx + 1,
                msg: err.msg,
            })?;
            let invalid = |msg: String| CatalogError::Invalid {
                path: path.to_string(),
                line: idx + 1,
                msg,
            };
            let schema = obj.get("schema").and_then(|v| v.as_int()).unwrap_or(0);
            if schema != 1 {
                return Err(invalid(format!("schema {} não suportado", schema)));
            }
            catalog.regions.push(parse_region_record(&obj, &invalid)?);
        }
        catalog
            .regions
            .sort_by(|a, b| a.key.cmp(&b.key).then(a.file.cmp(&b.file)));
        Ok(catalog)
    }

    pub fn region(&self, key: &str) -> Option<&CodeRegion> {
        self.regions.iter().find(|r| r.key == key)
    }

    pub fn search(&self, query: &str) -> Vec<&CodeRegion> {
        self.search_ranked(query)
            .into_iter()
            .map(|hit| hit.region)
            .collect()
    }

    /// Mesma ordenação de `search`, preservando a evidência de relevância para
    /// quem precisa mostrá-la ao agente.
    pub fn search_ranked(&self, query: &str) -> Vec<RegionMatch<'_>> {
        score_regions(&self.regions, query)
    }

    pub fn list(&self, selector: &str) -> Vec<&CodeRegion> {
        self.regions
            .iter()
            .filter(|r| {
                r.layer.as_deref() == Some(selector) || r.domain.as_deref() == Some(selector)
            })
            .collect()
    }

    /// Seleciona regiões para o mapa sem consultar as fontes. Um caminho de
    /// arquivo literal tem precedência; caso contrário, reutiliza a busca
    /// textual existente sem truncar ou escolher uma única ambiguidade.
    pub fn map_regions(&self, filter: Option<&str>) -> Vec<&CodeRegion> {
        let Some(filter) = filter else {
            return self.regions.iter().collect();
        };
        let exact: Vec<&CodeRegion> = self
            .regions
            .iter()
            .filter(|region| region.file == filter)
            .collect();
        if exact.is_empty() {
            self.search(filter)
        } else {
            exact
        }
    }
}

fn parse_region_record(
    obj: &jsonl::JsonObject,
    invalid: &impl Fn(String) -> CatalogError,
) -> Result<CodeRegion, CatalogError> {
    let req_str = |key: &str| -> Result<String, CatalogError> {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| invalid(format!("região sem campo '{}'", key)))
    };
    let req_int = |key: &str| -> Result<usize, CatalogError> {
        obj.get(key)
            .and_then(|v| v.as_int())
            .filter(|v| *v >= 0)
            .map(|v| v as usize)
            .ok_or_else(|| invalid(format!("região sem inteiro '{}'", key)))
    };
    let opt_str = |key: &str| obj.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let opt_list = |key: &str| -> Result<Vec<String>, CatalogError> {
        match obj.get(key) {
            Some(value) => value
                .as_str_array()
                .ok_or_else(|| invalid(format!("campo '{}' deve ser lista de strings", key))),
            None => Ok(Vec::new()),
        }
    };
    let symbols = opt_list("symbols")?
        .iter()
        .map(|value| {
            parse_symbol(value).ok_or_else(|| invalid("binding de símbolo inválido".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let symbol_docs = opt_list("symbol_docs")?
        .iter()
        .map(|value| {
            parse_symbol_doc(value)
                .ok_or_else(|| invalid("vínculo documental de símbolo inválido".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let related_symbols = opt_list("related_symbols")?
        .iter()
        .map(|value| {
            parse_symbol_identity(value)
                .ok_or_else(|| invalid("identidade relacionada inválida".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let test_for = opt_list("test_for")?
        .iter()
        .map(|value| {
            parse_symbol_identity(value)
                .ok_or_else(|| invalid("identidade de teste inválida".to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if has_duplicates(&symbols)
        || has_duplicates(&symbol_docs)
        || has_duplicates(&related_symbols)
        || has_duplicates(&test_for)
    {
        return Err(invalid(
            "metadado de símbolo duplicado no registro de região".to_string(),
        ));
    }
    Ok(CodeRegion {
        key: req_str("key")?,
        kind: opt_str("kind").unwrap_or_else(|| "region".to_string()),
        domain: opt_str("domain"),
        layer: opt_str("layer"),
        phase: obj.get("phase").and_then(|v| v.as_int()).map(|v| v as u64),
        file: req_str("file")?,
        start_marker: req_int("start_marker")?,
        content_start: req_int("content_start").unwrap_or(0),
        content_end: req_int("content_end").unwrap_or(0),
        end_marker: req_int("end_marker")?,
        summary: opt_str("summary").unwrap_or_default(),
        hash: opt_str("hash").unwrap_or_default(),
        status: opt_str("status").unwrap_or_else(|| "active".to_string()),
        symbols,
        related_symbols,
        test_for,
        symbol_docs,
    })
}

fn has_duplicates<T: Ord>(values: &[T]) -> bool {
    let mut seen = BTreeSet::new();
    values.iter().any(|value| !seen.insert(value))
}

/// Extrai as linhas de conteúdo de uma região a partir do texto-fonte atual,
/// no intervalo `[content_start, content_end]` (1-indexado, inclusivo).
pub fn extract_region_content(source: &str, region: &CodeRegion) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    if region.content_start == 0 || region.content_end == 0 || region.content_end > lines.len() {
        return Vec::new();
    }
    let start = region.content_start - 1;
    let end = region.content_end;
    lines[start..end].iter().map(|s| s.to_string()).collect()
}

/// Valida que os marcadores ainda delimitam o intervalo com a chave esperada e
/// que o hash do conteúdo continua igual ao registrado (§5).
pub fn validate_region(source: &str, region: &CodeRegion) -> RegionCheck {
    let lines: Vec<&str> = source.lines().collect();
    if region.start_marker == 0 || region.end_marker == 0 || region.end_marker > lines.len() {
        return RegionCheck::AnchorDrift;
    }
    let start_line = lines[region.start_marker - 1];
    let end_line = lines[region.end_marker - 1];
    let start_ok = parse_marker(start_line.trim(), START).as_deref() == Some(region.key.as_str());
    let end_ok = parse_marker(end_line.trim(), END).as_deref() == Some(region.key.as_str());
    if !start_ok || !end_ok {
        return RegionCheck::AnchorDrift;
    }
    // Recalcula o hash do conteúdo (linhas não-vazias do intervalo).
    let content = extract_region_content(source, region);
    let body: Vec<&str> = content
        .iter()
        .map(|s| s.as_str())
        .filter(|l| !l.trim().is_empty())
        .collect();
    let found = fnv1a64(&body.join("\n"));
    if !region.hash.is_empty() && found != region.hash {
        return RegionCheck::HashMismatch {
            expected: region.hash.clone(),
            found,
        };
    }
    RegionCheck::Ok
}
// @pinker-nav:end trama.code.query
// @pinker-nav:start evidence.navigation.catalog-scan
// @pinker-nav:domain navigation
// @pinker-nav:layer evidence
// @pinker-nav:summary Proofs of the navigation scan and catalog: multiple regions per file, preservation of domain and layer, determinism of the render, refusal of a marker inside a literal or a block comment, the distinction between a lifetime and a character literal, the strict metadata prefix and the two dialect grammars.

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_src(name: &str) -> PathBuf {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pinker_nav_{name}_{now}"))
    }

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    const SAMPLE: &str = "// @pinker-nav:start cfg.logic.short-circuit\n// @pinker-nav:domain logica\n// @pinker-nav:layer cfg\n// @pinker-nav:summary Curto-circuito.\nfn curto() {\n    let x = 1;\n}\n// @pinker-nav:end cfg.logic.short-circuit\n";

    #[test]
    fn scans_region_with_metadata() {
        let dir = temp_src("scan");
        write(&dir, "cfg_ir.rs", SAMPLE);
        let index = CodeIndex::scan(&dir).unwrap();
        assert_eq!(index.regions.len(), 1);
        let r = &index.regions[0];
        assert_eq!(r.key, "cfg.logic.short-circuit");
        assert_eq!(r.domain.as_deref(), Some("logica"));
        assert_eq!(r.layer.as_deref(), Some("cfg"));
        assert_eq!(r.file, "cfg_ir.rs");
        assert_eq!(r.start_marker, 1);
        assert_eq!(r.content_start, 5);
        assert_eq!(r.content_end, 7);
        assert_eq!(r.end_marker, 8);
        assert!(r.hash.starts_with("fnv1a64:"));
        assert!(index.verify().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn jsonl_deterministic() {
        let dir = temp_src("jsonl");
        write(&dir, "cfg_ir.rs", SAMPLE);
        let index = CodeIndex::scan(&dir).unwrap();
        assert_eq!(index.render_jsonl(), index.render_jsonl());
        assert!(index
            .render_jsonl()
            .contains("\"key\":\"cfg.logic.short-circuit\""));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn start_without_end_detected() {
        let dir = temp_src("noend");
        write(&dir, "a.rs", "// @pinker-nav:start x.y\nfn a() {}\n");
        let index = CodeIndex::scan(&dir).unwrap();
        assert!(index
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::StartWithoutEnd { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn key_mismatch_detected() {
        let dir = temp_src("mismatch");
        write(
            &dir,
            "a.rs",
            "// @pinker-nav:start x.y\nfn a() {}\n// @pinker-nav:end x.z\n",
        );
        let index = CodeIndex::scan(&dir).unwrap();
        assert!(index
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::KeyMismatch { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn overlap_detected() {
        let dir = temp_src("overlap");
        write(
            &dir,
            "a.rs",
            "// @pinker-nav:start a.b\nfn a() {\n// @pinker-nav:start c.d\nlet x=1;\n// @pinker-nav:end c.d\n}\n// @pinker-nav:end a.b\n",
        );
        let index = CodeIndex::scan(&dir).unwrap();
        assert!(index
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::Overlap { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_key_detected() {
        let dir = temp_src("badkey");
        write(
            &dir,
            "a.rs",
            "// @pinker-nav:start Bad_Key\nfn a() {}\n// @pinker-nav:end Bad_Key\n",
        );
        let index = CodeIndex::scan(&dir).unwrap();
        assert!(index
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::InvalidKey { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn valid_key_rules() {
        assert!(valid_key("parser.intrinsecos.resolucao"));
        assert!(valid_key("cfg.logic.short-circuit"));
        assert!(valid_key("a"));
        assert!(!valid_key(""));
        assert!(!valid_key(".a"));
        assert!(!valid_key("a."));
        assert!(!valid_key("a..b"));
        assert!(!valid_key("Ab"));
    }

    // -----------------------------------------------------------------
    // Raízes controladas (Onda 6D — §21).
    // -----------------------------------------------------------------

    fn region_src(key: &str) -> String {
        format!(
            "// @pinker-nav:start {key}\n// @pinker-nav:domain d\n// @pinker-nav:layer l\nfn f() {{ let _x = 1; }}\n// @pinker-nav:end {key}\n"
        )
    }

    #[test]
    fn duas_raizes_produzem_regioes_com_caminhos_corretos() {
        let dir = temp_src("tworoots");
        write(&dir, "src/a.rs", &region_src("root.um.chave"));
        write(
            &dir,
            "runtime/pinker_rt/src/lib.rs",
            &region_src("root.dois.chave"),
        );
        let roots = vec![
            ScanRoot::new("src", &["rs"]),
            ScanRoot::new("runtime/pinker_rt/src", &["rs"]),
        ];
        let index = scan_roots(&dir, &roots).unwrap();
        assert_eq!(index.regions.len(), 2);
        assert!(index.verify().is_empty(), "{:?}", index.verify());
        let files: Vec<&str> = index.regions.iter().map(|r| r.file.as_str()).collect();
        assert!(files.contains(&"src/a.rs"));
        assert!(files.contains(&"runtime/pinker_rt/src/lib.rs"));
        // Nenhum prefixo duplicado (ex.: "src/src/a.rs").
        assert!(files.iter().all(|f| !f.contains("src/src")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn chave_duplicada_entre_raizes_reporta_os_dois_arquivos() {
        let dir = temp_src("duproots");
        write(&dir, "src/a.rs", &region_src("mesma.chave.aqui"));
        write(
            &dir,
            "runtime/pinker_rt/src/lib.rs",
            &region_src("mesma.chave.aqui"),
        );
        let roots = vec![
            ScanRoot::new("src", &["rs"]),
            ScanRoot::new("runtime/pinker_rt/src", &["rs"]),
        ];
        let index = scan_roots(&dir, &roots).unwrap();
        let errors = index.verify();
        let dup = errors.iter().find_map(|e| match e {
            NavVerifyError::DuplicateKey { key, files } if key == "mesma.chave.aqui" => {
                Some(files.clone())
            }
            _ => None,
        });
        let files = dup.expect("chave duplicada deveria ser reportada");
        assert!(files.contains(&"src/a.rs".to_string()));
        assert!(files.contains(&"runtime/pinker_rt/src/lib.rs".to_string()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ordem_das_raizes_nao_altera_o_jsonl() {
        let dir = temp_src("orderroots");
        write(&dir, "src/a.rs", &region_src("ordem.um.chave"));
        write(
            &dir,
            "runtime/pinker_rt/src/lib.rs",
            &region_src("ordem.dois.chave"),
        );
        let a = ScanRoot::new("src", &["rs"]);
        let b = ScanRoot::new("runtime/pinker_rt/src", &["rs"]);
        let forward = scan_roots(&dir, &[a.clone(), b.clone()]).unwrap();
        let backward = scan_roots(&dir, &[b, a]).unwrap();
        assert_eq!(forward.render_jsonl(), backward.render_jsonl());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn apenas_extensao_permitida_entra_no_indice() {
        let dir = temp_src("extfilter");
        write(&dir, "a.rs", &region_src("ext.rs.chave"));
        write(&dir, "b.md", &region_src("ext.md.chave"));
        write(&dir, "c.txt", &region_src("ext.txt.chave"));
        let root = ScanRoot::new("", &["rs"]);
        let index = scan_roots(&dir, &[root]).unwrap();
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "ext.rs.chave");
        assert_eq!(index.regions[0].file, "a.rs");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn raiz_obrigatoria_ausente_falha_sem_indice_parcial() {
        let dir = temp_src("missingroot");
        write(&dir, "src/a.rs", &region_src("presente.aqui.chave"));
        // `runtime/pinker_rt/src` não existe: o fluxo oficial deve falhar.
        let err = CodeIndex::scan_repo(&dir).unwrap_err();
        assert!(matches!(err, ScanError::RootMissing { .. }));
        assert!(err.to_string().starts_with("E-NAV-SCAN"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn todas_as_raizes_sao_validadas_antes_de_qualquer_coleta() {
        let dir = temp_src("validatefirst");
        // Raiz A válida e com conteúdo; se a coleta ocorresse antes da
        // validação global, este arquivo apareceria no índice mesmo com a
        // raiz B ausente.
        write(&dir, "src/a.rs", &region_src("preordem.a.chave"));
        let roots = vec![
            ScanRoot::new("src", &["rs"]),
            ScanRoot::new("nao/existe", &["rs"]),
        ];

        // `resolve_and_validate_roots` isolado: só resolve/valida, nunca
        // coleta. Deve falhar em B sem ter tocado o conteúdo de A.
        let err = resolve_and_validate_roots(&dir, &roots).unwrap_err();
        match &err {
            ScanError::RootMissing { path } => assert!(path.ends_with("nao/existe")),
            other => panic!("esperava RootMissing para a raiz ausente, obtive {other:?}"),
        }

        // `scan_roots` (fluxo completo) reporta o mesmo erro e não produz
        // índice parcial com a região de A.
        let full_err = scan_roots(&dir, &roots).unwrap_err();
        assert!(matches!(full_err, ScanError::RootMissing { .. }));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn erro_de_metadados_diferente_de_notfound_vira_io() {
        let dir = temp_src("ionotfound");
        // Cria um arquivo regular e tenta validar um caminho "filho" dele:
        // `symlink_metadata` falha com `NotADirectory`/erro de I/O, não
        // `NotFound` — não deve ser confundido com raiz ausente.
        write(&dir, "arquivo_regular", "conteudo");
        let child_of_file = dir.join("arquivo_regular").join("filho");

        let err = validate_root(&child_of_file).unwrap_err();
        assert!(
            matches!(err, ScanError::Io { .. }),
            "esperava ScanError::Io, obtive {err:?}"
        );
        assert!(err.to_string().starts_with("E-NAV-SCAN"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn caminhos_sao_repo_relativos_com_barra_e_sem_fuga() {
        let dir = temp_src("pathshape");
        write(&dir, "src/sub/a.rs", &region_src("caminho.sub.chave"));
        let root = ScanRoot::new("src", &["rs"]);
        let index = scan_roots(&dir, &[root]).unwrap();
        assert_eq!(index.regions.len(), 1);
        let file = &index.regions[0].file;
        assert_eq!(file, "src/sub/a.rs");
        assert!(!file.starts_with('/'));
        assert!(!file.contains(".."));
        assert!(!file.contains('\\'));
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_nao_sao_seguidos_nem_catalogados() {
        use std::os::unix::fs::symlink;

        let dir = temp_src("symlinks");
        write(&dir, "src/real.rs", &region_src("symlink.real.chave"));
        // Fora da raiz: não deve ser alcançado nem seguido.
        write(&dir, "outside/fora.rs", &region_src("symlink.fora.chave"));

        // Symlink de diretório apontando para si mesmo (ciclo).
        let cycle_dir = dir.join("src/ciclo");
        symlink(&cycle_dir, &cycle_dir).ok();
        // Symlink de diretório apontando para fora da raiz.
        symlink(dir.join("outside"), dir.join("src/fuga")).unwrap();
        // Symlink de arquivo apontando para um arquivo real.
        symlink(dir.join("src/real.rs"), dir.join("src/link.rs")).unwrap();

        let root = ScanRoot::new("src", &["rs"]);
        let index = scan_roots(&dir, &[root]).expect("scanner deve terminar normalmente");
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "symlink.real.chave");
        assert_eq!(index.regions[0].file, "src/real.rs");
        fs::remove_dir_all(dir).unwrap();
    }

    // -----------------------------------------------------------------
    // Dialeto léxico Pinker e despacho por raiz (Onda 9).
    // -----------------------------------------------------------------

    /// Varre uma raiz única no dialeto Pinker (fixtures `.pink`).
    fn scan_pink(dir: &Path) -> CodeIndex {
        let root = ScanRoot::with_dialect("", &["pink"], MarkerDialect::Pinker);
        scan_roots(dir, &[root]).unwrap()
    }

    fn pink_region(key: &str, body: &str) -> String {
        format!(
            "// @pinker-nav:start {key}\n// @pinker-nav:domain d\n// @pinker-nav:layer apps\n{body}\n// @pinker-nav:end {key}\n"
        )
    }

    #[test]
    fn raizes_oficiais_incluem_apps_com_dialeto_pinker() {
        let roots = official_scan_roots();
        assert_eq!(roots.len(), 4);
        let apps = roots
            .iter()
            .find(|r| r.relative_path == "apps")
            .expect("raiz apps é obrigatória");
        assert_eq!(apps.extensions, vec!["pink".to_string()]);
        assert_eq!(apps.dialect, MarkerDialect::Pinker);
        // As demais raízes permanecem Rust/`.rs`.
        for r in roots.iter().filter(|r| r.relative_path != "apps") {
            assert_eq!(r.dialect, MarkerDialect::Rust);
            assert_eq!(r.extensions, vec!["rs".to_string()]);
        }
    }

    #[test]
    fn raiz_apps_ausente_e_fatal_no_fluxo_oficial() {
        let dir = temp_src("noapps");
        write(&dir, "src/a.rs", &region_src("presente.no.src"));
        write(
            &dir,
            "runtime/pinker_rt/src/lib.rs",
            &region_src("presente.no.runtime"),
        );
        write(&dir, "tests/t.rs", &region_src("presente.nos.testes"));
        // `apps/` não existe: o fluxo oficial deve falhar sem índice parcial.
        let err = CodeIndex::scan_repo(&dir).unwrap_err();
        assert!(matches!(err, ScanError::RootMissing { .. }));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dialeto_pinker_reconhece_regiao_valida() {
        let dir = temp_src("pinkvalid");
        write(
            &dir,
            "guardiao.pink",
            &pink_region("apps.guardiao.exemplo", "carinho f() -> bombom { mimo 0; }"),
        );
        let index = scan_pink(&dir);
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "apps.guardiao.exemplo");
        assert_eq!(index.regions[0].layer.as_deref(), Some("apps"));
        assert!(index.verify().is_empty(), "{:?}", index.verify());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn raiz_apps_aceita_apenas_pink_e_ignora_rs() {
        let dir = temp_src("appsonlypink");
        write(
            &dir,
            "guardiao.pink",
            &pink_region("apps.real.chave", "carinho f() -> bombom { mimo 0; }"),
        );
        // Um `.rs` dentro da raiz Pinker não deve ser catalogado.
        write(&dir, "intruso.rs", &region_src("apps.intruso.rs"));
        let index = scan_pink(&dir);
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "apps.real.chave");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn raiz_rust_ignora_arquivos_pink() {
        let dir = temp_src("rustignorespink");
        write(&dir, "a.rs", &region_src("rust.real.chave"));
        write(
            &dir,
            "b.pink",
            &pink_region("pink.ignorada.chave", "carinho f() -> bombom { mimo 0; }"),
        );
        let root = ScanRoot::new("", &["rs"]);
        let index = scan_roots(&dir, &[root]).unwrap();
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "rust.real.chave");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn chave_duplicada_entre_rs_e_pink_e_reportada() {
        let dir = temp_src("dupacrossdialects");
        write(&dir, "src/a.rs", &region_src("mesma.chave.global"));
        write(
            &dir,
            "apps/b.pink",
            &pink_region("mesma.chave.global", "carinho f() -> bombom { mimo 0; }"),
        );
        let roots = vec![
            ScanRoot::new("src", &["rs"]),
            ScanRoot::with_dialect("apps", &["pink"], MarkerDialect::Pinker),
        ];
        let index = scan_roots(&dir, &roots).unwrap();
        let dup = index.verify().into_iter().find_map(|e| match e {
            NavVerifyError::DuplicateKey { key, files } if key == "mesma.chave.global" => {
                Some(files)
            }
            _ => None,
        });
        let files = dup.expect("chave global duplicada entre `.rs` e `.pink`");
        assert!(files.contains(&"src/a.rs".to_string()));
        assert!(files.contains(&"apps/b.pink".to_string()));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ordem_das_raizes_pink_e_rust_nao_altera_o_jsonl() {
        let dir = temp_src("orderpink");
        write(&dir, "src/a.rs", &region_src("ordem.rs.chave"));
        write(
            &dir,
            "apps/b.pink",
            &pink_region("ordem.pink.chave", "carinho f() -> bombom { mimo 0; }"),
        );
        let a = ScanRoot::new("src", &["rs"]);
        let b = ScanRoot::with_dialect("apps", &["pink"], MarkerDialect::Pinker);
        let forward = scan_roots(&dir, &[a.clone(), b.clone()]).unwrap();
        let backward = scan_roots(&dir, &[b, a]).unwrap();
        assert_eq!(forward.render_jsonl(), backward.render_jsonl());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn marcador_dentro_de_string_normal_pink_e_ignorado() {
        let dir = temp_src("pinknormstr");
        let body = "nova s: verso = \"// @pinker-nav:start fake.normal.chave\";";
        write(&dir, "a.pink", &pink_region("apps.strnormal.chave", body));
        let index = scan_pink(&dir);
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "apps.strnormal.chave");
        assert!(index.verify().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn marcador_dentro_de_string_tripla_pink_e_ignorado() {
        let dir = temp_src("pinktriplestr");
        let body = "nova s: verso = \"\"\"\n// @pinker-nav:start fake.tripla.chave\n\"\"\";";
        write(&dir, "a.pink", &pink_region("apps.strtripla.chave", body));
        let index = scan_pink(&dir);
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "apps.strtripla.chave");
        assert!(index.verify().is_empty(), "{:?}", index.verify());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn marcador_dentro_de_comentario_de_bloco_pink_e_ignorado() {
        let dir = temp_src("pinkblock");
        let body = "/* /* aninhado\n// @pinker-nav:start fake.bloco.chave\n*/ */\ncarinho f() -> bombom { mimo 0; }";
        write(&dir, "a.pink", &pink_region("apps.bloco.chave", body));
        let index = scan_pink(&dir);
        assert_eq!(index.regions.len(), 1);
        assert_eq!(index.regions[0].key, "apps.bloco.chave");
        assert!(index.verify().is_empty(), "{:?}", index.verify());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn erros_estruturais_pink_sao_reportados() {
        // Start sem end.
        let dir = temp_src("pinknoend");
        write(
            &dir,
            "a.pink",
            "// @pinker-nav:start apps.x.y\ncarinho f() {}\n",
        );
        assert!(scan_pink(&dir)
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::StartWithoutEnd { .. })));
        fs::remove_dir_all(dir).unwrap();

        // End sem start.
        let dir = temp_src("pinknostart");
        write(
            &dir,
            "a.pink",
            "carinho f() {}\n// @pinker-nav:end apps.x.y\n",
        );
        assert!(scan_pink(&dir)
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::EndWithoutStart { .. })));
        fs::remove_dir_all(dir).unwrap();

        // Chave/marcador incoerente.
        let dir = temp_src("pinkmismatch");
        write(
            &dir,
            "a.pink",
            "// @pinker-nav:start apps.x.y\ncarinho f() {}\n// @pinker-nav:end apps.x.z\n",
        );
        assert!(scan_pink(&dir)
            .verify()
            .iter()
            .any(|e| matches!(e, NavVerifyError::KeyMismatch { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    // Reconhecimento léxico puro do dialeto Pinker (`marker_comment_pinker`).

    #[test]
    fn pink_marcador_valido_e_reconhecido() {
        let mut st = PinkerState::Code;
        assert_eq!(
            marker_comment_pinker("// @pinker-nav:start a.b", &mut st),
            Some("// @pinker-nav:start a.b")
        );
        assert_eq!(st, PinkerState::Code);
    }

    #[test]
    fn pink_espacos_antes_do_marcador_sao_permitidos() {
        let mut st = PinkerState::Code;
        assert_eq!(
            marker_comment_pinker("    // @pinker-nav:end a.b", &mut st),
            Some("// @pinker-nav:end a.b")
        );
    }

    #[test]
    fn pink_marcador_apos_codigo_e_ignorado() {
        let mut st = PinkerState::Code;
        assert_eq!(
            marker_comment_pinker("carinho f() {} // @pinker-nav:start a.b", &mut st),
            None
        );
    }

    #[test]
    fn pink_string_normal_nao_atravessa_linha() {
        let mut st = PinkerState::Code;
        // String normal aberta e não fechada no fim da linha volta a `Code`.
        assert_eq!(
            marker_comment_pinker("nova s: verso = \"aberta", &mut st),
            None
        );
        assert_eq!(st, PinkerState::Code);
        // Logo, um marcador na linha seguinte é reconhecido normalmente.
        assert_eq!(
            marker_comment_pinker("// @pinker-nav:start a.b", &mut st),
            Some("// @pinker-nav:start a.b")
        );
    }

    #[test]
    fn pink_marcador_dentro_de_string_normal_fechada_e_ignorado() {
        let mut st = PinkerState::Code;
        assert_eq!(
            marker_comment_pinker("\"// @pinker-nav:start a.b\"", &mut st),
            None
        );
        assert_eq!(st, PinkerState::Code);
    }

    #[test]
    fn pink_string_interpolada_ignora_marcador() {
        let mut st = PinkerState::Code;
        assert_eq!(
            marker_comment_pinker("$\"// @pinker-nav:start a.b\"", &mut st),
            None
        );
        assert_eq!(st, PinkerState::Code);
    }

    #[test]
    fn pink_string_tripla_atravessa_linhas_e_ignora_marcador() {
        let mut st = PinkerState::Code;
        assert_eq!(marker_comment_pinker("txt = \"\"\"", &mut st), None);
        assert_eq!(st, PinkerState::TripleString);
        assert_eq!(
            marker_comment_pinker("// @pinker-nav:start a.b", &mut st),
            None
        );
        assert_eq!(st, PinkerState::TripleString);
        assert_eq!(marker_comment_pinker("\"\"\";", &mut st), None);
        assert_eq!(st, PinkerState::Code);
    }

    #[test]
    fn pink_comentario_de_bloco_aninhado_ignora_marcador() {
        let mut st = PinkerState::Code;
        assert_eq!(marker_comment_pinker("/* /* aninhado", &mut st), None);
        assert_eq!(st, PinkerState::BlockComment(2));
        assert_eq!(
            marker_comment_pinker("// @pinker-nav:start a.b", &mut st),
            None
        );
        assert_eq!(st, PinkerState::BlockComment(2));
        assert_eq!(marker_comment_pinker("*/", &mut st), None);
        assert_eq!(st, PinkerState::BlockComment(1));
        assert_eq!(marker_comment_pinker("*/", &mut st), None);
        assert_eq!(st, PinkerState::Code);
    }

    #[test]
    fn pink_variante_doc_comment_nao_e_marcador() {
        // `///` é comentário candidato, mas `parse_marker` rejeita a variante.
        let mut st = PinkerState::Code;
        let candidate = marker_comment_pinker("/// @pinker-nav:start a.b", &mut st);
        assert_eq!(candidate, Some("/// @pinker-nav:start a.b"));
        assert_eq!(parse_marker(candidate.unwrap(), START), None);
    }
}
// @pinker-nav:end evidence.navigation.catalog-scan
