//! Validador da IR estruturada (alto nível) do Pinker.
//!
//! Opera sobre `ProgramIR` antes do lowering para CFG IR. Verifica:
//! - constantes globais: nome, tipo e valor não nulo
//! - funções: bloco de entrada `entry`, slots únicos por parâmetro/local
//! - blocos: instruções `let`/`assign`/`return`/`if` com tipos compatíveis
//! - expressões: inferência recursiva de tipo via `infer_value_type`
//!
//! Ponto de entrada: [`validate_program`].

use crate::error::PinkerError;
use crate::ir::{
    BinaryOpIR, BlockIR, FunctionIR, InstructionIR, ProgramIR, TypeIR, UnaryOpIR, ValueIR,
};
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct FunctionSig {
    ret_type: TypeIR,
    params: Vec<TypeIR>,
}

// @pinker-nav:start ir.validacao.invariantes
// @pinker-nav:domain validacao
// @pinker-nav:layer ir
// @pinker-nav:summary Valida os invariantes da IR estruturada antes do lowering para CFG: constantes globais bem tipadas, bloco de entrada e slots únicos por função, e comandos/expressões com tipos compatíveis via inferência recursiva.
/// Confere a metadata publicada das variantes de `leque` (D1).
///
/// A defesa é repetida aqui, em vez de confiar no lowering, porque é
/// exatamente esta metadata que decide o helper de runtime e a compatibilidade
/// de uma carga: uma entrada fabricada com representação de uma palavra e
/// identidade de outro tipo passaria despercebida por todas as camadas
/// seguintes, que só enxergam `TypeIR`.
fn validate_enum_variant_metadata(program: &ProgramIR) -> Result<(), PinkerError> {
    for variant in &program.enum_variants {
        if variant.enum_name.trim().is_empty() || variant.variant_name.trim().is_empty() {
            return Err(ir_validation_error(
                "metadata de variante de leque sem nome",
                default_span(),
            ));
        }
        for (index, payload) in variant.payloads.iter().enumerate() {
            let posicao = format!(
                "carga {} de '{}.{}'",
                index + 1,
                variant.enum_name,
                variant.variant_name
            );
            let entry = program
                .resolved_types
                .get(payload.resolved_type_id.0 as usize)
                .filter(|entry| entry.id == payload.resolved_type_id)
                .ok_or_else(|| {
                    ir_validation_error(
                        &format!(
                            "E-IR-ENUM-PAYLOAD-METADATA: {posicao} referencia identidade {} ausente da tabela",
                            payload.resolved_type_id.0
                        ),
                        default_span(),
                    )
                })?;
            if entry.representation != payload.operational_type {
                return Err(ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} declara representação '{}' e a identidade internada é '{}'",
                        payload.operational_type.name(),
                        entry.representation.name()
                    ),
                    default_span(),
                ));
            }
            if entry.canonical_key != payload.canonical_key {
                return Err(ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} declara identidade '{}' e a tabela registra '{}'",
                        payload.canonical_key, entry.canonical_key
                    ),
                    default_span(),
                ));
            }
            if crate::union_canon::is_poisoned_key(&payload.canonical_key) {
                return Err(ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} carrega identidade não resolvida '{}'",
                        payload.canonical_key
                    ),
                    default_span(),
                ));
            }
            // A classe é derivada da representação por uma função total: se a
            // metadata declarasse outra, o helper escolhido divergiria da
            // categoria do valor.
            let esperada = classe_da_representacao(payload.operational_type).ok_or_else(|| {
                ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} usa representação '{}' fora do contrato de cargas",
                        payload.operational_type.name()
                    ),
                    default_span(),
                )
            })?;
            if esperada != payload.class {
                return Err(ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} declara classe '{}' e a representação '{}' exige '{}'",
                        payload.class.name(),
                        payload.operational_type.name(),
                        esperada.name()
                    ),
                    default_span(),
                ));
            }
            if payload.element_type_id != entry.element {
                return Err(ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} declara identidade de elemento divergente da identidade internada"
                    ),
                    default_span(),
                ));
            }
            // Só listas têm elemento; um elemento numa carga imediata ou `verso`
            // seria metadata inventada.
            let admite_elemento = matches!(payload.operational_type, TypeIR::ListBombom);
            if payload.element_type_id.is_some() && !admite_elemento {
                return Err(ir_validation_error(
                    &format!(
                        "E-IR-ENUM-PAYLOAD-METADATA: {posicao} declara elemento para a representação '{}'",
                        payload.operational_type.name()
                    ),
                    default_span(),
                ));
            }
        }
    }
    Ok(())
}

/// Classe de carga exigida por uma representação operacional.
///
/// É a inversa da escolha feita em [`crate::enum_payload`]: as duas precisam
/// concordar, e é justamente a divergência entre elas que este validador
/// existe para pegar.
fn classe_da_representacao(ty: TypeIR) -> Option<crate::enum_payload::EnumPayloadClass> {
    use crate::enum_payload::EnumPayloadClass;
    match ty {
        TypeIR::Bombom => Some(EnumPayloadClass::ImmediateDiscriminant),
        TypeIR::Verso => Some(EnumPayloadClass::Verso),
        TypeIR::ListBombom | TypeIR::ListVerso => Some(EnumPayloadClass::OpaqueWordHandle),
        _ => None,
    }
}

