use crate::token::Span;

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
        span: Span,
    },
}

impl PinkerError {
    pub fn span(&self) -> Span {
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
            | PinkerError::AbstractMachineValidation { span, .. }
            | PinkerError::Runtime { span, .. } => *span,
        }
    }
}

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
            PinkerError::Runtime { msg, span } => {
                write!(f, "Erro Runtime: {} em {}", msg, span)
            }
        }
    }
}

impl std::error::Error for PinkerError {}
