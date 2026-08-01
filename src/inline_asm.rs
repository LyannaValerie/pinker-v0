//! Política estrutural de `sussurro` (inline assembly).
//!
//! O contrato público de `sussurro` é definido por *statements*, não por linhas
//! físicas nem por uma lista de mnemônicos ou de diretivas conhecidas.
//!
//! A gramática de statement do GNU as, depois de removidos comentários e
//! labels, tem exatamente três formas: **diretiva** (começa com `.`),
//! **atribuição de símbolo** (`nome = expressão`) e **instrução**. As duas
//! primeiras definem ou alteram símbolo; `sussurro` não define símbolo. Ambas
//! são recusadas por construção, independentemente do nome usado — o que resta
//! é a instrução, cujo texto de operandos é entregue ao assembler real.
//!
//! Essa política vale sobre a fonte. O que o objeto realmente produzido contém
//! é verificado separadamente, pelo invariante de artefato (ver
//! [`verify_native_artifact`]), porque uma política de fonte não pode, sozinha,
//! provar o conteúdo do ELF.

// @pinker-nav:start sussurro.politica.scanner
// @pinker-nav:domain sussurro
// @pinker-nav:layer inline_asm
// @pinker-nav:summary Scanner estrutural determinístico de `sussurro`: normaliza continuações `\`+newline, divide statements por newline e `;` fora de comentários e regiões citadas, remove comentários de linha `#` e de bloco `/* */` sem interpretar o conteúdo, rejeita citação ou comentário não terminado, aceita statement vazio e label local numérico, exige mnemônico depois do label e rejeita por construção as duas formas de statement que definem símbolo — qualquer token inicial começando com `.` (toda diretiva, via `E-SEMANTIC-ASM-DIRECTIVE`), qualquer label nominal (`E-SEMANTIC-ASM-NAMED-LABEL`) e qualquer atribuição `token = expressão`, com espaçamento livre, com tab, sem espaço nenhum, depois de `;`, de comentário removido, de newline normalizado ou de label local numérico, inclusive na forma `==` do dialeto (`E-SEMANTIC-ASM-SYMBOL-ASSIGN`). O texto dos operandos é preservado para o assembler real validar.

/// Código e detalhe de uma recusa da política de `sussurro`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmPolicyError {
    pub code: &'static str,
    pub detail: String,
}

impl AsmPolicyError {
    fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for AsmPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}", self.code, self.detail)
    }
}

pub const E_ASM_NUL: &str = "E-SEMANTIC-ASM-NUL";
pub const E_ASM_UNTERMINATED_QUOTE: &str = "E-SEMANTIC-ASM-UNTERMINATED-QUOTE";
pub const E_ASM_UNTERMINATED_COMMENT: &str = "E-SEMANTIC-ASM-UNTERMINATED-COMMENT";
pub const E_ASM_SEPARATOR_IN_QUOTE: &str = "E-SEMANTIC-ASM-SEPARATOR-IN-QUOTE";
pub const E_ASM_DIRECTIVE: &str = "E-SEMANTIC-ASM-DIRECTIVE";
pub const E_ASM_NAMED_LABEL: &str = "E-SEMANTIC-ASM-NAMED-LABEL";
pub const E_ASM_SYMBOL_ASSIGN: &str = "E-SEMANTIC-ASM-SYMBOL-ASSIGN";
pub const E_ASM_UNEXPECTED_TOKEN: &str = "E-SEMANTIC-ASM-UNEXPECTED-TOKEN";
pub const E_ASM_ENVELOPE: &str = "E-BACKEND-ASM-ENVELOPE";
pub const E_ASM_ARTIFACT: &str = "E-BACKEND-ASM-ARTIFACT";

/// Um statement de assembler reconhecido estruturalmente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmStatement {
    /// Label local numérico opcional (`1:`), sem os dois-pontos.
    pub local_label: Option<String>,
    /// Mnemônico da instrução. `None` apenas em statement de label puro.
    pub mnemonic: Option<String>,
    /// Texto dos operandos, preservado para o assembler real validar.
    pub operands: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Normal,
    Single,
    Double,
    LineComment,
    BlockComment,
}

