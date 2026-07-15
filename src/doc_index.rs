//! Trama Pinker — Etapa 2 (Catálogo documental).
//!
//! Varre a árvore `docs/`, lê o frontmatter dos documentos estruturais e as
//! âncoras de seção `@pinker-doc:start/end`, e produz o catálogo derivado
//! `docs/navigation.jsonl` (especificação, seções 7, 8, 9 e 21).
//!
//! O catálogo é totalmente gerado, ordenado de forma determinística por `id` e
//! nunca editado à mão. Zero dependências externas, coerente com o compilador.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Raízes sentinela aceitas como `parent` sem serem documentos concretos.
const ROOT_PARENTS: &[&str] = &["atlas", "root"];

/// Campos de frontmatter obrigatórios para documentos estruturais (§7).
const REQUIRED_FRONTMATTER: &[&str] = &["id", "domain", "kind", "status", "parent"];

/// Valor mínimo de YAML: escalar ou lista de strings.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Yaml {
    Scalar(String),
    List(Vec<String>),
}

/// Um documento estrutural (arquivo `.md` com frontmatter `pinker-doc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocDocument {
    pub id: String,
    pub domain: String,
    pub kind: String,
    pub status: String,
    pub parent: Option<String>,
    pub file: String,
    pub title: String,
    pub canonical_for: Vec<String>,
    pub related: Vec<String>,
    pub missing_fields: Vec<String>,
}

/// Uma seção catalogada (par de âncoras `@pinker-doc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocSection {
    pub id: String,
    pub document: String,
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub title: String,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub summary: String,
}

/// Índice documental em memória.
#[derive(Debug, Clone, Default)]
pub struct DocIndex {
    pub documents: Vec<DocDocument>,
    pub sections: Vec<DocSection>,
    /// Problemas estruturais detectados já na varredura (âncoras desbalanceadas).
    pub scan_problems: Vec<DocVerifyError>,
}

/// Falha ao acessar a árvore de documentos.
#[derive(Debug)]
pub enum ScanError {
    Io { path: String, msg: String },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io { path, msg } => {
                write!(f, "E-DOC-SCAN\nFalha ao ler '{}': {}", path, msg)
            }
        }
    }
}

/// Divergências de validação documental (§21, subconjunto de catálogo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocVerifyError {
    DuplicateSectionId {
        id: String,
        files: Vec<String>,
    },
    AnchorStartWithoutEnd {
        id: String,
        file: String,
        line: usize,
    },
    AnchorEndWithoutStart {
        id: String,
        file: String,
        line: usize,
    },
    AnchorIdMismatch {
        start_id: String,
        end_id: String,
        file: String,
    },
    MissingFrontmatterField {
        file: String,
        field: String,
    },
    ParentNotFound {
        doc: String,
        parent: String,
    },
    RelatedNotFound {
        doc: String,
        related: String,
    },
    DuplicateCanonicalAuthority {
        concept: String,
        docs: Vec<String>,
    },
    StructuralDocWithoutPortal {
        doc: String,
        domain: String,
    },
    CatalogOutOfDate {
        path: String,
    },
}

impl fmt::Display for DocVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocVerifyError::DuplicateSectionId { id, files } => {
                write!(f, "id de seção duplicado '{}' em: {}", id, files.join(", "))
            }
            DocVerifyError::AnchorStartWithoutEnd { id, file, line } => write!(
                f,
                "âncora '{}' aberta sem fechamento em {}:{}",
                id, file, line
            ),
            DocVerifyError::AnchorEndWithoutStart { id, file, line } => write!(
                f,
                "âncora '{}' fechada sem abertura em {}:{}",
                id, file, line
            ),
            DocVerifyError::AnchorIdMismatch {
                start_id,
                end_id,
                file,
            } => write!(
                f,
                "par de âncora divergente em {}: início '{}' vs fim '{}'",
                file, start_id, end_id
            ),
            DocVerifyError::MissingFrontmatterField { file, field } => write!(
                f,
                "documento estrutural sem campo obrigatório '{}': {}",
                field, file
            ),
            DocVerifyError::ParentNotFound { doc, parent } => write!(
                f,
                "documento '{}' aponta parent inexistente '{}'",
                doc, parent
            ),
            DocVerifyError::RelatedNotFound { doc, related } => write!(
                f,
                "documento '{}' referencia related inexistente '{}'",
                doc, related
            ),
            DocVerifyError::DuplicateCanonicalAuthority { concept, docs } => write!(
                f,
                "conceito '{}' com autoridade duplicada em: {}",
                concept,
                docs.join(", ")
            ),
            DocVerifyError::StructuralDocWithoutPortal { doc, domain } => write!(
                f,
                "documento estrutural '{}' no território '{}' sem portal (README kind: portal)",
                doc, domain
            ),
            DocVerifyError::CatalogOutOfDate { path } => write!(
                f,
                "catálogo '{}' desatualizado ou editado à mão; rode `pink doc sincronizar`",
                path
            ),
        }
    }
}

