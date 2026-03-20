use crate::backend_text::{
    BackendTextFunction, BackendTextInstruction, BackendTextProgram, BackendTextTerminator,
};
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet};

pub fn validate_program(program: &BackendTextProgram) -> Result<(), PinkerError> {
    if program.module_name.trim().is_empty() {
        return Err(err("módulo textual com nome vazio"));
    }

    let mut globals = HashMap::new();
    for g in &program.globals {
        if g.name.trim().is_empty() {
            return Err(err("global textual com nome vazio"));
        }
        globals.insert(g.name.clone(), infer_literal_operand(&g.value)?);
    }

    let mut sigs = HashMap::new();
    for f in &program.functions {
        if !is_valid_ident(&f.name) {
            return Err(err(&format!(
                "nome de função textual inválido '{}'",
                f.name
            )));
        }
        sigs.insert(f.name.clone(), f.ret_type);
    }

    for f in &program.functions {
        validate_function(f, &globals, &sigs)?;
    }

    Ok(())
}

fn validate_function(
    function: &BackendTextFunction,
    globals: &HashMap<String, TypeIR>,
    sigs: &HashMap<String, TypeIR>,
) -> Result<(), PinkerError> {
    if function.blocks.is_empty() {
        return Err(err(&format!(
            "função textual '{}' sem blocos",
            function.name
        )));
    }

    let mut labels = HashSet::new();
    let mut entry_count = 0usize;
    for b in &function.blocks {
        if b.label.trim().is_empty() {
            return Err(err(&format!(
                "função textual '{}' com bloco sem label",
                function.name
            )));
        }
        if b.label == "entry" {
            entry_count += 1;
        }
        if !labels.insert(b.label.clone()) {
            return Err(err(&format!(
                "label duplicado '{}' em '{}'",
                b.label, function.name
            )));
        }
    }

    if entry_count != 1 {
        return Err(err(&format!(
            "função textual '{}' deve ter exatamente um bloco entry",
            function.name
        )));
    }

    let mut slots = HashMap::new();
    for p in &function.params {
        slots.insert(p.clone(), TypeIR::Bombom);
    }
    for l in &function.locals {
        slots.insert(l.clone(), TypeIR::Bombom);
    }

    for b in &function.blocks {
        let mut temps = HashMap::new();

        for inst in &b.instructions {
            match inst {
                BackendTextInstruction::Mov { dest, src } => {
                    if !slots.contains_key(dest) {
                        return Err(err(&format!(
                            "slot '{}' inexistente em '{}'",
                            dest, function.name
                        )));
                    }
                    let _ = infer_operand(src, &slots, &temps, globals)?;
                }
                BackendTextInstruction::Unary { dest, op, operand } => {
                    let op_ty = infer_operand(operand, &slots, &temps, globals)?;
                    let result = match op {
                        crate::ir::UnaryOpIR::Neg if op_ty.is_integer() => op_ty,
                        crate::ir::UnaryOpIR::Not if op_ty == TypeIR::Logica => TypeIR::Logica,
                        _ => return Err(err("unop textual com operando inválido")),
                    };
                    temps.insert(*dest, result);
                }
                BackendTextInstruction::Binary { dest, op, lhs, rhs } => {
                    let lhs_ty = infer_operand(lhs, &slots, &temps, globals)?;
                    let rhs_ty = infer_operand(rhs, &slots, &temps, globals)?;
                    let result = match op {
                        crate::ir::BinaryOpIR::LogicalAnd | crate::ir::BinaryOpIR::LogicalOr => {
                            if lhs_ty == TypeIR::Logica && rhs_ty == TypeIR::Logica {
                                TypeIR::Logica
                            } else {
                                return Err(err("binop lógica textual inválida"));
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
                            if lhs_ty.is_compatible_with(rhs_ty) && lhs_ty.is_integer() {
                                lhs_ty
                            } else if matches!(lhs, OperandIR::Int(_)) && rhs_ty.is_integer() {
                                rhs_ty
                            } else if matches!(rhs, OperandIR::Int(_)) && lhs_ty.is_integer() {
                                lhs_ty
                            } else {
                                return Err(err("binop aritmética/bitwise textual inválida"));
                            }
                        }
                        crate::ir::BinaryOpIR::Eq | crate::ir::BinaryOpIR::Neq => {
                            if lhs_ty.is_compatible_with(rhs_ty) && lhs_ty != TypeIR::Nulo {
                                TypeIR::Logica
                            } else {
                                return Err(err("binop comparação textual inválida"));
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
                                return Err(err("binop relacional textual inválida"));
                            }
                        }
                    };
                    temps.insert(*dest, result);
                }
                BackendTextInstruction::Call {
                    dest,
                    callee,
                    args,
                    ret_type,
                } => {
                    let fn_ret = sigs.get(callee).copied().ok_or_else(|| {
                        err(&format!(
                            "call para função textual inexistente '{}'",
                            callee
                        ))
                    })?;
                    for a in args {
                        let _ = infer_operand(a, &slots, &temps, globals)?;
                    }
                    if !fn_ret.is_compatible_with(*ret_type) {
                        return Err(err("ret_type textual de call incompatível"));
                    }
                    match (dest, ret_type) {
                        (Some(_), TypeIR::Nulo) => {
                            return Err(err("call_void textual não pode ter destino"));
                        }
                        (None, TypeIR::Nulo) => {}
                        (Some(t), _) => {
                            temps.insert(*t, *ret_type);
                        }
                        (None, _) => {
                            return Err(err("call textual com retorno exige destino"));
                        }
                    }
                }
                BackendTextInstruction::Falar { value: _, ty: _ } => {}
            }
        }

        match &b.terminator {
            BackendTextTerminator::Jump(target) => {
                if !labels.contains(target) {
                    return Err(err(&format!(
                        "jmp textual para label inexistente '{}'",
                        target
                    )));
                }
            }
            BackendTextTerminator::Branch {
                cond,
                then_label,
                else_label,
            } => {
                if infer_operand(cond, &slots, &temps, globals)? != TypeIR::Logica {
                    return Err(err("branch textual com condição não-lógica"));
                }
                if !labels.contains(then_label) || !labels.contains(else_label) {
                    return Err(err("branch textual para label inexistente"));
                }
            }
            BackendTextTerminator::Return(value) => match (function.ret_type, value) {
                (TypeIR::Nulo, Some(_)) => {
                    return Err(err("ret textual com valor em função nulo"));
                }
                (TypeIR::Nulo, None) => {}
                (_, None) => return Err(err("ret textual sem valor em função com retorno")),
                (expected, Some(v)) => {
                    let actual = infer_operand(v, &slots, &temps, globals)?;
                    if !actual.is_compatible_with(expected) || actual == TypeIR::Nulo {
                        return Err(err("ret textual com tipo inválido"));
                    }
                }
            },
        }
    }

    Ok(())
}

fn infer_operand(
    op: &OperandIR,
    slots: &HashMap<String, TypeIR>,
    temps: &HashMap<crate::cfg_ir::TempIR, TypeIR>,
    globals: &HashMap<String, TypeIR>,
) -> Result<TypeIR, PinkerError> {
    match op {
        OperandIR::Local(s) => slots
            .get(s)
            .copied()
            .ok_or_else(|| err(&format!("slot textual inexistente '{}'", s))),
        OperandIR::GlobalConst(g) => globals
            .get(g)
            .copied()
            .ok_or_else(|| err(&format!("global textual inexistente '{}'", g))),
        OperandIR::Int(_) => Ok(TypeIR::Bombom),
        OperandIR::Bool(_) => Ok(TypeIR::Logica),
        OperandIR::Str(_) => Ok(TypeIR::Verso),
        OperandIR::Temp(t) => temps
            .get(t)
            .copied()
            .ok_or_else(|| err(&format!("temporário textual não definido %t{}", t.0))),
    }
}

fn infer_literal_operand(op: &OperandIR) -> Result<TypeIR, PinkerError> {
    match op {
        OperandIR::Int(_) => Ok(TypeIR::Bombom),
        OperandIR::Bool(_) => Ok(TypeIR::Logica),
        OperandIR::Str(_) => Ok(TypeIR::Verso),
        OperandIR::GlobalConst(_) | OperandIR::Local(_) | OperandIR::Temp(_) => {
            Err(err("global textual com valor não-literal"))
        }
    }
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

fn err(msg: &str) -> PinkerError {
    PinkerError::BackendTextValidation {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}
