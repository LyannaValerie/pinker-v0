//! Trama Pinker — Etapa 2 (Catálogo documental).
//!
//! Varre a árvore `docs/`, lê o frontmatter dos documentos estruturais e as
//! âncoras de seção `@pinker-doc:start/end`, e produz o catálogo derivado
//! `docs/navigation.jsonl` (especificação, seções 7, 8, 9 e 21).
//!
//! O catálogo é totalmente gerado, ordenado de forma determinística por `id` e
//! nunca editado à mão. Zero dependências externas, coerente com o compilador.

use crate::jsonl;
use crate::text_norm;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Versão do formato do catálogo documental (`docs/navigation.jsonl`).
/// Formato discriminado (§6): registros `document` e `section` num único
/// catálogo crescente.
pub const CATALOG_SCHEMA: u64 = 2;

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
    pub audience: Vec<String>,
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
        if !docs_root.exists() {
            return Ok(DocIndex::default());
        }
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

    /// Serializa o catálogo discriminado em JSONL determinístico (§6): primeiro
    /// os registros `document` (ordenados por `id`), depois os registros
    /// `section` (ordenados por `id`, `file`). Um único catálogo crescente.
    pub fn render_jsonl(&self) -> String {
        let mut documents = self.documents.clone();
        documents.sort_by(|a, b| a.id.cmp(&b.id));
        let mut sections = self.sections.clone();
        sections.sort_by(|a, b| a.id.cmp(&b.id).then(a.file.cmp(&b.file)));

        let mut out = String::new();
        for document in &documents {
            out.push_str(&render_document_json(document));
            out.push('\n');
        }
        for section in &sections {
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

    /// Busca determinística sobre as seções (política §7.2). Devolve resultados
    /// ordenados por pontuação, cobertura de termos e id lexicográfico.
    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        search_sections(&self.sections, query)
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
    pub coverage: usize,
}

/// Pontuação e cobertura de uma seção para uma consulta já normalizada.
/// Política determinística da especificação (§7.2):
///
/// - ID exato: 100; alias exato: 90; ID contendo a consulta: 70;
/// - título contendo a consulta: 60; cobertura em aliases: 50;
/// - cobertura em tags: 40; cobertura no resumo: 20.
fn score_section(section: &DocSection, q_norm: &str, terms: &[String]) -> Option<(u32, usize)> {
    if q_norm.is_empty() {
        return None;
    }
    let id_norm = text_norm::normalize(&section.id);
    let title_norm = text_norm::normalize(&section.title);
    let alias_norms: Vec<String> = section
        .aliases
        .iter()
        .map(|a| text_norm::normalize(a))
        .collect();
    let tag_norms: Vec<String> = section
        .tags
        .iter()
        .map(|t| text_norm::normalize(t))
        .collect();
    let summary_norm = text_norm::normalize(&section.summary);

    let mut score = 0u32;
    if id_norm == q_norm {
        score += 100;
    }
    if alias_norms.iter().any(|a| a == q_norm) {
        score += 90;
    }
    if id_norm.contains(q_norm) {
        score += 70;
    }
    if title_norm.contains(q_norm) {
        score += 60;
    }
    if covers(&alias_norms.join(" "), terms) {
        score += 50;
    }
    if covers(&tag_norms.join(" "), terms) {
        score += 40;
    }
    if covers(&summary_norm, terms) {
        score += 20;
    }

    // Cobertura de termos = quantos termos distintos aparecem em qualquer campo
    // (usado como primeiro critério de desempate).
    let haystack = format!(
        "{} {} {} {} {}",
        id_norm,
        title_norm,
        alias_norms.join(" "),
        tag_norms.join(" "),
        summary_norm
    );
    let coverage = terms
        .iter()
        .filter(|t| haystack.split(' ').any(|w| w == t.as_str()))
        .count();

    if score == 0 {
        return None;
    }
    Some((score, coverage))
}

/// Verdadeiro se a maioria dos termos da consulta aparece como palavra no texto
/// normalizado. Usa maioria (≥ metade, mínimo 1) em vez de cobertura total para
/// tolerar conectivos ("qual é a próxima fase"), sem recorrer a stemming ou
/// fuzzy search — política determinística e equivalente à da especificação
/// (§7.2, "equivalente a").
fn covers(haystack_norm: &str, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let words: Vec<&str> = haystack_norm.split(' ').filter(|w| !w.is_empty()).collect();
    let matched = terms
        .iter()
        .filter(|t| words.iter().any(|w| *w == t.as_str()))
        .count();
    let needed = terms.len().div_ceil(2);
    matched >= needed
}

/// Núcleo de busca compartilhado por `DocIndex` (memória) e `DocCatalog`
/// (JSONL). Ordena por (pontuação desc, cobertura desc, id asc).
fn search_sections(sections: &[DocSection], query: &str) -> Vec<SearchHit> {
    let q_norm = text_norm::normalize(query);
    let terms = text_norm::terms(query);
    let mut hits: Vec<SearchHit> = Vec::new();
    for section in sections {
        if let Some((score, coverage)) = score_section(section, &q_norm, &terms) {
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
                coverage,
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.coverage.cmp(&a.coverage))
            .then(a.id.cmp(&b.id))
    });
    hits
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
        audience: get_list("audience"),
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

fn render_document_json(d: &DocDocument) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", CATALOG_SCHEMA));
    out.push_str(",\"record\":\"document\"");
    out.push_str(&format!(",\"id\":{}", json_string(&d.id)));
    out.push_str(&format!(",\"domain\":{}", json_string(&d.domain)));
    out.push_str(&format!(",\"kind\":{}", json_string(&d.kind)));
    out.push_str(&format!(",\"status\":{}", json_string(&d.status)));
    if let Some(parent) = &d.parent {
        out.push_str(&format!(",\"parent\":{}", json_string(parent)));
    }
    if !d.title.is_empty() {
        out.push_str(&format!(",\"title\":{}", json_string(&d.title)));
    }
    if !d.audience.is_empty() {
        out.push_str(&format!(",\"audience\":{}", json_string_array(&d.audience)));
    }
    if !d.canonical_for.is_empty() {
        out.push_str(&format!(
            ",\"canonical_for\":{}",
            json_string_array(&d.canonical_for)
        ));
    }
    if !d.related.is_empty() {
        out.push_str(&format!(",\"related\":{}", json_string_array(&d.related)));
    }
    out.push_str(&format!(",\"file\":{}", json_string(&d.file)));
    out.push('}');
    out
}