impl DocIndex {
    /// Varre recursivamente `docs_root` e constrói o índice em memória.
    pub fn scan(docs_root: &Path) -> Result<DocIndex, ScanError> {
        let mut files = Vec::new();
        collect_markdown(docs_root, &mut files)?;
        files.sort();

        let mut index = DocIndex::default();
        for file in files {
            let text = fs::read_to_string(&file).map_err(|err| ScanError::Io {
                path: file.display().to_string(),
                msg: err.to_string(),
            })?;
            let rel = relative_display(docs_root, &file);
            scan_file(&rel, &text, &mut index);
        }

        index.documents.sort_by(|a, b| a.id.cmp(&b.id));
        index
            .sections
            .sort_by(|a, b| a.id.cmp(&b.id).then(a.file.cmp(&b.file)));
        Ok(index)
    }

    /// Serializa as seções em JSONL determinístico (ordenado por `id`).
    pub fn render_jsonl(&self) -> String {
        let mut sorted = self.sections.clone();
        sorted.sort_by(|a, b| a.id.cmp(&b.id).then(a.file.cmp(&b.file)));
        let mut out = String::new();
        for section in &sorted {
            out.push_str(&render_section_json(section));
            out.push('\n');
        }
        out
    }

    /// Executa as validações documentais de catálogo (§21, subconjunto).
    pub fn verify(&self) -> Vec<DocVerifyError> {
        let mut errors = self.scan_problems.clone();

        // 1. IDs de seção duplicados.
        let mut by_id: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for section in &self.sections {
            by_id
                .entry(&section.id)
                .or_default()
                .push(section.file.clone());
        }
        for (id, files) in by_id {
            if files.len() > 1 {
                errors.push(DocVerifyError::DuplicateSectionId {
                    id: id.to_string(),
                    files,
                });
            }
        }

        let doc_ids: HashSet<&str> = self.documents.iter().map(|d| d.id.as_str()).collect();
        let section_ids: HashSet<&str> = self.sections.iter().map(|s| s.id.as_str()).collect();
        let mut concepts: HashSet<&str> = HashSet::new();
        for doc in &self.documents {
            for concept in &doc.canonical_for {
                concepts.insert(concept.as_str());
            }
        }

        // 2. Campos obrigatórios de frontmatter.
        for doc in &self.documents {
            for field in &doc.missing_fields {
                errors.push(DocVerifyError::MissingFrontmatterField {
                    file: doc.file.clone(),
                    field: field.clone(),
                });
            }
        }

        // 3. parent e related existentes.
        for doc in &self.documents {
            if let Some(parent) = &doc.parent {
                let ok =
                    doc_ids.contains(parent.as_str()) || ROOT_PARENTS.contains(&parent.as_str());
                if !ok {
                    errors.push(DocVerifyError::ParentNotFound {
                        doc: doc.id.clone(),
                        parent: parent.clone(),
                    });
                }
            }
            for related in &doc.related {
                let ok = doc_ids.contains(related.as_str())
                    || section_ids.contains(related.as_str())
                    || concepts.contains(related.as_str());
                if !ok {
                    errors.push(DocVerifyError::RelatedNotFound {
                        doc: doc.id.clone(),
                        related: related.clone(),
                    });
                }
            }
        }

        // 4. Autoridade canônica duplicada.
        let mut authorities: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for doc in &self.documents {
            for concept in &doc.canonical_for {
                authorities.entry(concept).or_default().push(doc.id.clone());
            }
        }
        for (concept, docs) in authorities {
            if docs.len() > 1 {
                errors.push(DocVerifyError::DuplicateCanonicalAuthority {
                    concept: concept.to_string(),
                    docs,
                });
            }
        }

        // 5. Documento estrutural em território sem portal.
        let portal_domains: HashSet<&str> = self
            .documents
            .iter()
            .filter(|d| d.kind == "portal")
            .map(|d| d.domain.as_str())
            .collect();
        for doc in &self.documents {
            if doc.kind != "portal" && !portal_domains.contains(doc.domain.as_str()) {
                errors.push(DocVerifyError::StructuralDocWithoutPortal {
                    doc: doc.id.clone(),
                    domain: doc.domain.clone(),
                });
            }
        }

        errors
    }

