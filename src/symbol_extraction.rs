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
use std::ops::Range;
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
pub const DECLARED_LIMITATIONS: [&str; 8] = [
    "macro_generated_declarations_not_expanded",
    "macro_token_tree_not_a_declaration",
    "cfg_attributes_not_evaluated",
    "semantic_name_resolution_absent",
    "reexports_and_aliases_not_resolved",
    "declaration_keyword_must_open_line_modulo_modifiers",
    "comment_and_string_text_excluded_from_extraction",
    "declaration_interval_covers_signature_only",
];

/// Motivo estável de uma ocorrência textual comum.
const TEXTUAL_LIMITATION: &str = "not_a_supported_structural_declaration";

/// Motivo estável de uma ocorrência dentro de um token tree de macro, seja a
/// definição `macro_rules!` ou uma invocação qualquer. O texto ali só vira
/// declaração depois de uma expansão que este módulo não faz, e sintaxe que
/// parece Rust ordinário continua sendo apenas token.
const MACRO_TEMPLATE_LIMITATION: &str = "macro_token_tree_not_a_declaration";

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
///
/// A fronteira de macro é consultada por posição de byte, nunca por linha: uma
/// linha pode começar fora do token tree e continuar dentro, ou começar dentro
/// e terminar fora, e nos dois casos só a posição do token observado decide.
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
    let line_starts = line_offsets(masked);
    let mut contexts: Vec<(usize, String)> = Vec::new();
    let mut depth: usize = 0;
    let mut attributes_present = false;
    let mut cfg_present = false;
    // Intervalos de bytes ocupados pelos token trees de macro do arquivo.
    let macro_spans = macro_token_tree_spans(masked);

    for (index, masked_line) in masked_lines.iter().enumerate() {
        let trimmed = masked_line.trim();
        // Deslocamento absoluto do primeiro byte de `trimmed` dentro da visão
        // mascarada: é a partir dele que cada token recupera sua posição.
        let base = line_starts[index] + (masked_line.len() - masked_line.trim_start().len());
        while contexts.last().is_some_and(|(opened, _)| *opened > depth) {
            contexts.pop();
        }

        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            attributes_present = true;
            cfg_present = cfg_present || contains_word(trimmed, "cfg");
            depth = next_depth(depth, trimmed, base, &macro_spans);
            continue;
        }

        // Dentro de um token tree de macro a fonte não é Rust ordinário: o que
        // parece declaração é token que ainda não foi expandido. O teste é
        // sobre os tokens materiais do reconhecimento, não sobre a linha: uma
        // linha que contém macro pode ainda declarar fora dela.
        let declared = declaration(trimmed).filter(|found| {
            !inside_macro(&macro_spans, base + found.keyword_at)
                && !inside_macro(&macro_spans, base + found.name_at)
        });
        let structural_hit = declared.as_ref().is_some_and(|found| found.name == query);

        if let Some(found) = &declared {
            if found.name == query {
                let end = signature_end(&masked_lines, index);
                let (snippet, snippet_truncated) = snippet_for(&source_lines, index, end);
                extracted.push(ExtractedCandidate {
                    path: path.to_string(),
                    kind: found.kind.to_string(),
                    name: found.name.to_string(),
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

        // O fallback textual continua lendo a mesma visão mascarada: o token
        // tree nunca é apagado, só delimitado, então o texto interno segue
        // pesquisável — com a limitação de macro quando a palavra encontrada
        // cai dentro do intervalo.
        if !structural_hit {
            if let Some(found_at) = word_at(trimmed, query) {
                let (snippet, snippet_truncated) = snippet_for(&source_lines, index, index);
                textual.push(TextualOccurrence {
                    path: path.to_string(),
                    line: index + 1,
                    snippet,
                    snippet_truncated,
                    limitation: if inside_macro(&macro_spans, base + found_at) {
                        MACRO_TEMPLATE_LIMITATION.to_string()
                    } else {
                        TEXTUAL_LIMITATION.to_string()
                    },
                });
            }
        }

        // Chave e palavra-chave internas a token tree não abrem contexto Rust
        // ordinário para candidato externo.
        if let Some((keyword_at, label)) = container_label(trimmed) {
            let brace_at = trimmed.find('{');
            if !inside_macro(&macro_spans, base + keyword_at)
                && brace_at.is_some_and(|at| !inside_macro(&macro_spans, base + at))
            {
                contexts.push((depth + 1, label));
            }
        }

        depth = next_depth(depth, trimmed, base, &macro_spans);
        attributes_present = false;
        cfg_present = false;
    }
}

/// Saldo de chaves da linha mascarada, ignorando as que pertencem a um token
/// tree de macro. Chave dentro de string ou comentário já foi apagada pela
/// máscara e por isso também não movimenta a profundidade.
fn next_depth(depth: usize, masked_line: &str, base: usize, spans: &[Range<usize>]) -> usize {
    let mut depth = depth;
    for (offset, byte) in masked_line.bytes().enumerate() {
        if inside_macro(spans, base + offset) {
            continue;
        }
        match byte {
            b'{' => depth = depth.saturating_add(1),
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

/// Deslocamento de byte onde cada linha começa, na mesma ordem e na mesma
/// quantidade que `lines()`.
fn line_offsets(text: &str) -> Vec<usize> {
    let mut offset = 0;
    text.split_inclusive('\n')
        .map(|chunk| {
            let start = offset;
            offset += chunk.len();
            start
        })
        .collect()
}

/// Se a posição de byte pertence a algum token tree de macro.
fn inside_macro(spans: &[Range<usize>], position: usize) -> bool {
    spans.iter().any(|span| span.contains(&position))
}

/// Declaração reconhecida e onde os tokens materiais do reconhecimento foram
/// observados, em deslocamento de byte relativo à linha mascarada. A posição
/// existe porque a fronteira de macro é posicional: saber *que* uma declaração
/// foi reconhecida não basta, é preciso saber *onde*.
struct Declaration<'a> {
    kind: &'static str,
    name: &'a str,
    keyword_at: usize,
    name_at: usize,
}

/// Reconhece uma declaração suportada quando a palavra-chave abre a linha,
/// admitidos apenas modificadores antes dela. A restrição é deliberada: ela
/// mantém o extrator previsível e transforma a sintaxe fora do subconjunto em
/// limitação declarada, nunca em identidade inventada.
fn declaration(masked_line: &str) -> Option<Declaration<'_>> {
    // `$` fora de literal só existe em token tree de macro, e `words` apagaria
    // o sigilo: `pub fn $name()` viraria a declaração `name`. Uma metavariável
    // não é um nome declarado, então a linha inteira deixa de ser estrutural.
    if masked_line.contains('$') {
        return None;
    }
    let tokens = words(masked_line);
    for (position, (keyword_at, token)) in tokens.iter().enumerate() {
        if let Some((_, kind)) = DECLARATION_KEYWORDS
            .iter()
            .find(|(keyword, _)| keyword == token)
        {
            if !tokens[..position]
                .iter()
                .all(|(_, earlier)| MODIFIERS.contains(earlier))
            {
                return None;
            }
            let (name_at, name) = *tokens.get(position + 1)?;
            if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
            if DECLARATION_KEYWORDS
                .iter()
                .any(|(keyword, _)| *keyword == name)
            {
                return None;
            }
            return Some(Declaration {
                kind,
                name,
                keyword_at: *keyword_at,
                name_at,
            });
        }
    }
    None
}

/// Rótulo do contexto aberto pela linha, preservado literalmente até a chave, e
/// onde a palavra-chave do contêiner foi observada. Preservar o texto observado
/// evita inventar uma resolução entre `impl` e `trait` que este módulo não
/// possui; preservar a posição permite recusar um contêiner que só existe
/// dentro de um token tree de macro.
fn container_label(masked_line: &str) -> Option<(usize, String)> {
    let tokens = words(masked_line);
    let position = tokens
        .iter()
        .position(|(_, token)| CONTAINER_KEYWORDS.contains(token))?;
    if !tokens[..position]
        .iter()
        .all(|(_, earlier)| MODIFIERS.contains(earlier))
    {
        return None;
    }
    let label = masked_line.split('{').next()?.split_whitespace();
    let label = label.collect::<Vec<_>>().join(" ");
    (!label.is_empty()).then_some((tokens[position].0, label))
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

/// Palavras de identificador da linha mascarada, com o deslocamento de byte de
/// cada uma dentro da linha.
fn words(masked_line: &str) -> Vec<(usize, &str)> {
    let bytes = masked_line.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !is_identifier_byte(bytes[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && is_identifier_byte(bytes[index]) {
            index += 1;
        }
        found.push((start, &masked_line[start..index]));
    }
    found
}

/// Intervalos de bytes ocupados pelos token trees de macro da visão mascarada.
///
/// A gramática de Rust é a autoridade aqui: uma invocação é `SimplePath !
/// DelimTokenTree`, e o delimitador pode ser `(`, `[` ou `{`. `macro_rules!`
/// não é caso especial nenhum — `macro_rules` é um identificador como qualquer
/// outro, e sua definição também admite os três delimitadores. Espaço em branco
/// separa tokens normalmente, inclusive quebra de linha, então o `!` e o
/// delimitador podem estar em linhas diferentes.
///
/// Este módulo não expande macro, logo não tem autoridade para ler o conteúdo
/// de um token tree como Rust ordinário — nem quando ele parece Rust ordinário.
///
/// A granularidade é a posição de byte, não a linha: a mesma linha pode abrir
/// um token tree depois de código externo, ou fechá-lo antes de código externo,
/// e uma marca por linha mentiria nos dois casos. O intervalo devolvido cobre o
/// delimitador de abertura, o conteúdo e o delimitador de fechamento; aninhamento
/// é absorvido pelo intervalo mais externo. Um token tree que nunca fecha é
/// reportado até o fim do arquivo, porque a fonte deixou de ser Rust ordinário
/// dali em diante.
fn macro_token_tree_spans(masked: &str) -> Vec<Range<usize>> {
    let mut spans: Vec<Range<usize>> = Vec::new();
    let mut closers: Vec<u8> = Vec::new();
    let mut opened_at = 0;
    let mut awaiting_delimiter = false;
    let mut allow_macro_name = false;
    let mut path_tail: Vec<u8> = Vec::new();

    for (position, byte) in masked.bytes().enumerate() {
        if !closers.is_empty() {
            if let Some(closer) = closing_delimiter(byte) {
                closers.push(closer);
            } else if closers.last() == Some(&byte) {
                closers.pop();
                if closers.is_empty() {
                    spans.push(opened_at..position + 1);
                }
            }
            continue;
        }

        if byte == b'!' {
            // Um `!` precedido de caminho abre uma invocação; precedido de
            // espaço ou de `>` ele é negação ou o tipo `!`, nunca macro.
            if !path_tail.is_empty() {
                awaiting_delimiter = true;
                allow_macro_name = path_tail == b"macro_rules";
            }
            path_tail.clear();
            continue;
        }

        if awaiting_delimiter {
            if byte.is_ascii_whitespace() {
                continue;
            }
            if let Some(closer) = closing_delimiter(byte) {
                closers.push(closer);
                opened_at = position;
                awaiting_delimiter = false;
                allow_macro_name = false;
                continue;
            }
            // `macro_rules ! IDENTIFIER MacroRulesDef`: só essa forma tem um
            // nome entre o `!` e o delimitador.
            if allow_macro_name && is_identifier_byte(byte) {
                continue;
            }
            awaiting_delimiter = false;
            allow_macro_name = false;
        }

        if is_identifier_byte(byte) {
            path_tail.push(byte);
        } else {
            path_tail.clear();
        }
    }

    if !closers.is_empty() {
        spans.push(opened_at..masked.len());
    }
    spans
}

/// O fechamento correspondente de um delimitador de token tree.
fn closing_delimiter(byte: u8) -> Option<u8> {
    match byte {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        _ => None,
    }
}

/// Byte que pode terminar o caminho de uma invocação de macro. Serve para
/// separar `nome!` de `!=`, de negação e do tipo `!`, que nunca vêm depois de
/// um caractere de identificador.
fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Deslocamento da primeira palavra inteira igual à consulta, nunca substring.
fn word_at(masked_line: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    words(masked_line)
        .into_iter()
        .find(|(_, word)| *word == query)
        .map(|(offset, _)| offset)
}

/// Igualdade de palavra inteira, nunca substring.
fn contains_word(masked_line: &str, query: &str) -> bool {
    word_at(masked_line, query).is_some()
}
// @pinker-nav:end trama.simbolos.extracao
