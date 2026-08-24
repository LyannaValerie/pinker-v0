use crate::token::Span;

// @pinker-nav:start error.diagnostico.taxonomia
// @pinker-nav:domain diagnostico
// @pinker-nav:layer error
// @pinker-nav:summary Taxonomia unificada de erros do compilador (léxico, sintático, semântico, cada validação de pipeline e runtime), cada variante carregando mensagem e span de origem.
#[derive(Debug)]
pub enum PinkerError {
    Lexer {
        msg: String,
        span: Span,
    },
    Parse {
        msg: String,
        span: Span,
    },
    Expected {
        expected: String,
        found: String,
        span: Span,
    },
    Semantic {
        msg: String,
        span: Span,
    },
    Ir {
        msg: String,
        span: Span,
    },
    IrValidation {
        msg: String,
        span: Span,
    },
    CfgIrValidation {
        msg: String,
        span: Span,
    },
    BackendTextValidation {
        msg: String,
        span: Span,
    },
    InstrSelectValidation {
        msg: String,
        span: Span,
    },
    AbstractMachineValidation {
        msg: String,
        span: Span,
    },
    Runtime {
        msg: String,
        span: Option<Span>,
    },
}
// @pinker-nav:end error.diagnostico.taxonomia

// @pinker-nav:start error.diagnostico.contexto-fonte
// @pinker-nav:domain diagnostico
// @pinker-nav:layer error
// @pinker-nav:summary Renderiza um erro para o CLI recuperando a linha de origem pelo span e desenhando um cursor `^` na coluna; formata mensagens e stack traces de runtime.
impl PinkerError {
    pub fn span(&self) -> Option<Span> {
        match self {
            PinkerError::Lexer { span, .. }
            | PinkerError::Parse { span, .. }
            | PinkerError::Expected { span, .. }
            | PinkerError::Semantic { span, .. }
            | PinkerError::Ir { span, .. }
            | PinkerError::IrValidation { span, .. }
            | PinkerError::CfgIrValidation { span, .. }
            | PinkerError::BackendTextValidation { span, .. }
            | PinkerError::InstrSelectValidation { span, .. }
            | PinkerError::AbstractMachineValidation { span, .. } => Some(*span),
            PinkerError::Runtime { span, .. } => *span,
        }
    }

    pub fn render_for_cli(&self) -> String {
        match self {
            PinkerError::Runtime { msg, span } => render_runtime_for_cli(msg, *span),
            _ => self.to_string(),
        }
    }

    /// Renderiza o erro para o CLI incluindo source context quando disponível.
    /// Para erros com span real (lexer, parser, semântica), extrai a linha
    /// de origem e acrescenta um indicador de coluna (`^`) abaixo.
    /// Para erros de runtime, delega ao renderer de runtime existente.
    pub fn render_for_cli_with_source(&self, source: &str) -> String {
        self.render_for_cli_with_sources(&crate::source_map::single("", source))
    }

    /// Renderiza o erro resolvendo o trecho pela fonte que o span reivindica.
    ///
    /// `SOURCE_LOCATION_INTEGRITY`: o texto vem de `span.source`, nunca do
    /// texto primário por conveniência. Um span originado em A é interpretado
    /// contra A ou contra nada; jamais contra B. Span sintético não reivindica
    /// fonte alguma e continua caindo no texto primário, que é o comportamento
    /// histórico de programa de arquivo único.
    ///
    /// Quando a fonte não é a primária, o rótulo dela entra no diagnóstico: um
    /// trecho correto que não diga de que arquivo veio ainda deixa o leitor
    /// procurando a linha no arquivo errado.
    pub fn render_for_cli_with_sources(&self, sources: &crate::source_map::SourceMap) -> String {
        match self {
            PinkerError::Runtime { msg, span } => render_runtime_for_cli(msg, *span),
            _ => {
                let base = self.to_string();
                let Some(span) = self.span() else {
                    return base;
                };
                let origem = match sources.label_for(span.source) {
                    Some(label) if span.source != crate::source_map::SourceId::ROOT => {
                        Some(format!("  em: {}", label))
                    }
                    _ => None,
                };
                let snippet = sources
                    .text_for(span.source)
                    .and_then(|text| extract_source_snippet(text, span));
                match (origem, snippet) {
                    (Some(origem), Some(snippet)) => format!("{}\n{}\n{}", base, origem, snippet),
                    (Some(origem), None) => format!("{}\n{}", base, origem),
                    (None, Some(snippet)) => format!("{}\n{}", base, snippet),
                    (None, None) => base,
                }
            }
        }
    }

