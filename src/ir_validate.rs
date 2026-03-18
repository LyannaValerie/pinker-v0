//! Validador da IR estruturada (alto nível) do Pinker.
//!
//! Opera sobre `ProgramIR` antes do lowering para CFG IR. Verifica:
//! - constantes globais: nome, tipo e valor não nulo
//! - funções: bloco de entrada `entry`, slots únicos por parâmetro/local
//! - blocos: instruções `let`/`assign`/`return`/`if` com tipos compatíveis
//! - expressões: inferência recursiva de tipo via [`infer_value_type`]
//!
//! Ponto de entrada: [`validate_program`].

use crate::error::PinkerError;
use crate::ir::{
    BinaryOpIR, BlockIR, FunctionIR, InstructionIR, ProgramIR, TypeIR, UnaryOpIR, ValueIR,
};
use crate::token::Span;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct FunctionSig {
    ret_type: TypeIR,
    params: Vec<TypeIR>,
}

pub fn validate_program(program: &ProgramIR) -> Result<(), PinkerError> {
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

    for konst in &program.consts {
        let ty = infer_value_type(&konst.value, &HashMap::new(), &consts, &funcs, konst.span)
            .map_err(|err| enrich_ir_error(err, None, None, Some("item='const'")))?;
        if ty != konst.ty {
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
                if actual_ty != *expected_ty {
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
            InstructionIR::Expr { value, span } => {
                let ty = infer_value_type(value, slots, consts, funcs, *span).map_err(|err| {
                    enrich_ir_error(err, Some(function), Some(block), Some("instr='expr'"))
                })?;
                if ty == TypeIR::Nulo {
                    match value {
                        ValueIR::Call { .. } => {}
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
                    if ty != expected {
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
        }
    }

    Ok(())
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
            .copied()
            .ok_or_else(|| ir_validation_error("uso de slot local inexistente", span)),
        ValueIR::GlobalConst(name) => consts
            .get(name)
            .copied()
            .ok_or_else(|| ir_validation_error("constante global inexistente", span)),
        ValueIR::Int(_) => Ok(TypeIR::Bombom),
        ValueIR::Bool(_) => Ok(TypeIR::Logica),
        ValueIR::Unary { op, operand } => {
            let op_ty = infer_value_type(operand, slots, consts, funcs, span)?;
            match op {
                UnaryOpIR::Neg if op_ty == TypeIR::Bombom => Ok(TypeIR::Bombom),
                UnaryOpIR::Not if op_ty == TypeIR::Logica => Ok(TypeIR::Logica),
                _ => Err(ir_validation_error(
                    "operação unária com operando inválido",
                    span,
                )),
            }
        }
        ValueIR::Binary { op, lhs, rhs } => {
            let lhs_ty = infer_value_type(lhs, slots, consts, funcs, span)?;
            let rhs_ty = infer_value_type(rhs, slots, consts, funcs, span)?;
            match op {
                BinaryOpIR::Add | BinaryOpIR::Sub | BinaryOpIR::Mul | BinaryOpIR::Div => {
                    if lhs_ty == TypeIR::Bombom && rhs_ty == TypeIR::Bombom {
                        Ok(TypeIR::Bombom)
                    } else {
                        Err(ir_validation_error(
                            "operação aritmética exige bombom",
                            span,
                        ))
                    }
                }
                BinaryOpIR::Eq | BinaryOpIR::Neq => {
                    if lhs_ty == rhs_ty && lhs_ty != TypeIR::Nulo {
                        Ok(TypeIR::Logica)
                    } else {
                        Err(ir_validation_error("comparação inválida", span))
                    }
                }
                BinaryOpIR::Lt | BinaryOpIR::Lte | BinaryOpIR::Gt | BinaryOpIR::Gte => {
                    if lhs_ty == TypeIR::Bombom && rhs_ty == TypeIR::Bombom {
                        Ok(TypeIR::Logica)
                    } else {
                        Err(ir_validation_error(
                            "comparação relacional exige bombom",
                            span,
                        ))
                    }
                }
            }
        }
        ValueIR::Call {
            callee,
            args,
            ret_type,
        } => {
            let sig = funcs
                .get(callee)
                .ok_or_else(|| ir_validation_error("chamada para função inexistente", span))?;
            if args.len() != sig.params.len() {
                return Err(ir_validation_error("aridade de chamada inválida", span));
            }
            for (arg, expected) in args.iter().zip(sig.params.iter()) {
                let actual = infer_value_type(arg, slots, consts, funcs, span)?;
                if actual != *expected {
                    return Err(ir_validation_error("tipo de argumento inválido", span));
                }
            }
            if *ret_type != sig.ret_type {
                return Err(ir_validation_error(
                    "tipo de retorno anotado na call não confere",
                    span,
                ));
            }
            Ok(sig.ret_type)
        }
    }
}

fn ir_validation_error(msg: &str, span: Span) -> PinkerError {
    PinkerError::IrValidation {
        msg: msg.to_string(),
        span,
    }
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
