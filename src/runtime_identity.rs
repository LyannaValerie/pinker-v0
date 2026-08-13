//! Autoridade das identidades de tipo reservadas por semântica de runtime.
//!
//! A reserva não deriva do recipiente usado para materializar o tipo no parser:
//! um leque interpretado por tag e um handle opaco produzido pelo runtime têm
//! categorias diferentes, embora ambos precisem impedir shadowing arbitrário.

// @pinker-nav:start runtime.identidade.reservada
// @pinker-nav:domain identidade
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade explícita das identidades semânticas reservadas pelo runtime, separando leques cujos discriminantes são interpretados de handles opacos nominais de uma palavra; TipoEntrada, LimiteTempo e SaidaProcesso derivam a guarda do parser desta tabela, não do recipiente acidental usado na materialização.
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

// @pinker-nav:end runtime.identidade.reservada

// @pinker-nav:start evidencia.runtime.identidade-reservada
// @pinker-nav:domain identidade
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fixa as categorias semânticas das três identidades reservadas e recusa transformar SaidaProcesso em leque apenas porque outras identidades runtime-reservadas são leques simples.
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
// @pinker-nav:end evidencia.runtime.identidade-reservada
