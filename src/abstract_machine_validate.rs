use crate::abstract_machine::{MachineInstr, MachineProgram, MachineTerminator};
use crate::error::PinkerError;
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet};

pub fn validate_program(program: &MachineProgram) -> Result<(), PinkerError> {
    let mut globals = HashSet::new();
    for g in &program.globals {
        if g.name.trim().is_empty() {
            return Err(err("global da máquina com nome vazio"));
        }
        globals.insert(g.name.clone());
    }

    let mut sigs = HashMap::new();
    for f in &program.functions {
        sigs.insert(f.name.clone(), f.ret_type);
    }

    for f in &program.functions {
        if f.blocks.is_empty() {
            return Err(err("função da máquina sem blocos"));
        }
        let mut labels = HashSet::new();
        let mut entry_count = 0usize;
        for b in &f.blocks {
            if !labels.insert(b.label.clone()) {
                return Err(err("label duplicado na máquina"));
            }
            if b.label == "entry" {
                entry_count += 1;
            }
        }
        if entry_count != 1 {
            return Err(err("função da máquina deve ter entry único"));
        }

        let mut known_slots = HashSet::new();
        for p in &f.params {
            known_slots.insert(p.clone());
        }
        for l in &f.locals {
            known_slots.insert(l.clone());
        }

        for b in &f.blocks {
            for i in &b.code {
                match i {
                    MachineInstr::LoadSlot(s) => {
                        if !known_slots.contains(s) {
                            return Err(err("load_slot para slot inexistente"));
                        }
                    }
                    MachineInstr::StoreSlot(s) => {
                        if !(known_slots.contains(s) || is_temp_slot(s)) {
                            return Err(err("store_slot para slot inválido"));
                        }
                        known_slots.insert(s.clone());
                    }
                    MachineInstr::LoadGlobal(g) => {
                        if !globals.contains(g) {
                            return Err(err("load_global para símbolo inexistente"));
                        }
                    }
                    MachineInstr::Call { callee, .. } => {
                        if let Some(ret) = sigs.get(callee) {
                            if *ret == TypeIR::Nulo {
                                return Err(err("call com retorno para função nulo"));
                            }
                        } else {
                            return Err(err("call para função inexistente"));
                        }
                    }
                    MachineInstr::CallVoid { callee, .. } => {
                        if let Some(ret) = sigs.get(callee) {
                            if *ret != TypeIR::Nulo {
                                return Err(err("call_void para função com retorno"));
                            }
                        } else {
                            return Err(err("call_void para função inexistente"));
                        }
                    }
                    _ => {}
                }
            }

            match &b.terminator {
                MachineTerminator::Jmp(label) => {
                    if !labels.contains(label) {
                        return Err(err("jmp para label inexistente"));
                    }
                }
                MachineTerminator::BrTrue {
                    then_label,
                    else_label,
                } => {
                    if !labels.contains(then_label) || !labels.contains(else_label) {
                        return Err(err("br_true para label inexistente"));
                    }
                }
                MachineTerminator::Ret => {
                    if f.ret_type == TypeIR::Nulo {
                        return Err(err("ret com valor em função nulo"));
                    }
                }
                MachineTerminator::RetVoid => {
                    if f.ret_type != TypeIR::Nulo {
                        return Err(err("ret_void em função com retorno"));
                    }
                }
            }
        }
    }

    Ok(())
}

fn is_temp_slot(slot: &str) -> bool {
    let Some(suffix) = slot.strip_prefix("%t") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
}

fn err(msg: &str) -> PinkerError {
    PinkerError::AbstractMachineValidation {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}
