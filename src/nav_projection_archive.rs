//! Arquivo histórico materializado da Trama.
//!
//! A história congelada da cartografia deixou de ser algo que o presente
//! reconstrói e passou a ser algo que o repositório guarda. Cada estado aceito
//! vive como os bytes exatos da sua projeção estável, e a verificação histórica
//! é integridade de arquivo: o payload existe, tem o comprimento preservado, o
//! FNV-1a64 preservado, a contagem de registros preservada e o SHA-256 que o
//! índice declara.
//!
//! O que esta autoridade deliberadamente **não** lê: `src/navigation.jsonl`,
//! chaves correntes, resumos correntes, hashes correntes, paths correntes,
//! mapa de renomeação e receitas de reconstrução. Uma mudança legítima do
//! catálogo corrente não toca em nenhum byte daqui.
//!
//! As medidas históricas continuam sendo `regions`, `length` e `fnv1a64`. O
//! SHA-256 é evidência adicional de integridade do arquivo e não recalibra
//! nem substitui nenhuma delas.

// @pinker-nav:start trama.archive.model
// @pinker-nav:domain archive
// @pinker-nav:layer trama
// @pinker-nav:summary Model of the materialized historical archive: the archive and preserved-metadata authorities with their canonical suffixes, the index schema, one entry per accepted snapshot with its preserved measures and its archive SHA-256, and the closed failure taxonomy of an unreadable, malformed, ambiguous or incomplete index plus an unconfined path, an accepted state the index omits, an entry naming no accepted state and a foreign file in the metadata authority — no current catalog field, no recipe and no reconstruction rule participates in this model.
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;

/// Diretório do arquivo histórico materializado, repo-relativo.
///
/// Fora de `.pinker/projections/` de propósito: aquele diretório é a autoridade
/// enumerada estritamente dos metadados FROZEN, e recusa qualquer entrada que
/// não seja um TOML de snapshot.
pub const ARCHIVE_DIR: &str = ".pinker/archive";

/// Índice de proveniência do arquivo, repo-relativo.
pub const INDEX_PATH: &str = ".pinker/archive/index.toml";

/// Autoridade dos metadados históricos preservados, repo-relativa.
///
/// É a raiz da história aceita: um TOML FROZEN por estado, e o `[measures]` de
/// cada um é a autoridade das medidas históricas. O índice do arquivo é uma
/// cópia escrita no cutover, e uma cópia nunca é a autoridade daquilo que copia.
pub const METADATA_DIR: &str = ".pinker/projections";

/// Sufixo canônico do payload materializado.
const PAYLOAD_SUFFIX: &str = ".stable";

/// Sufixo canônico do metadado histórico preservado.
const METADATA_SUFFIX: &str = ".toml";

/// Versão do formato do índice.
pub const ARCHIVE_SCHEMA: u64 = 1;

/// Prefixo canônico do FNV-1a64 nas medidas históricas.
const FNV_PREFIX: &str = "fnv1a64:";

/// Comprimento do SHA-256 na forma canônica hexadecimal minúscula.
const SHA256_LEN: usize = 64;

/// Uma entrada do índice: um estado histórico aceito e o que o prova.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub id: String,
    /// Metadado histórico original preservado, repo-relativo.
    pub metadata_path: String,
    /// SHA-256 dos bytes desse metadado no momento da exportação.
    pub metadata_sha256: String,
    /// Payload materializado, repo-relativo.
    pub payload_path: String,
    /// Medida histórica preservada: registros da projeção estável.
    pub regions: u64,
    /// Medida histórica preservada: comprimento em bytes.
    pub length: u64,
    /// Medida histórica preservada, na forma canônica `fnv1a64:<hex>`.
    pub fnv1a64: String,
    /// Integridade do arquivo. Adicional, nunca substituta das medidas.
    pub sha256: String,
}

/// O índice inteiro, com a proveniência que o torna auditável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveIndex {
    pub schema: u64,
    pub export_source_main: String,
    pub export_source_tree: String,
    pub export_method: String,
    /// Declaração explícita do que os bytes são e do que não são.
    pub provenance: String,
    pub entries: Vec<ArchiveEntry>,
}

/// Falhas da autoridade de arquivo. Nenhuma delas é drift do catálogo corrente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveFailure {
    Unreadable {
        path: String,
        msg: String,
    },
    Malformed {
        line: usize,
        msg: String,
    },
    UnknownField {
        scope: String,
        field: String,
    },
    MissingField {
        scope: String,
        field: String,
    },
    UnsupportedSchema {
        schema: u64,
    },
    Empty,
    DuplicateId {
        id: String,
    },
    DuplicatePath {
        path: String,
    },
    InvalidValue {
        scope: String,
        field: String,
        msg: String,
    },
    /// Um path do índice que não é o path canônico e confinado da sua entrada.
    UnconfinedPath {
        scope: String,
        field: String,
        path: String,
        msg: String,
    },
    /// Um estado histórico aceito que o índice não arquiva.
    MissingArchivedState {
        id: String,
    },
    /// Uma entrada do índice que não nomeia nenhum estado histórico aceito.
    UnknownArchivedState {
        id: String,
    },
    /// Um arquivo na autoridade dos metadados que não é um estado preservado.
    ForeignMetadata {
        path: String,
    },
}

