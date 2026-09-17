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
// @pinker-nav:summary Bounded lexical extraction of ordinary Rust declarations from the current worktree, kept strictly below semantic identity: masked source hides comments, strings, raw strings and character literals, supported declaration keywords in opening position become EXTRACTED_CANDIDATE with structural context, every other word hit degrades to TEXTUAL_OCCURRENCE, explicit symbol locations keep precedence, declared limitations travel as stable English data, snippets match the observed interval, unstable files are reported instead of silently returned, one deterministic budget paginates the three classes without any cache, and the macro boundary is lexical: `SimplePath ! DelimTokenTree` and `macro_rules ! IDENTIFIER MacroRulesDef` are recognised over tokens separated by Rust `Pattern_White_Space` and ordinary comments but not by doc comments, every path segment is classified by one edition 2021 rule that keeps weak keywords and the `self`/`super`/`crate` grammar segments while refusing strict keywords, reserved forms and reserved raw identifiers, and a path whose Unicode class this module cannot decide suppresses structural promotion only up to its closing delimiter under a declared conservative limitation instead of publishing a proven macro.

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
pub const DECLARED_LIMITATIONS: [&str; 9] = [
    "macro_generated_declarations_not_expanded",
    "macro_token_tree_not_a_declaration",
    "unicode_identifier_class_conservatively_approximated",
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

/// Motivo estável de uma ocorrência dentro de um token tree cujo caminho não
/// pôde ser *provado* identificador. A supressão de promoção estrutural vale
/// igual — o conteúdo não vira declaração —, mas a saída não afirma que ali
/// existe macro: `POTENTIAL_MACRO` não é `PROVEN_MACRO`, e publicar a mesma
/// etiqueta dos dois casos seria vender certeza que este módulo não tem.
const POSSIBLE_MACRO_LIMITATION: &str = "possible_macro_token_tree_not_a_declaration";

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
        let doc_comments = nav::rust_doc_comment_spans(&source);
        scan_file(
            &file.path,
            &source,
            &masked,
            &doc_comments,
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
    doc_comments: &[Range<usize>],
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
    let macro_spans = macro_token_tree_spans(masked, doc_comments);

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
                    limitation: match macro_span_at(&macro_spans, base + found_at) {
                        Some(span) if span.proven => MACRO_TEMPLATE_LIMITATION.to_string(),
                        Some(_) => POSSIBLE_MACRO_LIMITATION.to_string(),
                        None => TEXTUAL_LIMITATION.to_string(),
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
fn next_depth(depth: usize, masked_line: &str, base: usize, spans: &[MacroSpan]) -> usize {
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
fn inside_macro(spans: &[MacroSpan], position: usize) -> bool {
    macro_span_at(spans, position).is_some()
}

/// O token tree que cobre a posição de byte, para que o chamador leia o grau de
/// certeza do reconhecimento junto com o fato da cobertura.
fn macro_span_at(spans: &[MacroSpan], position: usize) -> Option<&MacroSpan> {
    spans.iter().find(|span| span.range.contains(&position))
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

/// Palavras-chave estritas de Rust na edition 2021. Uma palavra-chave estrita
/// não é identificador, logo não pode ser segmento de um `SimplePath` — e é
/// essa distinção que separa `some_macro !` de `return !`, que é negação.
/// `async`, `await` e `dyn` passaram a estritas na edition 2018 e continuam
/// estritas aqui.
const STRICT_KEYWORDS: [&str; 38] = [
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn",
];

/// Palavras reservadas para uso futuro *nesta* edition. Também não são
/// identificadores, mas a lista pertence à edition e não à linguagem em geral:
/// `gen` só é reservada a partir da edition 2024, e esta crate declara
/// `edition = "2021"`. Importar a política de 2024 recusaria `gen ! { … }`, que
/// a toolchain pinada aceita.
const RESERVED_FOR_EDITION_2021: [&str; 13] = [
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
    "unsized", "virtual", "yield",
];

/// Palavras-chave fracas: contextuais, e portanto identificadores fora do
/// contexto que lhes dá sentido. `union ! { … }` é invocação de macro na
/// edition 2021, e classificá-la como estrita publicaria o conteúdo do token
/// tree como declaração ordinária. `macro_rules` é a outra: ela não é
/// palavra-chave nenhuma, é o identificador cuja forma de definição carrega o
/// nome da macro entre o `!` e o delimitador.
const WEAK_KEYWORDS: [&str; 2] = ["macro_rules", "union"];

/// Segmentos que `SimplePathSegment` admite além de IDENTIFIER, apesar de serem
/// palavras-chave estritas. Recusá-los só por serem palavras-chave rejeitaria
/// `crate::declara ! { … }`, que a gramática e a toolchain pinada aceitam.
///
/// `$crate` também pertence à gramática, mas só existe dentro do corpo de uma
/// definição de macro — isto é, dentro de um token tree já delimitado. Este
/// módulo não expande macro e não tem autoridade de higiene, então `$` segue
/// sendo token incompatível: transformar `$qualquer_coisa` em caminho ampliaria
/// o produto em vez de aplicar a gramática.
const SPECIAL_SIMPLE_PATH_SEGMENTS: [&str; 3] = ["super", "self", "crate"];

/// Formas que `r#` não transforma em identificador cru. `r#` não é passe livre:
/// a linguagem recusa exatamente estas cinco, e aceitá-las indiscriminadamente
/// abriria um token tree em `r#crate ! { … }`, que a toolchain pinada rejeita.
const RESERVED_RAW_IDENTIFIERS: [&str; 5] = ["_", "crate", "self", "Self", "super"];

/// Espaço em branco de Rust, que é `Pattern_White_Space` e não ASCII.
/// `is_ascii_whitespace` não serve como equivalente: ele omite a tabulação
/// vertical e todas as cinco formas não-ASCII, e a sequência
/// `SimplePath ! DelimTokenTree` precisa ser invariante às formas que a
/// toolchain aceita. Confirmado contra rustc 1.78.0 / edition 2021, que recusa
/// `U+00A0` e `U+3000` como `unknown start of token` — por isso eles não estão
/// aqui.
const RUST_WHITESPACE: [char; 11] = [
    '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{0085}', '\u{200E}',
    '\u{200F}', '\u{2028}', '\u{2029}',
];

/// Se a palavra ASCII é IDENTIFIER pela classificação da edition. Palavra-chave
/// fraca é identificador mesmo aparecendo em qualquer outra lista: a
/// contextualidade é a definição dela, não uma exceção.
fn word_is_identifier(word: &str) -> bool {
    // `_` é o curinga, não um identificador: `r#_` é recusado pela linguagem
    // pela mesma razão.
    word != "_"
        && (WEAK_KEYWORDS.contains(&word)
            || (!STRICT_KEYWORDS.contains(&word) && !RESERVED_FOR_EDITION_2021.contains(&word)))
}

/// Classe léxica de um token material para o reconhecimento de macro. Só
/// existem as classes que a sequência reconhecida consome; todo o resto é
/// `Incompatible` e cancela a tentativa em curso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    /// Identificador ASCII, classificado exatamente contra as palavras-chave da
    /// edition.
    Identifier,
    /// `r#nome`: identificador cru ASCII, nunca palavra-chave.
    RawIdentifier,
    /// Corrida que contém caractere não-ASCII. Rust define identificador por
    /// `XID_Start`/`XID_Continue`, e este módulo não carrega as tabelas do
    /// Unicode nem pode adquirir dependência para obtê-las, então a classe é
    /// aproximada por excesso: a corrida *pode* ser identificador, e nada aqui
    /// prova que ela é. `POTENTIAL_MACRO` não é `PROVEN_MACRO`.
    Uncertain,
    PathSeparator,
    Bang,
    /// Delimitador de abertura, carregando o fechamento que lhe corresponde.
    Open(u8),
    Close(u8),
    Incompatible,
}

/// Token da visão mascarada, com o deslocamento de byte onde começa e o texto
/// quando ele é identificador ASCII.
struct Token<'a> {
    kind: TokenKind,
    start: usize,
    text: &'a str,
}

/// Tokeniza a visão mascarada nas classes que o reconhecimento de macro
/// consome.
///
/// Espaço em branco separa tokens e não é token. Comentário comum já virou
/// espaço na máscara e por isso também separa. Comentário de documentação,
/// porém, é atributo em Rust, não espaço em branco: seus bytes chegam aqui
/// apagados como qualquer comentário, então os intervalos preservados por
/// `rust_doc_comment_spans` são reintroduzidos como token incompatível — tratá-lo
/// como separador reconheceria uma sequência que a linguagem recusa.
fn macro_tokens<'a>(masked: &'a str, doc_comments: &[Range<usize>]) -> Vec<Token<'a>> {
    let mut tokens: Vec<Token<'a>> = Vec::new();
    let mut doc = 0;
    let mut index = 0;

    while index < masked.len() {
        while doc < doc_comments.len() && doc_comments[doc].end <= index {
            doc += 1;
        }
        if let Some(span) = doc_comments.get(doc).filter(|span| span.contains(&index)) {
            push_token(&mut tokens, TokenKind::Incompatible, index, "");
            index = span.end;
            continue;
        }

        let rest = &masked[index..];
        let character = rest.chars().next().unwrap_or('\0');
        if RUST_WHITESPACE.contains(&character) {
            index += character.len_utf8();
            continue;
        }

        if let Some((length, valid)) = raw_identifier(rest) {
            let text = &rest[2..length];
            let kind = match (valid, text.is_ascii()) {
                (false, _) => TokenKind::Incompatible,
                (true, true) => TokenKind::RawIdentifier,
                (true, false) => TokenKind::Uncertain,
            };
            push_token(&mut tokens, kind, index, "");
            index += length;
            continue;
        }

        // A corrida precisa consumir pelo menos um caractere. O `length == 0`
        // não é alcançável pelas classes acima — espaço em branco já foi
        // consumido antes —, mas amarrar o avanço ao teste em vez de à leitura
        // do código mantém a varredura terminante por construção: um `index`
        // que não anda aqui é um laço infinito que aloca token sem parar.
        let (run, ascii) = identifier_run(rest);
        if run > 0 && (character.is_ascii_alphabetic() || character == '_' || !character.is_ascii())
        {
            let kind = if ascii {
                TokenKind::Identifier
            } else {
                TokenKind::Uncertain
            };
            push_token(&mut tokens, kind, index, &rest[..run]);
            index += run;
            continue;
        }

        // Literal numérico: a corrida inteira cancela a tentativa uma vez, em
        // vez de deixar o sufixo (`1u8`) virar identificador de caminho.
        if run > 0 && character.is_ascii_digit() {
            push_token(&mut tokens, TokenKind::Incompatible, index, "");
            index += run;
            continue;
        }

        if character == ':' && rest.as_bytes().get(1) == Some(&b':') {
            push_token(&mut tokens, TokenKind::PathSeparator, index, "");
            index += 2;
            continue;
        }

        let kind = match character {
            '!' => TokenKind::Bang,
            '(' => TokenKind::Open(b')'),
            '[' => TokenKind::Open(b']'),
            '{' => TokenKind::Open(b'}'),
            ')' => TokenKind::Close(b')'),
            ']' => TokenKind::Close(b']'),
            '}' => TokenKind::Close(b'}'),
            _ => TokenKind::Incompatible,
        };
        push_token(&mut tokens, kind, index, "");
        index += character.len_utf8();
    }

    tokens
}

/// Acrescenta o token, fundindo bytes incompatíveis consecutivos: um número, um
/// operador composto ou uma pontuação qualquer cancelam a tentativa uma única
/// vez, e guardar cada byte deles seria só volume.
fn push_token<'a>(tokens: &mut Vec<Token<'a>>, kind: TokenKind, start: usize, text: &'a str) {
    if kind == TokenKind::Incompatible
        && tokens
            .last()
            .is_some_and(|last| last.kind == TokenKind::Incompatible)
    {
        return;
    }
    tokens.push(Token { kind, start, text });
}