    /// Localiza uma seção por `id`.
    pub fn section(&self, id: &str) -> Option<&DocSection> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Localiza um documento por `id`.
    pub fn document(&self, id: &str) -> Option<&DocDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    /// Busca determinística por ids, títulos, tags, aliases e resumos.
    /// Devolve resultados ordenados por relevância e id.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        let needle = query.to_lowercase();
        let mut hits: Vec<SearchHit> = Vec::new();
        for section in &self.sections {
            if let Some(score) = section_score(section, &needle) {
                hits.push(SearchHit {
                    id: section.id.clone(),
                    file: section.file.clone(),
                    start: section.start,
                    end: section.end,
                    summary: if section.summary.is_empty() {
                        section.title.clone()
                    } else {
                        section.summary.clone()
                    },
                    score,
                });
            }
        }
        hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
        hits
    }
}

/// Um resultado de busca/rota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub id: String,
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub summary: String,
    pub score: u32,
}

fn section_score(section: &DocSection, needle: &str) -> Option<u32> {
    if needle.is_empty() {
        return None;
    }
    let mut score = 0u32;
    if section.id.to_lowercase() == needle {
        score += 100;
    } else if section.id.to_lowercase().contains(needle) {
        score += 40;
    }
    if section.title.to_lowercase().contains(needle) {
        score += 25;
    }
    for tag in &section.tags {
        if tag.to_lowercase() == needle {
            score += 20;
        } else if tag.to_lowercase().contains(needle) {
            score += 10;
        }
    }
    for alias in &section.aliases {
        if alias.to_lowercase().contains(needle) {
            score += 15;
        }
    }
    if section.summary.to_lowercase().contains(needle) {
        score += 8;
    }
    for word in needle.split_whitespace() {
        if word.len() < 3 {
            continue;
        }
        if section.title.to_lowercase().contains(word)
            || section.tags.iter().any(|t| t.to_lowercase().contains(word))
            || section
                .aliases
                .iter()
                .any(|a| a.to_lowercase().contains(word))
            || section.summary.to_lowercase().contains(word)
        {
            score += 3;
        }
    }
    if score > 0 {
        Some(score)
    } else {
        None
    }
}

fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ScanError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            return Err(ScanError::Io {
                path: dir.display().to_string(),
                msg: err.to_string(),
            })
        }
    };
    for entry in entries {
        let entry = entry.map_err(|err| ScanError::Io {
            path: dir.display().to_string(),
            msg: err.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

fn relative_display(root: &Path, file: &Path) -> String {
    // O catálogo sempre usa o prefixo canônico "docs/", independentemente do
    // nome real do diretório varrido (a raiz de documentos da Trama é `docs/`).
    let stripped = file.strip_prefix(root).unwrap_or(file);
    format!("docs/{}", stripped.display()).replace('\\', "/")
}

fn scan_file(rel_path: &str, text: &str, index: &mut DocIndex) {
    let lines: Vec<&str> = text.lines().collect();
    let (frontmatter, body_start) = extract_frontmatter(&lines);

    let doc_id = frontmatter
        .as_ref()
        .and_then(|fm| fm.get("id"))
        .and_then(scalar)
        .map(str::to_string);

    if let Some(fm) = &frontmatter {
        if fm.contains_key("pinker-doc") {
            index
                .documents
                .push(build_document(rel_path, fm, &lines, body_start));
        }
    }

    let owner = doc_id.unwrap_or_else(|| file_stem_id(rel_path));
    scan_anchors(rel_path, &owner, &lines, body_start, index);
}

/// Extrai o frontmatter YAML entre as cercas `---` iniciais.
/// Devolve (mapa, índice da primeira linha do corpo, base 0).
fn extract_frontmatter(lines: &[&str]) -> (Option<HashMap<String, Yaml>>, usize) {
    if lines.first().map(|l| l.trim()) != Some("---") {
        return (None, 0);
    }
    let mut end = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            end = Some(i);
            break;
        }
    }
    let Some(end) = end else {
        return (None, 0);
    };
    let block = lines[1..end].join("\n");
    (Some(parse_yaml_min(&block)), end + 1)
}

fn build_document(
    rel_path: &str,
    fm: &HashMap<String, Yaml>,
    lines: &[&str],
    body_start: usize,
) -> DocDocument {
    let get_scalar = |key: &str| fm.get(key).and_then(scalar).map(str::to_string);
    let get_list = |key: &str| fm.get(key).map(list_values).unwrap_or_default();

    let missing_fields = REQUIRED_FRONTMATTER
        .iter()
        .filter(|field| fm.get(**field).and_then(scalar).is_none())
        .map(|field| field.to_string())
        .collect();

    DocDocument {
        id: get_scalar("id").unwrap_or_else(|| file_stem_id(rel_path)),
        domain: get_scalar("domain").unwrap_or_default(),
        kind: get_scalar("kind").unwrap_or_default(),
        status: get_scalar("status").unwrap_or_default(),
        parent: get_scalar("parent"),
        file: rel_path.to_string(),
        title: first_heading(lines, body_start).unwrap_or_default(),
        canonical_for: get_list("canonical_for"),
        related: get_list("related"),
        missing_fields,
    }
}

fn scan_anchors(
    rel_path: &str,
    owner: &str,
    lines: &[&str],
    body_start: usize,
    index: &mut DocIndex,
) {
    let mut i = body_start;
    // Pilha de âncoras abertas: (id, linha do start, content_start, last_content, title).
    struct Open {
        id: String,
        start_line: usize,
        content_start: Option<usize>,
        last_content: usize,
        title: Option<String>,
        tags: Vec<String>,
        aliases: Vec<String>,
        summary: String,
    }
    let mut open: Vec<Open> = Vec::new();

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let line_no = i + 1;

        if trimmed.contains("@pinker-doc:start") {
            // Coleta metadados até a linha que fecha o comentário (`-->`).
            let mut meta_lines = Vec::new();
            let mut j = i + 1;
            while j < lines.len() && !lines[j].contains("-->") {
                meta_lines.push(lines[j]);
                j += 1;
            }
            let meta = parse_yaml_min(&meta_lines.join("\n"));
            let id = meta
                .get("id")
                .and_then(scalar)
                .map(str::to_string)
                .unwrap_or_default();
            open.push(Open {
                id,
                start_line: line_no,
                content_start: None,
                last_content: line_no,
                title: None,
                tags: meta.get("tags").map(list_values).unwrap_or_default(),
                aliases: meta.get("aliases").map(list_values).unwrap_or_default(),
                summary: meta
                    .get("summary")
                    .and_then(scalar)
                    .map(str::to_string)
                    .unwrap_or_default(),
            });
            i = j + 1; // pula a linha do `-->`
            continue;
        }

        if let Some(end_id) = parse_end_marker(trimmed) {
            match open.pop() {
                Some(top) => {
                    if top.id != end_id {
                        index.scan_problems.push(DocVerifyError::AnchorIdMismatch {
                            start_id: top.id.clone(),
                            end_id: end_id.clone(),
                            file: rel_path.to_string(),
                        });
                    }
                    let start = top.content_start.unwrap_or(top.start_line);
                    let end = top.last_content.max(start);
                    index.sections.push(DocSection {
                        id: if top.id.is_empty() { end_id } else { top.id },
                        document: owner.to_string(),
                        file: rel_path.to_string(),
                        start,
                        end,
                        title: top.title.unwrap_or_default(),
                        tags: top.tags,
                        aliases: top.aliases,
                        summary: top.summary,
                    });
                }
                None => index
                    .scan_problems
                    .push(DocVerifyError::AnchorEndWithoutStart {
                        id: end_id,
                        file: rel_path.to_string(),
                        line: line_no,
                    }),
            }
            i += 1;
            continue;
        }

        // Conteúdo comum: alimenta a âncora aberta mais interna.
        if let Some(top) = open.last_mut() {
            if !trimmed.is_empty() {
                if top.content_start.is_none() {
                    top.content_start = Some(line_no);
                }
                top.last_content = line_no;
                if top.title.is_none() {
                    if let Some(heading) = heading_text(trimmed) {
                        top.title = Some(heading);
                    }
                }
            }
        }
        i += 1;
    }

    for leftover in open {
        index
            .scan_problems
            .push(DocVerifyError::AnchorStartWithoutEnd {
                id: leftover.id,
                file: rel_path.to_string(),
                line: leftover.start_line,
            });
    }
}