impl fmt::Display for ArchiveFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveFailure::Unreadable { path, msg } => {
                write!(f, "E-ARCHIVE-IO\níndice de arquivo ilegível em '{path}': {msg}")
            }
            ArchiveFailure::Malformed { line, msg } => {
                write!(f, "E-ARCHIVE-TOML\nlinha {line}: {msg}")
            }
            ArchiveFailure::UnknownField { scope, field } => write!(
                f,
                "E-ARCHIVE-SCHEMA\ncampo desconhecido '{scope}{field}' no índice de arquivo"
            ),
            ArchiveFailure::MissingField { scope, field } => write!(
                f,
                "E-ARCHIVE-SCHEMA\ncampo obrigatório '{scope}{field}' ausente no índice de arquivo"
            ),
            ArchiveFailure::UnsupportedSchema { schema } => write!(
                f,
                "E-ARCHIVE-SCHEMA\nschema {schema} desconhecido para índice de arquivo; este formato aceita {ARCHIVE_SCHEMA}"
            ),
            ArchiveFailure::Empty => write!(
                f,
                "E-ARCHIVE-SCHEMA\no índice de arquivo precisa declarar ao menos uma entrada"
            ),
            ArchiveFailure::DuplicateId { id } => write!(
                f,
                "E-ARCHIVE-IDENTITY\nid '{id}' declarado em duas entradas do índice"
            ),
            ArchiveFailure::DuplicatePath { path } => write!(
                f,
                "E-ARCHIVE-IDENTITY\npath '{path}' declarado em duas entradas do índice"
            ),
            ArchiveFailure::InvalidValue { scope, field, msg } => write!(
                f,
                "E-ARCHIVE-SCHEMA\nvalor inválido em '{scope}{field}': {msg}"
            ),
            ArchiveFailure::UnconfinedPath {
                scope,
                field,
                path,
                msg,
            } => write!(
                f,
                "E-ARCHIVE-PATH\n'{scope}{field}' declara '{path}' fora da autoridade do arquivo: {msg}"
            ),
            ArchiveFailure::MissingArchivedState { id } => write!(
                f,
                "E-ARCHIVE-COVERAGE\no estado histórico aceito '{id}' não tem entrada no índice de arquivo"
            ),
            ArchiveFailure::UnknownArchivedState { id } => write!(
                f,
                "E-ARCHIVE-COVERAGE\na entrada '{id}' não corresponde a nenhum estado histórico aceito em '{METADATA_DIR}/'"
            ),
            ArchiveFailure::ForeignMetadata { path } => write!(
                f,
                "E-ARCHIVE-COVERAGE\n'{path}' não é um metadado histórico preservado"
            ),
        }
    }
}

impl std::error::Error for ArchiveFailure {}
// @pinker-nav:end trama.archive.model

// @pinker-nav:start trama.archive.reading
// @pinker-nav:domain archive
// @pinker-nav:layer trama
// @pinker-nav:summary Strict reader of the archive index: a root table of provenance fields plus one `[[entries]]` table per archived state, rejecting an unknown key, a duplicate key, a duplicate section, an unterminated string, an unsupported escape, trailing data after a value, a negative or overflowing integer, a measure outside its canonical form, a repeated id or payload path, an index that declares nothing, and a payload or metadata path that is not the lexically valid canonical path of its own entry — and a load that additionally proves one-to-one coverage against the enumerated preserved metadata, so an omitted accepted state, an entry naming no accepted state and a foreign file in that authority all fail before any consumer reads `entries`.

/// Valor escalar aceito pelo subconjunto TOML do índice.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Scalar {
    Text(String),
    Integer(u64),
}

/// Uma tabela em construção: pares na ordem de aparição, sem duplicidade.
#[derive(Debug, Default)]
struct Table {
    pairs: Vec<(String, Scalar)>,
}

impl Table {
    fn insert(&mut self, key: String, value: Scalar, line: usize) -> Result<(), ArchiveFailure> {
        if self.pairs.iter().any(|(existing, _)| existing == &key) {
            return Err(ArchiveFailure::Malformed {
                line,
                msg: format!("chave duplicada '{key}'"),
            });
        }
        self.pairs.push((key, value));
        Ok(())
    }

    fn get(&self, key: &str) -> Option<&Scalar> {
        self.pairs
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, value)| value)
    }

    fn keys(&self) -> Vec<&str> {
        self.pairs.iter().map(|(key, _)| key.as_str()).collect()
    }
}