/// Corrida de caracteres que pode compor um identificador, e se ela é
/// inteiramente ASCII.
///
/// Em ASCII a classe é exata. Fora dela a corrida é aproximada por excesso: ela
/// absorve todo caractere não-ASCII que não seja espaço em branco de Rust,
/// porque distinguir `XID_Continue` de pontuação Unicode exigiria as tabelas do
/// Unicode que este módulo não tem autoridade para adquirir. A corrida nunca
/// atravessa um caractere ASCII estrutural — `!`, `::` e os delimitadores são
/// ASCII —, então a aproximação não pode engolir a sequência que vem depois.
fn identifier_run(rest: &str) -> (usize, bool) {
    let mut length = 0;
    let mut ascii = true;
    for character in rest.chars() {
        if character.is_ascii() {
            if !character.is_ascii_alphanumeric() && character != '_' {
                break;
            }
        } else if RUST_WHITESPACE.contains(&character) {
            break;
        } else {
            ascii = false;
        }
        length += character.len_utf8();
    }
    (length, ascii)
}

/// Comprimento de `r#nome` e se ele é identificador cru válido. A string crua
/// `r#"..."#` já foi apagada pela máscara, então `r#` seguido de corrida de
/// identificador só pode ser tentativa de identificador cru — mas `r#` não
/// valida a corrida: as formas reservadas continuam recusadas, e o token
/// devolvido cobre `r#nome` inteiro para que o nome recusado não sobre solto
/// como segmento de caminho.
fn raw_identifier(rest: &str) -> Option<(usize, bool)> {
    let after = rest.strip_prefix("r#")?;
    let first = after.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '_' && first.is_ascii() {
        return None;
    }
    let (length, _) = identifier_run(after);
    if length == 0 {
        return None;
    }
    let valid = !RESERVED_RAW_IDENTIFIERS.contains(&&after[..length]);
    Some((2 + length, valid))
}