fn render_section_json(s: &DocSection) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", CATALOG_SCHEMA));
    out.push_str(",\"record\":\"section\"");
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

// ---------------------------------------------------------------------------
// Catálogo carregado do JSONL (superfície de consulta — §5).
// ---------------------------------------------------------------------------

/// Falha ao carregar o catálogo documental versionado.
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
                "E-DOC-CATALOG\nCatálogo documental ausente: '{}'. Rode `pink doc sincronizar`.",
                path
            ),
            CatalogError::Invalid { path, line, msg } => write!(
                f,
                "E-DOC-CATALOG\nCatálogo documental inválido em '{}' (linha {}): {}. Rode `pink doc sincronizar`.",
                path, line, msg
            ),
        }
    }
}

/// Catálogo documental em memória, reconstruído a partir do JSONL versionado.
/// As consultas (`mostrar`, `listar`, `buscar`, `rota`) usam esta superfície e
/// não revarrem `docs/` (§5).
#[derive(Debug, Clone, Default)]
pub struct DocCatalog {
    pub documents: Vec<DocDocument>,
    pub sections: Vec<DocSection>,
}

impl DocCatalog {
    /// Carrega o catálogo do arquivo JSONL versionado.
    pub fn load(path: &Path) -> Result<DocCatalog, CatalogError> {
        let text = fs::read_to_string(path).map_err(|_| CatalogError::Missing {
            path: path.display().to_string(),
        })?;
        Self::parse(&text, &path.display().to_string())
    }

