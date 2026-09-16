//! Extração lexical delimitada de declarações Rust para `pink nav localizar`.
//!
//! Este módulo observa a fonte corrente do worktree; ele nunca consulta Git,
//! nunca expande macro e nunca avalia `cfg`. O que ele produz é observação
//! lexical, não identidade semântica: um nome visto aqui foi **grafado**, não
//! **resolvido**. A autoridade de identidade continua sendo o contrato
//! explícito de `symbol_index`.

// @pinker-nav:start trama.simbolos.extracao
// @pinker-nav:domain simbolos
// @pinker-nav:layer trama
// @pinker-nav:symbol pinker_v0::symbol_extraction::extend|extend|rust-function|declaration
// @pinker-nav:symbol pinker_v0::symbol_extraction::extend|extend|rust-function|implementation
// @pinker-nav:summary Bounded lexical extraction of ordinary Rust declarations from the current worktree, kept strictly below semantic identity: masked source hides comments, strings, raw strings and character literals, supported declaration keywords in opening position become EXTRACTED_CANDIDATE with structural context, every other word hit degrades to TEXTUAL_OCCURRENCE, explicit symbol locations keep precedence, declared limitations travel as stable English data, snippets match the observed interval, unstable files are reported instead of silently returned, and one deterministic budget paginates the three classes without any cache.

use crate::nav::{self, MarkerDialect};
use crate::symbol_index::{ExtractedCandidate, LocateReport, TextualOccurrence};
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

/// Orçamento determinístico de resultados por página, somando as três classes.
pub const LOCATE_RESULT_BUDGET: usize = 20;

/// Teto de caracteres de um trecho devolvido. Contado em caracteres, nunca em
/// bytes: o marcador de truncamento precisa descrever o que foi cortado.
pub const SNIPPET_CHAR_BUDGET: usize = 240;

/// Quantas linhas seguintes o extrator aceita percorrer para fechar o
/// intervalo de uma assinatura de declaração.
const SIGNATURE_LOOKAHEAD: usize = 12;

/// Limitações declaradas da observação lexical. São dados estáveis em inglês,
/// não prosa: a saída precisa dizer o que ela *não* prova.
pub const DECLARED_LIMITATIONS: [&str; 7] = [
    "macro_generated_declarations_not_expanded",
    "cfg_attributes_not_evaluated",
    "semantic_name_resolution_absent",
    "reexports_and_aliases_not_resolved",
    "declaration_keyword_must_open_line_modulo_modifiers",
    "comment_and_string_text_excluded_from_extraction",
    "declaration_interval_covers_signature_only",
];

/// Motivo estável de uma ocorrência textual.
const TEXTUAL_LIMITATION: &str = "not_a_supported_structural_declaration";

/// Palavras que podem preceder a palavra-chave de declaração sem que a linha
/// deixe de abrir uma declaração.
const MODIFIERS: [&str; 9] = [
    "pub", "crate", "super", "self", "in", "const", "async", "unsafe", "default",
];

/// Subconjunto suportado de palavras-chave de declaração e a categoria que
/// cada uma publica.
const DECLARATION_KEYWORDS: [(&str, &str); 6] = [
    ("fn", "function"),
    ("struct", "struct"),
    ("enum", "enum"),
    ("trait", "trait"),
    ("mod", "module"),
    ("type", "type_alias"),
];

/// Palavras-chave que abrem um contexto estrutural reconhecível.
const CONTAINER_KEYWORDS: [&str; 3] = ["impl", "trait", "mod"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionError {
    Roots(String),
    Read { path: String, message: String },
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtractionError::Roots(message) => {
                write!(f, "E-NAV-SOURCE\nRaízes oficiais indisponíveis: {message}")
            }
            ExtractionError::Read { path, message } => write!(
                f,
                "E-NAV-SOURCE\nFonte '{path}' não pôde ser lida: {message}"
            ),
        }
    }
}