const ROOT_KEYS: [&str; 5] = [
    "schema",
    "export_source_main",
    "export_source_tree",
    "export_method",
    "provenance",
];

const ENTRY_KEYS: [&str; 8] = [
    "id",
    "metadata_path",
    "metadata_sha256",
    "payload_path",
    "regions",
    "length",
    "fnv1a64",
    "sha256",
];

/// Lê um escalar do subconjunto aceito: string básica com escapes explícitos ou
/// inteiro sem sinal.
fn scalar(raw: &str, line: usize) -> Result<Scalar, ArchiveFailure> {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix('"') {
        let mut out = String::with_capacity(rest.len());
        let mut chars = rest.chars();
        loop {
            let Some(ch) = chars.next() else {
                return Err(ArchiveFailure::Malformed {
                    line,
                    msg: "string sem aspas de fechamento".to_string(),
                });
            };
            match ch {
                '"' => break,
                '\\' => match chars.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some(other) => {
                        return Err(ArchiveFailure::Malformed {
                            line,
                            msg: format!("escape não suportado '\\{other}'"),
                        })
                    }
                    None => {
                        return Err(ArchiveFailure::Malformed {
                            line,
                            msg: "escape incompleto".to_string(),
                        })
                    }
                },
                other => out.push(other),
            }
        }
        if chars.as_str().trim().is_empty() {
            return Ok(Scalar::Text(out));
        }
        return Err(ArchiveFailure::Malformed {
            line,
            msg: "dado residual após o valor".to_string(),
        });
    }
    match raw.parse::<u64>() {
        Ok(value) => Ok(Scalar::Integer(value)),
        Err(_) => Err(ArchiveFailure::Malformed {
            line,
            msg: format!("valor não reconhecido '{raw}'"),
        }),
    }
}

fn require_text(table: &Table, scope: &str, field: &str) -> Result<String, ArchiveFailure> {
    match table.get(field) {
        Some(Scalar::Text(value)) => Ok(value.clone()),
        Some(Scalar::Integer(_)) => Err(ArchiveFailure::InvalidValue {
            scope: scope.to_string(),
            field: field.to_string(),
            msg: "esperado texto".to_string(),
        }),
        None => Err(ArchiveFailure::MissingField {
            scope: scope.to_string(),
            field: field.to_string(),
        }),
    }
}

fn require_integer(table: &Table, scope: &str, field: &str) -> Result<u64, ArchiveFailure> {
    match table.get(field) {
        Some(Scalar::Integer(value)) => Ok(*value),
        Some(Scalar::Text(_)) => Err(ArchiveFailure::InvalidValue {
            scope: scope.to_string(),
            field: field.to_string(),
            msg: "esperado inteiro".to_string(),
        }),
        None => Err(ArchiveFailure::MissingField {
            scope: scope.to_string(),
            field: field.to_string(),
        }),
    }
}

fn reject_unknown(table: &Table, allowed: &[&str], scope: &str) -> Result<(), ArchiveFailure> {
    for key in table.keys() {
        if !allowed.contains(&key) {
            return Err(ArchiveFailure::UnknownField {
                scope: scope.to_string(),
                field: key.to_string(),
            });
        }
    }
    Ok(())
}

fn require_hex(value: &str, scope: &str, field: &str) -> Result<(), ArchiveFailure> {
    if value.len() == SHA256_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
    {
        return Ok(());
    }
    Err(ArchiveFailure::InvalidValue {
        scope: scope.to_string(),
        field: field.to_string(),
        msg: "esperado SHA-256 em 64 dígitos hexadecimais minúsculos".to_string(),
    })
}

fn require_fnv(value: &str, scope: &str, field: &str) -> Result<(), ArchiveFailure> {
    let hex = value.strip_prefix(FNV_PREFIX).unwrap_or("");
    if hex.len() == 16
        && hex
            .chars()
            .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch))
    {
        return Ok(());
    }
    Err(ArchiveFailure::InvalidValue {
        scope: scope.to_string(),
        field: field.to_string(),
        msg: format!("esperada a forma canônica '{FNV_PREFIX}<16 hex>'"),
    })
}