/// Classificação de um único `SimplePathSegment`. `None` quando o token não
/// pode ser segmento; `Some(true)` quando ele é comprovadamente IDENTIFIER ou
/// um dos segmentos especiais da gramática; `Some(false)` quando a classe
/// Unicode ficou na aproximação conservadora.
///
/// A mesma regra vale para todos os segmentos do caminho. Uma palavra-chave
/// inválida não passa a ser aceita por ter aparecido antes de `::`.
fn segment_accepts(token: &Token<'_>) -> Option<bool> {
    match token.kind {
        TokenKind::RawIdentifier => Some(true),
        TokenKind::Uncertain => Some(false),
        TokenKind::Identifier => (word_is_identifier(token.text)
            || SPECIAL_SIMPLE_PATH_SEGMENTS.contains(&token.text))
        .then_some(true),
        _ => None,
    }
}

/// Reconhecimento do `SimplePath` imediatamente anterior ao `!`.
struct PathBeforeBang {
    /// O caminho é exatamente o identificador `macro_rules`, sem qualificação —
    /// a única forma que carrega o nome da macro entre o `!` e o delimitador.
    macro_rules_form: bool,
    /// Todos os segmentos foram classificados com certeza. Quando falso, o
    /// reconhecimento é conservador e a saída precisa dizer isso.
    proven: bool,
}