/// Estende um relatório explícito com a observação lexical da fonte corrente e
/// aplica a janela determinística de orçamento sobre as três classes.
///
/// A ordem das classes é a precedência do contrato: identidade explícita
/// primeiro, candidato extraído depois, ocorrência textual por último. O
/// orçamento nunca desloca um `EXPLICIT_SYMBOL` para fora da página.
pub fn extend(
    repo_root: &Path,
    report: &mut LocateReport,
    offset: usize,
) -> Result<(), ExtractionError> {
    let query = report.query.clone();
    let files =
        nav::official_source_files(repo_root).map_err(|e| ExtractionError::Roots(e.to_string()))?;

    let mut extracted: Vec<ExtractedCandidate> = Vec::new();
    let mut textual: Vec<TextualOccurrence> = Vec::new();
    let mut unstable: Vec<String> = Vec::new();

    for file in files
        .into_iter()
        .filter(|file| file.dialect == MarkerDialect::Rust)
    {
        let absolute = repo_root.join(&file.path);
        let source = match read_stable(&absolute) {
            Ok(Some(source)) => source,
            Ok(None) => {
                unstable.push(file.path.clone());
                continue;
            }
            Err(error) => {
                return Err(ExtractionError::Read {
                    path: file.path.clone(),
                    message: error.to_string(),
                })
            }
        };
        let masked = nav::rust_code_mask(&source);
        scan_file(
            &file.path,
            &source,
            &masked,
            &query,
            &mut extracted,
            &mut textual,
        );
    }

    extracted.retain(|candidate| !covered_by_explicit(report, candidate));

    extracted.sort_by(|a, b| {
        (&a.path, a.start, &a.kind, &a.name).cmp(&(&b.path, b.start, &b.kind, &b.name))
    });
    textual.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));

    let total = report.candidates.len() + extracted.len() + textual.len();
    let mut remaining_offset = offset;
    let mut budget = LOCATE_RESULT_BUDGET;

    report.candidates = window(&report.candidates, &mut remaining_offset, &mut budget);
    report.extracted_candidates = window(&extracted, &mut remaining_offset, &mut budget);
    report.textual_occurrences = window(&textual, &mut remaining_offset, &mut budget);

    let shown = report.candidates.len()
        + report.extracted_candidates.len()
        + report.textual_occurrences.len();
    let consumed = offset.min(total) + shown;

    report.limitations = DECLARED_LIMITATIONS.iter().map(|l| l.to_string()).collect();
    report.unstable_sources = unstable;
    report.total = total;
    report.offset = offset;
    report.truncated = consumed < total;
    report.continuation = report.truncated.then_some(consumed);
    Ok(())
}

/// Recorta uma página de uma classe consumindo primeiro o deslocamento
/// pendente e depois o orçamento restante.
fn window<T: Clone>(items: &[T], remaining_offset: &mut usize, budget: &mut usize) -> Vec<T> {
    let skip = (*remaining_offset).min(items.len());
    *remaining_offset -= skip;
    let available = items.len() - skip;
    let take = (*budget).min(available);
    *budget -= take;
    items[skip..skip + take].to_vec()
}

/// Uma declaração já coberta por uma identidade explícita não é rebaixada a
/// candidato extraído: a precedência do contrato existente é preservada.
fn covered_by_explicit(report: &LocateReport, candidate: &ExtractedCandidate) -> bool {
    report.candidates.iter().any(|explicit| {
        explicit.name == candidate.name
            && explicit
                .declaration
                .items
                .iter()
                .chain(explicit.implementation.items.iter())
                .any(|location| {
                    location.path == candidate.path
                        && location.start <= candidate.start
                        && candidate.start <= location.end
                })
    })
}

/// Lê o arquivo duas vezes e compara o conteúdo. Se a fonte mudar entre as
/// leituras, tenta uma vez mais; persistindo a divergência, devolve `None`
/// para que o arquivo seja reportado como instável em vez de produzir um
/// trecho stale apresentado como corrente.
fn read_stable(path: &Path) -> io::Result<Option<String>> {
    read_stable_with(|| fs::read_to_string(path))
}

/// Núcleo testável de `read_stable`, parametrizado pela leitura para que a
/// instabilidade possa ser provada sem depender de corrida real de disco.
pub fn read_stable_with<F>(mut read: F) -> io::Result<Option<String>>
where
    F: FnMut() -> io::Result<String>,
{
    for _ in 0..2 {
        let first = read()?;
        let second = read()?;
        if first == second {
            return Ok(Some(first));
        }
    }
    Ok(None)
}

/// Percorre a visão mascarada de um arquivo e classifica cada linha. A fonte
/// original só é usada para compor o trecho devolvido; a decisão estrutural
/// vem sempre do texto mascarado, então comentário, string, string crua e
/// literal de caractere não podem virar declaração.
fn scan_file(
    path: &str,
    source: &str,
    masked: &str,
    query: &str,
    extracted: &mut Vec<ExtractedCandidate>,
    textual: &mut Vec<TextualOccurrence>,
) {
    if query.is_empty() {
        return;
    }
    let source_lines: Vec<&str> = source.lines().collect();
    let masked_lines: Vec<&str> = masked.lines().collect();
    let mut contexts: Vec<(usize, String)> = Vec::new();
    let mut depth: usize = 0;
    let mut attributes_present = false;
    let mut cfg_present = false;

    for (index, masked_line) in masked_lines.iter().enumerate() {
        let trimmed = masked_line.trim();
        while contexts.last().is_some_and(|(opened, _)| *opened > depth) {
            contexts.pop();
        }

        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            attributes_present = true;
            cfg_present = cfg_present || contains_word(trimmed, "cfg");
            depth = next_depth(depth, trimmed);
            continue;
        }

        let declared = declaration(trimmed);
        let structural_hit = declared.is_some_and(|(_, name)| name == query);

        if let Some((kind, name)) = declared {
            if name == query {
                let end = signature_end(&masked_lines, index);
                let (snippet, snippet_truncated) = snippet_for(&source_lines, index, end);
                extracted.push(ExtractedCandidate {
                    path: path.to_string(),
                    kind: kind.to_string(),
                    name: name.to_string(),
                    start: index + 1,
                    end: end + 1,
                    context: context_label(&contexts),
                    attributes_present,
                    cfg_present,
                    snippet,
                    snippet_truncated,
                });
            }
        }

        if !structural_hit && contains_word(trimmed, query) {
            let (snippet, snippet_truncated) = snippet_for(&source_lines, index, index);
            textual.push(TextualOccurrence {
                path: path.to_string(),
                line: index + 1,
                snippet,
                snippet_truncated,
                limitation: TEXTUAL_LIMITATION.to_string(),
            });
        }

        if let Some(label) = container_label(trimmed) {
            if trimmed.contains('{') {
                contexts.push((depth + 1, label));
            }
        }

        depth = next_depth(depth, trimmed);
        attributes_present = false;
        cfg_present = false;
    }
}