/// Interpreta o texto do índice. Não toca no filesystem.
pub fn parse_index(text: &str) -> Result<ArchiveIndex, ArchiveFailure> {
    let mut root = Table::default();
    let mut entries: Vec<Table> = Vec::new();
    let mut in_entry = false;

    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("[[") {
            let Some(name) = rest.strip_suffix("]]") else {
                return Err(ArchiveFailure::Malformed {
                    line: line_no,
                    msg: "cabeçalho de array de tabelas sem ']]'".to_string(),
                });
            };
            if name.trim() != "entries" {
                return Err(ArchiveFailure::Malformed {
                    line: line_no,
                    msg: format!("array de tabelas desconhecido '[[{}]]'", name.trim()),
                });
            }
            entries.push(Table::default());
            in_entry = true;
            continue;
        }
        if line.starts_with('[') {
            return Err(ArchiveFailure::Malformed {
                line: line_no,
                msg: "o índice de arquivo não tem seções nomeadas".to_string(),
            });
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(ArchiveFailure::Malformed {
                line: line_no,
                msg: "linha sem '='".to_string(),
            });
        };
        let key = key.trim().to_string();
        let value = scalar(value, line_no)?;
        if in_entry {
            entries
                .last_mut()
                .expect("entrada aberta")
                .insert(key, value, line_no)?;
        } else {
            root.insert(key, value, line_no)?;
        }
    }

    reject_unknown(&root, &ROOT_KEYS, "")?;
    let schema = require_integer(&root, "", "schema")?;
    if schema != ARCHIVE_SCHEMA {
        return Err(ArchiveFailure::UnsupportedSchema { schema });
    }
    let index = ArchiveIndex {
        schema,
        export_source_main: require_nonempty(&root, "", "export_source_main")?,
        export_source_tree: require_nonempty(&root, "", "export_source_tree")?,
        export_method: require_nonempty(&root, "", "export_method")?,
        provenance: require_nonempty(&root, "", "provenance")?,
        entries: build_entries(&entries)?,
    };
    Ok(index)
}

/// Proveniência declarada mas vazia não é proveniência.
fn require_nonempty(table: &Table, scope: &str, field: &str) -> Result<String, ArchiveFailure> {
    let value = require_text(table, scope, field)?;
    if value.trim().is_empty() {
        return Err(ArchiveFailure::InvalidValue {
            scope: scope.to_string(),
            field: field.to_string(),
            msg: "proveniência declarada vazia".to_string(),
        });
    }
    Ok(value)
}

/// Exige que um path declarado seja exatamente o path canônico da sua entrada.
///
/// Duas obrigações distintas, nesta ordem. Primeiro a política lexical já
/// validada do núcleo de automação, que recusa path vazio, absoluto, com
/// travessia, com componente degenerado, com barra invertida, com caractere de
/// controle ou longo demais. Depois a identidade canônica: o arquivo nomeia
/// cada payload e cada metadado pelo id do estado, então um path que não é
/// `<dir>/<id><sufixo>` não descreve aquela entrada, ainda que seja
/// lexicamente inocente.
fn require_canonical_path(
    declared: &str,
    id: &str,
    dir: &str,
    suffix: &str,
    scope: &str,
    field: &str,
) -> Result<(), ArchiveFailure> {
    let relative = crate::automation::RelativePath::new(declared).map_err(|cause| {
        ArchiveFailure::UnconfinedPath {
            scope: scope.to_string(),
            field: field.to_string(),
            path: declared.to_string(),
            msg: cause.to_string(),
        }
    })?;
    let canonical = format!("{dir}/{id}{suffix}");
    if relative.as_str() != canonical {
        return Err(ArchiveFailure::UnconfinedPath {
            scope: scope.to_string(),
            field: field.to_string(),
            path: declared.to_string(),
            msg: format!("esperado o path canônico '{canonical}'"),
        });
    }
    Ok(())
}

fn build_entries(tables: &[Table]) -> Result<Vec<ArchiveEntry>, ArchiveFailure> {
    if tables.is_empty() {
        return Err(ArchiveFailure::Empty);
    }
    let mut entries = Vec::with_capacity(tables.len());
    for (position, table) in tables.iter().enumerate() {
        let scope = format!("entries[{position}].");
        reject_unknown(table, &ENTRY_KEYS, &scope)?;
        let entry = ArchiveEntry {
            id: require_text(table, &scope, "id")?,
            metadata_path: require_text(table, &scope, "metadata_path")?,
            metadata_sha256: require_text(table, &scope, "metadata_sha256")?,
            payload_path: require_text(table, &scope, "payload_path")?,
            regions: require_integer(table, &scope, "regions")?,
            length: require_integer(table, &scope, "length")?,
            fnv1a64: require_text(table, &scope, "fnv1a64")?,
            sha256: require_text(table, &scope, "sha256")?,
        };
        require_hex(&entry.metadata_sha256, &scope, "metadata_sha256")?;
        require_hex(&entry.sha256, &scope, "sha256")?;
        require_fnv(&entry.fnv1a64, &scope, "fnv1a64")?;
        if entry.id.is_empty() {
            return Err(ArchiveFailure::InvalidValue {
                scope: scope.clone(),
                field: "id".to_string(),
                msg: "identidade vazia".to_string(),
            });
        }
        entries.push(entry);
    }

    let mut ids: BTreeSet<&str> = BTreeSet::new();
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for entry in &entries {
        if !ids.insert(entry.id.as_str()) {
            return Err(ArchiveFailure::DuplicateId {
                id: entry.id.clone(),
            });
        }
        if !paths.insert(entry.payload_path.as_str()) {
            return Err(ArchiveFailure::DuplicatePath {
                path: entry.payload_path.clone(),
            });
        }
    }

    // Confinamento canônico depois da identidade: um índice ambíguo é um
    // defeito mais fundamental que um path fora do lugar, e reportá-lo
    // primeiro diz ao leitor o que consertar antes.
    for (position, entry) in entries.iter().enumerate() {
        let scope = format!("entries[{position}].");
        require_canonical_path(
            &entry.payload_path,
            &entry.id,
            ARCHIVE_DIR,
            PAYLOAD_SUFFIX,
            &scope,
            "payload_path",
        )?;
        require_canonical_path(
            &entry.metadata_path,
            &entry.id,
            METADATA_DIR,
            METADATA_SUFFIX,
            &scope,
            "metadata_path",
        )?;
    }

    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(entries)
}

