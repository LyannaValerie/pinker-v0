//! Autoridade canônica única da **especialização de operação de mapa** (U-02).
//!
//! Esta autoridade responde UMA pergunta:
//!
//! ```text
//! WHICH_MONOMORPHIC_EXECUTABLE_IDENTITY
//! =
//! SPECIALIZE(CONCRETE_MAP_CLASS, GENERIC_MAP_OPERATION)
//! ```
//!
//! Antes da consolidação o mesmo fato existia cinco vezes, em cinco
//! representações e três fases: `parser::CollectionKind::generic_map_callee`,
//! `semantic::calls::generic_map_monomorphic_callee`,
//! `ir::generic_map_monomorphic_callee`, as quatro seleções de
//! `mapa_<classe>_tamanho` do desugaring de `para cada` em `parser::lacos` e as
//! quatro seleções de `mapa_<classe>_criar` do lowering de `ir::lowering`.
//! Nada impedia que discordassem, e mudar uma célula exigia achar as cinco.
//!
//! ```text
//! SPECIALIZATION_AUTHORITY = SOURCE OF THIS RELATION
//! PARSER / SEMANTIC / IR   = CONSUMERS
//! ```
//!
//! # O que esta autoridade NÃO decide
//!
//! ```text
//! MAP_SPECIALIZATION_RELATION != PUBLIC_INTRINSIC_REGISTRY
//! MAP_SPECIALIZATION_RELATION != INTERNAL_OPERATION_CONTRACT
//! MAP_SPECIALIZATION_RELATION != MAP_TYPE_CLASSIFICATION
//! MAP_SPECIALIZATION_RELATION != OPERAND_OR_RESULT_CONTRACT
//! MAP_SPECIALIZATION_RELATION != RUNTIME_OR_INTERPRETER_BODY
//! MAP_SPECIALIZATION_RELATION != ABI_SYMBOL
//! ```
//!
//! - A **superfície pública** (C1) permanece inteira em
//!   [`crate::intrinsics::registry`] e [`crate::intrinsics::public_surface`].
//!   Esta autoridade não declara nenhuma intrínseca: ela ESCOLHE, entre
//!   identidades que já existem lá, qual delas um par `(classe, operação)`
//!   endereça. O lado direito da relação é sempre uma
//!   [`IntrinsicIdentity`] obtida da autoridade de identidade, nunca uma
//!   grafia fabricada aqui.
//! - O **contrato de operação interna** (U-01) permanece em
//!   [`crate::internal_operations`]. Nenhuma grafia `__pinker_internal_*` é
//!   alvo desta relação, e o fallback adulto do mapa genérico
//!   (`TypeIR::Map { key, .. }`) continua sendo respondido por U-01 na fase que
//!   o materializa.
//! - **Qual classe concreta é este tipo** continua sendo pergunta de fase.
//!   `CollectionKind`, `Type` e `TypeIR` são representações locais, e traduzir
//!   cada uma para [`CanonicalMapClass`] é adaptação legítima que mora com a
//!   representação. O que nenhuma fase pode responder de novo é a segunda
//!   pergunta — dada a classe e a operação, QUAL identidade executável.
//! - Os **corpos** continuam com seus donos: o interpretador hospeda a
//!   execução, `pinker_rt` implementa o símbolo nativo e `backend_s` faz o
//!   binding ABI. Receber uma identidade já especializada e executá-la não é
//!   especializar.
//!
//! # Domínio
//!
//! O domínio é finito e fechado: quatro classes concretas de mapa e seis
//! operações genéricas, todas preexistentes na superfície C1.
//!
//! ```text
//! MAP_CLASSES          = 4
//! GENERIC_OPERATIONS   = 6
//! SPECIALIZATION_CELLS = 24
//! ```
//!
//! Toda célula é `DEFINED`: não existe par `(classe, operação)` sem identidade
//! monomórfica, e por isso [`specialize`] não tem braço de escape. A
//! exaustividade é do compilador Rust, não de um `_ => None` que engoliria uma
//! célula material em silêncio.

