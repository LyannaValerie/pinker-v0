//! Trama Pinker — Etapa 3 (Navegação semântica do código).
//!
//! Varre um conjunto de raízes de código controladas do repositório (Onda 6D:
//! `src/` e `runtime/pinker_rt/src/`, ambas obrigatórias no fluxo oficial) em
//! busca dos marcadores `@pinker-nav:start/end` e gera o catálogo derivado
//! `src/navigation.jsonl` (especificação, seções 10, 11, 12-17 e 22). O
//! agente que altera o código mantém os marcadores; o script nunca decide
//! semanticamente onde inseri-los. Zero dependências externas.

use crate::jsonl;
use crate::text_norm;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
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
}

/// Índice de código em memória.
#[derive(Debug, Clone, Default)]
pub struct CodeIndex {
    pub regions: Vec<CodeRegion>,
    pub scan_problems: Vec<NavVerifyError>,
}

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
            NavVerifyError::IndexOutOfDate { path } => write!(
                f,
                "catálogo '{}' dessincronizado; rode `pink nav sincronizar`",
                path
            ),
        }
    }
}

// @pinker-nav:start trama.codigo.catalogo
// @pinker-nav:domain navegacao
// @pinker-nav:layer trama
// @pinker-nav:summary Gera o catálogo de navegação de código varrendo as raízes controladas do repositório (§ trama.codigo.raizes) pelos marcadores `@pinker-nav`: monta as regiões, calcula o hash do conteúdo, renderiza JSONL determinístico e valida chaves únicas, marcadores balanceados e ausência de sobreposição.
impl CodeIndex {
    /// Varre uma única raiz (uso em fixtures/testes; compatibilidade
    /// histórica). Delega à varredura multi-raiz (§ trama.codigo.raizes)
    /// tratando `src_root` como a raiz recebida diretamente: o caminho
    /// resultante é relativo a `src_root`, sem prefixo fabricado.
    pub fn scan(src_root: &Path) -> Result<CodeIndex, ScanError> {
        let root = ScanRoot::new("", &["rs"]);
        scan_roots(src_root, std::slice::from_ref(&root))
    }

    /// Varre o repositório real usando as raízes de código oficiais e
    /// obrigatórias (§15): `src/` e `runtime/pinker_rt/src/`. Usado pelo
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
        errors
    }

    pub fn region(&self, key: &str) -> Option<&CodeRegion> {
        self.regions.iter().find(|r| r.key == key)
    }

    /// Busca por chave, domínio, camada, resumo e caminho (prioridade §7.3).
    pub fn search(&self, query: &str) -> Vec<&CodeRegion> {
        let scored = score_regions(&self.regions, query);
        scored.into_iter().map(|(r, _, _)| r).collect()
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
// @pinker-nav:end trama.codigo.catalogo

/// Pontuação de código (§7.3). Prioridade mínima: chave exata, chave parcial,
/// domínio/camada exatos, termos no resumo, caminho. Devolve
/// `(região, pontuação, cobertura)` ordenado por (pontuação, cobertura, chave).
fn score_regions<'a>(regions: &'a [CodeRegion], query: &str) -> Vec<(&'a CodeRegion, u32, usize)> {
    let q_norm = text_norm::normalize(query);
    let terms = text_norm::terms(query);
    let mut hits: Vec<(&CodeRegion, u32, usize)> = Vec::new();
    for region in regions {
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
        let summary_norm = text_norm::normalize(&region.summary);
        let file_norm = text_norm::normalize(&region.file);

        let mut score = 0u32;
        if key_norm == q_norm {
            score += 100;
        } else if key_norm.contains(&q_norm) {
            score += 60;
        }
        if domain_norm == q_norm || layer_norm == q_norm {
            score += 40;
        }
        if covers(&summary_norm, &terms) {
            score += 20;
        }
        if covers(&file_norm, &terms) || file_norm.contains(&q_norm) {
            score += 10;
        }

        let haystack = format!(
            "{} {} {} {} {}",
            key_norm, domain_norm, layer_norm, summary_norm, file_norm
        );
        let coverage = terms
            .iter()
            .filter(|t| haystack.split(' ').any(|w| w == t.as_str()))
            .count();

        if score > 0 {
            hits.push((region, score, coverage));
        }
    }
    hits.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(b.2.cmp(&a.2))
            .then(a.0.key.cmp(&b.0.key))
    });
    hits
}

