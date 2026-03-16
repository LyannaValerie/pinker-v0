use crate::abstract_machine::{MachineFunction, MachineInstr, MachineProgram, MachineTerminator};
use crate::error::PinkerError;
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet, VecDeque};

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
        sigs.insert(f.name.clone(), (f.ret_type, f.params.len()));
    }

    for f in &program.functions {
        validate_function(f, &globals, &sigs)?;
    }

    Ok(())
}

fn validate_function(
    f: &MachineFunction,
    globals: &HashSet<String>,
    sigs: &HashMap<String, (TypeIR, usize)>,
) -> Result<(), PinkerError> {
    if f.blocks.is_empty() {
        return Err(err("função da máquina sem blocos"));
    }

    let mut labels = HashSet::new();
    let mut entry_count = 0usize;
    for b in &f.blocks {
        if !labels.insert(b.label.clone()) {
            return Err(err_ctx(f, Some(&b.label), "label duplicado na máquina"));
        }
        if b.label == "entry" {
            entry_count += 1;
        }
    }
    if entry_count != 1 {
        return Err(err_ctx(f, None, "função da máquina deve ter entry único"));
    }

    let mut known_named_slots = HashSet::new();
    for p in &f.params {
        known_named_slots.insert(p.clone());
    }
    for l in &f.locals {
        known_named_slots.insert(l.clone());
    }

    let mut known_temp_slots = HashSet::new();
    for b in &f.blocks {
        for i in &b.code {
            if let MachineInstr::StoreSlot(s) = i {
                if is_temp_slot(s) {
                    known_temp_slots.insert(s.clone());
                }
            }
        }
    }

    for b in &f.blocks {
        for i in &b.code {
            match i {
                MachineInstr::LoadSlot(s) => {
                    if !(known_named_slots.contains(s) || known_temp_slots.contains(s)) {
                        return Err(err_ctx(
                            f,
                            Some(&b.label),
                            "load_slot para slot inexistente",
                        ));
                    }
                }
                MachineInstr::StoreSlot(s) => {
                    if !(known_named_slots.contains(s) || is_temp_slot(s)) {
                        return Err(err_ctx(f, Some(&b.label), "store_slot para slot inválido"));
                    }
                }
                MachineInstr::LoadGlobal(g) => {
                    if !globals.contains(g) {
                        return Err(err_ctx(
                            f,
                            Some(&b.label),
                            "load_global para símbolo inexistente",
                        ));
                    }
                }
                MachineInstr::Call { callee, argc } => {
                    if let Some((ret, param_count)) = sigs.get(callee) {
                        if *ret == TypeIR::Nulo {
                            return Err(err_ctx(
                                f,
                                Some(&b.label),
                                "call com retorno para função nulo",
                            ));
                        }
                        if *argc != *param_count {
                            return Err(err_ctx(f, Some(&b.label), "call com aridade inválida"));
                        }
                    } else {
                        return Err(err_ctx(f, Some(&b.label), "call para função inexistente"));
                    }
                }
                MachineInstr::CallVoid { callee, argc } => {
                    if let Some((ret, param_count)) = sigs.get(callee) {
                        if *ret != TypeIR::Nulo {
                            return Err(err_ctx(
                                f,
                                Some(&b.label),
                                "call_void para função com retorno",
                            ));
                        }
                        if *argc != *param_count {
                            return Err(err_ctx(
                                f,
                                Some(&b.label),
                                "call_void com aridade inválida",
                            ));
                        }
                    } else {
                        return Err(err_ctx(
                            f,
                            Some(&b.label),
                            "call_void para função inexistente",
                        ));
                    }
                }
                _ => {}
            }
        }

        match &b.terminator {
            MachineTerminator::Jmp(label) => {
                if !labels.contains(label) {
                    return Err(err_ctx(f, Some(&b.label), "jmp para label inexistente"));
                }
            }
            MachineTerminator::BrTrue {
                then_label,
                else_label,
            } => {
                if !labels.contains(then_label) || !labels.contains(else_label) {
                    return Err(err_ctx(f, Some(&b.label), "br_true para label inexistente"));
                }
            }
            MachineTerminator::Ret => {
                if f.ret_type == TypeIR::Nulo {
                    return Err(err_ctx(f, Some(&b.label), "ret com valor em função nulo"));
                }
            }
            MachineTerminator::RetVoid => {
                if f.ret_type != TypeIR::Nulo {
                    return Err(err_ctx(f, Some(&b.label), "ret_void em função com retorno"));
                }
            }
        }
    }

    validate_stack_discipline(f)
}