/// Saldo de chaves da linha mascarada. Chave dentro de string ou comentário já
/// foi apagada pela máscara e por isso não movimenta a profundidade.
fn next_depth(depth: usize, masked_line: &str) -> usize {
    let opens = masked_line.bytes().filter(|byte| *byte == b'{').count();
    let closes = masked_line.bytes().filter(|byte| *byte == b'}').count();
    depth.saturating_add(opens).saturating_sub(closes)
}

/// Reconhece uma declaração suportada quando a palavra-chave abre a linha,
/// admitidos apenas modificadores antes dela. A restrição é deliberada: ela
/// mantém o extrator previsível e transforma a sintaxe fora do subconjunto em
/// limitação declarada, nunca em identidade inventada.
fn declaration(masked_line: &str) -> Option<(&'static str, &str)> {
    let tokens = words(masked_line);
    for (position, token) in tokens.iter().enumerate() {
        if let Some((_, kind)) = DECLARATION_KEYWORDS
            .iter()
            .find(|(keyword, _)| keyword == token)
        {
            if !tokens[..position]
                .iter()
                .all(|earlier| MODIFIERS.contains(earlier))
            {
                return None;
            }
            let name = tokens.get(position + 1)?;
            if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
            if DECLARATION_KEYWORDS
                .iter()
                .any(|(keyword, _)| keyword == name)
            {
                return None;
            }
            return Some((kind, name));
        }
    }
    None
}

/// Rótulo do contexto aberto pela linha, preservado literalmente até a chave.
/// Preservar o texto observado evita inventar uma resolução entre `impl` e
/// `trait` que este módulo não possui.
fn container_label(masked_line: &str) -> Option<String> {
    let tokens = words(masked_line);
    let position = tokens
        .iter()
        .position(|token| CONTAINER_KEYWORDS.contains(token))?;
    if !tokens[..position]
        .iter()
        .all(|earlier| MODIFIERS.contains(earlier))
    {
        return None;
    }
    let label = masked_line.split('{').next()?.split_whitespace();
    let label = label.collect::<Vec<_>>().join(" ");
    (!label.is_empty()).then_some(label)
}

/// Contexto estrutural acumulado, do mais externo ao mais interno. Homônimos
/// em contextos diferentes recebem rótulos diferentes e por isso continuam
/// sendo resultados distintos.
fn context_label(contexts: &[(usize, String)]) -> Option<String> {
    (!contexts.is_empty()).then(|| {
        contexts
            .iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    })
}

/// Última linha da assinatura: a primeira que abre corpo ou termina a
/// declaração. Sem fechamento dentro do alcance, o intervalo permanece na
/// linha inicial, e o trecho continua correspondendo exatamente ao intervalo.
fn signature_end(masked_lines: &[&str], start: usize) -> usize {
    let limit = (start + SIGNATURE_LOOKAHEAD).min(masked_lines.len().saturating_sub(1));
    masked_lines
        .iter()
        .enumerate()
        .take(limit + 1)
        .skip(start)
        .find(|(_, line)| {
            let line = line.trim_end();
            line.contains('{') || line.ends_with(';')
        })
        .map_or(start, |(index, _)| index)
}

/// Compõe o trecho do intervalo observado e declara o corte quando ele existe.
/// O trecho nunca é apresentado como corpo integral.
fn snippet_for(source_lines: &[&str], start: usize, end: usize) -> (String, bool) {
    let end = end.min(source_lines.len().saturating_sub(1));
    let text = source_lines
        .get(start..=end)
        .unwrap_or_default()
        .iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    let text = text.trim().to_string();
    let truncated = text.chars().count() > SNIPPET_CHAR_BUDGET;
    let snippet = text.chars().take(SNIPPET_CHAR_BUDGET).collect();
    (snippet, truncated)
}

/// Palavras de identificador da linha mascarada.
fn words(masked_line: &str) -> Vec<&str> {
    masked_line
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .collect()
}

/// Igualdade de palavra inteira, nunca substring.
fn contains_word(masked_line: &str, query: &str) -> bool {
    !query.is_empty() && words(masked_line).iter().any(|word| *word == query)
}
// @pinker-nav:end trama.simbolos.extracao
