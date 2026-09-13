//! Oráculo interno da autoridade de representação de mapa.
//!
//! As expectativas são literais: nenhuma delas é produzida pela própria
//! autoridade, e por isso uma tabela errada aqui não se justifica sozinha.

use super::*;
use crate::token::{Position, Span};

fn span() -> Span {
    Span::single(Position::new(1, 1))
}

fn other_span() -> Span {
    Span::single(Position::new(9, 9))
}

fn generic(key: Type, value: Type) -> Type {
    Type::Map {
        key: Box::new(key),
        value: Box::new(value),
        span: span(),
    }
}

/// As quatro linhas históricas, escritas à mão.
fn historical_rows() -> Vec<(Type, &'static str, &'static str)> {
    vec![
        (Type::MapVersoBombom(span()), "verso", "bombom"),
        (Type::MapVersoVerso(span()), "verso", "verso"),
        (Type::MapBombomBombom(span()), "bombom", "bombom"),
        (Type::MapBombomVerso(span()), "bombom", "verso"),
    ]
}

#[test]
fn variante_historica_decompoe_nos_componentes_literais() {
    for (historical, key, value) in historical_rows() {
        let parts = components(&historical).expect("variante histórica é mapa");
        assert_eq!(parts.key.name(), key, "chave de {}", historical.name());
        assert_eq!(parts.value.name(), value, "valor de {}", historical.name());
    }
}

#[test]
fn mapa_generico_decompoe_nos_componentes_escritos() {
    let ty = generic(Type::Verso(span()), Type::U64(span()));
    let parts = components(&ty).expect("mapa genérico é mapa");
    assert_eq!(parts.key.name(), "verso");
    assert_eq!(parts.value.name(), "u64");
}

#[test]
fn as_duas_grafias_da_mesma_classe_decompoem_igual() {
    for (historical, key, value) in historical_rows() {
        let historical_parts = components(&historical).expect("histórica é mapa");
        let adult = generic(
            historical_parts.key.as_ref().clone(),
            historical_parts.value.as_ref().clone(),
        );
        let adult_parts = components(&adult).expect("adulta é mapa");
        assert_eq!(adult_parts.key.name(), key);
        assert_eq!(adult_parts.value.name(), value);
        assert_eq!(
            class_of(&historical),
            class_of(&adult),
            "classe de {} nas duas grafias",
            historical.name()
        );
        assert!(class_of(&historical).is_some());
    }
}

#[test]
fn representacao_canonica_dos_quatro_pares() {
    let pares = [
        (
            Type::Verso(span()),
            Type::Bombom(span()),
            "mapa<verso,bombom>",
        ),
        (
            Type::Verso(span()),
            Type::Verso(span()),
            "mapa<verso,verso>",
        ),
        (
            Type::Bombom(span()),
            Type::Bombom(span()),
            "mapa<bombom,bombom>",
        ),
        (
            Type::Bombom(span()),
            Type::Verso(span()),
            "mapa<bombom,verso>",
        ),
    ];
    for (key, value, esperado) in pares {
        let canonical =
            canonical_representation(&key, &value, other_span()).expect("par histórico canoniza");
        assert_eq!(canonical.name(), esperado);
        assert_eq!(
            canonical.span(),
            other_span(),
            "a canonização usa o span do mapa, não o dos componentes"
        );
    }
}

#[test]
fn par_fora_das_quatro_classes_nao_tem_representacao_canonica() {
    let fora = [
        (Type::Verso(span()), Type::U64(span())),
        (Type::Verso(span()), Type::Logica(span())),
        (Type::Bombom(span()), Type::I32(span())),
        (Type::U64(span()), Type::Bombom(span())),
        (
            Type::Verso(span()),
            Type::Enum {
                name: "Cor".to_string(),
                span: span(),
            },
        ),
    ];
    for (key, value) in fora {
        assert!(
            canonical_representation(&key, &value, span()).is_none(),
            "par ({},{}) não é classe histórica",
            key.name(),
            value.name()
        );
        assert!(class_of_components(&key, &value).is_none());
    }
}

#[test]
fn tipo_que_nao_e_mapa_nao_decompoe() {
    let nao_mapas = [
        Type::Verso(span()),
        Type::Bombom(span()),
        Type::ListBombom(span()),
        Type::ListVerso(span()),
        Type::Nulo(span()),
        Type::Struct {
            name: "Ninho".to_string(),
            span: span(),
        },
    ];
    for ty in nao_mapas {
        assert!(components(&ty).is_none(), "{} não é mapa", ty.name());
        assert!(class_of(&ty).is_none(), "{} não tem classe", ty.name());
    }
}

#[test]
fn mapa_generico_fora_das_classes_nao_ganha_classe() {
    let ty = generic(Type::Verso(span()), Type::U64(span()));
    assert!(components(&ty).is_some());
    assert!(class_of(&ty).is_none());
}
