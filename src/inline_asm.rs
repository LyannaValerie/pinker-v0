//! Política estrutural de `sussurro` (inline assembly).
//!
//! O contrato público de `sussurro` é definido por *statements*, não por linhas
//! físicas nem por uma lista de diretivas proibidas. Toda diretiva do assembler
//! começa um statement com `.` depois da remoção de labels e comentários,
//! portanto é rejeitada por construção, independentemente do nome.

// @pinker-nav:start sussurro.politica.scanner
// @pinker-nav:domain sussurro
// @pinker-nav:layer inline_asm
// @pinker-nav:summary Scanner estrutural determinístico de `sussurro`: normaliza continuações `\`+newline, divide statements por newline e `;` fora de comentários e regiões citadas, remove comentários de linha `#` e de bloco `/* */` sem interpretar o conteúdo, rejeita citação ou comentário não terminado, aceita statement vazio e label local numérico, exige mnemônico depois do label e rejeita por construção qualquer token inicial começando com `.` (toda diretiva) e qualquer label nominal. O texto dos operandos é preservado para o assembler real validar.

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
pub const E_ASM_UNEXPECTED_TOKEN: &str = "E-SEMANTIC-ASM-UNEXPECTED-TOKEN";
pub const E_ASM_ENVELOPE: &str = "E-BACKEND-ASM-ENVELOPE";

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
