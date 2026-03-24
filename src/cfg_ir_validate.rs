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
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone)]
struct FunctionSigCfg {
    ret_type: TypeIR,
    params: Vec<TypeIR>,
}

pub fn validate_program(program: &ProgramCfgIR) -> Result<(), PinkerError> {
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

    let mut function_sigs = HashMap::new();
    for function in &program.functions {
        function_sigs.insert(
            function.name.clone(),
            FunctionSigCfg {
                ret_type: function.ret_type,
                params: function.params.iter().map(|p| p.ty).collect(),
            },
        );
    }
    function_sigs.insert(
        "ouvir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    function_sigs.insert(
        "ouvir_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![],
        },
    );
    function_sigs.insert(
        "ouvir_verso_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "argumento".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "argumento_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "ambiente_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "caminho_existe".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "e_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "e_diretorio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "juntar_caminho".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "tamanho_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "e_vazio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "criar_diretorio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "remover_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "remover_diretorio".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "diretorio_atual".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![],
        },
    );
    function_sigs.insert(
        "quantos_argumentos".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![],
        },
    );
    function_sigs.insert(
        "tem_argumento".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "sair".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "abrir".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "ler_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "ler_verso_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "ler_arquivo_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "arquivo_ou".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "fechar".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "criar_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "abrir_anexo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "escrever".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "escrever_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "truncar_arquivo".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "anexar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Nulo,
            params: vec![TypeIR::Bombom, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "juntar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "tamanho_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "indice_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso, TypeIR::Bombom],
        },
    );
    function_sigs.insert(
        "contem_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "comeca_com".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "termina_com".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "igual_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "vazio_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "aparar_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "minusculo_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "maiusculo_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Verso,
            params: vec![TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "indice_verso_em".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Bombom,
            params: vec![TypeIR::Verso, TypeIR::Verso],
        },
    );
    function_sigs.insert(
        "nao_vazio_verso".to_string(),
        FunctionSigCfg {
            ret_type: TypeIR::Logica,
            params: vec![TypeIR::Verso],
        },
    );

    for function in &program.functions {
        validate_function(function, &global_consts, &function_sigs)?;
    }

    Ok(())
}

fn validate_function(
    function: &crate::cfg_ir::FunctionCfgIR,
    global_consts: &HashMap<String, TypeIR>,
    function_sigs: &HashMap<String, FunctionSigCfg>,
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
    function_sigs: &HashMap<String, FunctionSigCfg>,
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
            InstructionCfgIR::Unary { dest, op, operand } => {
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
                let TypeIR::Pointer {
                    is_volatile: ptr_is_volatile,
                } = ptr_ty
                else {
                    return Err(cfg_error(
                        "deref_load exige operando do tipo ponteiro",
                        function.span,
                    ));
                };
                if ptr_is_volatile != *is_volatile {
                    return Err(cfg_error(
                        "deref_load com metadata de volatilidade inconsistente",
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
                let TypeIR::Pointer {
                    is_volatile: ptr_is_volatile,
                } = ptr_ty
                else {
                    return Err(cfg_error(
                        "deref_store exige operando do tipo ponteiro",
                        function.span,
                    ));
                };
                if ptr_is_volatile != *is_volatile {
                    return Err(cfg_error(
                        "deref_store com metadata de volatilidade inconsistente",
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
            InstructionCfgIR::Binary { dest, op, lhs, rhs } => {
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
                                && matches!(lhs_ty, TypeIR::Pointer { .. })
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
                temp_types.insert(*dest, result);
            }
            InstructionCfgIR::Call {
                dest,
                callee,
                args,
                ret_type,
            } => {
                let sig = function_sigs.get(callee).ok_or_else(|| {
                    cfg_error(
                        &format!("call para função inexistente '{}'", callee),
                        function.span,
                    )
                })?;
                if sig.params.len() != args.len() {
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
            InstructionCfgIR::Falar { args: _ } => {}
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
        (TypeIR::Bombom, TypeIR::Pointer { .. }) | (TypeIR::Pointer { .. }, TypeIR::Bombom)
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
        || (matches!(operand, OperandIR::Int(_)) && matches!(expected, TypeIR::Pointer { .. }))
}

fn default_span() -> Span {
    Span::single(Position::new(1, 1))
}