fn covers(haystack_norm: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let words: Vec<&str> = haystack_norm.split(' ').filter(|w| !w.is_empty()).collect();
    let matched = terms
        .iter()
        .filter(|t| words.iter().any(|w| *w == t.as_str()))
        .count();
    matched >= terms.len().div_ceil(2)
}

// ---------------------------------------------------------------------------
// Raízes controladas de código (Onda 6D — especificação §12 a 17 e 20).
// ---------------------------------------------------------------------------

// @pinker-nav:start trama.codigo.raizes
// @pinker-nav:domain navegacao
// @pinker-nav:layer trama
// @pinker-nav:summary Define as raízes de código controladas do repositório (`src/` e `runtime/pinker_rt/src/`, ambas obrigatórias), valida cada raiz antes de varrer — ausência, caminho que não é diretório ou link simbólico falham com `E-NAV-SCAN` antes de qualquer escrita —, coleta arquivos por extensão sem seguir links simbólicos e normaliza cada caminho para a forma repo-relativa com `/`, sem prefixo fabricado, garantindo que cada arquivo seja varrido no máximo uma vez, independente da ordem das raízes.
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
}

impl ScanRoot {
    pub fn new(relative_path: impl Into<String>, extensions: &[&str]) -> ScanRoot {
        ScanRoot {
            relative_path: relative_path.into(),
            extensions: extensions.iter().map(|e| e.to_string()).collect(),
        }
    }
}

/// As raízes oficiais varridas pelo fluxo `pink nav sincronizar`/`verificar`
/// (§15): ambas obrigatórias. Fonte única da política de raízes — a CLI
/// oficial e os testes que exercitam o caminho oficial usam esta mesma lista;
/// a API genérica [`scan_roots`] (via [`CodeIndex::scan`]) aceita listas
/// menores para fixtures.
pub fn official_scan_roots() -> Vec<ScanRoot> {
    vec![
        ScanRoot::new("src", &["rs"]),
        ScanRoot::new("runtime/pinker_rt/src", &["rs"]),
    ]
}

/// Varre múltiplas raízes controladas relativas a `repo_root` (§12-17). Cada
/// raiz é validada antes de qualquer leitura (§15): ausente, não-diretório ou
/// link simbólico falham imediatamente com `ScanError`, sem índice parcial —
/// nada é escrito até que todas as raízes sejam válidas. Os arquivos de todas
/// as raízes são combinados, deduplicados por caminho repo-relativo e
/// ordenados antes de varrer, então cada arquivo é lido e catalogado no
/// máximo uma vez, independente da ordem em que as raízes foram declaradas
/// (§17). A chave de região continua global: nenhuma raiz vira namespace.
fn scan_roots(repo_root: &Path, roots: &[ScanRoot]) -> Result<CodeIndex, ScanError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut files: Vec<(String, PathBuf)> = Vec::new();

    for root in roots {
        let abs_root = if root.relative_path.is_empty() {
            repo_root.to_path_buf()
        } else {
            repo_root.join(&root.relative_path)
        };
        validate_root(&abs_root)?;
        let mut collected = Vec::new();
        collect_source_files(&abs_root, &abs_root, &root.extensions, &mut collected)?;
        for rel in collected {
            let display = compose_display_path(&root.relative_path, &rel);
            if seen.insert(display.clone()) {
                files.push((display, abs_root.join(&rel)));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut index = CodeIndex::default();
    for (display, abs_path) in files {
        let text = fs::read_to_string(&abs_path).map_err(|err| ScanError::Io {
            path: abs_path.display().to_string(),
            msg: err.to_string(),
        })?;
        scan_file(&display, &text, &mut index);
    }
    index
        .regions
        .sort_by(|a, b| a.key.cmp(&b.key).then(a.file.cmp(&b.file)));
    Ok(index)
}

/// Valida uma raiz antes de varrê-la (§15): deve existir, ser diretório e não
/// ser um link simbólico (política explícita de segurança contra fuga/ciclo
/// via symlink de raiz; §14). Falha antes de qualquer leitura de arquivo.
fn validate_root(path: &Path) -> Result<(), ScanError> {
    let meta = fs::symlink_metadata(path).map_err(|_| ScanError::RootMissing {
        path: path.display().to_string(),
    })?;
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
// @pinker-nav:end trama.codigo.raizes

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
}

fn scan_file(rel_path: &str, text: &str, index: &mut CodeIndex) {
    let lines: Vec<&str> = text.lines().collect();
    let mut stack: Vec<OpenRegion> = Vec::new();

    for (i, raw) in lines.iter().enumerate() {
        let line_no = i + 1;
        let trimmed = raw.trim();

        if let Some(key) = parse_marker(trimmed, START) {
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
            });
            continue;
        }

        if let Some(end_key) = parse_marker(trimmed, END) {
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
                if let Some((field, value)) = parse_meta(trimmed) {
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
    });
}

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
        other => index.scan_problems.push(NavVerifyError::MalformedMeta {
            key: open.key.clone(),
            file: rel_path.to_string(),
            field: other.to_string(),
        }),
    }
}