fn validate_stack_discipline(f: &MachineFunction) -> Result<(), PinkerError> {
    let mut label_to_block = HashMap::new();
    for b in &f.blocks {
        label_to_block.insert(b.label.clone(), b);
    }

    let mut in_depth = HashMap::new();
    in_depth.insert("entry".to_string(), 0usize);
    let mut worklist = VecDeque::new();
    worklist.push_back("entry".to_string());

    while let Some(label) = worklist.pop_front() {
        let block = label_to_block.get(&label).unwrap();
        let mut depth = *in_depth.get(&label).unwrap();

        for i in &block.code {
            apply_instr_effect(f, &block.label, i, &mut depth)?;
        }

        let successors = apply_terminator_effect(f, &block.label, &block.terminator, &mut depth)?;

        for succ in successors {
            if let Some(previous) = in_depth.get(&succ) {
                if *previous != depth {
                    return Err(err_ctx(
                        f,
                        Some(&block.label),
                        "altura de pilha inconsistente entre predecessores",
                    ));
                }
            } else {
                in_depth.insert(succ.clone(), depth);
                worklist.push_back(succ);
            }
        }
    }

    Ok(())
}

fn apply_instr_effect(
    f: &MachineFunction,
    label: &str,
    i: &MachineInstr,
    depth: &mut usize,
) -> Result<(), PinkerError> {
    match i {
        MachineInstr::PushInt(_)
        | MachineInstr::PushBool(_)
        | MachineInstr::LoadSlot(_)
        | MachineInstr::LoadGlobal(_) => *depth += 1,
        MachineInstr::StoreSlot(_) => pop(f, label, depth, 1, "underflow em store_slot")?,
        MachineInstr::Neg | MachineInstr::Not => {
            pop(f, label, depth, 1, "underflow em operação unária")?;
            *depth += 1;
        }
        MachineInstr::Add
        | MachineInstr::Sub
        | MachineInstr::Mul
        | MachineInstr::Div
        | MachineInstr::CmpEq
        | MachineInstr::CmpNe
        | MachineInstr::CmpLt
        | MachineInstr::CmpLe
        | MachineInstr::CmpGt
        | MachineInstr::CmpGe => {
            pop(f, label, depth, 2, "underflow em operação binária")?;
            *depth += 1;
        }
        MachineInstr::Call { argc, .. } => {
            pop(f, label, depth, *argc, "underflow em call")?;
            *depth += 1;
        }
        MachineInstr::CallVoid { argc, .. } => {
            pop(f, label, depth, *argc, "underflow em call_void")?;
        }
    }

    Ok(())
}

fn apply_terminator_effect(
    f: &MachineFunction,
    label: &str,
    term: &MachineTerminator,
    depth: &mut usize,
) -> Result<Vec<String>, PinkerError> {
    match term {
        MachineTerminator::Jmp(target) => Ok(vec![target.clone()]),
        MachineTerminator::BrTrue {
            then_label,
            else_label,
        } => {
            pop(f, label, depth, 1, "underflow em br_true")?;
            Ok(vec![then_label.clone(), else_label.clone()])
        }
        MachineTerminator::Ret => {
            if *depth != 1 {
                return Err(err_ctx(
                    f,
                    Some(label),
                    "ret requer exatamente um valor na pilha",
                ));
            }
            *depth = 0;
            Ok(vec![])
        }
        MachineTerminator::RetVoid => {
            if *depth != 0 {
                return Err(err_ctx(f, Some(label), "ret_void requer pilha vazia"));
            }
            Ok(vec![])
        }
    }
}

fn pop(
    f: &MachineFunction,
    label: &str,
    depth: &mut usize,
    amount: usize,
    message: &str,
) -> Result<(), PinkerError> {
    if *depth < amount {
        return Err(err_ctx(f, Some(label), message));
    }
    *depth -= amount;
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

fn err_ctx(f: &MachineFunction, block: Option<&str>, msg: &str) -> PinkerError {
    let scoped = if let Some(block) = block {
        format!("{} (função '{}', bloco '{}')", msg, f.name, block)
    } else {
        format!("{} (função '{}')", msg, f.name)
    };
    err(&scoped)
}