/// Divide um pedaço de `sussurro` em statements, já sem comentários.
///
/// Não interpreta o conteúdo comentado e preserva as regiões citadas.
fn split_statements(chunk: &str) -> Result<Vec<String>, AsmPolicyError> {
    if chunk.contains('\0') {
        return Err(AsmPolicyError::new(
            E_ASM_NUL,
            "bloco de 'sussurro' não pode conter NUL",
        ));
    }

    let normalized = chunk.replace("\r\n", "\n");
    let bytes: Vec<char> = normalized.chars().collect();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut state = ScanState::Normal;
    let mut index = 0;

    while index < bytes.len() {
        let ch = bytes[index];
        let next = bytes.get(index + 1).copied();
        match state {
            ScanState::Normal => match ch {
                // Continuação reconhecida: barra invertida seguida de newline.
                '\\' if next == Some('\n') => index += 2,
                '\n' | ';' => {
                    statements.push(std::mem::take(&mut current));
                    index += 1;
                }
                '#' => {
                    state = ScanState::LineComment;
                    index += 1;
                }
                '/' if next == Some('*') => {
                    // Comentário de bloco vale como espaço em branco.
                    current.push(' ');
                    state = ScanState::BlockComment;
                    index += 2;
                }
                '\'' => {
                    current.push(ch);
                    state = ScanState::Single;
                    index += 1;
                }
                '"' => {
                    current.push(ch);
                    state = ScanState::Double;
                    index += 1;
                }
                _ => {
                    current.push(ch);
                    index += 1;
                }
            },
            ScanState::Single | ScanState::Double => {
                let closing = if state == ScanState::Single {
                    '\''
                } else {
                    '"'
                };
                match ch {
                    // Caractere escapado: preserva os dois caracteres.
                    '\\' if next.is_some() && next != Some('\n') => {
                        current.push(ch);
                        current.push(next.expect("next verificado"));
                        index += 2;
                    }
                    // Um separador de statement dentro de uma região citada faria
                    // o scanner e o assembler discordarem sobre onde o statement
                    // termina. A divergência é recusada, não adivinhada.
                    '\n' | ';' => {
                        return Err(AsmPolicyError::new(
                            E_ASM_SEPARATOR_IN_QUOTE,
                            format!(
                                "separador de statement '{}' dentro de região citada em 'sussurro'",
                                if ch == '\n' {
                                    "\\n".to_string()
                                } else {
                                    ch.to_string()
                                }
                            ),
                        ));
                    }
                    _ if ch == closing => {
                        current.push(ch);
                        state = ScanState::Normal;
                        index += 1;
                    }
                    _ => {
                        current.push(ch);
                        index += 1;
                    }
                }
            }
            ScanState::LineComment => {
                if ch == '\n' {
                    statements.push(std::mem::take(&mut current));
                    state = ScanState::Normal;
                }
                index += 1;
            }
            ScanState::BlockComment => {
                if ch == '*' && next == Some('/') {
                    state = ScanState::Normal;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        }
    }

    match state {
        ScanState::Single | ScanState::Double => {
            return Err(AsmPolicyError::new(
                E_ASM_UNTERMINATED_QUOTE,
                "região citada não terminada em 'sussurro'",
            ));
        }
        ScanState::BlockComment => {
            return Err(AsmPolicyError::new(
                E_ASM_UNTERMINATED_COMMENT,
                "comentário de bloco não terminado em 'sussurro'",
            ));
        }
        ScanState::Normal | ScanState::LineComment => {}
    }
    statements.push(current);
    Ok(statements)
}

fn is_mnemonic_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_mnemonic_continuation(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Reconhece um statement já isolado, aplicando a política estrutural.
fn scan_statement(statement: &str) -> Result<Option<AsmStatement>, AsmPolicyError> {
    let mut rest = statement.trim();
    if rest.is_empty() {
        return Ok(None);
    }

    // Label local numérico opcional no início do statement.
    let mut local_label = None;
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let after_digits = rest[digits.len()..].trim_start();
        if let Some(tail) = after_digits.strip_prefix(':') {
            local_label = Some(digits);
            rest = tail.trim();
        }
    }

    if rest.is_empty() {
        // `1:` isolado é um label local válido.
        return Ok(Some(AsmStatement {
            local_label,
            mnemonic: None,
            operands: String::new(),
        }));
    }

    // Toda diretiva do assembler começa o statement com `.` neste ponto, já sem
    // label nem comentário. A recusa é estrutural, não por nome.
    if rest.starts_with('.') {
        let directive: String = rest
            .chars()
            .take_while(|ch| !ch.is_whitespace())
            .take(64)
            .collect();
        return Err(AsmPolicyError::new(
            E_ASM_DIRECTIVE,
            format!("diretiva '{directive}' não é permitida dentro de 'sussurro'"),
        ));
    }

    let first = rest.chars().next().expect("rest não vazio");
    if !is_mnemonic_start(first) {
        return Err(AsmPolicyError::new(
            E_ASM_UNEXPECTED_TOKEN,
            format!("token estrutural inesperado '{first}' antes do mnemônico em 'sussurro'"),
        ));
    }

    let token: String = rest
        .chars()
        .take_while(|ch| is_mnemonic_continuation(*ch))
        .collect();
    let after_token = &rest[token.len()..];

    // Um label nominal define um símbolo; `sussurro` não define símbolos.
    if after_token.trim_start().starts_with(':') {
        return Err(AsmPolicyError::new(
            E_ASM_NAMED_LABEL,
            format!("label nominal '{token}:' não é permitido dentro de 'sussurro'"),
        ));
    }

    // Atribuição de símbolo. Na gramática do GNU as um statement é uma de três
    // coisas depois do label: diretiva (`.`), atribuição (`nome = expressão`) ou
    // instrução. As duas primeiras definem símbolo; `sussurro` não define
    // símbolo, então as duas são recusadas aqui — a instrução é o que sobra.
    //
    // A recusa é estrutural: qualquer token seguido de `=`, com qualquer
    // espaçamento (inclusive nenhum) e em qualquer posição de statement. A forma
    // `==` também é atribuição no dialeto aceito pelo build — confirmado contra
    // o assembler real, não suposto —, e cai na mesma regra por ser o mesmo
    // primeiro caractere.
    if after_token.trim_start().starts_with('=') {
        return Err(AsmPolicyError::new(
            E_ASM_SYMBOL_ASSIGN,
            format!("atribuição de símbolo '{token} = ...' não é permitida dentro de 'sussurro'"),
        ));
    }

    // O mnemônico precisa terminar em espaço ou no fim do statement; qualquer
    // outra coisa é um token estrutural inesperado.
    if let Some(unexpected) = after_token.chars().next() {
        if !unexpected.is_whitespace() {
            return Err(AsmPolicyError::new(
                E_ASM_UNEXPECTED_TOKEN,
                format!("token estrutural inesperado '{unexpected}' no mnemônico '{token}'"),
            ));
        }
    }

    Ok(Some(AsmStatement {
        local_label,
        mnemonic: Some(token),
        operands: after_token.trim().to_string(),
    }))
}

/// Aplica a política estrutural a um pedaço de `sussurro`.
pub fn scan_chunk(chunk: &str) -> Result<Vec<AsmStatement>, AsmPolicyError> {
    let mut scanned = Vec::new();
    for statement in split_statements(chunk)? {
        if let Some(statement) = scan_statement(&statement)? {
            scanned.push(statement);
        }
    }
    Ok(scanned)
}
// @pinker-nav:end sussurro.politica.scanner

// @pinker-nav:start sussurro.envelope.sentinelas
// @pinker-nav:domain sussurro
// @pinker-nav:layer inline_asm
// @pinker-nav:summary Envelope de `sussurro` no backend: sentinelas geradas pelo compilador delimitam cada bloco, com troca para sintaxe Intel na entrada e restauração de AT&T na saída. A validação confirma que cada begin tem exatamente um end, que os identificadores são únicos, que os wrappers de sintaxe estão balanceados dentro do envelope e reaplica a política estrutural aos statements reconstruídos, provando que nenhum texto da fonte escapou do envelope.

pub const SENTINEL_BEGIN_PREFIX: &str = "# PINKER-SUSSURRO-BEGIN:";
pub const SENTINEL_END_PREFIX: &str = "# PINKER-SUSSURRO-END:";
pub const INTEL_SYNTAX_WRAPPER: &str = ".intel_syntax noprefix";
pub const ATT_SYNTAX_WRAPPER: &str = ".att_syntax prefix";

/// Um envelope de `sussurro` extraído do assembly gerado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmEnvelope {
    pub id: String,
    /// Linhas da fonte dentro do envelope, sem os wrappers de sintaxe.
    pub source_lines: Vec<String>,
}

/// Extrai e valida os envelopes de `sussurro` do assembly gerado.
///
/// As sentinelas são geradas pelo compilador; nenhuma vem da fonte.
pub fn validate_envelopes(asm: &str) -> Result<Vec<AsmEnvelope>, AsmPolicyError> {
    let mut envelopes: Vec<AsmEnvelope> = Vec::new();
    let mut open: Option<(String, Vec<String>)> = None;

    for line in asm.lines() {
        let trimmed = line.trim();
        if let Some(id) = trimmed.strip_prefix(SENTINEL_BEGIN_PREFIX) {
            if open.is_some() {
                return Err(AsmPolicyError::new(
                    E_ASM_ENVELOPE,
                    "envelope de 'sussurro' aninhado",
                ));
            }
            let id = id.trim().to_string();
            if envelopes.iter().any(|envelope| envelope.id == id) {
                return Err(AsmPolicyError::new(
                    E_ASM_ENVELOPE,
                    format!("envelope de 'sussurro' duplicado: '{id}'"),
                ));
            }
            open = Some((id, Vec::new()));
            continue;
        }
        if let Some(id) = trimmed.strip_prefix(SENTINEL_END_PREFIX) {
            let id = id.trim().to_string();
            let Some((open_id, lines)) = open.take() else {
                return Err(AsmPolicyError::new(
                    E_ASM_ENVELOPE,
                    format!("fim de envelope de 'sussurro' sem início: '{id}'"),
                ));
            };
            if open_id != id {
                return Err(AsmPolicyError::new(
                    E_ASM_ENVELOPE,
                    format!("envelope de 'sussurro' desbalanceado: '{open_id}' fechado por '{id}'"),
                ));
            }
            // Wrappers de sintaxe precisam abrir e restaurar dentro do envelope.
            if lines.first().map(String::as_str) != Some(INTEL_SYNTAX_WRAPPER) {
                return Err(AsmPolicyError::new(
                    E_ASM_ENVELOPE,
                    format!("envelope '{id}' não abre com '{INTEL_SYNTAX_WRAPPER}'"),
                ));
            }
            if lines.last().map(String::as_str) != Some(ATT_SYNTAX_WRAPPER) {
                return Err(AsmPolicyError::new(
                    E_ASM_ENVELOPE,
                    format!("envelope '{id}' não restaura '{ATT_SYNTAX_WRAPPER}'"),
                ));
            }
            let source_lines = lines[1..lines.len() - 1].to_vec();
            // A política estrutural é reconfirmada sobre o texto realmente emitido.
            for source_line in &source_lines {
                if source_line.trim_start().starts_with('#') {
                    continue;
                }
                scan_chunk(source_line)?;
            }
            envelopes.push(AsmEnvelope { id, source_lines });
            continue;
        }
        if let Some((_, lines)) = open.as_mut() {
            lines.push(trimmed.to_string());
        }
    }

    if let Some((id, _)) = open {
        return Err(AsmPolicyError::new(
            E_ASM_ENVELOPE,
            format!("envelope de 'sussurro' sem fim: '{id}'"),
        ));
    }
    Ok(envelopes)
}
// @pinker-nav:end sussurro.envelope.sentinelas

// @pinker-nav:start sussurro.artefato.invariante
// @pinker-nav:domain sussurro
// @pinker-nav:layer inline_asm
// @pinker-nav:summary Invariante do artefato realmente produzido, aplicado no caminho de `pink build --nativo` e não apenas em fixture: `strip_envelope_bodies` deriva do próprio `.s` emitido uma baseline byte a byte idêntica exceto pelos envelopes de `sussurro`, removidos por inteiro (sentinelas e wrappers inclusive); `verify_native_artifact` monta as duas variantes com o mesmo driver C, sob o mesmo nome de arquivo em diretórios irmãos para que o símbolo `STT_FILE` não vire delta falso, lê os dois objetos com o leitor de ELF próprio (`crate::elf`, sem depender de saída textual de ferramenta externa) e delega a `compare_artifact_surfaces`. A superfície comparada é o conjunto de seções e o conjunto de símbolos **definidos** (`SHN_UNDEF` excluído, porque o contrato publicado admite referência a símbolo já existente) normalizados como (nome, ligação, visibilidade, tipo, rótulo de seção, tamanho) — de modo que símbolo novo, alias novo, seção nova, mudança de ligação, de visibilidade, de tipo ou de tamanho em símbolo reservado do runtime aparecem como delta e falham com `E-BACKEND-ASM-ARTIFACT`. Sem envelope no `.s`, não há nada atribuível ao bloco e a verificação é dispensada.

use crate::elf::{ElfObject, SHN_ABS, SHN_COMMON, SHN_UNDEF};
use std::path::Path;

/// Símbolo definido, normalizado para comparação entre as duas montagens.
///
/// O índice de seção cru não serve: ele é posicional. O rótulo textual é o que
/// permanece comparável entre dois objetos distintos.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArtifactSymbol {
    pub name: String,
    pub bind: u8,
    pub visibility: u8,
    pub symbol_type: u8,
    pub section: String,
    pub size: u64,
}

