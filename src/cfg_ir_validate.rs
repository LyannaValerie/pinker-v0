//! Validador da CFG IR (blocos básicos com terminadores) do Pinker.
//!
//! Opera sobre `ProgramCfgIR` após o lowering da IR estruturada. Verifica:
//! - estrutura de cada função: bloco `entry` único, labels sem duplicata
//! - alcançabilidade de todos os blocos via BFS a partir de `entry`
//! - instruções por bloco: tipos de slots, temporários e argumentos de call
//! - terminadores: `jump`/`branch`/`return` com targets e tipos corretos
//!
//! Temporários (`%tN`) têm escopo por bloco; são criados em `Unary`,
//! `Binary` e `Call` e consultados em operandos subsequentes do mesmo bloco.
//!
//! Ponto de entrada: [`validate_program`].

use crate::cfg_ir::{InstructionCfgIR, OperandIR, ProgramCfgIR, TempIR, TerminatorIR};
use crate::error::PinkerError;
use crate::ir::{MapKeyIR, TypeIR};
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone)]
struct FunctionSigCfg {
    ret_type: TypeIR,
    params: Vec<TypeIR>,
}

// @pinker-nav:start cfg.unioes.validacao-operacoes
// @pinker-nav:domain unioes
// @pinker-nav:layer cfg
// @pinker-nav:summary Fronteira de validação das operações internas tipadas de união no CFG: cada `UnionTag` confirma a existência do `UnionTypeId` e cada `UnionExtract` é confrontado com a tabela internada — tag pertencente ao registry, chave canônica coincidente com a tag, tipo e layout do payload iguais. Nenhuma tag é recalculada; o registry é a única fonte, e nenhuma chamada comum substitui estas operações.
fn validate_union_operations(program: &ProgramCfgIR) -> Result<(), PinkerError> {
    for function in &program.functions {
        for block in &function.blocks {
            for instruction in &block.instructions {
                match instruction {
                    InstructionCfgIR::UnionTag { union_type_id, .. } => {
                        crate::ir::validate_union_reference(&program.union_types, *union_type_id)
                            .map_err(|message| cfg_error(&message, function.span))?;
                    }
                    InstructionCfgIR::UnionExtract {
                        union_type_id,
                        tag,
                        canonical_member_key,
                        payload_type,
                        payload_layout,
                        ..
                    } => {
                        crate::ir::validate_union_member_reference(
                            &program.union_types,
                            *union_type_id,
                            *tag,
                            canonical_member_key,
                            *payload_type,
                            *payload_layout,
                        )
                        .map_err(|message| cfg_error(&message, function.span))?;
                    }
                    InstructionCfgIR::UnionInject { union_type_id, .. } => {
                        crate::ir::validate_union_reference(&program.union_types, *union_type_id)
                            .map_err(|message| cfg_error(&message, function.span))?;
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
// @pinker-nav:end cfg.unioes.validacao-operacoes

// @pinker-nav:start cfg.validacao.invariantes
// @pinker-nav:domain validacao
// @pinker-nav:layer cfg
// @pinker-nav:summary Valida os invariantes do CFG IR: blocos rotulados com terminadores bem formados, alvos de salto existentes, ausência de fall-through implícito e consistência de tipos entre blocos.
pub fn validate_program(program: &ProgramCfgIR) -> Result<(), PinkerError> {
    crate::ir::validate_union_registry(&program.union_types)
        .map_err(|message| cfg_error(&message, default_span()))?;
    validate_union_operations(program)?;
    let mut global_consts = HashMap::new();
    for konst in &program.consts {
        if konst.name.trim().is_empty() {
            return Err(cfg_error(
                "constante global da CFG IR sem nome",
                default_span(),
            ));
        }
        global_consts.insert(konst.name.clone(), konst.ty);
    }

    let mut sigs_usuario = HashMap::new();
    let mut sigs_intrinsecas = HashMap::new();
    for function in &program.functions {
        sigs_usuario.insert(
            function.name.clone(),
            FunctionSigCfg {
                ret_type: function.ret_type,
                params: function.params.iter().map(|p| p.ty).collect(),
            },
        );
    }
    sigs_intrinsecas.insert(
        "ouvir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "ouvir_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "ouvir_verso_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "aleatorio_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "aleatorio_proximo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::ListBombom,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "alocar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Pointer { is_volatile: false },
            params: vec![TypeIR::U64],
        },
    );
    sigs_intrinsecas.insert(
        "liberar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Pointer { is_volatile: false }],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_anexar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_obter".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_tamanho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListBombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_definir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_tirar_ultimo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListBombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::ListVerso,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_anexar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListVerso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_obter".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::ListVerso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_tamanho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::ListVerso],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_definir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListVerso, TypeIR::Bombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_tirar_ultimo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::ListVerso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_bombom_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::MapVersoBombom,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_bombom_definir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_bombom_obter".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_bombom_tem".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_bombom_tamanho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoBombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_bombom_iterador_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoBombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_verso_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::MapVersoVerso,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_verso_definir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_verso_obter".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_verso_tem".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_verso_tamanho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoVerso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_verso_remover".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoVerso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapVersoVerso],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_bombom_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::MapBombomBombom,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_bombom_definir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_bombom_obter".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_bombom_tem".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_bombom_tamanho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomBombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_bombom_remover".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomBombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomBombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_verso_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::MapBombomVerso,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_verso_definir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_verso_obter".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_verso_tem".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_verso_tamanho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomVerso],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_bombom_verso_remover".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapBombomVerso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::MapBombomVerso],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_criar_0".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_anexar_b".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_anexar_v".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_tag".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_carga_b".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_carga_v".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    // D1: cargas de lista, mesmo caminho de uma palavra com a categoria
    // operacional preservada.
    sigs_intrinsecas.insert(
        crate::enum_payload::ANEXAR_LISTA_BOMBOM.to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::ListBombom],
        },
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::ANEXAR_LISTA_VERSO.to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::ListVerso],
        },
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::CARGA_LISTA_BOMBOM.to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::ListBombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::CARGA_LISTA_VERSO.to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::ListVerso,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::ANEXAR_SAIDA_PROCESSO.to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::OpaqueWordHandle],
        },
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::CARGA_SAIDA_PROCESSO.to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::OpaqueWordHandle,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    // Não há assinatura chamável de união: tag e extração são instruções CFG
    // tipadas (`UnionTag`/`UnionExtract`), nunca `Call`.
    sigs_intrinsecas.insert(
        "argumento".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "argumento_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "tem_chave".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "tem_argumento_nomeado".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "pedir_argumento".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "argumento_nomeado_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "tem_flag".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "ambiente_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "buscar_contexto".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "argumento_nomeado_ou_ambiente_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "caminho_existe".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "e_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "e_diretorio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "juntar_caminho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "tamanho_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "e_vazio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "criar_diretorio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "remover_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "remover_diretorio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "diretorio_atual".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "quantos_argumentos".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "tem_argumento".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "sair".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "abrir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "ler_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "ler_verso_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "ler_arquivo_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    // Parte B: as superfícies falíveis devolvem um leque com carga, que na IR é
    // o handle de uma palavra — logo `bombom`, como qualquer outro leque.
    for superficie in crate::falha_operacional::SUPERFICIES_FALIVEIS {
        sigs_intrinsecas.insert(
            superficie.intrinseca.to_string(),
            FunctionSigCfg {
                ret_type: TypeIR::Bombom,
                params: superficie
                    .argumentos
                    .iter()
                    .map(|tipo| tipo.representacao_ir())
                    .collect(),
            },
        );
    }
    for nome in crate::valor_json::ACESSORES {
        let (ret_type, params) = crate::valor_json::assinatura_ir(nome)
            .expect("acessor JSON sem assinatura na autoridade");
        sigs_intrinsecas.insert(nome.to_string(), FunctionSigCfg { ret_type, params });
    }
    for nome in crate::sha256::ACESSORES {
        let (ret_type, params) = crate::sha256::assinatura_ir(nome)
            .expect("acessor SHA-256 sem assinatura na autoridade");
        sigs_intrinsecas.insert(nome.to_string(), FunctionSigCfg { ret_type, params });
    }
    for (nome, retorno) in [
        (crate::saida_processo::ACESSOR_CODIGO, TypeIR::Bombom),
        (crate::saida_processo::ACESSOR_SAIDA, TypeIR::Verso),
        (crate::saida_processo::ACESSOR_ERRO, TypeIR::Verso),
    ] {
        sigs_intrinsecas.insert(
            nome.to_string(),
            FunctionSigCfg {
                ret_type: retorno,
                params: vec![TypeIR::OpaqueWordHandle],
            },
        );
    }
    sigs_intrinsecas.insert(
        "arquivo_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "fechar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "criar_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "abrir_anexo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "escrever".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "escrever_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "truncar_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "anexar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "juntar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "tamanho_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "indice_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "fatiar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "contem_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "comeca_com".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "termina_com".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "igual_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "vazio_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "aparar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "minusculo_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "maiusculo_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "indice_verso_em".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 140
    sigs_intrinsecas.insert(
        "buscar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "nao_vazio_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    // Fase 137
    sigs_intrinsecas.insert(
        "dividir_verso_em".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "dividir_verso_contar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 138
    sigs_intrinsecas.insert(
        "substituir_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 139
    sigs_intrinsecas.insert(
        "juntar_verso_com".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "formatar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    // Fase 158
    sigs_intrinsecas.insert(
        "ler_linha_csv_bombom".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::ListBombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "emitir_linha_csv_bombom".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::ListBombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "ler_json_plano_bombom".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::MapVersoBombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "emitir_json_plano_bombom".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::MapVersoBombom],
        },
    );
    // Fase 160
    sigs_intrinsecas.insert(
        "tempo_unix".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    sigs_intrinsecas.insert(
        "formatar_tempo_unix".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    // Fase 161
    sigs_intrinsecas.insert(
        "executar_processo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 165
    sigs_intrinsecas.insert(
        "executar_com_entrada".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 166
    sigs_intrinsecas.insert(
        "pipeline_minimo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    // Fase 163
    sigs_intrinsecas.insert(
        "capturar_stdout".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    // Fase 164
    sigs_intrinsecas.insert(
        "capturar_stderr".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "afirmar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Logica],
        },
    );
    sigs_intrinsecas.insert(
        "dormir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "copiar_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "renomear_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "verso_para_bombom".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "bombom_para_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "aleatorio_entre".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "mapa_verso_bombom_remover".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::MapVersoBombom, TypeIR::Verso],
        },
    );
    sigs_intrinsecas.insert(
        "lista_bombom_inserir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListBombom, TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    sigs_intrinsecas.insert(
        "lista_verso_inserir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::ListVerso, TypeIR::Bombom, TypeIR::Verso],
        },
    );

    // #532: a escolha entre a assinatura da intrínseca e a da função homônima
    // do usuário é feita pela identidade do callee, não pela chave textual.
    let function_sigs = crate::intrinsic_authority::TabelaPorIdentidade {
        usuario: sigs_usuario,
        intrinsecas: sigs_intrinsecas,
    };
    for function in &program.functions {
        validate_function(function, &global_consts, &function_sigs)?;
    }

    Ok(())
}

fn validate_function(
    function: &crate::cfg_ir::FunctionCfgIR,
    global_consts: &HashMap<String, TypeIR>,
    function_sigs: &crate::intrinsic_authority::TabelaPorIdentidade<FunctionSigCfg>,
) -> Result<(), PinkerError> {
    if function.blocks.is_empty() {
        return Err(cfg_error_ctx(
            function,
            None,
            &format!("função '{}' sem blocos na CFG IR", function.name),
            None,
            function.span,
        ));
    }

    if function.entry != "entry" {
        return Err(cfg_error_ctx(
            function,
            None,
            &format!(
                "função '{}' deve usar label de entrada 'entry'",
                function.name
            ),
            None,
            function.span,
        ));
    }

    let mut labels = HashSet::new();
    let mut entry_count = 0usize;
    for block in &function.blocks {
        if block.label.trim().is_empty() {
            return Err(cfg_error_ctx(
                function,
                None,
                &format!("função '{}' contém bloco sem label", function.name),
                None,
                function.span,
            ));
        }
        if block.label == "entry" {
            entry_count += 1;
        }
        if !labels.insert(block.label.clone()) {
            return Err(cfg_error_ctx(
                function,
                None,
                &format!(
                    "função '{}' contém label duplicado '{}'",
                    function.name, block.label
                ),
                None,
                function.span,
            ));
        }
    }

    if entry_count != 1 {
        return Err(cfg_error_ctx(
            function,
            None,
            &format!(
                "função '{}' deve ter exatamente um bloco 'entry'",
                function.name
            ),
            None,
            function.span,
        ));
    }

    let mut slot_types = HashMap::new();
    for param in &function.params {
        if param.slot.trim().is_empty() {
            return Err(cfg_error_ctx(
                function,
                None,
                &format!("função '{}' possui parâmetro com slot vazio", function.name),
                Some("item='param'"),
                function.span,
            ));
        }
        slot_types.insert(param.slot.clone(), param.ty);
    }
    for local in &function.locals {
        if local.slot.trim().is_empty() {
            return Err(cfg_error_ctx(
                function,
                None,
                &format!("função '{}' possui local com slot vazio", function.name),
                Some("item='local'"),
                function.span,
            ));
        }
        slot_types.insert(local.slot.clone(), local.ty);
    }

    validate_reachability(function, &labels)?;

    for block in &function.blocks {
        validate_block(
            block,
            function,
            &labels,
            &slot_types,
            global_consts,
            function_sigs,
        )?;
    }

    Ok(())
}

// BFS a partir de `entry` para garantir que todos os blocos declarados são
// alcançáveis. Blocos inalcançáveis são erro: a CFG IR não aceita código morto.
fn validate_reachability(
    function: &crate::cfg_ir::FunctionCfgIR,
    labels: &HashSet<String>,
) -> Result<(), PinkerError> {
    let mut queue = VecDeque::new();
    let mut seen = HashSet::new();
    queue.push_back(function.entry.clone());

    while let Some(label) = queue.pop_front() {
        if !seen.insert(label.clone()) {
            continue;
        }
        let block = function
            .blocks
            .iter()
            .find(|b| b.label == label)
            .ok_or_else(|| {
                cfg_error(
                    &format!(
                        "função '{}' referencia bloco inexistente '{}'",
                        function.name, label
                    ),
                    function.span,
                )
            })?;

        match &block.terminator {
            TerminatorIR::Jump(target) => {
                if !labels.contains(target) {
                    return Err(cfg_error(
                        &format!(
                            "jump para bloco inexistente '{}' em '{}'",
                            target, function.name
                        ),
                        function.span,
                    ));
                }
                queue.push_back(target.clone());
            }
            TerminatorIR::Branch {
                then_label,
                else_label,
                ..
            } => {
                if !labels.contains(then_label) {
                    return Err(cfg_error(
                        &format!(
                            "branch then para bloco inexistente '{}' em '{}'",
                            then_label, function.name
                        ),
                        function.span,
                    ));
                }
                if !labels.contains(else_label) {
                    return Err(cfg_error(
                        &format!(
                            "branch else para bloco inexistente '{}' em '{}'",
                            else_label, function.name
                        ),
                        function.span,
                    ));
                }
                queue.push_back(then_label.clone());
                queue.push_back(else_label.clone());
            }
            TerminatorIR::Return(_) => {}
        }
    }

    if seen.len() != function.blocks.len() {
        let unreachable = function
            .blocks
            .iter()
            .filter(|b| !seen.contains(&b.label))
            .map(|b| b.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(cfg_error(
            &format!(
                "função '{}' contém blocos inalcançáveis: {}",
                function.name, unreachable
            ),
            function.span,
        ));
    }

    Ok(())
}

// Valida instruções e o terminador de um bloco básico.
// `temp_types` cresce durante as instruções do bloco (escopo local ao bloco).
fn validate_block(
    block: &crate::cfg_ir::BasicBlockIR,
    function: &crate::cfg_ir::FunctionCfgIR,
    labels: &HashSet<String>,
    slot_types: &HashMap<String, TypeIR>,
    global_consts: &HashMap<String, TypeIR>,
    function_sigs: &crate::intrinsic_authority::TabelaPorIdentidade<FunctionSigCfg>,
) -> Result<(), PinkerError> {
    let mut temp_types: HashMap<TempIR, TypeIR> = HashMap::new();

    for inst in &block.instructions {
        match inst {
            InstructionCfgIR::Let { slot, value } | InstructionCfgIR::Assign { slot, value } => {
                let Some(expected) = slot_types.get(slot) else {
                    return Err(cfg_error(
                        &format!(
                            "uso de slot inexistente '{}' no bloco '{}'",
                            slot, block.label
                        ),
                        function.span,
                    ));
                };
                let actual = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if actual == TypeIR::Nulo {
                    return Err(cfg_error(
                        "operando nulo inválido em let/assign",
                        function.span,
                    ));
                }
                if !operand_matches_expected(value, actual, *expected) {
                    return Err(cfg_error_ctx(
                        function,
                        Some(block.label.as_str()),
                        &format!("tipo incompatível em slot '{}'", slot),
                        Some(&format!(
                            "instr='let/assign', esperado={:?}, recebido={:?}",
                            expected, actual
                        )),
                        function.span,
                    ));
                }
            }
            InstructionCfgIR::Unary {
                dest,
                op,
                operand,
                ty,
            } => {
                let operand_ty = infer_operand_type(
                    operand,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                let result = match op {
                    crate::ir::UnaryOpIR::Neg if operand_ty.is_integer() => operand_ty,
                    crate::ir::UnaryOpIR::Not if operand_ty == TypeIR::Logica => TypeIR::Logica,
                    crate::ir::UnaryOpIR::BitNot if operand_ty.is_integer() => operand_ty,
                    _ => return Err(cfg_error("operando inválido para unário", function.span)),
                };
                if *ty != result {
                    return Err(cfg_error(
                        "tipo operacional divergente em unário",
                        function.span,
                    ));
                }
                temp_types.insert(*dest, result);
            }
            InstructionCfgIR::DerefLoad {
                dest,
                ptr,
                ty,
                is_volatile,
            } => {
                let ptr_ty =
                    infer_operand_type(ptr, slot_types, &temp_types, global_consts, function.span)?;
                let ptr_is_volatile = match ptr_ty {
                    TypeIR::Pointer { is_volatile } => Some(is_volatile),
                    // HR3: um agregado — array fixo de `bombom` ou `ninho` —
                    // já é o endereço do seu storage e serve de base de acesso
                    // sem ponteiro intermediário.
                    TypeIR::FixedArray {
                        element: crate::ir::ScalarTypeIR::Bombom,
                        ..
                    }
                    | TypeIR::Struct => None,
                    _ => {
                        return Err(cfg_error(
                            "deref_load exige operando do tipo ponteiro",
                            function.span,
                        ));
                    }
                };
                if let Some(ptr_is_volatile) = ptr_is_volatile {
                    if ptr_is_volatile != *is_volatile {
                        return Err(cfg_error(
                            "deref_load com metadata de volatilidade inconsistente",
                            function.span,
                        ));
                    }
                } else if *is_volatile {
                    return Err(cfg_error(
                        "deref_load com array por valor não aceita metadata fragil nesta fase",
                        function.span,
                    ));
                }
                temp_types.insert(*dest, *ty);
            }
            InstructionCfgIR::DerefStore {
                ptr,
                value,
                ty,
                is_volatile,
            } => {
                let ptr_ty =
                    infer_operand_type(ptr, slot_types, &temp_types, global_consts, function.span)?;
                let ptr_is_volatile_opt = match ptr_ty {
                    TypeIR::Pointer {
                        is_volatile: ptr_is_volatile,
                    } => Some(ptr_is_volatile),
                    // Mesma convenção da leitura: o valor agregado é o endereço.
                    TypeIR::FixedArray {
                        element: crate::ir::ScalarTypeIR::Bombom,
                        ..
                    }
                    | TypeIR::Struct => None,
                    _ => {
                        return Err(cfg_error(
                            "deref_store exige operando do tipo ponteiro, array fixo de bombom ou 'ninho'",
                            function.span,
                        ));
                    }
                };
                if let Some(ptr_is_volatile) = ptr_is_volatile_opt {
                    if ptr_is_volatile != *is_volatile {
                        return Err(cfg_error(
                            "deref_store com metadata de volatilidade inconsistente",
                            function.span,
                        ));
                    }
                } else if *is_volatile {
                    return Err(cfg_error(
                        "deref_store com array por valor não aceita metadata fragil nesta fase",
                        function.span,
                    ));
                }
                let value_ty = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if !operand_matches_expected(value, value_ty, *ty) {
                    return Err(cfg_error(
                        "deref_store com valor incompatível com o tipo esperado",
                        function.span,
                    ));
                }
            }
            InstructionCfgIR::Cast {
                dest,
                value,
                target_type,
            } => {
                let source_ty = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if !is_cfg_cast_allowed(source_ty, *target_type) {
                    return Err(cfg_error_ctx(
                        function,
                        Some(&block.label),
                        "cast inválido na CFG IR para subset operacional desta fase",
                        Some(&format!(
                            "source='{}', target='{}'",
                            source_ty.name(),
                            target_type.name()
                        )),
                        function.span,
                    ));
                }
                temp_types.insert(*dest, *target_type);
            }
            InstructionCfgIR::UnionInject {
                dest,
                value,
                union_type_id,
                payload_type,
                payload_layout,
                ..
            } => {
                let source_ty = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if !operand_matches_expected(value, source_ty, *payload_type)
                    || !payload_layout.is_well_formed()
                {
                    return Err(cfg_error("union_inject inválido na CFG", function.span));
                }
                temp_types.insert(*dest, TypeIR::Union(*union_type_id));
            }
            InstructionCfgIR::UnionTag {
                dest,
                value,
                union_type_id,
            } => {
                let source_ty = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if source_ty != TypeIR::Union(*union_type_id) {
                    return Err(cfg_error(
                        "union_tag exige operando da união associada",
                        function.span,
                    ));
                }
                temp_types.insert(*dest, TypeIR::Bombom);
            }
            InstructionCfgIR::UnionExtract {
                dest,
                value,
                union_type_id,
                payload_type,
                payload_layout,
                ..
            } => {
                let source_ty = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if source_ty != TypeIR::Union(*union_type_id) || !payload_layout.is_well_formed() {
                    return Err(cfg_error("union_extract inválido na CFG", function.span));
                }
                temp_types.insert(*dest, *payload_type);
            }
            InstructionCfgIR::Binary {
                dest,
                op,
                lhs,
                rhs,
                ty,
            } => {
                let lhs_ty =
                    infer_operand_type(lhs, slot_types, &temp_types, global_consts, function.span)?;
                let rhs_ty =
                    infer_operand_type(rhs, slot_types, &temp_types, global_consts, function.span)?;
                let result = match op {
                    crate::ir::BinaryOpIR::LogicalAnd | crate::ir::BinaryOpIR::LogicalOr => {
                        if lhs_ty == TypeIR::Logica && rhs_ty == TypeIR::Logica {
                            TypeIR::Logica
                        } else {
                            return Err(cfg_error(
                                "operação lógica com tipos inválidos",
                                function.span,
                            ));
                        }
                    }
                    crate::ir::BinaryOpIR::Add
                    | crate::ir::BinaryOpIR::Sub
                    | crate::ir::BinaryOpIR::Mul
                    | crate::ir::BinaryOpIR::Div
                    | crate::ir::BinaryOpIR::Mod
                    | crate::ir::BinaryOpIR::BitAnd
                    | crate::ir::BinaryOpIR::BitOr
                    | crate::ir::BinaryOpIR::BitXor
                    | crate::ir::BinaryOpIR::Shl
                    | crate::ir::BinaryOpIR::Shr => {
                        let pointer_offset_ok =
                            matches!(op, crate::ir::BinaryOpIR::Add | crate::ir::BinaryOpIR::Sub)
                                && (matches!(lhs_ty, TypeIR::Pointer { .. })
                                    // HR3: um agregado é o endereço do seu
                                    // storage; deslocar dentro dele é a mesma
                                    // aritmética de ponteiro.
                                    || matches!(lhs_ty, TypeIR::Struct)
                                    || matches!(
                                        lhs_ty,
                                        TypeIR::FixedArray {
                                            element: crate::ir::ScalarTypeIR::Bombom,
                                            ..
                                        }
                                    ))
                                && matches!(rhs_ty, TypeIR::Bombom);
                        if pointer_offset_ok
                            || (lhs_ty.is_compatible_with(rhs_ty) && lhs_ty.is_integer())
                        {
                            lhs_ty
                        } else if matches!(lhs, OperandIR::Int(_)) && rhs_ty.is_integer() {
                            rhs_ty
                        } else if matches!(rhs, OperandIR::Int(_)) && lhs_ty.is_integer() {
                            lhs_ty
                        } else {
                            return Err(cfg_error(
                                "operação aritmética/bitwise com tipos inválidos",
                                function.span,
                            ));
                        }
                    }
                    crate::ir::BinaryOpIR::Eq | crate::ir::BinaryOpIR::Neq => {
                        if (lhs_ty.is_compatible_with(rhs_ty) && lhs_ty != TypeIR::Nulo)
                            || (matches!(lhs, OperandIR::Int(_))
                                && rhs_ty.is_integer()
                                && rhs_ty != TypeIR::Nulo)
                            || (matches!(rhs, OperandIR::Int(_))
                                && lhs_ty.is_integer()
                                && lhs_ty != TypeIR::Nulo)
                            || (matches!(lhs, OperandIR::Int(0))
                                && rhs_ty == TypeIR::FunctionPointer)
                            || (matches!(rhs, OperandIR::Int(0))
                                && lhs_ty == TypeIR::FunctionPointer)
                        {
                            TypeIR::Logica
                        } else {
                            return Err(cfg_error("comparação inválida", function.span));
                        }
                    }
                    crate::ir::BinaryOpIR::Lt
                    | crate::ir::BinaryOpIR::Lte
                    | crate::ir::BinaryOpIR::Gt
                    | crate::ir::BinaryOpIR::Gte => {
                        if (lhs_ty.is_compatible_with(rhs_ty) && lhs_ty.is_integer())
                            || (matches!(lhs, OperandIR::Int(_)) && rhs_ty.is_integer())
                            || (matches!(rhs, OperandIR::Int(_)) && lhs_ty.is_integer())
                        {
                            TypeIR::Logica
                        } else {
                            return Err(cfg_error(
                                "comparação relacional com tipos inválidos",
                                function.span,
                            ));
                        }
                    }
                };
                let numeric_operation_type =
                    if matches!(lhs, OperandIR::Int(_)) && rhs_ty.is_integer() {
                        rhs_ty
                    } else {
                        lhs_ty
                    };
                if numeric_operation_type.is_integer() && *ty != numeric_operation_type {
                    return Err(cfg_error(
                        "tipo operacional divergente em operação numérica",
                        function.span,
                    ));
                }
                temp_types.insert(*dest, result);
            }
            InstructionCfgIR::PointerOffset {
                dest,
                pointer,
                offset,
                pointer_type,
                element_size,
                element_align,
            } => {
                let actual_pointer = infer_operand_type(
                    pointer,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                let actual_offset = infer_operand_type(
                    offset,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if actual_pointer != *pointer_type
                    || !matches!(pointer_type, TypeIR::Pointer { .. })
                    || actual_offset != TypeIR::Bombom
                    || *element_size == 0
                    || *element_align == 0
                    || !element_align.is_power_of_two()
                    || *element_size % *element_align != 0
                {
                    return Err(cfg_error(
                        "pointer_offset tipado inválido na CFG",
                        function.span,
                    ));
                }
                temp_types.insert(*dest, *pointer_type);
            }
            InstructionCfgIR::Call {
                dest,
                callee,
                args,
                ret_type,
                identidade,
            } => {
                // #532: as relaxações por nome desta validação valem para a
                // intrínseca, não para uma função do usuário homônima.
                let builtin = identidade.dispatches_as_builtin();
                if builtin && callee == "__ternario" {
                    if args.len() != 3 {
                        return Err(cfg_error(
                            "aridade inválida em call __ternario",
                            function.span,
                        ));
                    }
                    let then_ty = infer_operand_type(
                        &args[1],
                        slot_types,
                        &temp_types,
                        global_consts,
                        function.span,
                    )?;
                    match dest {
                        Some(dest) => {
                            temp_types.insert(*dest, then_ty);
                        }
                        None => {}
                    }
                    continue;
                }
                if crate::ir::is_generic_map_intrinsic(callee) {
                    let actual = args
                        .iter()
                        .map(|arg| {
                            infer_operand_type(
                                arg,
                                slot_types,
                                &temp_types,
                                global_consts,
                                function.span,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let invalid = |message: &str| cfg_error(message, function.span);
                    let map_parts = |ty: TypeIR| match ty {
                        TypeIR::Map { key, value } => Some((key, value)),
                        _ => None,
                    };
                    let first_map = actual.first().copied().and_then(map_parts);
                    let valid = match callee.as_str() {
                        "__pinker_internal_mapa_criar_chave_bombom" => {
                            actual.is_empty()
                                && matches!(
                                    ret_type,
                                    TypeIR::Map {
                                        key: MapKeyIR::Bombom,
                                        ..
                                    }
                                )
                        }
                        "__pinker_internal_mapa_criar_chave_verso" => {
                            actual.is_empty()
                                && matches!(
                                    ret_type,
                                    TypeIR::Map {
                                        key: MapKeyIR::Verso,
                                        ..
                                    }
                                )
                        }
                        "__pinker_internal_mapa_definir" => {
                            first_map.is_some_and(|(key, value)| {
                                actual.len() == 3
                                    && actual[1] == key.type_ir()
                                    && actual[2] == value.type_ir()
                                    && *ret_type == TypeIR::Nulo
                            })
                        }
                        "__pinker_internal_mapa_obter" => first_map.is_some_and(|(key, value)| {
                            actual.len() == 2
                                && actual[1] == key.type_ir()
                                && *ret_type == value.type_ir()
                        }),
                        "__pinker_internal_mapa_tem" => first_map.is_some_and(|(key, _)| {
                            actual.len() == 2
                                && actual[1] == key.type_ir()
                                && *ret_type == TypeIR::Logica
                        }),
                        "__pinker_internal_mapa_tamanho" => {
                            actual.len() == 1 && first_map.is_some() && *ret_type == TypeIR::Bombom
                        }
                        "__pinker_internal_mapa_remover" => first_map.is_some_and(|(key, _)| {
                            actual.len() == 2
                                && actual[1] == key.type_ir()
                                && *ret_type == TypeIR::Nulo
                        }),
                        "__pinker_internal_mapa_iterador_criar" => {
                            actual.len() == 1 && first_map.is_some() && *ret_type == TypeIR::Bombom
                        }
                        "__pinker_internal_mapa_iterador_proxima_chave_bombom" => {
                            actual == [TypeIR::Bombom] && *ret_type == TypeIR::Bombom
                        }
                        "__pinker_internal_mapa_iterador_proxima_chave_verso" => {
                            actual == [TypeIR::Bombom] && *ret_type == TypeIR::Verso
                        }
                        _ => false,
                    };
                    if !valid {
                        return Err(invalid(&format!(
                            "intrínseco genérico de mapa '{}' inválido na CFG IR",
                            callee
                        )));
                    }
                    if let Some(dest) = dest {
                        temp_types.insert(*dest, *ret_type);
                    }
                    continue;
                }
                let sig = function_sigs.resolver(*identidade, callee).ok_or_else(|| {
                    cfg_error(
                        &format!("call para função inexistente '{}'", callee),
                        function.span,
                    )
                })?;
                if builtin && callee == "formatar_verso" {
                    if args.len() < 2 {
                        return Err(cfg_error(
                            "aridade inválida em call da CFG IR",
                            function.span,
                        ));
                    }
                    let modelo_ty = infer_operand_type(
                        &args[0],
                        slot_types,
                        &temp_types,
                        global_consts,
                        function.span,
                    )?;
                    if !operand_matches_expected(&args[0], modelo_ty, TypeIR::Verso) {
                        return Err(cfg_error_ctx(
                            function,
                            Some(block.label.as_str()),
                            "tipo de argumento inválido em call",
                            Some("instr='call formatar_verso', esperado=Verso"),
                            function.span,
                        ));
                    }
                    for arg in &args[1..] {
                        let actual = infer_operand_type(
                            arg,
                            slot_types,
                            &temp_types,
                            global_consts,
                            function.span,
                        )?;
                        if !(operand_matches_expected(arg, actual, TypeIR::Bombom)
                            || operand_matches_expected(arg, actual, TypeIR::Verso))
                        {
                            return Err(cfg_error_ctx(
                                function,
                                Some(block.label.as_str()),
                                "tipo de argumento inválido em call",
                                Some("instr='call formatar_verso', esperado=Bombom ou Verso"),
                                function.span,
                            ));
                        }
                    }
                    if !ret_type.is_compatible_with(TypeIR::Verso) {
                        return Err(cfg_error(
                            "ret_type anotado em call diverge da assinatura",
                            function.span,
                        ));
                    }
                    match dest {
                        Some(dest) => {
                            temp_types.insert(*dest, TypeIR::Verso);
                        }
                        None => {
                            return Err(cfg_error(
                                "call com retorno de valor exige destino temporário",
                                function.span,
                            ))
                        }
                    }
                    continue;
                }
                if sig.params.len() != args.len() {
                    if builtin
                        && (callee == "executar_processo"
                            || callee == "executar_com_entrada"
                            || callee == "capturar_stdout"
                            || callee == "capturar_stderr")
                        && ((callee == "executar_com_entrada"
                            && (args.len() == 2 || args.len() == 3))
                            || (callee != "executar_com_entrada"
                                && (args.len() == 1 || args.len() == 2)))
                    {
                        // aceita a camada 1 conservadora de argv explícito sem abrir argv geral
                    } else if builtin && callee == "afirmar" && (args.len() == 1 || args.len() == 2)
                    {
                        // aceita afirmar com 1 ou 2 argumentos
                    } else {
                        return Err(cfg_error(
                            "aridade inválida em call da CFG IR",
                            function.span,
                        ));
                    }
                }
                if builtin && callee == "executar_processo" && !(args.len() == 1 || args.len() == 2)
                {
                    return Err(cfg_error(
                        "aridade inválida em call da CFG IR",
                        function.span,
                    ));
                }
                if builtin
                    && callee == "executar_com_entrada"
                    && !(args.len() == 2 || args.len() == 3)
                {
                    return Err(cfg_error(
                        "aridade inválida em call da CFG IR",
                        function.span,
                    ));
                }
                if builtin && callee == "capturar_stdout" && !(args.len() == 1 || args.len() == 2) {
                    return Err(cfg_error(
                        "aridade inválida em call da CFG IR",
                        function.span,
                    ));
                }
                if builtin && callee == "capturar_stderr" && !(args.len() == 1 || args.len() == 2) {
                    return Err(cfg_error(
                        "aridade inválida em call da CFG IR",
                        function.span,
                    ));
                }
                if builtin && callee == "afirmar" && !(args.len() == 1 || args.len() == 2) {
                    return Err(cfg_error(
                        "aridade inválida em call da CFG IR",
                        function.span,
                    ));
                }
                for (arg, expected) in args.iter().zip(sig.params.iter()) {
                    let actual = infer_operand_type(
                        arg,
                        slot_types,
                        &temp_types,
                        global_consts,
                        function.span,
                    )?;
                    if !operand_matches_expected(arg, actual, *expected) {
                        return Err(cfg_error_ctx(
                            function,
                            Some(block.label.as_str()),
                            "tipo de argumento inválido em call",
                            Some(&format!(
                                "instr='call {}', esperado={:?}, recebido={:?}",
                                callee, expected, actual
                            )),
                            function.span,
                        ));
                    }
                }
                if !ret_type.is_compatible_with(sig.ret_type) {
                    return Err(cfg_error(
                        "ret_type anotado em call diverge da assinatura",
                        function.span,
                    ));
                }
                match (dest, ret_type) {
                    (Some(_), TypeIR::Nulo) => {
                        return Err(cfg_error(
                            "call nulo não pode definir temporário",
                            function.span,
                        ))
                    }
                    (None, TypeIR::Nulo) => {}
                    (Some(dest), ty) => {
                        temp_types.insert(*dest, *ty);
                    }
                    (None, _) => {
                        return Err(cfg_error(
                            "call com retorno de valor exige destino temporário",
                            function.span,
                        ))
                    }
                }
            }
            InstructionCfgIR::MakeTraitObject {
                dest,
                value,
                trait_name,
                concrete_type,
                concrete_type_name,
                concrete_size,
                vtable_methods,
            } => {
                if trait_name.trim().is_empty() {
                    return Err(cfg_error(
                        "make_trait_object sem nome nominal do trato",
                        function.span,
                    ));
                }

                if concrete_type_name.trim().is_empty() {
                    return Err(cfg_error(
                        "make_trait_object sem nome do tipo concreto",
                        function.span,
                    ));
                }

                if *concrete_size == 0 {
                    return Err(cfg_error(
                        "make_trait_object com snapshot de tamanho zero",
                        function.span,
                    ));
                }

                if vtable_methods.is_empty()
                    || vtable_methods.iter().any(|method| method.trim().is_empty())
                {
                    return Err(cfg_error(
                        "make_trait_object exige vtable não vazia",
                        function.span,
                    ));
                }

                if matches!(*concrete_type, TypeIR::TraitObject | TypeIR::Nulo) {
                    return Err(cfg_error(
                        "make_trait_object com tipo concreto inválido",
                        function.span,
                    ));
                }

                let actual = infer_operand_type(
                    value,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;

                if !operand_matches_expected(value, actual, *concrete_type) {
                    return Err(cfg_error(
                        "make_trait_object com valor concreto incompatível",
                        function.span,
                    ));
                }

                temp_types.insert(*dest, TypeIR::TraitObject);
            }
            InstructionCfgIR::TraitCall {
                dest,
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
                    return Err(cfg_error(
                        "trait_call sem identidade nominal completa",
                        function.span,
                    ));
                }
                if *method_count == 0 || *method_slot >= *method_count {
                    return Err(cfg_error(
                        "trait_call referencia slot fora da vtable",
                        function.span,
                    ));
                }

                let object_type = infer_operand_type(
                    object,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;

                if object_type != TypeIR::TraitObject {
                    return Err(cfg_error(
                        "trait_call exige operando de objeto de trato",
                        function.span,
                    ));
                }

                if args.len() != param_types.len() {
                    return Err(cfg_error(
                        "trait_call com aridade inconsistente",
                        function.span,
                    ));
                }

                for (arg, expected) in args.iter().zip(param_types.iter()) {
                    let actual = infer_operand_type(
                        arg,
                        slot_types,
                        &temp_types,
                        global_consts,
                        function.span,
                    )?;

                    if !operand_matches_expected(arg, actual, *expected) {
                        return Err(cfg_error(
                            "trait_call com tipo de argumento incompatível",
                            function.span,
                        ));
                    }
                }

                match (*dest, *ret_type) {
                    (Some(_), TypeIR::Nulo) => {
                        return Err(cfg_error(
                            "trait_call nulo não pode definir temporário",
                            function.span,
                        ));
                    }
                    (None, TypeIR::Nulo) => {}
                    (Some(dest), ty) => {
                        temp_types.insert(dest, ty);
                    }
                    (None, _) => {
                        return Err(cfg_error(
                            "trait_call com retorno exige temporário",
                            function.span,
                        ));
                    }
                }
            }
            InstructionCfgIR::CallIndirect {
                dest,
                callee,
                args,
                ret_type,
            } => {
                let callee_ty = infer_operand_type(
                    callee,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if !operand_matches_expected(callee, callee_ty, TypeIR::Function) {
                    return Err(cfg_error(
                        "call_indirect exige operando de tipo função",
                        function.span,
                    ));
                }
                for arg in args {
                    infer_operand_type(arg, slot_types, &temp_types, global_consts, function.span)?;
                }
                if *ret_type == TypeIR::Nulo {
                    return Err(cfg_error(
                        "call_indirect não pode ter retorno nulo",
                        function.span,
                    ));
                }
                temp_types.insert(*dest, *ret_type);
            }
            InstructionCfgIR::CallRaw {
                dest,
                callee,
                args,
                param_types,
                ret_type,
            } => {
                let callee_ty = infer_operand_type(
                    callee,
                    slot_types,
                    &temp_types,
                    global_consts,
                    function.span,
                )?;
                if !operand_matches_expected(callee, callee_ty, TypeIR::FunctionPointer) {
                    return Err(cfg_error(
                        "call_raw exige ponteiro cru de função",
                        function.span,
                    ));
                }
                if args.len() != param_types.len() {
                    return Err(cfg_error(
                        "call_raw com aridade inconsistente",
                        function.span,
                    ));
                }
                for (arg, expected) in args.iter().zip(param_types.iter()) {
                    let actual = infer_operand_type(
                        arg,
                        slot_types,
                        &temp_types,
                        global_consts,
                        function.span,
                    )?;
                    if !operand_matches_expected(arg, actual, *expected) {
                        return Err(cfg_error(
                            "call_raw com argumento incompatível",
                            function.span,
                        ));
                    }
                }
                match (dest, *ret_type) {
                    (Some(_), TypeIR::Nulo) => {
                        return Err(cfg_error(
                            "call_raw nulo não pode definir temporário",
                            function.span,
                        ));
                    }
                    (Some(dest), ty) => {
                        temp_types.insert(*dest, ty);
                    }
                    (None, TypeIR::Nulo) => {}
                    (None, _) => {
                        return Err(cfg_error(
                            "call_raw com retorno exige temporário",
                            function.span,
                        ));
                    }
                }
            }
            InstructionCfgIR::MakeClosure {
                dest,
                function_name,
                captures,
            } => {
                if !function_sigs.contem_grafia(function_name) {
                    return Err(cfg_error(
                        &format!("make_closure para função inexistente '{}'", function_name),
                        function.span,
                    ));
                }
                for capture in captures {
                    infer_operand_type(
                        capture,
                        slot_types,
                        &temp_types,
                        global_consts,
                        function.span,
                    )?;
                }
                temp_types.insert(*dest, TypeIR::Function);
            }
            InstructionCfgIR::Falar { args: _ } => {}
            InstructionCfgIR::InlineAsm {
                chunks,
                operands,
                clobbers,
                ..
            } => {
                if chunks.is_empty() || chunks.iter().any(|chunk| chunk.trim().is_empty()) {
                    return Err(cfg_error(
                        "inline_asm da CFG exige chunks não vazios",
                        function.span,
                    ));
                }
                let mut specs = Vec::new();
                for operand in operands {
                    match operand {
                        crate::cfg_ir::InlineAsmOperandCfgIR::Input {
                            name,
                            constraint,
                            value,
                            ty,
                        } => {
                            let inferred = infer_operand_type(
                                value,
                                slot_types,
                                &temp_types,
                                global_consts,
                                function.span,
                            )?;
                            if inferred != *ty {
                                return Err(cfg_error(
                                    "input inline_asm possui tipo divergente",
                                    function.span,
                                ));
                            }
                            specs.push((name.clone(), *constraint));
                        }
                        crate::cfg_ir::InlineAsmOperandCfgIR::Output {
                            name,
                            constraint,
                            slot,
                            ty,
                        } => {
                            if slot_types.get(slot) != Some(ty) {
                                return Err(cfg_error(
                                    "output inline_asm aponta para slot ausente ou de tipo divergente",
                                    function.span,
                                ));
                            }
                            specs.push((name.clone(), *constraint));
                        }
                    }
                }
                crate::inline_asm::validate_bound_operands(chunks, &specs, clobbers)
                    .map_err(|failure| cfg_error(&failure.to_string(), function.span))?;
            }
        }
    }

    match &block.terminator {
        TerminatorIR::Jump(target) => {
            if !labels.contains(target) {
                return Err(cfg_error(
                    &format!(
                        "jump para bloco inexistente '{}' em '{}'",
                        target, block.label
                    ),
                    function.span,
                ));
            }
        }
        TerminatorIR::Branch {
            cond,
            then_label,
            else_label,
        } => {
            let cond_ty =
                infer_operand_type(cond, slot_types, &temp_types, global_consts, function.span)?;
            if cond_ty != TypeIR::Logica {
                return Err(cfg_error_ctx(
                    function,
                    Some(block.label.as_str()),
                    &format!("branch em '{}' exige condição lógica", block.label),
                    Some(&format!("term='branch', recebido={:?}", cond_ty)),
                    function.span,
                ));
            }
            if !labels.contains(then_label) {
                return Err(cfg_error(
                    &format!("branch then para label inexistente '{}'", then_label),
                    function.span,
                ));
            }
            if !labels.contains(else_label) {
                return Err(cfg_error(
                    &format!("branch else para label inexistente '{}'", else_label),
                    function.span,
                ));
            }
        }
        TerminatorIR::Return(value) => match (function.ret_type, value) {
            (TypeIR::Nulo, Some(_)) => {
                return Err(cfg_error(
                    "return com valor em função nulo (CFG IR)",
                    function.span,
                ))
            }
            (TypeIR::Nulo, None) => {}
            (_, None) => {
                return Err(cfg_error(
                    "return sem valor em função com retorno (CFG IR)",
                    function.span,
                ))
            }
            (expected, Some(v)) => {
                let actual =
                    infer_operand_type(v, slot_types, &temp_types, global_consts, function.span)?;
                if actual == TypeIR::Nulo {
                    return Err(cfg_error(
                        "return com operando nulo inválido",
                        function.span,
                    ));
                }
                if !operand_matches_expected(v, actual, expected) {
                    return Err(cfg_error(
                        "tipo de return inválido na CFG IR",
                        function.span,
                    ));
                }
            }
        },
    }

    Ok(())
}

fn is_cfg_cast_allowed(source: TypeIR, target: TypeIR) -> bool {
    if source.is_integer() && target.is_integer() {
        return true;
    }
    matches!(
        (source, target),
        (TypeIR::Bombom, TypeIR::Pointer { .. })
            | (TypeIR::Pointer { .. }, TypeIR::Bombom)
            | (TypeIR::Pointer { .. }, TypeIR::Pointer { .. })
    )
}

fn infer_operand_type(
    operand: &OperandIR,
    slots: &HashMap<String, TypeIR>,
    temps: &HashMap<TempIR, TypeIR>,
    globals: &HashMap<String, TypeIR>,
    span: Span,
) -> Result<TypeIR, PinkerError> {
    match operand {
        OperandIR::Local(slot) => slots
            .get(slot)
            .copied()
            .ok_or_else(|| cfg_error(&format!("slot inexistente '{}'", slot), span)),
        OperandIR::GlobalConst(name) => globals
            .get(name)
            .copied()
            .ok_or_else(|| cfg_error(&format!("constante global inexistente '{}'", name), span)),
        OperandIR::Int(_) => Ok(TypeIR::Bombom),
        OperandIR::Bool(_) => Ok(TypeIR::Logica),
        OperandIR::Str(_) => Ok(TypeIR::Verso),
        OperandIR::Temp(temp) => temps
            .get(temp)
            .copied()
            .ok_or_else(|| cfg_error(&format!("temporário não definido '%t{}'", temp.0), span)),
        OperandIR::FunctionRef(_) => Ok(TypeIR::Function),
        OperandIR::RawFunctionRef(_) => Ok(TypeIR::FunctionPointer),
    }
}

fn cfg_error(msg: &str, span: Span) -> PinkerError {
    PinkerError::CfgIrValidation {
        msg: msg.to_string(),
        span,
    }
}

fn cfg_error_ctx(
    function: &crate::cfg_ir::FunctionCfgIR,
    block: Option<&str>,
    msg: &str,
    detail: Option<&str>,
    span: Span,
) -> PinkerError {
    let prefix = if let Some(detail) = detail {
        format!("{} [{}]", msg, detail)
    } else {
        msg.to_string()
    };
    let scoped = if let Some(block) = block {
        format!("{} (função '{}', bloco '{}')", prefix, function.name, block)
    } else {
        format!("{} (função '{}')", prefix, function.name)
    };
    cfg_error(&scoped, span)
}

fn operand_matches_expected(operand: &OperandIR, actual: TypeIR, expected: TypeIR) -> bool {
    actual.is_compatible_with(expected)
        || (matches!(operand, OperandIR::Int(_)) && expected.is_integer())
        || (matches!(operand, OperandIR::Int(_))
            && matches!(expected, TypeIR::Pointer { .. } | TypeIR::FunctionPointer))
        || (actual.is_integer() && expected.is_integer())
}

fn default_span() -> Span {
    Span::single(Position::new(1, 1))
}

// @pinker-nav:end cfg.validacao.invariantes
