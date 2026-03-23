use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet};

pub fn validate_program(program: &SelectedProgram) -> Result<(), PinkerError> {
    let mut globals = HashSet::new();
    for g in &program.globals {
        globals.insert(g.name.clone());
    }

    let mut sigs = HashMap::new();
    for f in &program.functions {
        sigs.insert(f.name.clone(), f.ret_type);
    }
    sigs.insert("ouvir".to_string(), TypeIR::Bombom);
    sigs.insert("abrir".to_string(), TypeIR::Bombom);
    sigs.insert("ler_arquivo".to_string(), TypeIR::Bombom);
    sigs.insert("fechar".to_string(), TypeIR::Nulo);
    sigs.insert("escrever".to_string(), TypeIR::Nulo);
    sigs.insert("juntar_verso".to_string(), TypeIR::Verso);
    sigs.insert("tamanho_verso".to_string(), TypeIR::Bombom);
    sigs.insert("indice_verso".to_string(), TypeIR::Verso);

    for f in &program.functions {
        if f.blocks.is_empty() {
            return Err(err("selected function sem blocos"));
        }
        let mut labels = HashSet::new();
        let mut entry_count = 0usize;
        for b in &f.blocks {
            if !labels.insert(b.label.clone()) {
                return Err(err("selected label duplicado"));
            }
            if b.label == "entry" {
                entry_count += 1;
            }
        }
        if entry_count != 1 {
            return Err(err("selected function deve conter entry único"));
        }

        let mut slots = HashSet::new();
        for p in &f.params {
            slots.insert(p.clone());
        }
        for l in &f.locals {
            slots.insert(l.clone());
        }

        for b in &f.blocks {
            let mut temps = HashSet::new();
            for i in &b.instructions {
                match i {
                    SelectedInstr::Mov { dest, src } => {
                        if !slots.contains(dest) {
                            return Err(err("selected mov para slot inexistente"));
                        }
                        check_operand(src, &slots, &temps, &globals)?;
                    }
                    SelectedInstr::Neg { dest, operand } | SelectedInstr::Not { dest, operand } => {
                        check_operand(operand, &slots, &temps, &globals)?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::DerefLoad { dest, ptr, ty, .. } => {
                        check_operand(ptr, &slots, &temps, &globals)?;
                        if *ty == TypeIR::Nulo {
                            return Err(err("selected deref_load não pode retornar nulo"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::DerefStore { ptr, value, ty, .. } => {
                        check_operand(ptr, &slots, &temps, &globals)?;
                        check_operand(value, &slots, &temps, &globals)?;
                        if *ty == TypeIR::Nulo {
                            return Err(err("selected deref_store não pode receber nulo"));
                        }
                    }
                    SelectedInstr::Cast {
                        dest,
                        value,
                        target_type,
                    } => {
                        check_operand(value, &slots, &temps, &globals)?;
                        if *target_type == TypeIR::Nulo {
                            return Err(err("selected cast não pode ter alvo nulo"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::Add { dest, lhs, rhs }
                    | SelectedInstr::BitAnd { dest, lhs, rhs }
                    | SelectedInstr::BitOr { dest, lhs, rhs }
                    | SelectedInstr::BitXor { dest, lhs, rhs }
                    | SelectedInstr::Shl { dest, lhs, rhs }
                    | SelectedInstr::Shr { dest, lhs, rhs }
                    | SelectedInstr::Sub { dest, lhs, rhs }
                    | SelectedInstr::Mul { dest, lhs, rhs }
                    | SelectedInstr::Div { dest, lhs, rhs }
                    | SelectedInstr::Mod { dest, lhs, rhs }
                    | SelectedInstr::CmpEq { dest, lhs, rhs }
                    | SelectedInstr::CmpNe { dest, lhs, rhs }
                    | SelectedInstr::CmpLt { dest, lhs, rhs }
                    | SelectedInstr::CmpLe { dest, lhs, rhs }
                    | SelectedInstr::CmpGt { dest, lhs, rhs }
                    | SelectedInstr::CmpGe { dest, lhs, rhs } => {
                        check_operand(lhs, &slots, &temps, &globals)?;
                        check_operand(rhs, &slots, &temps, &globals)?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::Call {
                        dest,
                        callee,
                        args,
                        ret_type,
                    } => {
                        for a in args {
                            check_operand(a, &slots, &temps, &globals)?;
                        }
                        let Some(sig) = sigs.get(callee) else {
                            return Err(err("selected call para função inexistente"));
                        };
                        if !sig.is_compatible_with(*ret_type) {
                            return Err(err("selected call com ret_type inválido"));
                        }
                        if *ret_type == TypeIR::Nulo {
                            return Err(err("selected call nulo não pode ter destino"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::CallVoid { callee, args } => {
                        for a in args {
                            check_operand(a, &slots, &temps, &globals)?;
                        }
                        let Some(sig) = sigs.get(callee) else {
                            return Err(err("selected call_void para função inexistente"));
                        };
                        if !sig.is_compatible_with(TypeIR::Nulo) {
                            return Err(err("selected call_void exige função nulo"));
                        }
                    }
                    SelectedInstr::Falar { value: _, ty: _ } => {}
                }
            }

            match &b.terminator {
                SelectedTerminator::Jmp(t) => {
                    if !labels.contains(t) {
                        return Err(err("selected jmp para label inexistente"));
                    }
                }
                SelectedTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => {
                    check_operand(cond, &slots, &temps, &globals)?;
                    if !labels.contains(then_label) || !labels.contains(else_label) {
                        return Err(err("selected br para label inexistente"));
                    }
                }
                SelectedTerminator::Ret(v) => match (f.ret_type, v) {
                    (TypeIR::Nulo, Some(_)) => {
                        return Err(err("selected ret com valor em função nulo"));
                    }
                    (TypeIR::Nulo, None) => {}
                    (_, None) => return Err(err("selected ret vazio em função com retorno")),
                    (_, Some(v)) => {
                        check_operand(v, &slots, &temps, &globals)?;
                    }
                },
            }
        }
    }

    Ok(())
}

fn check_operand(
    op: &OperandIR,
    slots: &HashSet<String>,
    temps: &HashSet<crate::cfg_ir::TempIR>,
    globals: &HashSet<String>,
) -> Result<(), PinkerError> {
    match op {
        OperandIR::Local(s) if !slots.contains(s) => Err(err("selected operand local inexistente")),
        OperandIR::GlobalConst(g) if !globals.contains(g) => {
            Err(err("selected operand global inexistente"))
        }
        OperandIR::Temp(t) if !temps.contains(t) => Err(err("selected operand temp inexistente")),
        _ => Ok(()),
    }
}

fn err(msg: &str) -> PinkerError {
    PinkerError::InstrSelectValidation {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}