/// A superfície de um objeto que o contrato de `sussurro` governa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSurface {
    pub sections: Vec<String>,
    pub defined_symbols: Vec<ArtifactSymbol>,
}

/// Resumo do que a verificação de artefato inspecionou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactCheck {
    pub envelopes: usize,
    pub sections: usize,
    pub defined_symbols: usize,
}

/// Tipo de símbolo `STT_SECTION`: existe para toda seção e não é nomeado pelo
/// autor; compará-lo apenas duplicaria a comparação de seções.
const STT_SECTION: u8 = 3;

fn section_label(object: &ElfObject, index: u16) -> String {
    match index {
        SHN_ABS => "ABS".to_string(),
        SHN_COMMON => "COMMON".to_string(),
        _ => object
            .sections
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| format!("secao#{index}")),
    }
}

/// Extrai de um objeto a superfície governada pelo contrato.
///
/// Símbolos apenas referenciados (`SHN_UNDEF`) ficam de fora: o contrato
/// publicado admite referência a símbolo já existente, e só proíbe definição.
pub fn artifact_surface(object: &ElfObject) -> ArtifactSurface {
    let mut sections: Vec<String> = object.sections.clone();
    sections.sort();
    sections.dedup();

    let mut defined_symbols: Vec<ArtifactSymbol> = object
        .symbols
        .iter()
        .filter(|symbol| symbol.section_index != SHN_UNDEF)
        .filter(|symbol| symbol.symbol_type != STT_SECTION)
        .filter(|symbol| !symbol.name.is_empty())
        .map(|symbol| ArtifactSymbol {
            name: symbol.name.clone(),
            bind: symbol.bind,
            visibility: symbol.visibility,
            symbol_type: symbol.symbol_type,
            section: section_label(object, symbol.section_index),
            size: symbol.size,
        })
        .collect();
    defined_symbols.sort();
    defined_symbols.dedup();

    ArtifactSurface {
        sections,
        defined_symbols,
    }
}