    /// Vincula o diagnóstico a uma unidade-fonte quando ele ainda não
    /// reivindica nenhuma. Erro já vinculado nunca é reatribuído.
    pub fn com_fonte_padrao(self, source: crate::source_map::SourceId) -> Self {
        match self {
            PinkerError::Lexer { msg, span } => PinkerError::Lexer {
                msg,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::Parse { msg, span } => PinkerError::Parse {
                msg,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::Expected {
                expected,
                found,
                span,
            } => PinkerError::Expected {
                expected,
                found,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::Semantic { msg, span } => PinkerError::Semantic {
                msg,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::Ir { msg, span } => PinkerError::Ir {
                msg,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::IrValidation { msg, span } => PinkerError::IrValidation {
                msg,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::CfgIrValidation { msg, span } => PinkerError::CfgIrValidation {
                msg,
                span: span.com_fonte_padrao(source),
            },
            PinkerError::BackendTextValidation { msg, span } => {
                PinkerError::BackendTextValidation {
                    msg,
                    span: span.com_fonte_padrao(source),
                }
            }
            PinkerError::InstrSelectValidation { msg, span } => {
                PinkerError::InstrSelectValidation {
                    msg,
                    span: span.com_fonte_padrao(source),
                }
            }
            PinkerError::AbstractMachineValidation { msg, span } => {
                PinkerError::AbstractMachineValidation {
                    msg,
                    span: span.com_fonte_padrao(source),
                }
            }
            PinkerError::Runtime { msg, span } => PinkerError::Runtime {
                msg,
                span: span.map(|span| span.com_fonte_padrao(source)),
            },
        }
    }
}

fn render_runtime_for_cli(msg: &str, span: Option<Span>) -> String {
    let (main_msg, trace) = split_runtime_message_and_trace(msg);
    let mut out = String::from("Erro Runtime:\n");
    out.push_str("  mensagem: ");
    out.push_str(main_msg);
    out.push('\n');
    if let Some(trace) = trace {
        out.push_str("stack trace:\n");
        for line in trace.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    match span {
        Some(s) => {
            out.push_str("  span: ");
            out.push_str(&s.to_string());
        }
        None => {
            out.push_str("  localização: indisponível (erro detectado na instrução de máquina)");
        }
    }
    out
}

/// Extrai a linha de origem correspondente ao span e acrescenta um indicador
/// de coluna (`^`) alinhado à posição inicial do erro.
/// Retorna `None` se o número de linha for inválido ou a linha não existir.
fn extract_source_snippet(source: &str, span: Span) -> Option<String> {
    let line_num = span.start.line;
    if line_num == 0 {
        return None;
    }
    let line_text = source.lines().nth(line_num - 1)?;
    let col = span.start.col.saturating_sub(1);
    let mut out = String::new();
    out.push_str("  | ");
    out.push_str(line_text);
    out.push('\n');
    out.push_str("  | ");
    for _ in 0..col {
        out.push(' ');
    }
    out.push('^');
    Some(out)
}

fn split_runtime_message_and_trace(msg: &str) -> (&str, Option<&str>) {
    match msg.split_once("\nstack trace:\n") {
        Some((main_msg, trace)) => (main_msg, Some(trace)),
        None => (msg, None),
    }
}
// @pinker-nav:end error.diagnostico.contexto-fonte

impl std::fmt::Display for PinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinkerError::Lexer { msg, span } => {
                write!(f, "Erro Léxico: {} em {}", msg, span)
            }
            PinkerError::Parse { msg, span } => {
                write!(f, "Erro Sintático: {} em {}", msg, span)
            }
            PinkerError::Expected {
                expected,
                found,
                span,
            } => {
                let found = if found.is_empty() {
                    "fim do arquivo"
                } else {
                    found
                };
                write!(
                    f,
                    "Erro Sintático: esperado '{}', encontrado '{}' em {}",
                    expected, found, span
                )
            }
            PinkerError::Semantic { msg, span } => {
                write!(f, "Erro Semântico: {} em {}", msg, span)
            }
            PinkerError::Ir { msg, span } => {
                write!(f, "Erro IR: {} em {}", msg, span)
            }
            PinkerError::IrValidation { msg, span } => {
                write!(f, "Erro Validação IR: {} em {}", msg, span)
            }
            PinkerError::CfgIrValidation { msg, span } => {
                write!(f, "Erro Validação CFG IR: {} em {}", msg, span)
            }
            PinkerError::BackendTextValidation { msg, span } => {
                write!(f, "Erro Validação Backend Textual: {} em {}", msg, span)
            }
            PinkerError::InstrSelectValidation { msg, span } => {
                write!(
                    f,
                    "Erro Validação Seleção de Instruções: {} em {}",
                    msg, span
                )
            }
            PinkerError::AbstractMachineValidation { msg, span } => {
                write!(f, "Erro Validação Máquina Abstrata: {} em {}", msg, span)
            }
            PinkerError::Runtime { msg, span } => match span {
                Some(s) => write!(f, "Erro Runtime: {} em {}", msg, s),
                None => write!(f, "Erro Runtime: {}", msg),
            },
        }
    }
}

impl std::error::Error for PinkerError {}