pub fn validate_program(program: &ProgramIR) -> Result<(), PinkerError> {
    crate::ir::validate_union_registry(&program.union_types)
        .map_err(|message| ir_validation_error(&message, default_span()))?;
    crate::ir::validate_resolved_type_table(&program.resolved_types)
        .map_err(|message| ir_validation_error(&message, default_span()))?;
    crate::ir::validate_union_registry_identities(&program.union_types, &program.resolved_types)
        .map_err(|message| ir_validation_error(&message, default_span()))?;
    validate_resolved_identities(program)?;
    validate_enum_variant_metadata(program)?;
    validate_union_operations(program)?;
    let mut consts = HashMap::new();
    for konst in &program.consts {
        if konst.name.trim().is_empty() {
            return Err(ir_validation_error("constante global sem nome", konst.span));
        }
        consts.insert(konst.name.clone(), konst.ty);
    }

    let mut funcs = HashMap::new();
    for function in &program.functions {
        funcs.insert(
            function.name.clone(),
            FunctionSig {
                ret_type: function.ret_type,
                params: function.params.iter().map(|p| p.ty).collect(),
            },
        );
    }
    funcs.insert(
        "ouvir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    funcs.insert(
        "ouvir_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![],
        },
    );
    funcs.insert(
        "ouvir_verso_ou".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "aleatorio_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "aleatorio_proximo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "lista_bombom_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::ListBombom,
            params: vec![],
        },
    );
    funcs.insert(
        "alocar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Pointer { is_volatile: false },
            params: vec![TypeIR::U64],
        },
    );
    funcs.insert(
        "liberar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Pointer { is_volatile: false }],
        },
    );
    funcs.insert(
        "lista_bombom_anexar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "lista_bombom_obter".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "lista_bombom_tamanho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListBombom],
        },
    );
    funcs.insert(
        "lista_bombom_definir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "lista_bombom_tirar_ultimo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListBombom],
        },
    );
    funcs.insert(
        "lista_verso_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::ListVerso,
            params: vec![],
        },
    );
    funcs.insert(
        "lista_verso_anexar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListVerso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "lista_verso_obter".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::ListVerso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "lista_verso_tamanho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListVerso],
        },
    );
    funcs.insert(
        "lista_verso_definir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListVerso, TypeIR::Bombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "lista_verso_tirar_ultimo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::ListVerso],
        },
    );
    funcs.insert(
        "mapa_verso_bombom_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::MapVersoBombom,
            params: vec![],
        },
    );
    funcs.insert(
        "mapa_verso_bombom_definir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_verso_bombom_obter".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "mapa_verso_bombom_tem".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "mapa_verso_bombom_tamanho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoBombom],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_verso_bombom_iterador_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoBombom],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_verso_verso_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::MapVersoVerso,
            params: vec![],
        },
    );
    funcs.insert(
        "mapa_verso_verso_definir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "mapa_verso_verso_obter".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "mapa_verso_verso_tem".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "mapa_verso_verso_tamanho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoVerso],
        },
    );
    funcs.insert(
        "mapa_verso_verso_remover".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoVerso],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_bombom_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::MapBombomBombom,
            params: vec![],
        },
    );
    funcs.insert(
        "mapa_bombom_bombom_definir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_bombom_obter".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_bombom_tem".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_bombom_tamanho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomBombom],
        },
    );
    funcs.insert(
        "mapa_bombom_bombom_remover".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomBombom],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_verso_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::MapBombomVerso,
            params: vec![],
        },
    );
    funcs.insert(
        "mapa_bombom_verso_definir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "mapa_bombom_verso_obter".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_verso_tem".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_bombom_verso_tamanho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomVerso],
        },
    );
    funcs.insert(
        "mapa_bombom_verso_remover".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomVerso],
        },
    );
    funcs.insert(
        "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_leque_criar_0".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_leque_anexar_b".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_leque_anexar_v".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "__pinker_internal_leque_tag".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_leque_carga_b".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "__pinker_internal_leque_carga_v".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    // D1: handles de lista como carga. O parâmetro e o retorno são de uma
    // palavra, exatamente como o caminho `_b`; o que muda é a categoria
    // operacional exigida, que preserva a identidade do valor.
    funcs.insert(
        crate::enum_payload::ANEXAR_LISTA_BOMBOM.to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::ListBombom],
        },
    );
    funcs.insert(
        crate::enum_payload::ANEXAR_LISTA_VERSO.to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::ListVerso],
        },
    );
    funcs.insert(
        crate::enum_payload::CARGA_LISTA_BOMBOM.to_string(),
        FunctionSig {
            ret_type: TypeIR::ListBombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        crate::enum_payload::CARGA_LISTA_VERSO.to_string(),
        FunctionSig {
            ret_type: TypeIR::ListVerso,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    // Não há assinatura chamável de união: tag e extração são
    // `ValueIR::UnionTag`/`ValueIR::UnionExtract`, nós tipados da IR.
    funcs.insert(
        "argumento".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "argumento_ou".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "tem_chave".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "tem_argumento_nomeado".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "pedir_argumento".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "argumento_nomeado_ou".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "tem_flag".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "ambiente_ou".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "buscar_contexto".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "argumento_nomeado_ou_ambiente_ou".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "caminho_existe".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "e_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "e_diretorio".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "juntar_caminho".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "tamanho_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "e_vazio".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "criar_diretorio".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "remover_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "remover_diretorio".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "diretorio_atual".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![],
        },
    );
    funcs.insert(
        "quantos_argumentos".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    funcs.insert(
        "tem_argumento".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "sair".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "abrir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "ler_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "ler_verso_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "ler_arquivo_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "arquivo_ou".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "fechar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "criar_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "abrir_anexo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "escrever".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "escrever_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "truncar_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "anexar_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "juntar_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "tamanho_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "indice_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "contem_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "comeca_com".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "termina_com".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "igual_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "vazio_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "aparar_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "minusculo_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "maiusculo_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "indice_verso_em".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 140
    funcs.insert(
        "buscar_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "nao_vazio_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    // Fase 137
    funcs.insert(
        "dividir_verso_em".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "dividir_verso_contar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 138
    funcs.insert(
        "substituir_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 139
    funcs.insert(
        "juntar_verso_com".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "formatar_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    // Fase 158
    funcs.insert(
        "ler_linha_csv_bombom".to_string(),
        FunctionSig {
            ret_type: TypeIR::ListBombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "emitir_linha_csv_bombom".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::ListBombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "ler_json_plano_bombom".to_string(),
        FunctionSig {
            ret_type: TypeIR::MapVersoBombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "emitir_json_plano_bombom".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::MapVersoBombom],
        },
    );
    // Fase 160
    funcs.insert(
        "tempo_unix".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    funcs.insert(
        "formatar_tempo_unix".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    // Fase 161
    funcs.insert(
        "executar_processo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 165
    funcs.insert(
        "executar_com_entrada".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 166
    funcs.insert(
        "pipeline_minimo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 163
    funcs.insert(
        "capturar_stdout".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    // Fase 164
    funcs.insert(
        "capturar_stderr".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "afirmar".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Logica],
        },
    );
    funcs.insert(
        "dormir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "copiar_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "renomear_arquivo".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    funcs.insert(
        "verso_para_bombom".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    funcs.insert(
        "bombom_para_verso".to_string(),
        FunctionSig {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    funcs.insert(
        "aleatorio_entre".to_string(),
        FunctionSig {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "mapa_verso_bombom_remover".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso],
        },
    );
    funcs.insert(
        "lista_bombom_inserir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    funcs.insert(
        "lista_verso_inserir".to_string(),
        FunctionSig {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListVerso, TypeIR::Bombom, TypeIR::Verso],
        },
    );

    for konst in &program.consts {
        let ty = infer_value_type(&konst.value, &HashMap::new(), &consts, &funcs, konst.span)
            .map_err(|err| enrich_ir_error(err, None, None, Some("item='const'")))?;
        if !value_matches_expected(&konst.value, ty, konst.ty) {
            return Err(ir_validation_error(
                "tipo da constante global não confere com o valor",
                konst.span,
            ));
        }
        if ty == TypeIR::Nulo {
            return Err(ir_validation_error(
                "constante global não pode ter tipo nulo",
                konst.span,
            ));
        }
    }

    for function in &program.functions {
        validate_function(function, &consts, &funcs)?;
    }

    Ok(())
}

fn validate_function(
    function: &FunctionIR,
    consts: &HashMap<String, TypeIR>,
    funcs: &HashMap<String, FunctionSig>,
) -> Result<(), PinkerError> {
    if function.entry.label != "entry" {
        return Err(ir_validation_error_ctx(
            function,
            None,
            "função IR deve ter bloco de entrada com rótulo 'entry'",
            None,
            function.span,
        ));
    }

    let mut slots = HashMap::new();
    let mut seen = HashSet::new();

    for param in &function.params {
        if param.slot.trim().is_empty() {
            return Err(ir_validation_error_ctx(
                function,
                None,
                "parâmetro IR com slot vazio",
                Some("item='param'"),
                function.span,
            ));
        }
        if !seen.insert(param.slot.clone()) {
            return Err(ir_validation_error_ctx(
                function,
                None,
                "slot duplicado em parâmetros",
                Some(&format!("slot='{}'", param.slot)),
                function.span,
            ));
        }
        slots.insert(param.slot.clone(), param.ty);
    }

    for local in &function.locals {
        if local.slot.trim().is_empty() {
            return Err(ir_validation_error_ctx(
                function,
                None,
                "local IR com slot vazio",
                Some("item='local'"),
                function.span,
            ));
        }
        if !seen.insert(local.slot.clone()) {
            return Err(ir_validation_error_ctx(
                function,
                None,
                "slot duplicado em locais",
                Some(&format!("slot='{}'", local.slot)),
                function.span,
            ));
        }
        slots.insert(local.slot.clone(), local.ty);
    }

    validate_block(&function.entry, function, &slots, consts, funcs)
}

fn validate_block(
    block: &BlockIR,
    function: &FunctionIR,
    slots: &HashMap<String, TypeIR>,
    consts: &HashMap<String, TypeIR>,
    funcs: &HashMap<String, FunctionSig>,
) -> Result<(), PinkerError> {
    if block.label.trim().is_empty() {
        return Err(ir_validation_error_ctx(
            function,
            Some(block),
            "bloco IR sem rótulo",
            None,
            block.span,
        ));
    }

    for instruction in &block.instructions {
        match instruction {
            InstructionIR::Let { slot, value, span }
            | InstructionIR::Assign { slot, value, span } => {
                let Some(expected_ty) = slots.get(slot) else {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "slot local inexistente",
                        Some(&format!("slot='{}', instr='let/assign'", slot)),
                        *span,
                    ));
                };
                let actual_ty =
                    infer_value_type(value, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some(&format!("slot='{}', instr='let/assign'", slot)),
                        )
                    })?;
                if actual_ty == TypeIR::Nulo {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "valor nulo em posição inválida",
                        Some("instr='let/assign'"),
                        *span,
                    ));
                }
                if !value_matches_expected(value, actual_ty, *expected_ty) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "atribuição IR com tipo incompatível",
                        Some(&format!(
                            "instr='let/assign', esperado={:?}, recebido={:?}",
                            expected_ty, actual_ty
                        )),
                        *span,
                    ));
                }
            }
            InstructionIR::StoreIndirect {
                ptr,
                value,
                value_type,
                is_volatile,
                span,
            } => {
                let ptr_ty = infer_value_type(ptr, slots, consts, funcs, *span).map_err(|err| {
                    enrich_ir_error(
                        err,
                        Some(function),
                        Some(block),
                        Some("instr='store_indirect'"),
                    )
                })?;
                let TypeIR::Pointer {
                    is_volatile: ptr_is_volatile,
                } = ptr_ty
                else {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "store_indirect exige ponteiro",
                        Some("instr='store_indirect'"),
                        *span,
                    ));
                };
                if ptr_is_volatile != *is_volatile {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "store_indirect com metadata de volatilidade inconsistente",
                        Some("instr='store_indirect'"),
                        *span,
                    ));
                }
                let actual_ty =
                    infer_value_type(value, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some("instr='store_indirect'"),
                        )
                    })?;
                if !value_matches_expected(value, actual_ty, *value_type) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "store_indirect com tipo incompatível",
                        Some(&format!(
                            "instr='store_indirect', esperado={:?}, recebido={:?}",
                            value_type, actual_ty
                        )),
                        *span,
                    ));
                }
            }
            InstructionIR::StoreFieldIndirect {
                base,
                value,
                value_type,
                span,
                ..
            } => {
                let _base_ty =
                    infer_value_type(base, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some("instr='store_field_indirect'"),
                        )
                    })?;
                let actual_ty =
                    infer_value_type(value, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some("instr='store_field_indirect'"),
                        )
                    })?;
                if !value_matches_expected(value, actual_ty, *value_type) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "store_field_indirect com tipo incompatível",
                        Some(&format!(
                            "instr='store_field_indirect', esperado={:?}, recebido={:?}",
                            value_type, actual_ty
                        )),
                        *span,
                    ));
                }
            }
            InstructionIR::StoreIndexed {
                base,
                index,
                value,
                element_type,
                span,
            } => {
                let base_ty =
                    infer_value_type(base, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some("instr='store_indexed'"),
                        )
                    })?;
                if !matches!(
                    base_ty,
                    TypeIR::FixedArray {
                        element: crate::ir::ScalarTypeIR::Bombom,
                        ..
                    }
                ) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "store_indexed exige base de array fixo '[bombom; N]'",
                        Some("instr='store_indexed'"),
                        *span,
                    ));
                }
                let _index_ty =
                    infer_value_type(index, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some("instr='store_indexed'"),
                        )
                    })?;
                let actual_ty =
                    infer_value_type(value, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(
                            err,
                            Some(function),
                            Some(block),
                            Some("instr='store_indexed'"),
                        )
                    })?;
                if !value_matches_expected(value, actual_ty, *element_type) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "store_indexed com tipo incompatível",
                        Some(&format!(
                            "instr='store_indexed', esperado={:?}, recebido={:?}",
                            element_type, actual_ty
                        )),
                        *span,
                    ));
                }
            }
            InstructionIR::Expr { value, span } => {
                let ty = infer_value_type(value, slots, consts, funcs, *span).map_err(|err| {
                    enrich_ir_error(err, Some(function), Some(block), Some("instr='expr'"))
                })?;
                if ty == TypeIR::Nulo {
                    match value {
                        ValueIR::Call { .. }
                        | ValueIR::CallRaw { .. }
                        | ValueIR::TraitCall { .. } => {}
                        _ => {
                            return Err(ir_validation_error_ctx(
                                function,
                                Some(block),
                                "valor nulo em expressão inválida",
                                Some("instr='expr'"),
                                *span,
                            ));
                        }
                    }
                }
            }
            InstructionIR::Return { value, span } => match (function.ret_type, value) {
                (TypeIR::Nulo, Some(_)) => {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "return com valor em função nulo",
                        Some("instr='return'"),
                        *span,
                    ))
                }
                (TypeIR::Nulo, None) => {}
                (_, None) => {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "return sem valor em função que exige retorno",
                        Some("instr='return'"),
                        *span,
                    ))
                }
                (expected, Some(v)) => {
                    let ty = infer_value_type(v, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(err, Some(function), Some(block), Some("instr='return'"))
                    })?;
                    if ty == TypeIR::Nulo {
                        return Err(ir_validation_error_ctx(
                            function,
                            Some(block),
                            "return com valor nulo inválido",
                            Some("instr='return'"),
                            *span,
                        ));
                    }
                    if !value_matches_expected(v, ty, expected) {
                        return Err(ir_validation_error_ctx(
                            function,
                            Some(block),
                            "tipo de return incompatível",
                            Some(&format!(
                                "instr='return', esperado={:?}, recebido={:?}",
                                expected, ty
                            )),
                            *span,
                        ));
                    }
                }
            },
            InstructionIR::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                let cond_ty =
                    infer_value_type(condition, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(err, Some(function), Some(block), Some("instr='if'"))
                    })?;
                if cond_ty != TypeIR::Logica {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "condição de if deve ser lógica",
                        Some(&format!("instr='if', recebido={:?}", cond_ty)),
                        *span,
                    ));
                }
                validate_block(then_block, function, slots, consts, funcs)?;
                if let Some(else_block) = else_block {
                    validate_block(else_block, function, slots, consts, funcs)?;
                }
            }

            InstructionIR::While {
                condition,
                body_block,
                span,
            } => {
                let cond_ty =
                    infer_value_type(condition, slots, consts, funcs, *span).map_err(|err| {
                        enrich_ir_error(err, Some(function), Some(block), Some("instr='while'"))
                    })?;
                if cond_ty != TypeIR::Logica {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "condição de while deve ser lógica",
                        Some(&format!("instr='while', recebido={:?}", cond_ty)),
                        *span,
                    ));
                }
                validate_block(body_block, function, slots, consts, funcs)?;
            }

            InstructionIR::Break {
                loop_exit_label: _,
                span: _,
            } => {}
            InstructionIR::Continue {
                loop_continue_label: _,
                span: _,
            } => {}
            InstructionIR::Falar { args: _, span: _ } => {}
            InstructionIR::InlineAsm {
                chunks,
                operands,
                clobbers,
                span,
            } => {
                if chunks.is_empty() || chunks.iter().any(|chunk| chunk.trim().is_empty()) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "inline asm inválido: bloco vazio",
                        Some("instr='inline_asm'"),
                        *span,
                    ));
                }
                let mut specs = Vec::new();
                for operand in operands {
                    match operand {
                        crate::ir::InlineAsmOperandIR::Input {
                            name,
                            constraint,
                            value,
                            ty,
                        } => {
                            let inferred = infer_value_type(value, slots, consts, funcs, *span)?;
                            if inferred != *ty {
                                return Err(ir_validation_error_ctx(
                                    function,
                                    Some(block),
                                    "tipo de input inline asm divergente",
                                    Some("instr='inline_asm'"),
                                    *span,
                                ));
                            }
                            specs.push((name.clone(), *constraint));
                        }
                        crate::ir::InlineAsmOperandIR::Output {
                            name,
                            constraint,
                            slot,
                            ty,
                        } => {
                            if slots.get(slot) != Some(ty) {
                                return Err(ir_validation_error_ctx(
                                    function,
                                    Some(block),
                                    "output inline asm aponta para slot ausente ou de tipo divergente",
                                    Some("instr='inline_asm'"),
                                    *span,
                                ));
                            }
                            specs.push((name.clone(), *constraint));
                        }
                    }
                }
                crate::inline_asm::validate_bound_operands(chunks, &specs, clobbers).map_err(
                    |failure| {
                        ir_validation_error_ctx(
                            function,
                            Some(block),
                            &failure.to_string(),
                            Some("instr='inline_asm'"),
                            *span,
                        )
                    },
                )?;
            }
            InstructionIR::UnionMatch(union_match) => {
                let scrutinee_ty = infer_value_type(
                    &union_match.scrutinee,
                    slots,
                    consts,
                    funcs,
                    union_match.span,
                )
                .map_err(|err| {
                    enrich_ir_error(
                        err,
                        Some(function),
                        Some(block),
                        Some("instr='union_match'"),
                    )
                })?;
                if scrutinee_ty != TypeIR::Union(union_match.union_type_id) {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "scrutinee de union_match não é a união associada",
                        Some(&format!(
                            "instr='union_match', esperado=Union({}), recebido={:?}",
                            union_match.union_type_id.0, scrutinee_ty
                        )),
                        union_match.span,
                    ));
                }
                if slots.get(&union_match.scrutinee_binding.slot)
                    != Some(&TypeIR::Union(union_match.union_type_id))
                    || union_match.scrutinee_binding.ty != TypeIR::Union(union_match.union_type_id)
                {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "slot do scrutinee de union_match ausente ou com tipo divergente",
                        Some(&format!(
                            "instr='union_match', slot='{}'",
                            union_match.scrutinee_binding.slot
                        )),
                        union_match.span,
                    ));
                }
                if slots.get(&union_match.tag_binding.slot) != Some(&TypeIR::Bombom)
                    || union_match.tag_binding.ty != TypeIR::Bombom
                {
                    return Err(ir_validation_error_ctx(
                        function,
                        Some(block),
                        "slot da tag de union_match ausente ou com tipo divergente",
                        Some(&format!(
                            "instr='union_match', slot='{}'",
                            union_match.tag_binding.slot
                        )),
                        union_match.span,
                    ));
                }
                for arm in &union_match.arms {
                    let Some(expected) = slots.get(&arm.binding.slot) else {
                        return Err(ir_validation_error_ctx(
                            function,
                            Some(block),
                            "binding de braço de union_match sem slot local",
                            Some(&format!("instr='union_match', slot='{}'", arm.binding.slot)),
                            arm.span,
                        ));
                    };
                    if *expected != arm.payload_type || arm.binding.ty != arm.payload_type {
                        return Err(ir_validation_error_ctx(
                            function,
                            Some(block),
                            "binding de braço de union_match com tipo divergente do payload",
                            Some(&format!(
                                "instr='union_match', slot='{}', esperado={:?}, recebido={:?}",
                                arm.binding.slot, arm.payload_type, expected
                            )),
                            arm.span,
                        ));
                    }
                    if !arm.payload_layout.is_well_formed() {
                        return Err(ir_validation_error_ctx(
                            function,
                            Some(block),
                            "layout de payload inválido em braço de union_match",
                            Some("instr='union_match'"),
                            arm.span,
                        ));
                    }
                    validate_block(&arm.body, function, slots, consts, funcs)?;
                }
            }
        }
    }

    Ok(())
}

