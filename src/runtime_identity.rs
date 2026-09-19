//! Autoridade das identidades de tipo reservadas por semântica de runtime.
//!
//! A reserva não deriva do recipiente usado para materializar o tipo no parser:
//! um leque interpretado por tag e um handle opaco produzido pelo runtime têm
//! categorias diferentes, embora ambos precisem impedir shadowing arbitrário.

// @pinker-nav:start runtime.identity.reserved
// @pinker-nav:domain identity
// @pinker-nav:layer semantic
// @pinker-nav:summary Explicit authority over the semantic identities reserved by the runtime, separating leques whose discriminants are interpreted from nominal one-word opaque handles; TipoEntrada, LimiteTempo and SaidaProcesso derive the parser's guard from this table, not from the accidental container used in the materialization.
/// Categoria semântica da identidade builtin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSemanticKind {
    /// Leque cujos discriminantes são produzidos ou interpretados pelo runtime.
    PlainEnum,
    /// Handle de uma palavra cuja identidade nominal não está na representação.
    OpaqueWordHandle,
}

/// Identidade reservada e sua categoria real.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeReservedIdentity {
    pub name: &'static str,
    pub kind: RuntimeSemanticKind,
}

/// Lista canônica das identidades sempre reservadas.
pub const RUNTIME_RESERVED_IDENTITIES: &[RuntimeReservedIdentity] = &[
    RuntimeReservedIdentity {
        name: crate::tipo_entrada::LEQUE_TIPO_ENTRADA,
        kind: RuntimeSemanticKind::PlainEnum,
    },
    RuntimeReservedIdentity {
        name: crate::limite_tempo::LEQUE_LIMITE_TEMPO,
        kind: RuntimeSemanticKind::PlainEnum,
    },
    RuntimeReservedIdentity {
        name: crate::saida_processo::TIPO_SAIDA_PROCESSO,
        kind: RuntimeSemanticKind::OpaqueWordHandle,
    },
    // Parte E1: a família JSON traz as duas categorias de uma vez — o valor é
    // handle produzido pelo runtime, e a classificação é leque cujos
    // discriminantes o runtime interpreta.
    RuntimeReservedIdentity {
        name: crate::valor_json::TIPO_VALOR_JSON,
        kind: RuntimeSemanticKind::OpaqueWordHandle,
    },
    RuntimeReservedIdentity {
        name: crate::valor_json::LEQUE_TIPO_JSON,
        kind: RuntimeSemanticKind::PlainEnum,
    },
];

/// Consulta única usada por parser e testes de autoridade.
pub fn runtime_reserved_identity(name: &str) -> Option<RuntimeReservedIdentity> {
    RUNTIME_RESERVED_IDENTITIES
        .iter()
        .copied()
        .find(|identity| identity.name == name)
}

pub fn conflict_message(identity: RuntimeReservedIdentity) -> String {
    let reason = match identity.kind {
        RuntimeSemanticKind::PlainEnum => {
            "seus discriminantes são produzidos ou interpretados pelo runtime"
        }
        RuntimeSemanticKind::OpaqueWordHandle => {
            "seus handles são produzidos pelo runtime e carregam identidade nominal própria"
        }
    };
    format!(
        "'{}' é uma identidade builtin reservada e não pode ser redeclarada: {reason}",
        identity.name
    )
}

// @pinker-nav:end runtime.identity.reserved

// @pinker-nav:start evidence.runtime.reserved-identity
// @pinker-nav:domain identity
// @pinker-nav:layer evidence
// @pinker-nav:summary Fixes the semantic categories of the three reserved identities and refuses to turn SaidaProcesso into a leque merely because other runtime-reserved identities are simple leques.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorias_semanticas_nao_colapsam_por_representacao() {
        assert_eq!(
            runtime_reserved_identity("SaidaProcesso").map(|id| id.kind),
            Some(RuntimeSemanticKind::OpaqueWordHandle)
        );
        assert_eq!(
            runtime_reserved_identity("LimiteTempo").map(|id| id.kind),
            Some(RuntimeSemanticKind::PlainEnum)
        );
    }
}
// @pinker-nav:end evidence.runtime.reserved-identity