use crate::intrinsics::identity::{intrinsic_from_public_spelling, IntrinsicIdentity};

/// Classe concreta de mapa, na forma em que a especialização a endereça.
///
/// É a identidade canônica de `mapa<K,V>` monomorfizado, independente da
/// representação com que cada fase a carrega.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalMapClass {
    /// `mapa<verso,bombom>`
    VersoBombom,
    /// `mapa<verso,verso>`
    VersoVerso,
    /// `mapa<bombom,bombom>`
    BombomBombom,
    /// `mapa<bombom,verso>`
    BombomVerso,
}

/// Operação genérica de mapa, endereçada pela identidade e não pela grafia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GenericMapOperation {
    Criar,
    Definir,
    Obter,
    Tem,
    Tamanho,
    Remover,
}

/// As quatro classes concretas, em ordem estável.
pub const CANONICAL_MAP_CLASSES: &[CanonicalMapClass] = &[
    CanonicalMapClass::VersoBombom,
    CanonicalMapClass::VersoVerso,
    CanonicalMapClass::BombomBombom,
    CanonicalMapClass::BombomVerso,
];

/// As seis operações genéricas, em ordem estável.
pub const GENERIC_MAP_OPERATIONS: &[GenericMapOperation] = &[
    GenericMapOperation::Criar,
    GenericMapOperation::Definir,
    GenericMapOperation::Obter,
    GenericMapOperation::Tem,
    GenericMapOperation::Tamanho,
    GenericMapOperation::Remover,
];

impl GenericMapOperation {
    /// Grafia pública da forma genérica, como ela existe em C1.
    pub fn generic_public_spelling(self) -> &'static str {
        match self {
            Self::Criar => "mapa_criar",
            Self::Definir => "mapa_definir",
            Self::Obter => "mapa_obter",
            Self::Tem => "mapa_tem",
            Self::Tamanho => "mapa_tamanho",
            Self::Remover => "mapa_remover",
        }
    }

    /// A operação endereçada por uma grafia genérica, quando há uma.
    ///
    /// `None` significa que a grafia não é operação genérica de mapa — não que
    /// ela seja inválida.
    pub fn from_generic_public_spelling(spelling: &str) -> Option<Self> {
        GENERIC_MAP_OPERATIONS
            .iter()
            .copied()
            .find(|operation| operation.generic_public_spelling() == spelling)
    }

    /// As operações cuja classe vem do mapa recebido no argumento 0.
    ///
    /// `Criar` fica fora por causa material: ela não recebe mapa nenhum, e a
    /// classe dela vem da anotação do destino.
    pub fn recebe_mapa_no_argumento_zero(self) -> bool {
        !matches!(self, Self::Criar)
    }
}

/// A relação. Vinte e quatro células, exaustivas por construção.
///
/// O literal escolhido aqui é endereço, não identidade: quem transforma o
/// endereço em identidade executável é [`specialize`], via a autoridade de
/// identidade de C1. Uma célula que apontasse para fora da superfície histórica
/// aplicável não sobreviveria a essa travessia.
fn monomorphic_public_spelling(
    class: CanonicalMapClass,
    operation: GenericMapOperation,
) -> &'static str {
    use CanonicalMapClass as C;
    use GenericMapOperation as O;
    match (class, operation) {
        (C::VersoBombom, O::Criar) => "mapa_verso_bombom_criar",
        (C::VersoBombom, O::Definir) => "mapa_verso_bombom_definir",
        (C::VersoBombom, O::Obter) => "mapa_verso_bombom_obter",
        (C::VersoBombom, O::Tem) => "mapa_verso_bombom_tem",
        (C::VersoBombom, O::Tamanho) => "mapa_verso_bombom_tamanho",
        (C::VersoBombom, O::Remover) => "mapa_verso_bombom_remover",

        (C::VersoVerso, O::Criar) => "mapa_verso_verso_criar",
        (C::VersoVerso, O::Definir) => "mapa_verso_verso_definir",
        (C::VersoVerso, O::Obter) => "mapa_verso_verso_obter",
        (C::VersoVerso, O::Tem) => "mapa_verso_verso_tem",
        (C::VersoVerso, O::Tamanho) => "mapa_verso_verso_tamanho",
        (C::VersoVerso, O::Remover) => "mapa_verso_verso_remover",

        (C::BombomBombom, O::Criar) => "mapa_bombom_bombom_criar",
        (C::BombomBombom, O::Definir) => "mapa_bombom_bombom_definir",
        (C::BombomBombom, O::Obter) => "mapa_bombom_bombom_obter",
        (C::BombomBombom, O::Tem) => "mapa_bombom_bombom_tem",
        (C::BombomBombom, O::Tamanho) => "mapa_bombom_bombom_tamanho",
        (C::BombomBombom, O::Remover) => "mapa_bombom_bombom_remover",

        (C::BombomVerso, O::Criar) => "mapa_bombom_verso_criar",
        (C::BombomVerso, O::Definir) => "mapa_bombom_verso_definir",
        (C::BombomVerso, O::Obter) => "mapa_bombom_verso_obter",
        (C::BombomVerso, O::Tem) => "mapa_bombom_verso_tem",
        (C::BombomVerso, O::Tamanho) => "mapa_bombom_verso_tamanho",
        (C::BombomVerso, O::Remover) => "mapa_bombom_verso_remover",
    }
}

