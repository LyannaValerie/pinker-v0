//! Trama Pinker — Etapa 3 (Navegação semântica do código).
//!
//! Varre `src/` em busca dos marcadores `@pinker-nav:start/end` e gera o
//! catálogo derivado `src/navigation.jsonl` (especificação, seções 10, 11 e 22).
//! O agente que altera o código mantém os marcadores; o script nunca decide
//! semanticamente onde inseri-los. Zero dependências externas.

use std::collections::BTreeMap;
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
    Io { path: String, msg: String },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io { path, msg } => {
                write!(f, "E-NAV-SCAN\nFalha ao ler '{}': {}", path, msg)
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

impl CodeIndex {
    /// Varre recursivamente `src_root` e constrói o índice.
    pub fn scan(src_root: &Path) -> Result<CodeIndex, ScanError> {
        if !src_root.exists() {
            return Ok(CodeIndex::default());
        }
        let mut files = Vec::new();
        collect_rust(src_root, &mut files)?;
        files.sort();

        let mut index = CodeIndex::default();
        for file in files {
            let text = fs::read_to_string(&file).map_err(|err| ScanError::Io {
                path: file.display().to_string(),
                msg: err.to_string(),
            })?;
            let rel = relative_display(src_root, &file);
            scan_file(&rel, &text, &mut index);
        }
        index
            .regions
            .sort_by(|a, b| a.key.cmp(&b.key).then(a.file.cmp(&b.file)));
        Ok(index)
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

    /// Busca por chave, domínio, camada e resumo.
    pub fn search(&self, query: &str) -> Vec<&CodeRegion> {
        let needle = query.to_lowercase();
        let mut hits: Vec<(&CodeRegion, u32)> = Vec::new();
        for region in &self.regions {
            let mut score = 0u32;
            if region.key.to_lowercase() == needle {
                score += 100;
            } else if region.key.to_lowercase().contains(&needle) {
                score += 40;
            }
            if region
                .domain
                .as_deref()
                .map(|d| d.to_lowercase().contains(&needle))
                == Some(true)
            {
                score += 20;
            }
            if region
                .layer
                .as_deref()
                .map(|l| l.to_lowercase().contains(&needle))
                == Some(true)
            {
                score += 15;
            }
            if region.summary.to_lowercase().contains(&needle) {
                score += 10;
            }
            if score > 0 {
                hits.push((region, score));
            }
        }
        hits.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.key.cmp(&b.0.key)));
        hits.into_iter().map(|(r, _)| r).collect()
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

fn collect_rust(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ScanError> {
    let entries = fs::read_dir(dir).map_err(|err| ScanError::Io {
        path: dir.display().to_string(),
        msg: err.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| ScanError::Io {
            path: dir.display().to_string(),
            msg: err.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn relative_display(root: &Path, file: &Path) -> String {
    let stripped = file.strip_prefix(root).unwrap_or(file);
    format!("src/{}", stripped.display()).replace('\\', "/")
}

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
        assert_eq!(r.file, "src/cfg_ir.rs");
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
}