/// Extrai a chave após um marcador (`@pinker-nav:start`/`:end`) em uma linha
/// que deve ser um comentário `//`.
fn parse_marker(trimmed: &str, marker: &str) -> Option<String> {
    if !trimmed.starts_with("//") {
        return None;
    }
    let idx = trimmed.find(marker)?;
    // Evita casar `@pinker-nav:start` quando o buscado é `@pinker-nav:end`
    // (ambos contêm `@pinker-nav:`), garantindo limite após o marcador.
    let after = &trimmed[idx + marker.len()..];
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
    if !trimmed.starts_with("//") {
        return None;
    }
    let idx = trimmed.find(FIELD_PREFIX)?;
    let rest = &trimmed[idx + FIELD_PREFIX.len()..];
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

fn fnv1a64(data: &str) -> String {
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
    out.push('}');
    out
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

// ---------------------------------------------------------------------------
// Catálogo carregado do JSONL (superfície de consulta — §5).
// ---------------------------------------------------------------------------

// @pinker-nav:start trama.codigo.consulta
// @pinker-nav:domain navegacao
// @pinker-nav:layer trama
// @pinker-nav:summary Reconstrói o catálogo de código do JSONL versionado e serve as consultas (`mostrar`/`buscar`/`listar`) a partir das fontes já catalogadas, sem revarrer as raízes de código controladas; ao extrair uma região, valida que os marcadores ainda a delimitam e que o hash do conteúdo confere, recusando drift.
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
/// consultas (`mostrar`, `listar`, `buscar`) usam esta superfície e não
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
        score_regions(&self.regions, query)
            .into_iter()
            .map(|(r, _, _)| r)
            .collect()
    }

    pub fn list(&self, selector: &str) -> Vec<&CodeRegion> {
        self.regions
            .iter()
            .filter(|r| {
                r.layer.as_deref() == Some(selector) || r.domain.as_deref() == Some(selector)
            })
            .collect()
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
    })
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
// @pinker-nav:end trama.codigo.consulta

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

    const SAMPLE: &str = "// @pinker-nav:start cfg.logica.curto-circuito\n// @pinker-nav:domain logica\n// @pinker-nav:layer cfg\n// @pinker-nav:summary Curto-circuito.\nfn curto() {\n    let x = 1;\n}\n// @pinker-nav:end cfg.logica.curto-circuito\n";

    #[test]
    fn scans_region_with_metadata() {
        let dir = temp_src("scan");
        write(&dir, "cfg_ir.rs", SAMPLE);
        let index = CodeIndex::scan(&dir).unwrap();
        assert_eq!(index.regions.len(), 1);
        let r = &index.regions[0];
        assert_eq!(r.key, "cfg.logica.curto-circuito");
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
            .contains("\"key\":\"cfg.logica.curto-circuito\""));
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
        assert!(valid_key("cfg.logica.curto-circuito"));
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
}