/// Enumera os estados históricos aceitos pela autoridade dos metadados.
///
/// Estritamente os TOML na raiz de `.pinker/projections/`. Um diretório aninhado
/// não é um estado — é o que sobrou de mecanismos aposentados, e ninguém o lê.
/// Um arquivo que não termina em `.toml` é recusado em vez de ignorado: ignorá-lo
/// permitiria retirar um estado da história apenas renomeando a sua extensão.
pub fn preserved_states(root: &Path) -> Result<Vec<String>, ArchiveFailure> {
    let dir = root.join(METADATA_DIR);
    let reader = fs::read_dir(dir).map_err(|error| ArchiveFailure::Unreadable {
        path: METADATA_DIR.to_string(),
        msg: error.to_string(),
    })?;
    let mut ids = Vec::new();
    for entry in reader {
        let entry = entry.map_err(|error| ArchiveFailure::Unreadable {
            path: METADATA_DIR.to_string(),
            msg: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        match name.strip_suffix(METADATA_SUFFIX) {
            Some(id) if !id.is_empty() => ids.push(id.to_string()),
            _ => {
                return Err(ArchiveFailure::ForeignMetadata {
                    path: format!("{METADATA_DIR}/{name}"),
                })
            }
        }
    }
    ids.sort();
    Ok(ids)
}

/// Exige correspondência um-para-um entre a história aceita e o índice.
///
/// A contagem sozinha não estabelece identidade: treze entradas podem ser treze
/// cópias do mesmo estado, ou doze estados mais um desconhecido. O que este
/// controle exige é que cada estado aceito tenha exatamente uma entrada e que
/// cada entrada nomeie exatamente um estado aceito.
fn require_complete_coverage(
    index: &ArchiveIndex,
    preserved: &[String],
) -> Result<(), ArchiveFailure> {
    let archived: BTreeSet<&str> = index
        .entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    for id in preserved {
        if !archived.contains(id.as_str()) {
            return Err(ArchiveFailure::MissingArchivedState { id: id.clone() });
        }
    }
    let accepted: BTreeSet<&str> = preserved.iter().map(String::as_str).collect();
    for entry in &index.entries {
        if !accepted.contains(entry.id.as_str()) {
            return Err(ArchiveFailure::UnknownArchivedState {
                id: entry.id.clone(),
            });
        }
    }
    Ok(())
}

/// Carrega o índice a partir da raiz do repositório.
///
/// O índice sozinho não é a autoridade do conjunto histórico: ele enumera o que
/// alguém escreveu nele. Por isso carregar inclui provar cobertura contra os
/// metadados preservados, antes que qualquer consumidor derive uma conclusão
/// de `entries`.
pub fn load(root: &Path) -> Result<ArchiveIndex, ArchiveFailure> {
    let path = root.join(INDEX_PATH);
    let text = fs::read_to_string(path).map_err(|error| ArchiveFailure::Unreadable {
        path: INDEX_PATH.to_string(),
        msg: error.to_string(),
    })?;
    let index = parse_index(&text)?;
    require_complete_coverage(&index, &preserved_states(root)?)?;
    Ok(index)
}
// @pinker-nav:end trama.archive.reading

// @pinker-nav:start trama.archive.verification
// @pinker-nav:domain archive
// @pinker-nav:layer trama
// @pinker-nav:summary Read-only integrity verification of the materialized archive anchored to the preserved FROZEN metadata: each entry reads its historical metadata first, requires it to still hash to what the index recorded and requires the index regions, length and FNV-1a64 to be exactly the literals of its `[measures]`, then measures the payload against those preserved measures plus the declared SHA-256 — so a coherent payload-and-index recalibration is ALTERED, never drift, and no current navigation catalog, key, summary, hash, path, rename map or recipe is read to decide it.

/// Uma medida divergente entre o índice e o payload observado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Divergence {
    pub measure: &'static str,
    pub expected: String,
    pub observed: String,
}

/// Resultado tipado de uma entrada verificada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryOutcome {
    Intact,
    Missing { path: String, msg: String },
    Altered(Vec<Divergence>),
}

