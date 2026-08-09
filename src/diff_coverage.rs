//! Cobertura somente leitura de unified diffs por autoridades explícitas.
//!
//! O módulo não executa Git, não lê fontes e não escreve. A entrada é um diff
//! textual já produzido pelo chamador; as relações vêm apenas dos catálogos e
//! artefatos canônicos carregados pelo adaptador CLI.

use crate::change::Manifests;
use crate::doc::{DocConfig, DocProjection};
use crate::doc_index::{DocCatalog, DocDocument, DocSection};
use crate::nav::{CodeCatalog, CodeRegion};
use crate::nav_projection_recipe;
use crate::nav_projection_store::ProjectionStore;
use crate::symbol_index;
use std::collections::BTreeSet;
use std::fmt;

pub const DIFF_COVERAGE_SCHEMA: u64 = 1;
pub const MAX_DIFF_BYTES: usize = 16 * 1024 * 1024;

// @pinker-nav:start trama.diff-cobertura.modelo
// @pinker-nav:domain diff-coverage
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::diff_coverage::CoverageReport|CoverageReport|rust-type|declaration
// @pinker-nav:symbol-doc pinker_v0::diff_coverage::CoverageReport|development.diff-coverage.contract
// @pinker-nav:summary Modelo público schema 1 da cobertura de diff: arquivos e linhas vêm do unified diff; regiões, documentos, projeções e testes carregam autoridade explícita e estados KNOWN, UNKNOWN ou UNAVAILABLE, sem inferência heurística.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl FileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FileStatus::Added => "ADDED",
            FileStatus::Modified => "MODIFIED",
            FileStatus::Deleted => "DELETED",
            FileStatus::Renamed => "RENAMED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Authority {
    pub source: String,
    pub path: String,
    pub record: Option<String>,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegionItem {
    pub id: String,
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub authority: Authority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocumentItem {
    pub id: String,
    pub document: String,
    pub path: String,
    pub title: String,
    pub authority: Authority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProjectionItem {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub authority: Authority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestItem {
    pub region: String,
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub authority: Authority,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoverageWarning {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCoverage {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub changed_lines: Vec<LineRange>,
    pub regions: Relation<RegionItem>,
    pub documents: Relation<DocumentItem>,
    pub projections: Relation<ProjectionItem>,
    pub tests: Relation<TestItem>,
    pub warnings: Vec<CoverageWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageReport {
    pub schema: u64,
    pub source: String,
    pub files: Vec<FileCoverage>,
}

pub struct CoverageAuthorities<'a> {
    pub code: &'a CodeCatalog,
    pub docs: Option<&'a DocCatalog>,
    pub projection_store: Option<&'a ProjectionStore>,
    pub doc_config: Option<&'a DocConfig>,
    pub manifests: Option<&'a Manifests>,
}
// @pinker-nav:end trama.diff-cobertura.modelo

// @pinker-nav:start trama.diff-cobertura.parser
// @pinker-nav:domain diff-coverage
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::diff_coverage::parse_unified_diff|parse_unified_diff|rust-function|declaration
// @pinker-nav:summary Parser finito de unified diff: valida paths repo-relativos, contagens de hunks e limite de entrada; registra somente linhas novas explicitamente adicionadas, deixando deleções sem âncora atual como UNKNOWN em vez de aproximá-las.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageError {
    TooLarge {
        bytes: usize,
        limit: usize,
    },
    InvalidUtf8,
    InvalidFormat {
        line: usize,
        detail: String,
    },
    InvalidPath {
        line: usize,
        path: String,
        detail: String,
    },
    Authority {
        detail: String,
    },
}

impl fmt::Display for CoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoverageError::TooLarge { bytes, limit } => write!(
                f,
                "E-DIFF-LIMIT\nDiff com {} bytes excede o limite de {} bytes.",
                bytes, limit
            ),
            CoverageError::InvalidUtf8 => {
                write!(f, "E-DIFF-UTF8\nA entrada de diff não é UTF-8 válida.")
            }
            CoverageError::InvalidFormat { line, detail } => write!(
                f,
                "E-DIFF-FORMAT\nUnified diff inválido na linha {}: {}.",
                line, detail
            ),
            CoverageError::InvalidPath { line, path, detail } => write!(
                f,
                "E-DIFF-PATH\nPath inválido na linha {} ('{}'): {}.",
                line, path, detail
            ),
            CoverageError::Authority { detail } => {
                write!(f, "E-DIFF-AUTHORITY\n{}", detail)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedFile {
    path: String,
    old_path: Option<String>,
    status: FileStatus,
    changed_lines: Vec<LineRange>,
    deletion_without_new_lines: bool,
    binary: bool,
}

#[derive(Debug, Default)]
struct FileBuilder {
    old_path: Option<String>,
    new_path: Option<String>,
    added_lines: Vec<usize>,
    saw_deletion: bool,
    binary: bool,
    declared_added: bool,
    declared_deleted: bool,
}

#[derive(Debug, Clone, Copy)]
struct Hunk {
    old_remaining: usize,
    new_remaining: usize,
    old_line: usize,
    new_line: usize,
}

fn parse_unified_diff(input: &str) -> Result<Vec<ParsedFile>, CoverageError> {
    if input.len() > MAX_DIFF_BYTES {
        return Err(CoverageError::TooLarge {
            bytes: input.len(),
            limit: MAX_DIFF_BYTES,
        });
    }
    let mut files = Vec::new();
    let mut current: Option<FileBuilder> = None;
    let mut hunk: Option<Hunk> = None;

    for (index, raw) in input.lines().enumerate() {
        let line_number = index + 1;
        if raw.starts_with("diff --git ") {
            finish_hunk(hunk.take(), line_number)?;
            if let Some(builder) = current.take() {
                files.push(finish_file(builder, line_number)?);
            }
            let (old_path, new_path) =
                parse_diff_git_paths(raw.trim_start_matches("diff --git "), line_number)?;
            current = Some(FileBuilder {
                old_path: Some(old_path),
                new_path: Some(new_path),
                ..FileBuilder::default()
            });
            continue;
        }
        if raw.starts_with("--- ") && hunk.is_none() {
            if current.is_none() {
                current = Some(FileBuilder::default());
            }
            let path = parse_header_path(&raw[4..], "a/", line_number)?;
            current.as_mut().expect("criado acima").old_path = path;
            continue;
        }
        if raw.starts_with("+++ ") && hunk.is_none() {
            let Some(builder) = current.as_mut() else {
                return Err(format_error(line_number, "cabeçalho +++ sem arquivo"));
            };
            builder.new_path = parse_header_path(&raw[4..], "b/", line_number)?;
            continue;
        }
        if raw == "new file mode 100644" || raw.starts_with("new file mode ") {
            let Some(builder) = current.as_mut() else {
                return Err(format_error(line_number, "modo de arquivo fora de um diff"));
            };
            builder.declared_added = true;
            continue;
        }
        if raw == "deleted file mode 100644" || raw.starts_with("deleted file mode ") {
            let Some(builder) = current.as_mut() else {
                return Err(format_error(line_number, "modo de arquivo fora de um diff"));
            };
            builder.declared_deleted = true;
            continue;
        }
        if let Some(value) = raw.strip_prefix("rename from ") {
            let Some(builder) = current.as_mut() else {
                return Err(format_error(line_number, "rename fora de um diff"));
            };
            builder.old_path = Some(validate_path(value, line_number)?);
            continue;
        }
        if let Some(value) = raw.strip_prefix("rename to ") {
            let Some(builder) = current.as_mut() else {
                return Err(format_error(line_number, "rename fora de um diff"));
            };
            builder.new_path = Some(validate_path(value, line_number)?);
            continue;
        }
        if raw.starts_with("Binary files ") || raw.starts_with("GIT binary patch") {
            let Some(builder) = current.as_mut() else {
                return Err(format_error(
                    line_number,
                    "marcador binário fora de um diff",
                ));
            };
            builder.binary = true;
            continue;
        }
        if raw.starts_with("@@ ") {
            finish_hunk(hunk.take(), line_number)?;
            if current.is_none() {
                return Err(format_error(line_number, "hunk sem cabeçalho de arquivo"));
            }
            hunk = Some(parse_hunk_header(raw, line_number)?);
            continue;
        }
        if let Some(active) = hunk.as_mut() {
            match raw.as_bytes().first().copied() {
                Some(b'+') => {
                    if active.new_remaining == 0 {
                        return Err(format_error(line_number, "linha adicionada excede o hunk"));
                    }
                    current
                        .as_mut()
                        .expect("hunk exige arquivo")
                        .added_lines
                        .push(active.new_line);
                    active.new_line += 1;
                    active.new_remaining -= 1;
                }
                Some(b'-') => {
                    if active.old_remaining == 0 {
                        return Err(format_error(line_number, "linha removida excede o hunk"));
                    }
                    current.as_mut().expect("hunk exige arquivo").saw_deletion = true;
                    active.old_line += 1;
                    active.old_remaining -= 1;
                }
                Some(b' ') => {
                    if active.old_remaining == 0 || active.new_remaining == 0 {
                        return Err(format_error(line_number, "contexto excede o hunk"));
                    }
                    active.old_line += 1;
                    active.new_line += 1;
                    active.old_remaining -= 1;
                    active.new_remaining -= 1;
                }
                Some(b'\\') => {}
                _ => return Err(format_error(line_number, "linha de hunk sem prefixo")),
            }
        }
    }
    finish_hunk(hunk.take(), input.lines().count() + 1)?;
    if let Some(builder) = current.take() {
        files.push(finish_file(builder, input.lines().count() + 1)?);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path).then(a.old_path.cmp(&b.old_path)));
    Ok(files)
}

fn format_error(line: usize, detail: &str) -> CoverageError {
    CoverageError::InvalidFormat {
        line,
        detail: detail.to_string(),
    }
}

fn finish_hunk(hunk: Option<Hunk>, line: usize) -> Result<(), CoverageError> {
    if let Some(hunk) = hunk {
        if hunk.old_remaining != 0 || hunk.new_remaining != 0 {
            return Err(format_error(
                line.saturating_sub(1).max(1),
                "contagem declarada do hunk não foi consumida",
            ));
        }
    }
    Ok(())
}

fn parse_hunk_header(raw: &str, line: usize) -> Result<Hunk, CoverageError> {
    let end = raw[3..]
        .find(" @@")
        .map(|offset| offset + 3)
        .ok_or_else(|| format_error(line, "cabeçalho de hunk sem fechamento @@"))?;
    let header = &raw[3..end];
    let mut parts = header.split_whitespace();
    let old = parts
        .next()
        .ok_or_else(|| format_error(line, "intervalo antigo ausente"))?;
    let new = parts
        .next()
        .ok_or_else(|| format_error(line, "intervalo novo ausente"))?;
    if parts.next().is_some() || !old.starts_with('-') || !new.starts_with('+') {
        return Err(format_error(line, "intervalos de hunk inválidos"));
    }
    let (old_line, old_remaining) = parse_range(&old[1..], line)?;
    let (new_line, new_remaining) = parse_range(&new[1..], line)?;
    Ok(Hunk {
        old_remaining,
        new_remaining,
        old_line,
        new_line,
    })
}

fn parse_range(raw: &str, line: usize) -> Result<(usize, usize), CoverageError> {
    let (start, count) = raw.split_once(',').unwrap_or((raw, "1"));
    let start = start
        .parse::<usize>()
        .map_err(|_| format_error(line, "início de intervalo inválido"))?;
    let count = count
        .parse::<usize>()
        .map_err(|_| format_error(line, "quantidade de intervalo inválida"))?;
    if start == 0 && count != 0 {
        return Err(format_error(
            line,
            "linha zero só é válida com quantidade zero",
        ));
    }
    Ok((start, count))
}

fn parse_header_path(
    raw: &str,
    expected_prefix: &str,
    line: usize,
) -> Result<Option<String>, CoverageError> {
    let raw = raw.split('\t').next().unwrap_or(raw).trim();
    if raw == "/dev/null" {
        return Ok(None);
    }
    let decoded = decode_git_path(raw, line)?;
    let path = decoded.strip_prefix(expected_prefix).unwrap_or(&decoded);
    Ok(Some(validate_path(path, line)?))
}

fn parse_diff_git_paths(raw: &str, line: usize) -> Result<(String, String), CoverageError> {
    let (old, new) = if raw.starts_with('"') {
        let (old, rest) = take_git_token(raw, line)?;
        let (new, trailing) = take_git_token(rest.trim_start(), line)?;
        if !trailing.trim().is_empty() {
            return Err(format_error(
                line,
                "cabeçalho diff --git possui dados residuais",
            ));
        }
        (decode_git_path(old, line)?, decode_git_path(new, line)?)
    } else if let Some(separator) = raw.rfind(" b/") {
        (
            raw[..separator].to_string(),
            raw[separator + 1..].to_string(),
        )
    } else {
        let (old, rest) = take_git_token(raw, line)?;
        let (new, trailing) = take_git_token(rest.trim_start(), line)?;
        if !trailing.trim().is_empty() {
            return Err(format_error(
                line,
                "cabeçalho diff --git possui dados residuais",
            ));
        }
        (old.to_string(), new.to_string())
    };
    let old = old.strip_prefix("a/").unwrap_or(&old);
    let new = new.strip_prefix("b/").unwrap_or(&new);
    Ok((validate_path(old, line)?, validate_path(new, line)?))
}

fn take_git_token(raw: &str, line: usize) -> Result<(&str, &str), CoverageError> {
    if raw.is_empty() {
        return Err(format_error(line, "path ausente em diff --git"));
    }
    if !raw.starts_with('"') {
        return Ok(raw.split_once(char::is_whitespace).unwrap_or((raw, "")));
    }
    let bytes = raw.as_bytes();
    let mut escaped = false;
    for index in 1..bytes.len() {
        match (bytes[index], escaped) {
            (_, true) => escaped = false,
            (b'\\', false) => escaped = true,
            (b'"', false) => return Ok((&raw[..=index], &raw[index + 1..])),
            _ => {}
        }
    }
    Err(format_error(
        line,
        "path entre aspas não fechado em diff --git",
    ))
}

fn decode_git_path(raw: &str, line: usize) -> Result<String, CoverageError> {
    if !raw.starts_with('"') {
        return Ok(raw.to_string());
    }
    if !raw.ends_with('"') || raw.len() < 2 {
        return Err(format_error(line, "path Git entre aspas não foi fechado"));
    }
    let bytes = raw.as_bytes();
    let mut out = Vec::new();
    let mut i = 1usize;
    while i + 1 < bytes.len() {
        if bytes[i] != b'\\' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i + 1 > bytes.len() {
            return Err(format_error(line, "escape incompleto em path Git"));
        }
        match bytes[i] {
            b'\\' | b'"' => {
                out.push(bytes[i]);
                i += 1;
            }
            b't' => {
                out.push(b'\t');
                i += 1;
            }
            b'n' => {
                out.push(b'\n');
                i += 1;
            }
            b'0'..=b'7' => {
                let mut value = 0u16;
                let mut consumed = 0usize;
                while consumed < 3 && i < bytes.len() - 1 && matches!(bytes[i], b'0'..=b'7') {
                    value = value * 8 + u16::from(bytes[i] - b'0');
                    i += 1;
                    consumed += 1;
                }
                if value > 255 {
                    return Err(format_error(line, "escape octal fora de byte"));
                }
                out.push(value as u8);
            }
            _ => return Err(format_error(line, "escape não suportado em path Git")),
        }
    }
    String::from_utf8(out).map_err(|_| CoverageError::InvalidPath {
        line,
        path: raw.to_string(),
        detail: "path decodificado não é UTF-8".to_string(),
    })
}

fn validate_path(raw: &str, line: usize) -> Result<String, CoverageError> {
    crate::automation::RelativePath::new(raw)
        .map(|path| path.as_str().to_string())
        .map_err(|error| CoverageError::InvalidPath {
            line,
            path: raw.to_string(),
            detail: error.to_string(),
        })
}

fn finish_file(builder: FileBuilder, line: usize) -> Result<ParsedFile, CoverageError> {
    let status = if builder.declared_deleted || builder.new_path.is_none() {
        FileStatus::Deleted
    } else if builder.declared_added || builder.old_path.is_none() {
        FileStatus::Added
    } else if builder.old_path != builder.new_path {
        FileStatus::Renamed
    } else {
        FileStatus::Modified
    };
    let path = builder
        .new_path
        .clone()
        .or_else(|| builder.old_path.clone())
        .ok_or_else(|| format_error(line.saturating_sub(1).max(1), "arquivo sem path"))?;
    let mut added = builder.added_lines;
    added.sort_unstable();
    added.dedup();
    let changed_lines = merge_lines(&added);
    Ok(ParsedFile {
        path,
        old_path: if status == FileStatus::Renamed || status == FileStatus::Deleted {
            builder.old_path
        } else {
            None
        },
        status,
        changed_lines,
        deletion_without_new_lines: builder.saw_deletion && added.is_empty(),
        binary: builder.binary,
    })
}

fn merge_lines(lines: &[usize]) -> Vec<LineRange> {
    let mut ranges = Vec::new();
    for line in lines.iter().copied() {
        match ranges.last_mut() {
            Some(LineRange { end, .. }) if *end + 1 == line => *end = line,
            _ => ranges.push(LineRange {
                start: line,
                end: line,
            }),
        }
    }
    ranges
}
// @pinker-nav:end trama.diff-cobertura.parser

// @pinker-nav:start trama.diff-cobertura.derivacao
// @pinker-nav:domain diff-coverage
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::diff_coverage::analyze|analyze|rust-function|declaration
// @pinker-nav:symbol pinker_v0::diff_coverage::analyze|analyze|rust-function|implementation
// @pinker-nav:symbol-doc pinker_v0::diff_coverage::analyze|development.diff-coverage.contract
// @pinker-nav:summary Relaciona linhas novas a spans publicados pela Trama, resolve docs e testes pelo índice de símbolos, snapshots históricos pela composição oficial e projeções documentais por config/updates; ausência de vínculo ou deleção sem coordenada atual vira UNKNOWN com aviso explícito.

pub fn analyze(
    input: &str,
    authorities: CoverageAuthorities<'_>,
) -> Result<CoverageReport, CoverageError> {
    let parsed = parse_unified_diff(input)?;
    let mut files = Vec::with_capacity(parsed.len());
    for file in parsed {
        files.push(analyze_file(&file, &authorities)?);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path).then(a.old_path.cmp(&b.old_path)));
    Ok(CoverageReport {
        schema: DIFF_COVERAGE_SCHEMA,
        source: "stdin-unified-diff".to_string(),
        files,
    })
}

fn analyze_file(
    file: &ParsedFile,
    authorities: &CoverageAuthorities<'_>,
) -> Result<FileCoverage, CoverageError> {
    let mut warnings = BTreeSet::new();
    if file.binary {
        warning(
            &mut warnings,
            "W-DIFF-BINARY",
            "diff binário não publica coordenadas de linha",
        );
    }
    if file.deletion_without_new_lines {
        warning(
            &mut warnings,
            "W-DIFF-DELETION-ONLY",
            "deleções sem linhas novas não são aproximadas contra o catálogo corrente",
        );
    }

    let touched = touched_regions(authorities.code, &file.path, &file.changed_lines);
    let regions = if touched.is_empty() {
        warning(
            &mut warnings,
            "W-DIFF-REGION-UNKNOWN",
            "nenhuma região publicada intersecta linhas novas do arquivo",
        );
        Relation::unknown("relação com região não pôde ser estabelecida por span explícito")
    } else {
        Relation::known(touched.iter().map(|region| region_item(region)).collect())
    };

    let identities = touched_identities(&touched);
    let documents = derive_documents(file, &touched, &identities, authorities, &mut warnings)?;
    let tests = derive_tests(&touched, &identities, authorities, &mut warnings)?;
    let projections = derive_projections(file, &touched, authorities, &mut warnings);

    Ok(FileCoverage {
        path: file.path.clone(),
        old_path: file.old_path.clone(),
        status: file.status,
        changed_lines: file.changed_lines.clone(),
        regions,
        documents,
        projections,
        tests,
        warnings: warnings.into_iter().collect(),
    })
}

fn warning(warnings: &mut BTreeSet<CoverageWarning>, code: &str, message: &str) {
    warnings.insert(CoverageWarning {
        code: code.to_string(),
        message: message.to_string(),
    });
}

fn touched_regions<'a>(
    code: &'a CodeCatalog,
    path: &str,
    ranges: &[LineRange],
) -> Vec<&'a CodeRegion> {
    code.regions
        .iter()
        .filter(|region| region.file == path)
        .filter(|region| {
            ranges
                .iter()
                .any(|range| range.start <= region.end_marker && range.end >= region.start_marker)
        })
        .collect()
}

fn region_item(region: &CodeRegion) -> RegionItem {
    RegionItem {
        id: region.key.clone(),
        path: region.file.clone(),
        start: region.start_marker,
        end: region.end_marker,
        authority: code_authority(region, "span"),
    }
}

fn code_authority(region: &CodeRegion, field: &str) -> Authority {
    Authority {
        source: "code-catalog".to_string(),
        path: "src/navigation.jsonl".to_string(),
        record: Some(region.key.clone()),
        field: field.to_string(),
    }
}

fn touched_identities(regions: &[&CodeRegion]) -> BTreeSet<String> {
    let mut identities = BTreeSet::new();
    for region in regions {
        identities.extend(region.symbols.iter().map(|symbol| symbol.identity.clone()));
        identities.extend(region.related_symbols.iter().cloned());
        identities.extend(region.test_for.iter().cloned());
        identities.extend(region.symbol_docs.iter().map(|link| link.identity.clone()));
    }
    identities
}

fn derive_documents(
    file: &ParsedFile,
    touched: &[&CodeRegion],
    identities: &BTreeSet<String>,
    authorities: &CoverageAuthorities<'_>,
    warnings: &mut BTreeSet<CoverageWarning>,
) -> Result<Relation<DocumentItem>, CoverageError> {
    let Some(docs) = authorities.docs else {
        warning(
            warnings,
            "W-DIFF-DOCS-UNAVAILABLE",
            "catálogo docs/navigation.jsonl indisponível",
        );
        return Ok(Relation::unavailable(
            "catálogo docs/navigation.jsonl indisponível",
        ));
    };
    let mut items = BTreeSet::new();
    for document in docs
        .documents
        .iter()
        .filter(|doc| doc_path(doc) == file.path)
    {
        let sections = docs
            .sections
            .iter()
            .filter(|section| section.document == document.id)
            .filter(|section| intersects(&file.changed_lines, section.start, section.end))
            .collect::<Vec<_>>();
        if sections.is_empty() {
            items.insert(direct_document_item(document, None));
        } else {
            for section in sections {
                items.insert(direct_document_item(document, Some(section)));
            }
        }
    }
    for identity in identities {
        let report =
            symbol_index::locate(authorities.code, Some(docs), identity).map_err(|error| {
                CoverageError::Authority {
                    detail: error.to_string(),
                }
            })?;
        for candidate in report.candidates {
            for item in candidate.documentation.items {
                items.insert(DocumentItem {
                    id: item.id,
                    document: item.document,
                    path: item.path,
                    title: item.title,
                    authority: Authority {
                        source: "symbol-index".to_string(),
                        path: item.authority.catalog,
                        record: item.authority.region,
                        field: item.authority.field,
                    },
                });
            }
        }
    }
    if items.is_empty() {
        let detail = if touched.is_empty() {
            "arquivo sem documento catalogado ou região com vínculo documental"
        } else {
            "regiões tocadas sem vínculo documental explícito"
        };
        warning(warnings, "W-DIFF-DOCUMENT-UNKNOWN", detail);
        Ok(Relation::unknown(detail))
    } else {
        Ok(Relation::known(items.into_iter().collect()))
    }
}

fn doc_path(document: &DocDocument) -> String {
    if document.file == "docs" || document.file.starts_with("docs/") {
        document.file.clone()
    } else {
        format!("docs/{}", document.file)
    }
}

fn direct_document_item(document: &DocDocument, section: Option<&DocSection>) -> DocumentItem {
    DocumentItem {
        id: section.map_or_else(|| document.id.clone(), |section| section.id.clone()),
        document: document.id.clone(),
        path: doc_path(document),
        title: section.map_or_else(|| document.title.clone(), |section| section.title.clone()),
        authority: Authority {
            source: "documentation-catalog".to_string(),
            path: "docs/navigation.jsonl".to_string(),
            record: Some(document.id.clone()),
            field: "file+span".to_string(),
        },
    }
}

fn intersects(ranges: &[LineRange], start: usize, end: usize) -> bool {
    ranges
        .iter()
        .any(|range| range.start <= end && range.end >= start)
}

fn derive_tests(
    touched: &[&CodeRegion],
    identities: &BTreeSet<String>,
    authorities: &CoverageAuthorities<'_>,
    warnings: &mut BTreeSet<CoverageWarning>,
) -> Result<Relation<TestItem>, CoverageError> {
    let mut items = BTreeSet::new();
    for region in touched
        .iter()
        .filter(|region| region.layer.as_deref() == Some("evidencia"))
    {
        items.insert(TestItem {
            region: region.key.clone(),
            path: region.file.clone(),
            start: region.start_marker,
            end: region.end_marker,
            authority: code_authority(region, "layer=evidencia+span"),
        });
    }
    for identity in identities {
        let report = symbol_index::locate(authorities.code, authorities.docs, identity).map_err(
            |error| CoverageError::Authority {
                detail: error.to_string(),
            },
        )?;
        for candidate in report.candidates {
            for item in candidate.tests.items {
                items.insert(TestItem {
                    region: item.region,
                    path: item.path,
                    start: item.start,
                    end: item.end,
                    authority: Authority {
                        source: "symbol-index".to_string(),
                        path: item.authority.catalog,
                        record: item.authority.region,
                        field: item.authority.field,
                    },
                });
            }
        }
    }
    if items.is_empty() {
        let reason = if identities.is_empty() {
            "regiões tocadas sem identidade estrutural para vínculo de teste"
        } else {
            "nenhum vínculo test-for explícito publicado para os símbolos tocados"
        };
        warning(warnings, "W-DIFF-TEST-UNKNOWN", reason);
        Ok(Relation::unknown(reason))
    } else {
        Ok(Relation::known(items.into_iter().collect()))
    }
}

fn derive_projections(
    file: &ParsedFile,
    touched: &[&CodeRegion],
    authorities: &CoverageAuthorities<'_>,
    warnings: &mut BTreeSet<CoverageWarning>,
) -> Relation<ProjectionItem> {
    let mut items = BTreeSet::new();
    let mut unavailable = Vec::new();

    if let (Some(config), Some(manifests)) = (authorities.doc_config, authorities.manifests) {
        add_documentary_projections(file, config, manifests, &mut items, warnings);
    } else {
        unavailable.push("autoridade de projeções documentais indisponível");
    }

    if let Some(store) = authorities.projection_store {
        if !store.errors().is_empty() {
            unavailable.push("store .pinker/projections contém artefatos inválidos");
            warning(
                warnings,
                "W-DIFF-PROJECTION-HARNESS",
                "store .pinker/projections contém artefatos inválidos",
            );
        }
        add_navigation_projections(file, touched, authorities.code, store, &mut items, warnings);
    } else {
        unavailable.push("store .pinker/projections indisponível");
    }

    if !items.is_empty() {
        Relation::known(items.into_iter().collect())
    } else if !unavailable.is_empty() {
        let reason = unavailable.join("; ");
        warning(warnings, "W-DIFF-PROJECTION-UNAVAILABLE", &reason);
        Relation::unavailable(&reason)
    } else {
        let reason = "nenhuma projeção pôde ser relacionada por autoridade explícita";
        warning(warnings, "W-DIFF-PROJECTION-UNKNOWN", reason);
        Relation::unknown(reason)
    }
}

fn add_documentary_projections(
    file: &ParsedFile,
    config: &DocConfig,
    manifests: &Manifests,
    items: &mut BTreeSet<ProjectionItem>,
    warnings: &mut BTreeSet<CoverageWarning>,
) {
    for projection in &config.projections {
        if projection.file == file.path || file.path == ".pinker/doc.toml" {
            items.insert(documentary_projection_item(
                projection,
                ".pinker/doc.toml",
                "projections",
            ));
        }
    }
    let Some(pr) = manifest_number(&file.path) else {
        return;
    };
    let change = manifests
        .changes
        .iter()
        .find(|change| change.source.as_ref().map(|source| source.number) == Some(pr));
    let Some(change) = change else {
        warning(
            warnings,
            "W-DIFF-MANIFEST-UNKNOWN",
            "manifesto alterado não está disponível na autoridade corrente",
        );
        return;
    };
    for (name, enabled) in &change.updates {
        if !enabled {
            continue;
        }
        if let Some(projection) = config.projections.iter().find(|item| &item.name == name) {
            items.insert(documentary_projection_item(
                projection,
                &file.path,
                &format!("updates.{}", name),
            ));
        }
    }
}

fn documentary_projection_item(
    projection: &DocProjection,
    authority_path: &str,
    field: &str,
) -> ProjectionItem {
    ProjectionItem {
        id: projection.name.clone(),
        kind: "documentation".to_string(),
        path: projection.file.clone(),
        authority: Authority {
            source: "doc-projection-config".to_string(),
            path: authority_path.to_string(),
            record: Some(projection.region.clone()),
            field: field.to_string(),
        },
    }
}

fn manifest_number(path: &str) -> Option<u64> {
    path.strip_prefix(".pinker/changes/pr-")?
        .strip_suffix(".yaml")?
        .parse()
        .ok()
}

fn add_navigation_projections(
    file: &ParsedFile,
    touched: &[&CodeRegion],
    code: &CodeCatalog,
    store: &ProjectionStore,
    items: &mut BTreeSet<ProjectionItem>,
    warnings: &mut BTreeSet<CoverageWarning>,
) {
    let library = match store.library() {
        Ok(library) => Some(library),
        Err(error) => {
            warning(
                warnings,
                "W-DIFF-PROJECTION-HARNESS",
                &format!("biblioteca de projeções inválida: {}", error),
            );
            None
        }
    };
    let touched_keys = touched
        .iter()
        .map(|region| region.key.as_str())
        .collect::<BTreeSet<_>>();
    for stored in store.snapshots() {
        let direct = stored.path == file.path || file.path == "src/navigation.jsonl";
        let mut composed = false;
        let mut composed_recipe = false;
        if let Some(library) = &library {
            match nav_projection_recipe::resolve(library, &stored.snapshot.id, &code.regions) {
                Ok(composition) => {
                    composed = composition
                        .regions
                        .iter()
                        .any(|region| touched_keys.contains(region.key.as_str()));
                    if let Some(recipe_id) = file
                        .path
                        .strip_prefix(".pinker/projections/recipes/")
                        .and_then(|path| path.strip_suffix(".toml"))
                    {
                        composed_recipe = composition
                            .ledger
                            .iter()
                            .any(|entry| entry.scope == format!("recipe:{}", recipe_id));
                    }
                }
                Err(error) => warning(
                    warnings,
                    "W-DIFF-PROJECTION-HARNESS",
                    &format!(
                        "snapshot '{}' não pôde ser resolvido: {}",
                        stored.snapshot.id, error
                    ),
                ),
            }
        }
        if direct || composed || composed_recipe {
            items.insert(ProjectionItem {
                id: stored.snapshot.id.clone(),
                kind: "navigation-snapshot".to_string(),
                path: stored.path.clone(),
                authority: Authority {
                    source: "projection-store".to_string(),
                    path: stored.path.clone(),
                    record: Some(stored.snapshot.id.clone()),
                    field: if direct {
                        "input-path".to_string()
                    } else if composed_recipe {
                        "reconstruction.recipes".to_string()
                    } else {
                        "reconstruction-membership".to_string()
                    },
                },
            });
        }
    }
}
// @pinker-nav:end trama.diff-cobertura.derivacao

// @pinker-nav:start trama.diff-cobertura.renderizacao
// @pinker-nav:domain diff-coverage
// @pinker-nav:layer relatorios
// @pinker-nav:summary Renderizadores humano e JSON determinísticos derivados do mesmo CoverageReport, com ordem fixa, ausência explícita, paths repo-relativos e nenhum ANSI ou dado incidental.

pub fn render_json(report: &CoverageReport) -> String {
    format!(
        "{{\"schema\":{},\"source\":{},\"files\":[{}]}}",
        report.schema,
        json_string(&report.source),
        report
            .files
            .iter()
            .map(file_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub fn render_human(report: &CoverageReport) -> String {
    if report.files.is_empty() {
        return "Cobertura de diff: nenhum arquivo alterado.\n".to_string();
    }
    let mut out = format!("Cobertura de diff ({} arquivo(s))\n", report.files.len());
    for file in &report.files {
        out.push_str(&format!("\n{} [{}]\n", file.path, file.status.as_str()));
        if let Some(old) = &file.old_path {
            out.push_str(&format!("  path anterior: {}\n", old));
        }
        out.push_str("  linhas novas:");
        if file.changed_lines.is_empty() {
            out.push_str(" nenhuma\n");
        } else {
            out.push(' ');
            out.push_str(
                &file
                    .changed_lines
                    .iter()
                    .map(|range| {
                        if range.start == range.end {
                            range.start.to_string()
                        } else {
                            format!("{}-{}", range.start, range.end)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            out.push('\n');
        }
        append_relation(&mut out, "regiões", &file.regions, |item| {
            format!("{} em {}:{}-{}", item.id, item.path, item.start, item.end)
        });
        append_relation(&mut out, "documentos", &file.documents, |item| {
            format!("{} [{}] em {}", item.id, item.title, item.path)
        });
        append_relation(&mut out, "projeções", &file.projections, |item| {
            format!("{} ({}) em {}", item.id, item.kind, item.path)
        });
        append_relation(&mut out, "testes", &file.tests, |item| {
            format!(
                "{} em {}:{}-{}",
                item.region, item.path, item.start, item.end
            )
        });
        for warning in &file.warnings {
            out.push_str(&format!("  aviso {}: {}\n", warning.code, warning.message));
        }
    }
    out
}

fn append_relation<T, F>(out: &mut String, label: &str, relation: &Relation<T>, render: F)
where
    F: Fn(&T) -> String,
{
    out.push_str(&format!("  {}: {}", label, relation.status.as_str()));
    if let Some(reason) = &relation.reason {
        out.push_str(&format!(" ({})", reason));
    }
    out.push('\n');
    for item in &relation.items {
        out.push_str(&format!("    - {}\n", render(item)));
    }
}

fn file_json(file: &FileCoverage) -> String {
    format!(
        "{{\"path\":{},\"old_path\":{},\"status\":{},\"changed_lines\":[{}],\"regions\":{},\"documents\":{},\"projections\":{},\"tests\":{},\"warnings\":[{}]}}",
        json_string(&file.path),
        option_string(file.old_path.as_deref()),
        json_string(file.status.as_str()),
        file.changed_lines
            .iter()
            .map(|range| format!("{{\"start\":{},\"end\":{}}}", range.start, range.end))
            .collect::<Vec<_>>()
            .join(","),
        relation_json(&file.regions, region_json),
        relation_json(&file.documents, document_json),
        relation_json(&file.projections, projection_json),
        relation_json(&file.tests, test_json),
        file.warnings
            .iter()
            .map(|warning| format!(
                "{{\"code\":{},\"message\":{}}}",
                json_string(&warning.code),
                json_string(&warning.message)
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn relation_json<T>(relation: &Relation<T>, render: fn(&T) -> String) -> String {
    format!(
        "{{\"status\":{},\"reason\":{},\"items\":[{}]}}",
        json_string(relation.status.as_str()),
        option_string(relation.reason.as_deref()),
        relation
            .items
            .iter()
            .map(render)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn region_json(item: &RegionItem) -> String {
    format!(
        "{{\"id\":{},\"path\":{},\"start\":{},\"end\":{},\"authority\":{}}}",
        json_string(&item.id),
        json_string(&item.path),
        item.start,
        item.end,
        authority_json(&item.authority)
    )
}

fn document_json(item: &DocumentItem) -> String {
    format!(
        "{{\"id\":{},\"document\":{},\"path\":{},\"title\":{},\"authority\":{}}}",
        json_string(&item.id),
        json_string(&item.document),
        json_string(&item.path),
        json_string(&item.title),
        authority_json(&item.authority)
    )
}

fn projection_json(item: &ProjectionItem) -> String {
    format!(
        "{{\"id\":{},\"kind\":{},\"path\":{},\"authority\":{}}}",
        json_string(&item.id),
        json_string(&item.kind),
        json_string(&item.path),
        authority_json(&item.authority)
    )
}

fn test_json(item: &TestItem) -> String {
    format!(
        "{{\"region\":{},\"path\":{},\"start\":{},\"end\":{},\"authority\":{}}}",
        json_string(&item.region),
        json_string(&item.path),
        item.start,
        item.end,
        authority_json(&item.authority)
    )
}

fn authority_json(authority: &Authority) -> String {
    format!(
        "{{\"source\":{},\"path\":{},\"record\":{},\"field\":{}}}",
        json_string(&authority.source),
        json_string(&authority.path),
        option_string(authority.record.as_deref()),
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
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
// @pinker-nav:end trama.diff-cobertura.renderizacao

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_aceita_modificacao_adicao_delecao_rename_e_binario() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,3 @@\n antiga\n-removida\n+nova\n+outra\ndiff --git a/old.rs b/new.rs\nrename from old.rs\nrename to new.rs\ndiff --git a/blob b/blob\nBinary files a/blob and b/blob differ\n";
        let parsed = parse_unified_diff(diff).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].path, "blob");
        assert!(parsed[0].binary);
        assert_eq!(parsed[1].path, "new.rs");
        assert_eq!(parsed[1].status, FileStatus::Renamed);
        assert_eq!(
            parsed[2].changed_lines,
            vec![LineRange { start: 2, end: 3 }]
        );
    }

    #[test]
    fn parser_preserva_path_com_espacos_no_cabecalho_git() {
        let diff = "diff --git a/docs/com espaco.md b/docs/com espaco.md\n--- a/docs/com espaco.md\n+++ b/docs/com espaco.md\n@@ -1 +1 @@\n-a\n+b\n";
        let parsed = parse_unified_diff(diff).unwrap();
        assert_eq!(parsed[0].path, "docs/com espaco.md");
    }

    #[test]
    fn parser_rejeita_travessia_hunk_incompleto_e_path_nao_utf8() {
        let traversal = "--- a/src/a.rs\n+++ b/../fora.rs\n@@ -1 +1 @@\n-a\n+b\n";
        assert!(matches!(
            parse_unified_diff(traversal),
            Err(CoverageError::InvalidPath { .. })
        ));
        let short = "--- a/a\n+++ b/a\n@@ -1,2 +1,2 @@\n-a\n+b\n";
        assert!(matches!(
            parse_unified_diff(short),
            Err(CoverageError::InvalidFormat { .. })
        ));
        let quoted = "--- \"a/\\377\"\n+++ b/a\n";
        assert!(matches!(
            parse_unified_diff(quoted),
            Err(CoverageError::InvalidPath { .. })
        ));
    }

    #[test]
    fn delecao_pura_nao_fabrica_linha_atual() {
        let diff = "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -3 +2,0 @@\n-removida\n";
        let parsed = parse_unified_diff(diff).unwrap();
        assert!(parsed[0].changed_lines.is_empty());
        assert!(parsed[0].deletion_without_new_lines);
    }

    #[test]
    fn renderizadores_sao_deterministicos_e_sem_ansi() {
        let report = CoverageReport {
            schema: 1,
            source: "stdin-unified-diff".to_string(),
            files: Vec::new(),
        };
        assert_eq!(render_json(&report), render_json(&report));
        assert_eq!(render_human(&report), render_human(&report));
        assert!(!render_json(&report).contains('\u{1b}'));
    }
}