fn describe(symbol: &ArtifactSymbol) -> String {
    format!(
        "{} (ligação {}, visibilidade {}, tipo {}, seção {}, tamanho {})",
        symbol.name,
        symbol.bind,
        symbol.visibility,
        symbol.symbol_type,
        symbol.section,
        symbol.size
    )
}

/// Compara a superfície da baseline com a do objeto real.
///
/// A baseline é o mesmo programa sem os envelopes, então **todo** delta é
/// atribuível ao bloco de `sussurro`; nada produzido pelo compilador ou pelo
/// toolchain aparece dos dois lados de forma assimétrica.
pub fn compare_artifact_surfaces(
    baseline: &ArtifactSurface,
    real: &ArtifactSurface,
) -> Result<ArtifactCheck, AsmPolicyError> {
    let mut faults: Vec<String> = Vec::new();

    for section in &real.sections {
        if !baseline.sections.contains(section) {
            faults.push(format!("seção nova '{section}'"));
        }
    }
    for section in &baseline.sections {
        if !real.sections.contains(section) {
            faults.push(format!("seção perdida '{section}'"));
        }
    }
    for symbol in &real.defined_symbols {
        if !baseline.defined_symbols.contains(symbol) {
            faults.push(format!(
                "símbolo definido novo ou alterado: {}",
                describe(symbol)
            ));
        }
    }
    for symbol in &baseline.defined_symbols {
        if !real.defined_symbols.contains(symbol) {
            faults.push(format!(
                "símbolo definido perdido ou alterado: {}",
                describe(symbol)
            ));
        }
    }

    if !faults.is_empty() {
        faults.sort();
        return Err(AsmPolicyError::new(
            E_ASM_ARTIFACT,
            format!(
                "bloco de 'sussurro' alterou o objeto produzido:\n  - {}",
                faults.join("\n  - ")
            ),
        ));
    }

    Ok(ArtifactCheck {
        envelopes: 0,
        sections: real.sections.len(),
        defined_symbols: real.defined_symbols.len(),
    })
}