impl EntryOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntryOutcome::Intact => "INTACT",
            EntryOutcome::Missing { .. } => "MISSING",
            EntryOutcome::Altered(_) => "ALTERED",
        }
    }
}

/// Relatório de uma entrada do arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryReport {
    pub id: String,
    pub payload_path: String,
    pub metadata_path: String,
    pub regions: u64,
    pub length: u64,
    pub fnv1a64: String,
    pub sha256: String,
    pub outcome: EntryOutcome,
}

/// Relatório do arquivo inteiro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveVerification {
    pub index_path: String,
    pub export_source_main: String,
    pub export_source_tree: String,
    pub entries: Vec<EntryReport>,
}

impl ArchiveVerification {
    /// `INTACT` apenas quando toda entrada está íntegra.
    pub fn outcome(&self) -> &'static str {
        if self
            .entries
            .iter()
            .any(|entry| matches!(entry.outcome, EntryOutcome::Missing { .. }))
        {
            return "MISSING";
        }
        if self
            .entries
            .iter()
            .any(|entry| matches!(entry.outcome, EntryOutcome::Altered(_)))
        {
            return "ALTERED";
        }
        "INTACT"
    }
}

/// FNV-1a64 sobre bytes crus.
///
/// É a mesma função que definiu as medidas históricas quando elas ainda nasciam
/// de reconstrução. Ela permanece aqui porque a medida preservada só significa
/// alguma coisa se puder ser recalculada sobre o payload materializado.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Forma canônica do FNV-1a64.
pub fn fnv1a64_canonical(bytes: &[u8]) -> String {
    format!("{}{:016x}", FNV_PREFIX, fnv1a64(bytes))
}

/// As medidas históricas como o metadado FROZEN preservado as declara.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenMeasures {
    pub regions: u64,
    pub length: u64,
    pub fnv1a64: String,
}

/// Lê `[measures]` de um metadado histórico preservado.
///
/// Leitor mínimo e específico do arquivo, deliberadamente escopado à seção: o
/// mesmo TOML carrega `[reconstruction]` e `[[rules]]`, que também trazem
/// contagens e hashes, e um leitor frouxo confundiria uma regra com uma medida.
/// Nada aqui interpreta essas seções — elas descrevem um mecanismo aposentado e
/// permanecem nos bytes apenas porque os bytes preservados não se editam.
pub fn frozen_measures(text: &str) -> Result<FrozenMeasures, String> {
    let mut regions = None;
    let mut length = None;
    let mut fnv1a64 = None;
    let mut inside = false;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            inside = line == "[measures]";
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"');
        match key.trim() {
            "regions" => {
                regions = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("regions não é um inteiro: '{value}'"))?,
                )
            }
            "length" => {
                length = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("length não é um inteiro: '{value}'"))?,
                )
            }
            "fnv1a64" => fnv1a64 = Some(value.to_string()),
            _ => {}
        }
    }
    Ok(FrozenMeasures {
        regions: regions.ok_or_else(|| "[measures] não declara regions".to_string())?,
        length: length.ok_or_else(|| "[measures] não declara length".to_string())?,
        fnv1a64: fnv1a64.ok_or_else(|| "[measures] não declara fnv1a64".to_string())?,
    })
}

/// Registros de uma projeção estável: um por linha terminada em `\n`.
fn record_count(bytes: &[u8]) -> u64 {
    bytes.iter().filter(|byte| **byte == b'\n').count() as u64
}

fn divergence(
    measure: &'static str,
    expected: impl fmt::Display,
    observed: impl fmt::Display,
) -> Divergence {
    Divergence {
        measure,
        expected: expected.to_string(),
        observed: observed.to_string(),
    }
}

/// Verifica uma entrada contra os bytes que o repositório guarda.
///
/// A ordem é a da autoridade, não a da conveniência: o metadado histórico
/// preservado vem primeiro porque é ele que diz quais são as medidas, o índice
/// é obrigado a repetir o que ele diz, e só então o payload é medido contra
/// elas. Verificar o payload contra o índice sozinho deixaria a história
/// recalibrável por uma edição coerente dos dois.
pub fn verify_entry(root: &Path, entry: &ArchiveEntry) -> EntryReport {
    let outcome = verify_outcome(root, entry);
    EntryReport {
        id: entry.id.clone(),
        payload_path: entry.payload_path.clone(),
        metadata_path: entry.metadata_path.clone(),
        regions: entry.regions,
        length: entry.length,
        fnv1a64: entry.fnv1a64.clone(),
        sha256: entry.sha256.clone(),
        outcome,
    }
}