/// A identidade executável monomórfica do par `(classe, operação)`.
///
/// O lado direito da relação é sempre uma identidade que C1 já declara: a
/// travessia por [`intrinsic_from_public_spelling`] é o que impede esta
/// autoridade de virar um segundo registry público.
pub fn specialize(class: CanonicalMapClass, operation: GenericMapOperation) -> IntrinsicIdentity {
    let spelling = monomorphic_public_spelling(class, operation);
    intrinsic_from_public_spelling(spelling)
        .expect("célula de especialização de mapa endereça grafia pública registrada em C1")
}

/// A grafia canônica da identidade especializada.
///
/// Conveniência para as fases que transportam grafia; a grafia devolvida é a da
/// própria entrada de C1, não o literal desta tabela.
pub fn specialize_spelling(
    class: CanonicalMapClass,
    operation: GenericMapOperation,
) -> &'static str {
    specialize(class, operation).canonical_public_spelling()
}

/// Especialização a partir da grafia genérica, quando ela é operação de mapa.
pub fn specialize_generic_spelling(
    class: CanonicalMapClass,
    generic_spelling: &str,
) -> Option<&'static str> {
    GenericMapOperation::from_generic_public_spelling(generic_spelling)
        .map(|operation| specialize_spelling(class, operation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toda_celula_do_dominio_enderecada_existe_em_c1() {
        for &class in CANONICAL_MAP_CLASSES {
            for &operation in GENERIC_MAP_OPERATIONS {
                let spelling = specialize_spelling(class, operation);
                assert!(
                    crate::intrinsics::registry::e_historica(spelling),
                    "{class:?}/{operation:?} saiu da superfície histórica"
                );
            }
        }
    }

    #[test]
    fn a_relacao_e_injetiva() {
        let mut vistos = std::collections::BTreeSet::new();
        for &class in CANONICAL_MAP_CLASSES {
            for &operation in GENERIC_MAP_OPERATIONS {
                assert!(
                    vistos.insert(specialize_spelling(class, operation)),
                    "{class:?}/{operation:?} repetiu um alvo já usado"
                );
            }
        }
        assert_eq!(vistos.len(), 24);
    }

    #[test]
    fn a_grafia_generica_ida_e_volta() {
        for &operation in GENERIC_MAP_OPERATIONS {
            assert_eq!(
                GenericMapOperation::from_generic_public_spelling(
                    operation.generic_public_spelling()
                ),
                Some(operation)
            );
        }
        assert_eq!(
            GenericMapOperation::from_generic_public_spelling("lista_obter"),
            None
        );
        assert_eq!(
            GenericMapOperation::from_generic_public_spelling("mapa_verso_bombom_obter"),
            None
        );
    }
}
