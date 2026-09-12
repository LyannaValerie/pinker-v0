//! Autoridade única da correspondência entre a **representação física** de um
//! tipo de mapa e seus **componentes semânticos**.
//!
//! Esta autoridade responde UMA pergunta, nas duas direções em que ela é feita:
//!
//! ```text
//! IF THIS TYPE IS A MAP,
//! WHAT ARE ITS SEMANTIC KEY AND VALUE COMPONENTS?
//!
//! GIVEN RESOLVED KEY AND VALUE,
//! WHICH PHYSICAL REPRESENTATION IS CANONICAL FOR THAT MAP?
//! ```
//!
//! O parser dobra `mapa<K,V>` para uma das quatro variantes históricas quando
//! chave e valor aparecem **literalmente** em combinação conhecida, e mantém
//! [`Type::Map`] quando qualquer componente chega por apelido. Depois da
//! resolução de apelidos as duas grafias denotam o mesmo mapa, mas continuavam
//! carregando representações físicas diferentes — e cada consumidor que
//! decidisse pela variante física passava a responder coisas diferentes para o
//! mesmo tipo resolvido.
//!
//! ```text
//! ONE QUESTION -> ONE AUTHORITY -> MANY CONSUMERS
//! ```
//!
//! # O que esta autoridade NÃO decide
//!
//! ```text
//! MAP_REPRESENTATION_RELATION != TYPE_COMPATIBILITY_POLICY
//! MAP_REPRESENTATION_RELATION != EXACT_SEMANTIC_IDENTITY
//! MAP_REPRESENTATION_RELATION != MAP_KEY_OR_VALUE_ADMISSIBILITY
//! MAP_REPRESENTATION_RELATION != OPERATION_SPECIALIZATION
//! MAP_REPRESENTATION_RELATION != RUNTIME_STORAGE_CONTRACT
//! ```
//!
//! - **Compatibilidade** (`M`) continua sendo de `semantic::check_type_match`.
//!   Esta autoridade só entrega os componentes; quem decide se dois componentes
//!   casam é a política de compatibilidade, inalterada. Decompor não é
//!   igualar: `E` (identidade exata) e `M` (compatibilidade) seguem relações
//!   diferentes, e nenhuma delas é `canonical_type_key(a) == canonical_type_key(b)`.
//! - **Identidade semântica exata** continua sendo a chave canônica de
//!   [`crate::union_canon`], que já rendia o mesmo texto para as duas grafias.
//! - **Quais chaves e valores são admissíveis** continua sendo da semântica:
//!   [`canonical_representation`] só é consultada depois da validação, e um par
//!   fora das quatro classes históricas simplesmente não tem representação
//!   canônica especializada — segue no mapa genérico adulto.
//! - **Qual identidade executável** um par `(classe, operação)` endereça
//!   continua em [`crate::map_specialization`]. Esta autoridade nomeia a
//!   classe; aquela escolhe a operação monomórfica.
//!
//! # Por que a variante histórica é a forma canônica
//!
//! A escolha não nasce aqui: a IR já declara, em
//! `ir::model::expected_representation_for_key`, que a chave canônica
//! `mapa<verso,bombom>` exige a representação operacional
//! `TypeIR::MapVersoBombom` — e assim para as outras três. Um mapa resolvido
//! dessas quatro classes que chegasse à IR como `Type::Map` produziria a mesma
//! chave canônica com representação divergente. Canonizar para a variante
//! histórica é fazer a resolução semântica concordar com a autoridade que já
//! existia, não inventar uma nova.

use std::borrow::Cow;

use crate::ast::Type;
use crate::map_specialization::CanonicalMapClass;
use crate::token::Span;

// @pinker-nav:start mapa.representacao.canonica
// @pinker-nav:domain tipos
// @pinker-nav:layer semantica
// @pinker-nav:summary Autoridade única da correspondência entre a representação física de um tipo de mapa e seus componentes semânticos: `components` decompõe qualquer mapa — as quatro variantes históricas e o `Type::Map` adulto — no par (chave, valor), `class_of` nomeia a classe canônica independentemente da grafia e `canonical_representation` devolve a variante física canônica de um par já resolvido. Não decide compatibilidade, identidade exata, admissibilidade de componente nem especialização de operação.