/// Se os tokens imediatamente anteriores ao `!` formam um `SimplePath`.
///
/// `SimplePath` é `::? SimplePathSegment (:: SimplePathSegment)*`, e cada
/// segmento é validado pela mesma regra: conferir só o último e confiar em todo
/// `Identifier` anterior aceitaria `return::baz !`, que a toolchain pinada
/// recusa com `expected item, found keyword `return``.
fn simple_path_before(tokens: &[Token<'_>], bang: usize) -> Option<PathBeforeBang> {
    let last = bang.checked_sub(1)?;
    let mut proven = segment_accepts(&tokens[last])?;
    let mut first = last;
    while first >= 2 && tokens[first - 1].kind == TokenKind::PathSeparator {
        proven &= segment_accepts(&tokens[first - 2])?;
        first -= 2;
    }
    let leading = first >= 1 && tokens[first - 1].kind == TokenKind::PathSeparator;
    let qualified = first != last || leading;

    Some(PathBeforeBang {
        macro_rules_form: !qualified
            && tokens[last].kind == TokenKind::Identifier
            && tokens[last].text == "macro_rules",
        proven,
    })
}

/// Intervalo de bytes de um token tree de macro e o grau de certeza do
/// reconhecimento que o abriu.
struct MacroSpan {
    range: Range<usize>,
    /// `false` quando algum segmento do caminho caiu na aproximação Unicode
    /// conservadora: o intervalo continua suprimindo promoção estrutural, mas
    /// nada ali prova que a fonte tem uma macro.
    proven: bool,
}

/// Intervalos de bytes ocupados pelos token trees de macro da visão mascarada.
///
/// A gramática de Rust é a autoridade aqui. A sequência reconhecida é
/// `SimplePath ! DelimTokenTree`, e o delimitador pode ser `(`, `[` ou `{`.
/// `macro_rules` não é palavra-chave e sim um identificador comum, mas sua
/// definição tem uma forma própria: `macro_rules ! IDENTIFIER MacroRulesDef`.
///
/// O reconhecimento é por sequência de tokens, nunca por adjacência de bytes:
/// espaço em branco de Rust — inclusive as formas não-ASCII de
/// `Pattern_White_Space` — e comentário comum separam esses tokens sem desfazer
/// a sequência. Comentário de documentação não separa, porque é atributo.
/// Qualquer outro token cancela a tentativa imediatamente, em vez de deixar
/// sobrar "a última palavra vista".
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
/// dali em diante — mas um que fecha termina exatamente no seu delimitador de
/// fechamento, e o código seguinte continua observável, inclusive quando o
/// caminho ficou na classe conservadora.
fn macro_token_tree_spans(masked: &str, doc_comments: &[Range<usize>]) -> Vec<MacroSpan> {
    let tokens = macro_tokens(masked, doc_comments);
    let mut spans: Vec<MacroSpan> = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if tokens[index].kind != TokenKind::Bang {
            index += 1;
            continue;
        }
        let Some(path) = simple_path_before(&tokens, index) else {
            index += 1;
            continue;
        };

        let mut delimiter = index + 1;
        if path.macro_rules_form
            && tokens.get(delimiter).is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Identifier | TokenKind::RawIdentifier | TokenKind::Uncertain
                )
            })
        {
            delimiter += 1;
        }
        if !tokens
            .get(delimiter)
            .is_some_and(|token| matches!(token.kind, TokenKind::Open(_)))
        {
            index += 1;
            continue;
        }

        let (end, resume) = close_token_tree(&tokens, delimiter, masked.len());
        spans.push(MacroSpan {
            range: tokens[delimiter].start..end,
            proven: path.proven,
        });
        index = resume;
    }

    spans
}

/// Fecha o token tree aberto em `open`, devolvendo o byte seguinte ao
/// delimitador de fechamento e o token onde a varredura externa recomeça. Um
/// token tree que nunca fecha consome o resto do arquivo.
fn close_token_tree(tokens: &[Token<'_>], open: usize, end_of_file: usize) -> (usize, usize) {
    let mut closers: Vec<u8> = Vec::new();
    for (position, token) in tokens.iter().enumerate().skip(open) {
        match token.kind {
            TokenKind::Open(closer) => closers.push(closer),
            TokenKind::Close(byte) if closers.last() == Some(&byte) => {
                closers.pop();
                if closers.is_empty() {
                    return (token.start + 1, position + 1);
                }
            }
            _ => {}
        }
    }
    (end_of_file, tokens.len())
}

/// Byte que pode compor um identificador. O primeiro byte de um identificador
/// nunca é dígito, e essa distinção é do chamador.
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