/// Remove por inteiro cada envelope de `sussurro`, preservando todo o resto.
///
/// O resultado é a baseline explícita: o mesmo assembly que o compilador
/// emitiria para o mesmo programa sem os blocos. As sentinelas e os wrappers de
/// sintaxe também saem, porque também são consequência do bloco.
pub fn strip_envelope_bodies(asm: &str) -> Result<String, AsmPolicyError> {
    // A integridade do envelope precede a derivação: recortar um envelope
    // desbalanceado produziria uma baseline inventada.
    validate_envelopes(asm)?;

    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in asm.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(SENTINEL_BEGIN_PREFIX) {
            inside = true;
            continue;
        }
        if trimmed.starts_with(SENTINEL_END_PREFIX) {
            inside = false;
            continue;
        }
        if !inside {
            kept.push(line);
        }
    }

    let mut baseline = kept.join("\n");
    if asm.ends_with('\n') {
        baseline.push('\n');
    }
    Ok(baseline)
}

fn assemble(driver: &str, dir: &Path, asm: &str) -> Result<Vec<u8>, AsmPolicyError> {
    let artifact_error = |detail: String| AsmPolicyError::new(E_ASM_ARTIFACT, detail);

    std::fs::create_dir_all(dir)
        .map_err(|err| artifact_error(format!("falha ao criar '{}': {err}", dir.display())))?;
    // O mesmo nome de arquivo nos dois lados: o assembler pode gravar um símbolo
    // `STT_FILE` com o nome da fonte, que viraria um delta falso.
    let source = dir.join("sussurro.s");
    let object = dir.join("sussurro.o");
    std::fs::write(&source, asm)
        .map_err(|err| artifact_error(format!("falha ao gravar '{}': {err}", source.display())))?;

    let output = std::process::Command::new(driver)
        .arg("-c")
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .map_err(|err| artifact_error(format!("falha ao invocar '{driver}': {err}")))?;
    if !output.status.success() {
        return Err(artifact_error(format!(
            "'{driver}' recusou o assembly de verificação:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    std::fs::read(&object)
        .map_err(|err| artifact_error(format!("falha ao ler '{}': {err}", object.display())))
}

/// Verifica, no caminho real de `pink build --nativo`, que os blocos de
/// `sussurro` não mudaram a superfície do objeto produzido.
///
/// Monta duas vezes — o assembly emitido e a baseline sem os envelopes — e
/// compara as duas superfícies. Sem envelope, não há nada atribuível ao bloco e
/// a verificação não tem o que fazer.
pub fn verify_native_artifact(
    asm: &str,
    driver: &str,
    workdir: &Path,
) -> Result<ArtifactCheck, AsmPolicyError> {
    let envelopes = validate_envelopes(asm)?;
    if envelopes.is_empty() {
        return Ok(ArtifactCheck {
            envelopes: 0,
            sections: 0,
            defined_symbols: 0,
        });
    }

    let baseline_asm = strip_envelope_bodies(asm)?;
    let real_bytes = assemble(driver, &workdir.join("real"), asm)?;
    let baseline_bytes = assemble(driver, &workdir.join("baseline"), &baseline_asm)?;

    let parse = |bytes: &[u8], label: &str| -> Result<ElfObject, AsmPolicyError> {
        crate::elf::parse(bytes).map_err(|detail| {
            AsmPolicyError::new(
                E_ASM_ARTIFACT,
                format!("objeto '{label}' não pôde ser lido: {detail}"),
            )
        })
    };
    let real = artifact_surface(&parse(&real_bytes, "real")?);
    let baseline = artifact_surface(&parse(&baseline_bytes, "baseline")?);

    let mut check = compare_artifact_surfaces(&baseline, &real)?;
    check.envelopes = envelopes.len();
    Ok(check)
}
// @pinker-nav:end sussurro.artefato.invariante