fn verify_outcome(root: &Path, entry: &ArchiveEntry) -> EntryOutcome {
    let metadata = match fs::read(root.join(&entry.metadata_path)) {
        Err(error) => {
            return EntryOutcome::Missing {
                path: entry.metadata_path.clone(),
                msg: error.to_string(),
            }
        }
        Ok(metadata) => metadata,
    };
    let mut divergences = Vec::new();
    let observed_metadata_sha = pinker_sha256_contract::sha256_hex(&metadata);
    if observed_metadata_sha != entry.metadata_sha256 {
        divergences.push(divergence(
            "metadata_sha256",
            &entry.metadata_sha256,
            observed_metadata_sha,
        ));
    }

    // A terceira aresta: o índice ainda diz o que o TOML congelado sempre disse.
    let frozen = match std::str::from_utf8(&metadata)
        .map_err(|error| error.to_string())
        .and_then(frozen_measures)
    {
        Ok(frozen) => {
            if frozen.regions != entry.regions {
                divergences.push(divergence("frozen_regions", frozen.regions, entry.regions));
            }
            if frozen.length != entry.length {
                divergences.push(divergence("frozen_length", frozen.length, entry.length));
            }
            if frozen.fnv1a64 != entry.fnv1a64 {
                divergences.push(divergence(
                    "frozen_fnv1a64",
                    &frozen.fnv1a64,
                    &entry.fnv1a64,
                ));
            }
            Some(frozen)
        }
        Err(msg) => {
            divergences.push(divergence(
                "frozen_measures",
                "regions, length e fnv1a64 preservados",
                msg,
            ));
            None
        }
    };

    let payload = match fs::read(root.join(&entry.payload_path)) {
        Err(error) => {
            return EntryOutcome::Missing {
                path: entry.payload_path.clone(),
                msg: error.to_string(),
            }
        }
        Ok(payload) => payload,
    };

    // Medido contra a medida preservada. O índice só serve de referência
    // quando o metadado histórico não pôde ser lido — e nesse caso a
    // divergência de `frozen_measures` já marcou a entrada.
    let (regions, length, fnv1a64) = match &frozen {
        Some(frozen) => (frozen.regions, frozen.length, frozen.fnv1a64.as_str()),
        None => (entry.regions, entry.length, entry.fnv1a64.as_str()),
    };
    let observed_length = payload.len() as u64;
    if observed_length != length {
        divergences.push(divergence("length", length, observed_length));
    }
    let observed_regions = record_count(&payload);
    if observed_regions != regions {
        divergences.push(divergence("regions", regions, observed_regions));
    }
    let observed_fnv = fnv1a64_canonical(&payload);
    if observed_fnv != fnv1a64 {
        divergences.push(divergence("fnv1a64", fnv1a64, observed_fnv));
    }
    let observed_sha = pinker_sha256_contract::sha256_hex(&payload);
    if observed_sha != entry.sha256 {
        divergences.push(divergence("sha256", &entry.sha256, observed_sha));
    }

    if divergences.is_empty() {
        EntryOutcome::Intact
    } else {
        EntryOutcome::Altered(divergences)
    }
}

/// Verifica o arquivo inteiro. Somente leitura.
pub fn verify(root: &Path, index: &ArchiveIndex) -> ArchiveVerification {
    ArchiveVerification {
        index_path: INDEX_PATH.to_string(),
        export_source_main: index.export_source_main.clone(),
        export_source_tree: index.export_source_tree.clone(),
        entries: index
            .entries
            .iter()
            .map(|entry| verify_entry(root, entry))
            .collect(),
    }
}
// @pinker-nav:end trama.archive.verification

// @pinker-nav:start trama.archive.report
// @pinker-nav:domain archive
// @pinker-nav:layer reports
// @pinker-nav:summary Deterministic human and JSON renderers of the archive over the same model: fixed key order, explicit escaping, repo-relative paths only, and no ANSI, PID, user, locale or clock — listing, inspection of one entry and full verification all derive from the model the reader produced.