fn parse_end_marker(trimmed: &str) -> Option<String> {
    let idx = trimmed.find("@pinker-doc:end")?;
    let rest = trimmed[idx + "@pinker-doc:end".len()..].trim();
    let id = rest.trim_end_matches("-->").trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn heading_text(trimmed: &str) -> Option<String> {
    if trimmed.starts_with('#') {
        Some(trimmed.trim_start_matches('#').trim().to_string())
    } else {
        None
    }
}

fn first_heading(lines: &[&str], body_start: usize) -> Option<String> {
    lines
        .iter()
        .skip(body_start)
        .map(|l| l.trim())
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string())
}

fn file_stem_id(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("documento")
        .to_string()
}

fn scalar(value: &Yaml) -> Option<&str> {
    match value {
        Yaml::Scalar(s) => Some(s.as_str()),
        Yaml::List(_) => None,
    }
}

fn list_values(value: &Yaml) -> Vec<String> {
    match value {
        Yaml::List(items) => items.clone(),
        Yaml::Scalar(s) if !s.is_empty() => vec![s.clone()],
        Yaml::Scalar(_) => Vec::new(),
    }
}

/// Parser mínimo de um subconjunto de YAML usado no frontmatter e nas âncoras.
fn parse_yaml_min(text: &str) -> HashMap<String, Yaml> {
    let mut map: HashMap<String, Yaml> = HashMap::new();
    let mut current_list: Option<String> = None;

    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Item de lista em bloco pertencente à chave corrente.
        if let Some(item) = trimmed.strip_prefix("- ") {
            if let Some(key) = &current_list {
                if let Some(Yaml::List(items)) = map.get_mut(key) {
                    items.push(unquote(item.trim()));
                }
            }
            continue;
        }

        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let key = trimmed[..colon].trim().to_string();
        let rest = trimmed[colon + 1..].trim();

        if rest.is_empty() {
            map.insert(key.clone(), Yaml::List(Vec::new()));
            current_list = Some(key);
        } else if let Some(inner) = rest.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            let items = inner
                .split(',')
                .map(|s| unquote(s.trim()))
                .filter(|s| !s.is_empty())
                .collect();
            map.insert(key, Yaml::List(items));
            current_list = None;
        } else {
            map.insert(key, Yaml::Scalar(unquote(rest)));
            current_list = None;
        }
    }

    map
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn render_section_json(s: &DocSection) -> String {
    let mut out = String::new();
    out.push_str("{\"schema\":1");
    out.push_str(&format!(",\"id\":{}", json_string(&s.id)));
    out.push_str(&format!(",\"document\":{}", json_string(&s.document)));
    out.push_str(&format!(",\"file\":{}", json_string(&s.file)));
    out.push_str(&format!(",\"start\":{}", s.start));
    out.push_str(&format!(",\"end\":{}", s.end));
    out.push_str(&format!(",\"title\":{}", json_string(&s.title)));
    if !s.tags.is_empty() {
        out.push_str(&format!(",\"tags\":{}", json_string_array(&s.tags)));
    }
    if !s.aliases.is_empty() {
        out.push_str(&format!(",\"aliases\":{}", json_string_array(&s.aliases)));
    }
    if !s.summary.is_empty() {
        out.push_str(&format!(",\"summary\":{}", json_string(&s.summary)));
    }
    out.push('}');
    out
}