/// Chave e valor semânticos de um tipo de mapa.
///
/// Os componentes das variantes históricas não existem como nós na AST: eles
/// são sintetizados no span do próprio mapa. Os do mapa genérico adulto são
/// emprestados sem cópia.
pub struct MapComponents<'a> {
    pub key: Cow<'a, Type>,
    pub value: Cow<'a, Type>,
}

/// A tabela — o único lugar onde as quatro classes históricas relacionam
/// variante física, chave e valor.
///
/// `key_is_verso` e `value_is_verso` descrevem os componentes: `false` é
/// `bombom`. Duas grafias da mesma linha são a mesma coisa.
const HISTORICAL_CLASSES: [(CanonicalMapClass, bool, bool); 4] = [
    (CanonicalMapClass::VersoBombom, true, false),
    (CanonicalMapClass::VersoVerso, true, true),
    (CanonicalMapClass::BombomBombom, false, false),
    (CanonicalMapClass::BombomVerso, false, true),
];

fn component(is_verso: bool, span: Span) -> Type {
    if is_verso {
        Type::Verso(span)
    } else {
        Type::Bombom(span)
    }
}

fn component_flag(ty: &Type) -> Option<bool> {
    match ty {
        Type::Verso(_) => Some(true),
        Type::Bombom(_) => Some(false),
        _ => None,
    }
}

/// A variante física histórica de uma classe canônica.
pub fn representation(class: CanonicalMapClass, span: Span) -> Type {
    match class {
        CanonicalMapClass::VersoBombom => Type::MapVersoBombom(span),
        CanonicalMapClass::VersoVerso => Type::MapVersoVerso(span),
        CanonicalMapClass::BombomBombom => Type::MapBombomBombom(span),
        CanonicalMapClass::BombomVerso => Type::MapBombomVerso(span),
    }
}

/// A classe canônica denotada por um par de componentes já resolvidos.
///
/// `None` quando o par não é uma das quatro classes históricas — um mapa
/// perfeitamente válido pode não ter forma especializada.
pub fn class_of_components(key: &Type, value: &Type) -> Option<CanonicalMapClass> {
    let key_is_verso = component_flag(key)?;
    let value_is_verso = component_flag(value)?;
    HISTORICAL_CLASSES
        .iter()
        .find(|(_, k, v)| *k == key_is_verso && *v == value_is_verso)
        .map(|(class, _, _)| *class)
}

/// Chave e valor semânticos deste tipo, se ele for um mapa.
///
/// `None` significa "não é mapa", nunca "é um mapa sem componentes".
pub fn components(ty: &Type) -> Option<MapComponents<'_>> {
    if let Type::Map { key, value, .. } = ty {
        return Some(MapComponents {
            key: Cow::Borrowed(key.as_ref()),
            value: Cow::Borrowed(value.as_ref()),
        });
    }
    let (class, span) = match ty {
        Type::MapVersoBombom(span) => (CanonicalMapClass::VersoBombom, *span),
        Type::MapVersoVerso(span) => (CanonicalMapClass::VersoVerso, *span),
        Type::MapBombomBombom(span) => (CanonicalMapClass::BombomBombom, *span),
        Type::MapBombomVerso(span) => (CanonicalMapClass::BombomVerso, *span),
        _ => return None,
    };
    let (_, key_is_verso, value_is_verso) = HISTORICAL_CLASSES
        .iter()
        .find(|(candidate, _, _)| *candidate == class)
        .copied()
        .expect("as quatro classes históricas estão na tabela");
    Some(MapComponents {
        key: Cow::Owned(component(key_is_verso, span)),
        value: Cow::Owned(component(value_is_verso, span)),
    })
}

/// A classe canônica deste tipo, qualquer que seja sua representação física.
///
/// `None` para todo tipo que não é mapa e para o mapa cujo par de componentes
/// não é uma das quatro classes históricas.
pub fn class_of(ty: &Type) -> Option<CanonicalMapClass> {
    let parts = components(ty)?;
    class_of_components(parts.key.as_ref(), parts.value.as_ref())
}

/// A representação canônica de um mapa cujos componentes já estão resolvidos.
///
/// `None` quando o par não é uma das quatro classes históricas: aí o mapa
/// genérico adulto já é a forma canônica e o chamador o preserva.
pub fn canonical_representation(key: &Type, value: &Type, span: Span) -> Option<Type> {
    class_of_components(key, value).map(|class| representation(class, span))
}
// @pinker-nav:end mapa.representacao.canonica

#[cfg(test)]
mod tests;