/// Confere que nenhuma identidade semântica foi perdida em parâmetro ou local.
///
/// Um slot cuja representação já é a identidade completa (escalares, `verso`,
/// listas e mapas monomórficos, `nulo`, arrays desses e uniões, cujo
/// `UnionTypeId` é nominal) pode dispensar a identidade explícita. Um slot cuja
/// representação é ambígua — `ninho`, `seta<T>`, `carinho(...)`,
/// `seta<carinho>` e `trato<...>` — precisa carregá-la: são exatamente as
/// categorias em que HR4 mostra que a representação não identifica o tipo.
///
/// Toda identidade presente também precisa existir na tabela do programa e
/// concordar com ela na representação.
fn validate_resolved_identities(program: &ProgramIR) -> Result<(), PinkerError> {
    let exige_identidade = |ty: TypeIR| {
        matches!(
            ty,
            TypeIR::Struct
                | TypeIR::Pointer { .. }
                | TypeIR::FunctionPointer
                | TypeIR::Function
                | TypeIR::TraitObject
        )
    };

    for function in &program.functions {
        let mut slots: Vec<(&str, TypeIR, Option<crate::ir::ResolvedTypeId>)> = Vec::new();
        for param in &function.params {
            slots.push((param.slot.as_str(), param.ty, param.resolved));
        }
        for local in &function.locals {
            slots.push((local.slot.as_str(), local.ty, local.resolved));
        }
        for (slot, ty, resolved) in slots {
            match resolved {
                None => {
                    if exige_identidade(ty) {
                        return Err(ir_validation_error(
                            &format!(
                                "E-IR-TYPE-IDENTITY-LOST: slot '{slot}' de '{}' na função '{}' \
                                 não carrega identidade semântica resolvida",
                                ty.name(),
                                function.name
                            ),
                            function.span,
                        ));
                    }
                }
                Some(resolved) => {
                    crate::ir::validate_resolved_type_reference(
                        &program.resolved_types,
                        resolved,
                        ty,
                    )
                    .map_err(|message| ir_validation_error(&message, function.span))?;
                }
            }
        }
    }

    Ok(())
}