fn json_string_array(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| json_string(s)).collect();
    format!("[{}]", parts.join(","))
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

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn temp_docs(name: &str) -> PathBuf {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pinker_docidx_{name}_{now}"))
    }

    const PORTAL: &str = "---\npinker-doc: 1\nid: rosa\ndomain: rosa\nkind: portal\nstatus: active\nparent: atlas\ncanonical_for:\n  - rosa.territory\nrelated:\n  - rosa.core\n---\n\n# Rosa\n\nPortal.\n";

    const CORE: &str = "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\ncanonical_for:\n  - rosa.identity\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\ntags: [rosa, identidade]\naliases:\n  - quem é rosa\nsummary: Identidade de Rosa.\n-->\n## Identidade\n\nRosa é guia.\n<!-- @pinker-doc:end rosa.identity -->\n\nfim\n";

    fn build(name: &str) -> (PathBuf, DocIndex) {
        let dir = temp_docs(name);
        write(&dir, "rosa/README.md", PORTAL);
        write(&dir, "rosa/core.md", CORE);
        let index = DocIndex::scan(&dir).unwrap();
        (dir, index)
    }

    #[test]
    fn scans_documents_and_sections() {
        let (dir, index) = build("scan");
        assert_eq!(index.documents.len(), 2);
        assert_eq!(index.sections.len(), 1);
        let section = index.section("rosa.identity").unwrap();
        assert_eq!(section.document, "rosa.core");
        assert_eq!(section.file, "docs/rosa/core.md");
        assert_eq!(section.title, "Identidade");
        assert_eq!(section.tags, vec!["rosa", "identidade"]);
        assert_eq!(section.aliases, vec!["quem é rosa"]);
        assert_eq!(section.summary, "Identidade de Rosa.");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn jsonl_is_deterministic_and_sorted() {
        let (dir, index) = build("jsonl");
        let a = index.render_jsonl();
        let b = index.render_jsonl();
        assert_eq!(a, b);
        assert!(a.contains("\"id\":\"rosa.identity\""));
        assert!(a.contains("\"document\":\"rosa.core\""));
        assert!(a.ends_with('\n'));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn clean_tree_has_no_verify_errors() {
        let (dir, index) = build("verify_ok");
        assert!(index.verify().is_empty(), "{:?}", index.verify());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn duplicate_section_id_is_detected() {
        let dir = temp_docs("dup");
        write(&dir, "rosa/README.md", PORTAL);
        write(&dir, "rosa/core.md", CORE);
        write(
            &dir,
            "rosa/other.md",
            "---\npinker-doc: 1\nid: rosa.other\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Other\n\n<!-- @pinker-doc:start\nid: rosa.identity\n-->\n## Dup\nx\n<!-- @pinker-doc:end rosa.identity -->\n",
        );
        let index = DocIndex::scan(&dir).unwrap();
        let errors = index.verify();
        assert!(errors
            .iter()
            .any(|e| matches!(e, DocVerifyError::DuplicateSectionId { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unbalanced_anchor_is_detected() {
        let dir = temp_docs("unbal");
        write(&dir, "rosa/README.md", PORTAL);
        write(
            &dir,
            "rosa/core.md",
            "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\n-->\n## Identidade\nsem fim\n",
        );
        let index = DocIndex::scan(&dir).unwrap();
        assert!(index
            .verify()
            .iter()
            .any(|e| matches!(e, DocVerifyError::AnchorStartWithoutEnd { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_frontmatter_field_is_detected() {
        let dir = temp_docs("missfm");
        write(&dir, "rosa/README.md", PORTAL);
        write(
            &dir,
            "rosa/bad.md",
            "---\npinker-doc: 1\nid: rosa.bad\ndomain: rosa\nkind: reference\n---\n\n# Bad\n",
        );
        let index = DocIndex::scan(&dir).unwrap();
        assert!(index
            .verify()
            .iter()
            .any(|e| matches!(e, DocVerifyError::MissingFrontmatterField { .. })));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn search_ranks_exact_id_first() {
        let (dir, index) = build("search");
        let hits = index.search("rosa.identity");
        assert_eq!(hits[0].id, "rosa.identity");
        let by_tag = index.search("identidade");
        assert!(by_tag.iter().any(|h| h.id == "rosa.identity"));
        fs::remove_dir_all(dir).unwrap();
    }
}