    /// Interpreta o conteúdo textual do catálogo (útil também para testes).
    pub fn parse(text: &str, path: &str) -> Result<DocCatalog, CatalogError> {
        let mut catalog = DocCatalog::default();
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
            let invalid = |msg: &str| CatalogError::Invalid {
                path: path.to_string(),
                line: idx + 1,
                msg: msg.to_string(),
            };
            let schema = obj.get("schema").and_then(|v| v.as_int()).unwrap_or(0);
            if schema != CATALOG_SCHEMA as i64 {
                return Err(invalid(&format!("schema {} não suportado", schema)));
            }
            let record = obj
                .get("record")
                .and_then(|v| v.as_str())
                .ok_or_else(|| invalid("campo 'record' ausente"))?;
            match record {
                "document" => catalog
                    .documents
                    .push(parse_document_record(&obj, &invalid)?),
                "section" => catalog.sections.push(parse_section_record(&obj, &invalid)?),
                other => return Err(invalid(&format!("record desconhecido '{}'", other))),
            }
        }
        catalog.documents.sort_by(|a, b| a.id.cmp(&b.id));
        catalog
            .sections
            .sort_by(|a, b| a.id.cmp(&b.id).then(a.file.cmp(&b.file)));
        Ok(catalog)
    }

    pub fn section(&self, id: &str) -> Option<&DocSection> {
        self.sections.iter().find(|s| s.id == id)
    }

    pub fn document(&self, id: &str) -> Option<&DocDocument> {
        self.documents.iter().find(|d| d.id == id)
    }

    pub fn documents_in_domain(&self, domain: &str) -> Vec<&DocDocument> {
        self.documents
            .iter()
            .filter(|d| d.domain == domain)
            .collect()
    }

    pub fn sections_of(&self, document: &str) -> Vec<&DocSection> {
        self.sections
            .iter()
            .filter(|s| s.document == document)
            .collect()
    }

    pub fn search(&self, query: &str) -> Vec<SearchHit> {
        search_sections(&self.sections, query)
    }
}

fn parse_document_record(
    obj: &jsonl::JsonObject,
    invalid: &impl Fn(&str) -> CatalogError,
) -> Result<DocDocument, CatalogError> {
    let req_str = |key: &str| -> Result<String, CatalogError> {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| invalid(&format!("documento sem campo '{}'", key)))
    };
    let opt_str = |key: &str| obj.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let opt_list = |key: &str| {
        obj.get(key)
            .and_then(|v| v.as_str_array())
            .unwrap_or_default()
    };
    Ok(DocDocument {
        id: req_str("id")?,
        domain: req_str("domain")?,
        kind: req_str("kind")?,
        status: req_str("status")?,
        parent: opt_str("parent"),
        file: req_str("file")?,
        title: opt_str("title").unwrap_or_default(),
        audience: opt_list("audience"),
        canonical_for: opt_list("canonical_for"),
        related: opt_list("related"),
        missing_fields: Vec::new(),
    })
}

fn parse_section_record(
    obj: &jsonl::JsonObject,
    invalid: &impl Fn(&str) -> CatalogError,
) -> Result<DocSection, CatalogError> {
    let req_str = |key: &str| -> Result<String, CatalogError> {
        obj.get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| invalid(&format!("seção sem campo '{}'", key)))
    };
    let req_int = |key: &str| -> Result<usize, CatalogError> {
        obj.get(key)
            .and_then(|v| v.as_int())
            .filter(|v| *v >= 0)
            .map(|v| v as usize)
            .ok_or_else(|| invalid(&format!("seção sem inteiro '{}'", key)))
    };
    let opt_list = |key: &str| {
        obj.get(key)
            .and_then(|v| v.as_str_array())
            .unwrap_or_default()
    };
    Ok(DocSection {
        id: req_str("id")?,
        document: req_str("document")?,
        file: req_str("file")?,
        start: req_int("start")?,
        end: req_int("end")?,
        title: obj
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        tags: opt_list("tags"),
        aliases: opt_list("aliases"),
        summary: obj
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

/// Valida uma seção ao extrair conteúdo (§5): reescaneia apenas o arquivo-fonte
/// que o catálogo apontou e confirma que a seção de mesmo id ainda ocupa
/// exatamente o mesmo intervalo `[start, end]`. Abrir o único arquivo indicado
/// pelo catálogo é permitido (não é revarrer `docs/`); qualquer divergência de
/// posição significa catálogo desatualizado e a resposta é recusada.
pub fn validate_section_anchor(source: &str, section: &DocSection) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let (_, body_start) = extract_frontmatter(&lines);
    // Reaproveita o mesmo scanner de âncoras usado pela sincronização.
    let mut index = DocIndex::default();
    scan_anchors(
        &section.file,
        &section.document,
        &lines,
        body_start,
        &mut index,
    );
    match index.sections.iter().find(|s| s.id == section.id) {
        Some(fresh) => fresh.start == section.start && fresh.end == section.end,
        None => false,
    }
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