// Fronteira de validação das operações internas tipadas de união na IR estruturada: percorre todo o programa e confronta cada `UnionMatch`, `UnionTag` e `UnionExtract` com a tabela internada — existência do `UnionTypeId`, pertencimento da tag, coincidência entre chave canônica e tag, tipo e layout do payload, ausência de braço repetido e cobertura integral. Nenhuma tag é recalculada aqui; o registry é a única fonte.
fn validate_union_operations(program: &ProgramIR) -> Result<(), PinkerError> {
    for function in &program.functions {
        validate_union_operations_block(&function.entry, &program.union_types)?;
    }
    for konst in &program.consts {
        validate_union_operations_value(&konst.value, &program.union_types, konst.span)?;
    }
    Ok(())
}

fn validate_union_operations_block(
    block: &BlockIR,
    unions: &[crate::ir::UnionTypeIR],
) -> Result<(), PinkerError> {
    for instruction in &block.instructions {
        match instruction {
            InstructionIR::UnionMatch(union_match) => {
                validate_union_operations_value(&union_match.scrutinee, unions, union_match.span)?;
                let arm_keys = union_match
                    .arms
                    .iter()
                    .map(|arm| (arm.tag, arm.canonical_member_key.clone()))
                    .collect::<Vec<_>>();
                crate::ir::validate_union_match_coverage(
                    unions,
                    union_match.union_type_id,
                    &arm_keys,
                )
                .map_err(|message| ir_validation_error(&message, union_match.span))?;
                for arm in &union_match.arms {
                    crate::ir::validate_union_member_reference(
                        unions,
                        union_match.union_type_id,
                        arm.tag,
                        &arm.canonical_member_key,
                        arm.payload_type,
                        arm.payload_layout,
                    )
                    .map_err(|message| ir_validation_error(&message, arm.span))?;
                    validate_union_operations_block(&arm.body, unions)?;
                }
            }
            InstructionIR::Let { value, span, .. }
            | InstructionIR::Assign { value, span, .. }
            | InstructionIR::Expr { value, span } => {
                validate_union_operations_value(value, unions, *span)?
            }
            InstructionIR::Return { value, span } => {
                if let Some(value) = value {
                    validate_union_operations_value(value, unions, *span)?;
                }
            }
            InstructionIR::StoreIndirect {
                ptr, value, span, ..
            } => {
                validate_union_operations_value(ptr, unions, *span)?;
                validate_union_operations_value(value, unions, *span)?;
            }
            InstructionIR::StoreFieldIndirect {
                base, value, span, ..
            } => {
                validate_union_operations_value(base, unions, *span)?;
                validate_union_operations_value(value, unions, *span)?;
            }
            InstructionIR::StoreIndexed {
                base,
                index,
                value,
                span,
                ..
            } => {
                validate_union_operations_value(base, unions, *span)?;
                validate_union_operations_value(index, unions, *span)?;
                validate_union_operations_value(value, unions, *span)?;
            }
            InstructionIR::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                validate_union_operations_value(condition, unions, *span)?;
                validate_union_operations_block(then_block, unions)?;
                if let Some(else_block) = else_block {
                    validate_union_operations_block(else_block, unions)?;
                }
            }
            InstructionIR::While {
                condition,
                body_block,
                span,
            } => {
                validate_union_operations_value(condition, unions, *span)?;
                validate_union_operations_block(body_block, unions)?;
            }
            InstructionIR::Falar { args, span } => {
                for arg in args {
                    validate_union_operations_value(&arg.value, unions, *span)?;
                }
            }
            InstructionIR::Break { .. }
            | InstructionIR::Continue { .. }
            | InstructionIR::InlineAsm { .. } => {}
        }
    }
    Ok(())
}