/// Escapa um texto para string JSON.
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
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn entry_json(report: &EntryReport) -> String {
    let mut out = format!(
        "{{\"id\":{},\"payload\":{},\"metadata\":{},\"regions\":{},\"length\":{},\"fnv1a64\":{},\"sha256\":{},\"outcome\":{}",
        json_string(&report.id),
        json_string(&report.payload_path),
        json_string(&report.metadata_path),
        report.regions,
        report.length,
        json_string(&report.fnv1a64),
        json_string(&report.sha256),
        json_string(report.outcome.as_str())
    );
    match &report.outcome {
        EntryOutcome::Intact => {}
        EntryOutcome::Missing { path, msg } => {
            out.push_str(&format!(
                ",\"missing\":{},\"detail\":{}",
                json_string(path),
                json_string(msg)
            ));
        }
        EntryOutcome::Altered(divergences) => {
            let items = divergences
                .iter()
                .map(|divergence| {
                    format!(
                        "{{\"measure\":{},\"expected\":{},\"observed\":{}}}",
                        json_string(divergence.measure),
                        json_string(&divergence.expected),
                        json_string(&divergence.observed)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(",\"divergences\":[{items}]"));
        }
    }
    out.push('}');
    out
}

/// Inventário humano do arquivo.
pub fn render_inventory_human(index: &ArchiveIndex) -> String {
    let mut out = String::from("arquivo histórico materializado\n");
    out.push_str(&format!("origem: {}\n", index.export_source_main));
    out.push_str(&format!("árvore: {}\n", index.export_source_tree));
    for entry in &index.entries {
        out.push_str(&format!(
            "{} regioes={} comprimento={} {}\n",
            entry.id, entry.regions, entry.length, entry.fnv1a64
        ));
    }
    out
}

/// Inventário JSON determinístico.
pub fn render_inventory_json(index: &ArchiveIndex) -> String {
    let entries = index
        .entries
        .iter()
        .map(|entry| {
            format!(
                "{{\"id\":{},\"payload\":{},\"metadata\":{},\"regions\":{},\"length\":{},\"fnv1a64\":{},\"sha256\":{}}}",
                json_string(&entry.id),
                json_string(&entry.payload_path),
                json_string(&entry.metadata_path),
                entry.regions,
                entry.length,
                json_string(&entry.fnv1a64),
                json_string(&entry.sha256)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"export_source_main\":{},\"export_source_tree\":{},\"export_method\":{},\"provenance\":{},\"entries\":[{}]}}",
        index.schema,
        json_string(&index.export_source_main),
        json_string(&index.export_source_tree),
        json_string(&index.export_method),
        json_string(&index.provenance),
        entries
    )
}

/// Uma entrada, em texto humano.
pub fn render_entry_human(index: &ArchiveIndex, report: &EntryReport) -> String {
    let mut out = format!("{}\n", report.id);
    out.push_str(&format!("payload: {}\n", report.payload_path));
    out.push_str(&format!("metadado histórico: {}\n", report.metadata_path));
    out.push_str(&format!(
        "medidas preservadas: regioes={} comprimento={} {}\n",
        report.regions, report.length, report.fnv1a64
    ));
    out.push_str(&format!("sha256: {}\n", report.sha256));
    out.push_str(&format!("integridade: {}\n", report.outcome.as_str()));
    out.push_str(&format!("proveniência: {}\n", index.provenance));
    out
}

/// Uma entrada, em JSON determinístico.
pub fn render_entry_json(index: &ArchiveIndex, report: &EntryReport) -> String {
    format!(
        "{{\"schema\":{},\"provenance\":{},\"entry\":{}}}",
        index.schema,
        json_string(&index.provenance),
        entry_json(report)
    )
}

/// Verificação completa, em texto humano.
pub fn render_verification_human(verification: &ArchiveVerification) -> String {
    let mut out = format!("verificar: {}\n", verification.outcome());
    for entry in &verification.entries {
        out.push_str(&format!(
            "{} {} regioes={} comprimento={} {}\n",
            entry.payload_path,
            entry.outcome.as_str(),
            entry.regions,
            entry.length,
            entry.fnv1a64
        ));
        match &entry.outcome {
            EntryOutcome::Intact => {}
            EntryOutcome::Missing { path, msg } => {
                out.push_str(&format!("  ausente: {path}: {msg}\n"));
            }
            EntryOutcome::Altered(divergences) => {
                for divergence in divergences {
                    out.push_str(&format!(
                        "  {}: esperado {} observado {}\n",
                        divergence.measure, divergence.expected, divergence.observed
                    ));
                }
            }
        }
    }
    out
}

/// Verificação completa, em JSON determinístico de uma linha.
pub fn render_verification_json(verification: &ArchiveVerification) -> String {
    let entries = verification
        .entries
        .iter()
        .map(entry_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"index\":{},\"export_source_main\":{},\"export_source_tree\":{},\"outcome\":{},\"entries\":[{}]}}",
        ARCHIVE_SCHEMA,
        json_string(&verification.index_path),
        json_string(&verification.export_source_main),
        json_string(&verification.export_source_tree),
        json_string(verification.outcome()),
        entries
    )
}

/// Erro do arquivo, em JSON determinístico.
pub fn render_failure_json(command: &str, failure: &ArchiveFailure) -> String {
    format!(
        "{{\"schema\":{},\"command\":{},\"outcome\":\"ARCHIVE_FAILURE\",\"error\":{}}}",
        ARCHIVE_SCHEMA,
        json_string(command),
        json_string(&failure.to_string())
    )
}
// @pinker-nav:end trama.archive.report