fn validate_union_operations_value(
    value: &ValueIR,
    unions: &[crate::ir::UnionTypeIR],
    span: Span,
) -> Result<(), PinkerError> {
    match value {
        ValueIR::UnionTag {
            value,
            union_type_id,
        } => {
            crate::ir::validate_union_reference(unions, *union_type_id)
                .map_err(|message| ir_validation_error(&message, span))?;
            validate_union_operations_value(value, unions, span)
        }
        ValueIR::UnionExtract {
            value,
            union_type_id,
            tag,
            resolved_member_type_id,
            canonical_member_key,
            payload_type,
            payload_layout,
        } => {
            crate::ir::validate_union_member_reference(
                unions,
                *union_type_id,
                *tag,
                canonical_member_key,
                *payload_type,
                *payload_layout,
            )
            .map_err(|message| ir_validation_error(&message, span))?;
            crate::ir::validate_union_member_identity(
                unions,
                *union_type_id,
                *tag,
                *resolved_member_type_id,
            )
            .map_err(|message| ir_validation_error(&message, span))?;
            validate_union_operations_value(value, unions, span)
        }
        ValueIR::UnionInject {
            value,
            union_type_id,
            tag,
            resolved_member_type_id,
            canonical_member_key,
            payload_type,
            payload_layout,
        } => {
            // A injeção é validada com o mesmo rigor da extração: tag, chave
            // canônica, layout e identidade resolvida têm de descrever o mesmo
            // membro. Antes desta fase a injeção só conferia a existência da
            // união, o que deixava a associação membro↔tag sem verificação.
            crate::ir::validate_union_member_reference(
                unions,
                *union_type_id,
                *tag,
                canonical_member_key,
                *payload_type,
                *payload_layout,
            )
            .map_err(|message| ir_validation_error(&message, span))?;
            crate::ir::validate_union_member_identity(
                unions,
                *union_type_id,
                *tag,
                *resolved_member_type_id,
            )
            .map_err(|message| ir_validation_error(&message, span))?;
            validate_union_operations_value(value, unions, span)
        }
        ValueIR::Unary { operand, .. } => validate_union_operations_value(operand, unions, span),
        ValueIR::Deref { ptr, .. } => validate_union_operations_value(ptr, unions, span),
        ValueIR::Binary { lhs, rhs, .. } => {
            validate_union_operations_value(lhs, unions, span)?;
            validate_union_operations_value(rhs, unions, span)
        }
        ValueIR::Call { args, .. } | ValueIR::MakeClosure { captures: args, .. } => {
            for arg in args {
                validate_union_operations_value(arg, unions, span)?;
            }
            Ok(())
        }
        ValueIR::MakeTraitObject { value, .. } => {
            validate_union_operations_value(value, unions, span)
        }
        ValueIR::TraitCall { object, args, .. } => {
            validate_union_operations_value(object, unions, span)?;
            for arg in args {
                validate_union_operations_value(arg, unions, span)?;
            }
            Ok(())
        }
        ValueIR::CallIndirect { callee, args, .. } | ValueIR::CallRaw { callee, args, .. } => {
            validate_union_operations_value(callee, unions, span)?;
            for arg in args {
                validate_union_operations_value(arg, unions, span)?;
            }
            Ok(())
        }
        ValueIR::FieldAccess { base, .. } => validate_union_operations_value(base, unions, span),
        ValueIR::Index { base, index, .. } => {
            validate_union_operations_value(base, unions, span)?;
            validate_union_operations_value(index, unions, span)
        }
        ValueIR::Cast { value, .. } => validate_union_operations_value(value, unions, span),
        ValueIR::Local(_)
        | ValueIR::GlobalConst(_)
        | ValueIR::Int(_)
        | ValueIR::Bool(_)
        | ValueIR::String(_)
        | ValueIR::FunctionRef(_)
        | ValueIR::RawFunctionRef(_) => Ok(()),
    }
}

fn infer_value_type(
    value: &ValueIR,
    slots: &HashMap<String, TypeIR>,
    consts: &HashMap<String, TypeIR>,
    funcs: &HashMap<String, FunctionSig>,
    span: Span,
) -> Result<TypeIR, PinkerError> {
    match value {
        ValueIR::Local(slot) => slots
            .get(slot)
            .cloned()
            .ok_or_else(|| ir_validation_error("uso de slot local inexistente", span)),
        ValueIR::GlobalConst(name) => consts
            .get(name)
            .cloned()
            .ok_or_else(|| ir_validation_error("constante global inexistente", span)),
        ValueIR::Int(_) => Ok(TypeIR::Bombom),
        ValueIR::Bool(_) => Ok(TypeIR::Logica),
        ValueIR::String(_) => Ok(TypeIR::Verso),
        ValueIR::Unary { op, operand, ty } => {
            let op_ty = infer_value_type(operand, slots, consts, funcs, span)?;
            let inferred = match op {
                UnaryOpIR::Neg if op_ty.is_integer() => Ok(op_ty),
                UnaryOpIR::Not if op_ty == TypeIR::Logica => Ok(TypeIR::Logica),
                UnaryOpIR::BitNot if op_ty.is_integer() => Ok(op_ty),
                UnaryOpIR::Deref => Err(ir_validation_error(
                    "deref deve usar nó dedicado na IR desta fase",
                    span,
                )),
                _ => Err(ir_validation_error(
                    "operação unária com operando inválido",
                    span,
                )),
            }?;
            if !inferred.is_compatible_with(*ty) {
                return Err(ir_validation_error(
                    "operação unária com tipo operacional inconsistente",
                    span,
                ));
            }
            Ok(*ty)
        }
        ValueIR::Deref {
            ptr,
            result_type,
            is_volatile,
        } => {
            let ptr_ty = infer_value_type(ptr, slots, consts, funcs, span)?;
            let TypeIR::Pointer {
                is_volatile: ptr_is_volatile,
            } = ptr_ty
            else {
                return Err(ir_validation_error(
                    "deref exige operando ponteiro na IR",
                    span,
                ));
            };
            if ptr_is_volatile != *is_volatile {
                return Err(ir_validation_error(
                    "deref com metadata de volatilidade inconsistente na IR",
                    span,
                ));
            }
            Ok(*result_type)
        }
        ValueIR::Binary { op, lhs, rhs, ty } => {
            let lhs_ty = infer_value_type(lhs, slots, consts, funcs, span)?;
            let rhs_ty = infer_value_type(rhs, slots, consts, funcs, span)?;
            let inferred = match op {
                BinaryOpIR::LogicalAnd | BinaryOpIR::LogicalOr => {
                    if lhs_ty == TypeIR::Logica && rhs_ty == TypeIR::Logica {
                        Ok(TypeIR::Logica)
                    } else {
                        Err(ir_validation_error("operação lógica exige logica", span))
                    }
                }
                BinaryOpIR::Add
                | BinaryOpIR::Sub
                | BinaryOpIR::Mul
                | BinaryOpIR::Div
                | BinaryOpIR::Mod
                | BinaryOpIR::BitAnd
                | BinaryOpIR::BitOr
                | BinaryOpIR::BitXor
                | BinaryOpIR::Shl
                | BinaryOpIR::Shr => {
                    let pointer_offset_ok = matches!(op, BinaryOpIR::Add | BinaryOpIR::Sub)
                        && matches!(lhs_ty, TypeIR::Pointer { .. })
                        && matches!(rhs_ty, TypeIR::Bombom);
                    if pointer_offset_ok
                        || (lhs_ty.is_compatible_with(rhs_ty) && lhs_ty.is_integer())
                    {
                        Ok(lhs_ty)
                    } else if matches!(lhs.as_ref(), ValueIR::Int(_)) && rhs_ty.is_integer() {
                        Ok(rhs_ty)
                    } else if matches!(rhs.as_ref(), ValueIR::Int(_)) && lhs_ty.is_integer() {
                        Ok(lhs_ty)
                    } else {
                        Err(ir_validation_error(
                            "operação aritmética/bitwise exige inteiro compatível",
                            span,
                        ))
                    }
                }
                BinaryOpIR::Eq | BinaryOpIR::Neq => {
                    if (lhs_ty.is_compatible_with(rhs_ty) && lhs_ty != TypeIR::Nulo)
                        || (matches!(lhs.as_ref(), ValueIR::Int(_))
                            && rhs_ty.is_integer()
                            && rhs_ty != TypeIR::Nulo)
                        || (matches!(rhs.as_ref(), ValueIR::Int(_))
                            && lhs_ty.is_integer()
                            && lhs_ty != TypeIR::Nulo)
                        || (matches!(lhs.as_ref(), ValueIR::Int(0))
                            && rhs_ty == TypeIR::FunctionPointer)
                        || (matches!(rhs.as_ref(), ValueIR::Int(0))
                            && lhs_ty == TypeIR::FunctionPointer)
                    {
                        Ok(TypeIR::Logica)
                    } else {
                        Err(ir_validation_error("comparação inválida", span))
                    }
                }
                BinaryOpIR::Lt | BinaryOpIR::Lte | BinaryOpIR::Gt | BinaryOpIR::Gte => {
                    if (lhs_ty.is_compatible_with(rhs_ty) && lhs_ty.is_integer())
                        || (matches!(lhs.as_ref(), ValueIR::Int(_)) && rhs_ty.is_integer())
                        || (matches!(rhs.as_ref(), ValueIR::Int(_)) && lhs_ty.is_integer())
                    {
                        Ok(TypeIR::Logica)
                    } else {
                        Err(ir_validation_error(
                            "comparação relacional exige inteiro compatível",
                            span,
                        ))
                    }
                }
            }?;
            let expected = if matches!(
                op,
                BinaryOpIR::LogicalAnd
                    | BinaryOpIR::LogicalOr
                    | BinaryOpIR::Eq
                    | BinaryOpIR::Neq
                    | BinaryOpIR::Lt
                    | BinaryOpIR::Lte
                    | BinaryOpIR::Gt
                    | BinaryOpIR::Gte
            ) {
                TypeIR::Logica
            } else {
                *ty
            };
            if !inferred.is_compatible_with(expected) {
                return Err(ir_validation_error(
                    "operação binária com tipo operacional inconsistente",
                    span,
                ));
            }
            Ok(inferred)
        }
        ValueIR::Call {
            callee,
            args,
            ret_type,
        } => {
            if callee == "__ternario" {
                if args.len() != 3 {
                    return Err(ir_validation_error("aridade de __ternario inválida", span));
                }
                let cond_ty = infer_value_type(&args[0], slots, consts, funcs, span)?;
                if !value_matches_expected(&args[0], cond_ty, TypeIR::Logica) {
                    return Err(ir_validation_error(
                        "condição de __ternario deve ser logica",
                        span,
                    ));
                }
                let then_ty = infer_value_type(&args[1], slots, consts, funcs, span)?;
                return Ok(then_ty);
            }
            if callee == "formatar_verso" {
                if args.len() < 2 {
                    return Err(ir_validation_error("aridade de chamada inválida", span));
                }
                let modelo_ty = infer_value_type(&args[0], slots, consts, funcs, span)?;
                if !value_matches_expected(&args[0], modelo_ty, TypeIR::Verso) {
                    return Err(ir_validation_error("tipo de argumento inválido", span));
                }
                for arg in &args[1..] {
                    let actual = infer_value_type(arg, slots, consts, funcs, span)?;
                    if !(value_matches_expected(arg, actual, TypeIR::Bombom)
                        || value_matches_expected(arg, actual, TypeIR::Verso))
                    {
                        return Err(ir_validation_error("tipo de argumento inválido", span));
                    }
                }
                if !ret_type.is_compatible_with(TypeIR::Verso) {
                    return Err(ir_validation_error(
                        "tipo de retorno anotado na call não confere",
                        span,
                    ));
                }
                return Ok(TypeIR::Verso);
            }
            if callee == "executar_com_entrada" && !(args.len() == 2 || args.len() == 3) {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            if callee == "executar_processo" && !(args.len() == 1 || args.len() == 2) {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            if callee == "capturar_stdout" && !(args.len() == 1 || args.len() == 2) {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            if callee == "capturar_stderr" && !(args.len() == 1 || args.len() == 2) {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            if callee == "afirmar" && !(args.len() == 1 || args.len() == 2) {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            let sig = funcs
                .get(callee)
                .ok_or_else(|| ir_validation_error("chamada para função inexistente", span))?;
            if callee != "executar_processo"
                && callee != "executar_com_entrada"
                && callee != "capturar_stdout"
                && callee != "capturar_stderr"
                && callee != "afirmar"
                && args.len() != sig.params.len()
            {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            for (arg, expected) in args.iter().zip(sig.params.iter()) {
                let actual = infer_value_type(arg, slots, consts, funcs, span)?;
                if !value_matches_expected(arg, actual, *expected) {
                    return Err(ir_validation_error("tipo de argumento inválido", span));
                }
            }
            if !ret_type.is_compatible_with(sig.ret_type) {
                return Err(ir_validation_error(
                    "tipo de retorno anotado na call não confere",
                    span,
                ));
            }
            Ok(sig.ret_type)
        }
        ValueIR::FunctionRef(name) => {
            funcs
                .get(name)
                .ok_or_else(|| ir_validation_error("referência a função inexistente", span))?;
            Ok(TypeIR::Function)
        }
        ValueIR::RawFunctionRef(name) => {
            funcs
                .get(name)
                .ok_or_else(|| ir_validation_error("referência crua a função inexistente", span))?;
            Ok(TypeIR::FunctionPointer)
        }
        ValueIR::MakeClosure {
            function_name,
            captures,
        } => {
            funcs
                .get(function_name)
                .ok_or_else(|| ir_validation_error("closure para função inexistente", span))?;
            for capture in captures {
                infer_value_type(capture, slots, consts, funcs, span)?;
            }
            Ok(TypeIR::Function)
        }
        ValueIR::MakeTraitObject {
            value,
            trait_name,
            concrete_type,
            concrete_type_name,
            concrete_size,
            vtable_methods,
        } => {
            if trait_name.trim().is_empty() {
                return Err(ir_validation_error(
                    "objeto de trato sem nome nominal",
                    span,
                ));
            }

            if concrete_type_name.trim().is_empty() {
                return Err(ir_validation_error(
                    "objeto de trato sem nome do tipo concreto",
                    span,
                ));
            }

            if *concrete_size == 0 {
                return Err(ir_validation_error(
                    "objeto de trato com snapshot de tamanho zero",
                    span,
                ));
            }

            if vtable_methods.is_empty() || vtable_methods.iter().any(|name| name.trim().is_empty())
            {
                return Err(ir_validation_error(
                    "objeto de trato exige vtable não vazia",
                    span,
                ));
            }

            if matches!(concrete_type, TypeIR::Nulo | TypeIR::TraitObject) {
                return Err(ir_validation_error(
                    "tipo concreto inválido em objeto de trato",
                    span,
                ));
            }

            let actual = infer_value_type(value, slots, consts, funcs, span)?;

            if !value_matches_expected(value, actual, *concrete_type) {
                return Err(ir_validation_error(
                    "valor concreto incompatível na materialização de objeto de trato",
                    span,
                ));
            }

            Ok(TypeIR::TraitObject)
        }
        ValueIR::TraitCall {
            object,
            trait_name,
            method_name,
            method_slot,
            method_count,
            args,
            param_types,
            ret_type,
        } => {
            if trait_name.trim().is_empty() || method_name.trim().is_empty() {
                return Err(ir_validation_error(
                    "chamada dinâmica sem identidade nominal completa",
                    span,
                ));
            }
            if *method_count == 0 || *method_slot >= *method_count {
                return Err(ir_validation_error(
                    "chamada dinâmica referencia slot fora da vtable",
                    span,
                ));
            }

            let object_type = infer_value_type(object, slots, consts, funcs, span)?;

            if object_type != TypeIR::TraitObject {
                return Err(ir_validation_error(
                    "chamada dinâmica exige objeto de trato",
                    span,
                ));
            }

            if args.len() != param_types.len() {
                return Err(ir_validation_error(
                    "chamada dinâmica com aridade inconsistente na IR",
                    span,
                ));
            }

            for (arg, expected) in args.iter().zip(param_types.iter()) {
                let actual = infer_value_type(arg, slots, consts, funcs, span)?;

                if !value_matches_expected(arg, actual, *expected) {
                    return Err(ir_validation_error(
                        "chamada dinâmica com tipo de argumento incompatível",
                        span,
                    ));
                }
            }

            Ok(*ret_type)
        }
        ValueIR::CallIndirect {
            callee,
            args,
            ret_type,
        } => {
            let callee_ty = infer_value_type(callee, slots, consts, funcs, span)?;
            if callee_ty != TypeIR::Function {
                return Err(ir_validation_error(
                    "chamada indireta exige valor de tipo função",
                    span,
                ));
            }
            for arg in args {
                infer_value_type(arg, slots, consts, funcs, span)?;
            }
            if *ret_type == TypeIR::Nulo {
                return Err(ir_validation_error(
                    "chamada indireta não pode ter retorno nulo",
                    span,
                ));
            }
            Ok(*ret_type)
        }
        ValueIR::CallRaw {
            callee,
            args,
            param_types,
            ret_type,
        } => {
            let callee_ty = infer_value_type(callee, slots, consts, funcs, span)?;
            if callee_ty != TypeIR::FunctionPointer {
                return Err(ir_validation_error(
                    "chamada crua exige ponteiro de função",
                    span,
                ));
            }
            if args.len() != param_types.len() {
                return Err(ir_validation_error(
                    "chamada crua com aridade inconsistente na IR",
                    span,
                ));
            }
            for (arg, expected) in args.iter().zip(param_types.iter()) {
                let actual = infer_value_type(arg, slots, consts, funcs, span)?;
                if !value_matches_expected(arg, actual, *expected) {
                    return Err(ir_validation_error(
                        "chamada crua com tipo de argumento incompatível",
                        span,
                    ));
                }
            }
            Ok(*ret_type)
        }
        ValueIR::FieldAccess {
            base,
            field: _,
            field_offset: _,
            result_type,
        } => {
            let base_ty = infer_value_type(base, slots, consts, funcs, span)?;
            if !matches!(base_ty, TypeIR::Struct) {
                return Err(ir_validation_error(
                    "acesso de campo exige base struct na IR",
                    span,
                ));
            }
            Ok(*result_type)
        }
        ValueIR::Index {
            base,
            index,
            element_type,
        } => {
            let base_ty = infer_value_type(base, slots, consts, funcs, span)?;
            let index_ty = infer_value_type(index, slots, consts, funcs, span)?;
            if !matches!(base_ty, TypeIR::FixedArray { .. }) || index_ty != TypeIR::Bombom {
                return Err(ir_validation_error(
                    "indexação inválida na IR estruturada",
                    span,
                ));
            }
            Ok(*element_type)
        }
        ValueIR::Cast { value, target_type } => {
            let source_ty = infer_value_type(value, slots, consts, funcs, span)?;
            let pointer_cast_ok = matches!(
                (source_ty, target_type),
                (TypeIR::Bombom, TypeIR::Pointer { .. })
                    | (TypeIR::Pointer { .. }, TypeIR::Bombom)
                    | (TypeIR::Pointer { .. }, TypeIR::Pointer { .. })
            );
            if (source_ty.is_integer() && target_type.is_integer()) || pointer_cast_ok {
                Ok(*target_type)
            } else {
                Err(ir_validation_error(
                    "cast IR inválido: aceita inteiro->inteiro, bombom<->seta e seta<T>->seta<U>",
                    span,
                ))
            }
        }
        ValueIR::UnionInject {
            value,
            union_type_id,
            payload_type,
            payload_layout,
            ..
        } => {
            let source_ty = infer_value_type(value, slots, consts, funcs, span)?;
            if !source_ty.is_compatible_with(*payload_type) || !payload_layout.is_well_formed() {
                return Err(ir_validation_error("injeção de união inválida na IR", span));
            }
            Ok(TypeIR::Union(*union_type_id))
        }
        ValueIR::UnionTag {
            value,
            union_type_id,
        } => {
            let source_ty = infer_value_type(value, slots, consts, funcs, span)?;
            if source_ty != TypeIR::Union(*union_type_id) {
                return Err(ir_validation_error(
                    "leitura de tag exige valor da união associada",
                    span,
                ));
            }
            Ok(TypeIR::Bombom)
        }
        ValueIR::UnionExtract {
            value,
            union_type_id,
            payload_type,
            payload_layout,
            ..
        } => {
            let source_ty = infer_value_type(value, slots, consts, funcs, span)?;
            if source_ty != TypeIR::Union(*union_type_id) || !payload_layout.is_well_formed() {
                return Err(ir_validation_error(
                    "extração de payload de união inválida na IR",
                    span,
                ));
            }
            Ok(*payload_type)
        }
    }
}

fn ir_validation_error(msg: &str, span: Span) -> PinkerError {
    PinkerError::IrValidation {
        msg: msg.to_string(),
        span,
    }
}

fn default_span() -> Span {
    Span::single(Position::new(1, 1))
}

fn is_int_literal_value(value: &ValueIR) -> bool {
    matches!(value, ValueIR::Int(_))
        || matches!(
            value,
            ValueIR::Unary {
                op: UnaryOpIR::Neg,
                operand,
                ..
            }
                if matches!(operand.as_ref(), ValueIR::Int(_))
        )
}

fn value_matches_expected(value: &ValueIR, actual: TypeIR, expected: TypeIR) -> bool {
    actual.is_compatible_with(expected)
        || (is_int_literal_value(value) && expected.is_integer())
        || (matches!(value, ValueIR::Int(_))
            && matches!(expected, TypeIR::Pointer { .. } | TypeIR::FunctionPointer))
}

fn ir_validation_error_ctx(
    function: &FunctionIR,
    block: Option<&BlockIR>,
    msg: &str,
    detail: Option<&str>,
    span: Span,
) -> PinkerError {
    let mut scoped = if let Some(detail) = detail {
        format!("{} [{}]", msg, detail)
    } else {
        msg.to_string()
    };
    if let Some(block) = block {
        scoped.push_str(&format!(
            " (função '{}', bloco '{}')",
            function.name, block.label
        ));
    } else {
        scoped.push_str(&format!(" (função '{}')", function.name));
    }
    ir_validation_error(&scoped, span)
}

// Enriquece um `IrValidation` existente com contexto de função/bloco/detalhe.
// Erros de outras variantes passam direto sem modificação.
fn enrich_ir_error(
    err: PinkerError,
    function: Option<&FunctionIR>,
    block: Option<&BlockIR>,
    detail: Option<&str>,
) -> PinkerError {
    match err {
        PinkerError::IrValidation { msg, span } => {
            if let Some(function) = function {
                ir_validation_error_ctx(function, block, &msg, detail, span)
            } else if let Some(detail) = detail {
                ir_validation_error(&format!("{} [{}]", msg, detail), span)
            } else {
                ir_validation_error(&msg, span)
            }
        }
        _ => err,
    }
}

// @pinker-nav:end ir.validacao.invariantes
