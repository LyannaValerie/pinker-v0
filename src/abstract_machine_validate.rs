//! Validador estrutural e de disciplina de pilha da Machine abstrata.
//!
//! Dois passes principais:
//! 1. **Validação estrutural** (`validate_function`): verifica slots, labels,
//!    labels duplicados, aridade de calls e consistência de ret/ret_void.
//! 2. **Disciplina de pilha** (`validate_stack_discipline`): simula a pilha
//!    via worklist (BFS sobre os blocos), propagando e mesclando tipos de
//!    stack entre predecessores. Detecta underflow, tipos incompatíveis e
//!    altura inconsistente entre caminhos.
//!
//! Ponto de entrada: [`validate_program`].

use crate::abstract_machine::{MachineFunction, MachineInstr, MachineProgram, MachineTerminator};
use crate::error::PinkerError;
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet, VecDeque};

// Tipo de valor de pilha inferido estaticamente.
// `Unknown` representa tipo não resolvido (e.g. slot sem anotação ainda);
// é compatível com qualquer tipo para não bloquear caminhos sem informação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackValueType {
    Bombom,
    Logica,
    Unknown,
}

pub fn validate_program(program: &MachineProgram) -> Result<(), PinkerError> {
    let mut globals = HashMap::new();
    for g in &program.globals {
        if g.name.trim().is_empty() {
            return Err(err("global da máquina com nome vazio"));
        }
        globals.insert(g.name.clone(), infer_operand_type(&g.value));
    }

    let mut sigs = HashMap::new();
    for f in &program.functions {
        let param_types = f
            .params
            .iter()
            .map(|p| {
                f.slot_types
                    .get(p)
                    .copied()
                    .map(type_to_stack)
                    .unwrap_or(StackValueType::Unknown)
            })
            .collect::<Vec<_>>();
        sigs.insert(f.name.clone(), (f.ret_type, param_types));
    }

    for f in &program.functions {
        validate_function(f, &globals, &sigs)?;
    }

    Ok(())
}

fn validate_function(
    f: &MachineFunction,
    globals: &HashMap<String, StackValueType>,
    sigs: &HashMap<String, (TypeIR, Vec<StackValueType>)>,
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
                    if !globals.contains_key(g) {
                        return Err(err_ctx(
                            f,
                            Some(&b.label),
                            "load_global para símbolo inexistente",
                        ));
                    }
                }
                MachineInstr::Call { callee, argc } => {
                    if let Some((ret, param_types)) = sigs.get(callee) {
                        if *ret == TypeIR::Nulo {
                            return Err(err_ctx(
                                f,
                                Some(&b.label),
                                "call com retorno para função nulo",
                            ));
                        }
                        if *argc != param_types.len() {
                            return Err(err_ctx(f, Some(&b.label), "call com aridade inválida"));
                        }
                    } else {
                        return Err(err_ctx(f, Some(&b.label), "call para função inexistente"));
                    }
                }
                MachineInstr::CallVoid { callee, argc } => {
                    if let Some((ret, param_types)) = sigs.get(callee) {
                        if *ret != TypeIR::Nulo {
                            return Err(err_ctx(
                                f,
                                Some(&b.label),
                                "call_void para função com retorno",
                            ));
                        }
                        if *argc != param_types.len() {
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

    validate_stack_discipline(f, globals, sigs)
}

// Simula a pilha de tipos para cada bloco usando um worklist (BFS).
// `in_state` guarda a altura/tipos da pilha na entrada de cada bloco.
// Quando dois predecessores geram tipos diferentes para o mesmo slot da
// pilha, o tipo resultante vira `Unknown` via `merge_stack_types`.
fn validate_stack_discipline(
    f: &MachineFunction,
    globals: &HashMap<String, StackValueType>,
    sigs: &HashMap<String, (TypeIR, Vec<StackValueType>)>,
) -> Result<(), PinkerError> {
    let mut label_to_block = HashMap::new();
    for b in &f.blocks {
        label_to_block.insert(b.label.clone(), b);
    }

    let mut in_state = HashMap::new();
    in_state.insert("entry".to_string(), Vec::<StackValueType>::new());
    let mut worklist = VecDeque::new();
    worklist.push_back("entry".to_string());

    let mut slot_types = f
        .slot_types
        .iter()
        .map(|(slot, ty)| (slot.clone(), type_to_stack(*ty)))
        .collect::<HashMap<_, _>>();

    while let Some(label) = worklist.pop_front() {
        let block = label_to_block.get(&label).unwrap();
        let mut stack = in_state.get(&label).cloned().unwrap();

        for i in &block.code {
            apply_instr_effect(
                f,
                &block.label,
                i,
                &mut stack,
                &mut slot_types,
                globals,
                sigs,
            )?;
        }

        let successors =
            apply_terminator_effect(f, &block.label, &block.terminator, &mut stack, sigs)?;

        for succ in successors {
            if let Some(previous) = in_state.get(&succ) {
                if previous.len() != stack.len() {
                    return Err(err_ctx(
                        f,
                        Some(&block.label),
                        "altura de pilha inconsistente entre predecessores",
                    ));
                }
                let merged = merge_stack_types(previous, &stack);
                if &merged != previous {
                    in_state.insert(succ.clone(), merged);
                    worklist.push_back(succ);
                }
            } else {
                in_state.insert(succ.clone(), stack.clone());
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
    stack: &mut Vec<StackValueType>,
    slot_types: &mut HashMap<String, StackValueType>,
    globals: &HashMap<String, StackValueType>,
    sigs: &HashMap<String, (TypeIR, Vec<StackValueType>)>,
) -> Result<(), PinkerError> {
    match i {
        MachineInstr::PushInt(_) => stack.push(StackValueType::Bombom),
        MachineInstr::PushBool(_) => stack.push(StackValueType::Logica),
        MachineInstr::LoadSlot(slot) => {
            stack.push(*slot_types.get(slot).unwrap_or(&StackValueType::Unknown));
        }
        MachineInstr::LoadGlobal(g) => {
            stack.push(*globals.get(g).unwrap_or(&StackValueType::Unknown));
        }
        MachineInstr::StoreSlot(slot) => {
            let top = pop_typed(
                f,
                label,
                stack,
                1,
                "underflow em store_slot",
                Some(&format!("instr='store_slot {}'", slot)),
            )?;
            if let Some(expected) = slot_types.get(slot).copied() {
                ensure_compatible(
                    f,
                    label,
                    top[0],
                    expected,
                    "store_slot com tipo incompatível",
                    Some(&format!("instr='store_slot {}', slot='{}'", slot, slot)),
                )?;
                let merged = merge_types(expected, top[0]);
                slot_types.insert(slot.clone(), merged);
            } else {
                slot_types.insert(slot.clone(), top[0]);
            }
        }
        MachineInstr::Neg | MachineInstr::Not => {
            let top = pop_typed(
                f,
                label,
                stack,
                1,
                "underflow em operação unária",
                Some(&format!("instr='{}'", instr_name(i))),
            )?;
            let expected = match i {
                MachineInstr::Neg => StackValueType::Bombom,
                MachineInstr::Not => StackValueType::Logica,
                _ => StackValueType::Unknown,
            };
            ensure_compatible(
                f,
                label,
                top[0],
                expected,
                "tipo inválido em operação unária",
                Some(&format!("instr='{}'", instr_name(i))),
            )?;
            stack.push(expected);
        }
        MachineInstr::Add
        | MachineInstr::BitAnd
        | MachineInstr::BitOr
        | MachineInstr::BitXor
        | MachineInstr::Shl
        | MachineInstr::Shr
        | MachineInstr::Sub
        | MachineInstr::Mul
        | MachineInstr::Div
        | MachineInstr::Mod
        | MachineInstr::CmpEq
        | MachineInstr::CmpNe
        | MachineInstr::CmpLt
        | MachineInstr::CmpLe
        | MachineInstr::CmpGt
        | MachineInstr::CmpGe => {
            let pair = pop_typed(
                f,
                label,
                stack,
                2,
                "underflow em operação binária",
                Some(&format!("instr='{}'", instr_name(i))),
            )?;
            let out_ty = match i {
                MachineInstr::CmpEq
                | MachineInstr::CmpNe
                | MachineInstr::CmpLt
                | MachineInstr::CmpLe
                | MachineInstr::CmpGt
                | MachineInstr::CmpGe => StackValueType::Logica,
                _ => {
                    ensure_compatible(
                        f,
                        label,
                        pair[0],
                        StackValueType::Bombom,
                        "tipo inválido em operação binária",
                        Some(&format!("instr='{}'", instr_name(i))),
                    )?;
                    ensure_compatible(
                        f,
                        label,
                        pair[1],
                        StackValueType::Bombom,
                        "tipo inválido em operação binária",
                        Some(&format!("instr='{}'", instr_name(i))),
                    )?;
                    StackValueType::Bombom
                }
            };
            if out_ty == StackValueType::Logica
                && pair[0] != StackValueType::Unknown
                && pair[1] != StackValueType::Unknown
                && pair[0] != pair[1]
            {
                return Err(err_ctx(
                    f,
                    Some(label),
                    &format!(
                        "tipo inválido em comparação binária [instr='{}', lhs={}, rhs={}]",
                        instr_name(i),
                        render_stack_type(pair[0]),
                        render_stack_type(pair[1])
                    ),
                ));
            }
            stack.push(out_ty);
        }
        MachineInstr::Call { callee, argc } => {
            let args = pop_typed(
                f,
                label,
                stack,
                *argc,
                "underflow em call",
                Some(&format!(
                    "instr='call {}, {}', callee='{}'",
                    callee, argc, callee
                )),
            )?;
            if let Some((_ret, param_types)) = sigs.get(callee) {
                for (arg, expected) in args.iter().zip(param_types.iter().rev()) {
                    ensure_compatible(
                        f,
                        label,
                        *arg,
                        *expected,
                        "call com tipo de argumento incompatível",
                        Some(&format!(
                            "instr='call {}, {}', callee='{}'",
                            callee, argc, callee
                        )),
                    )?;
                }
            }
            let ret = sigs
                .get(callee)
                .map(|(ret, _)| *ret)
                .unwrap_or(TypeIR::Bombom);
            stack.push(type_to_stack(ret));
        }
        MachineInstr::CallVoid { callee, argc } => {
            let args = pop_typed(
                f,
                label,
                stack,
                *argc,
                "underflow em call_void",
                Some(&format!(
                    "instr='call_void {}, {}', callee='{}'",
                    callee, argc, callee
                )),
            )?;
            if let Some((_ret, param_types)) = sigs.get(callee) {
                for (arg, expected) in args.iter().zip(param_types.iter().rev()) {
                    ensure_compatible(
                        f,
                        label,
                        *arg,
                        *expected,
                        "call_void com tipo de argumento incompatível",
                        Some(&format!(
                            "instr='call_void {}, {}', callee='{}'",
                            callee, argc, callee
                        )),
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn apply_terminator_effect(
    f: &MachineFunction,
    label: &str,
    term: &MachineTerminator,
    stack: &mut Vec<StackValueType>,
    _sigs: &HashMap<String, (TypeIR, Vec<StackValueType>)>,
) -> Result<Vec<String>, PinkerError> {
    match term {
        MachineTerminator::Jmp(target) => Ok(vec![target.clone()]),
        MachineTerminator::BrTrue {
            then_label,
            else_label,
        } => {
            let top = pop_typed(
                f,
                label,
                stack,
                1,
                "underflow em br_true",
                Some(&format!("term='br_true {}, {}'", then_label, else_label)),
            )?;
            ensure_compatible(
                f,
                label,
                top[0],
                StackValueType::Logica,
                "br_true requer condição lógica",
                Some(&format!("term='br_true {}, {}'", then_label, else_label)),
            )?;
            Ok(vec![then_label.clone(), else_label.clone()])
        }
        MachineTerminator::Ret => {
            if stack.len() != 1 {
                return Err(err_ctx(
                    f,
                    Some(label),
                    "ret requer exatamente um valor na pilha",
                ));
            }
            let v = stack[0];
            ensure_compatible(
                f,
                label,
                v,
                type_to_stack(f.ret_type),
                "ret com tipo incompatível",
                Some("term='ret'"),
            )?;
            stack.clear();
            Ok(vec![])
        }
        MachineTerminator::RetVoid => {
            if !stack.is_empty() {
                return Err(err_ctx(f, Some(label), "ret_void requer pilha vazia"));
            }
            Ok(vec![])
        }
    }
}

fn pop_typed(
    f: &MachineFunction,
    label: &str,
    stack: &mut Vec<StackValueType>,
    amount: usize,
    message: &str,
    detail: Option<&str>,
) -> Result<Vec<StackValueType>, PinkerError> {
    if stack.len() < amount {
        return Err(err_ctx_with_detail(f, Some(label), message, detail));
    }
    let mut out = Vec::with_capacity(amount);
    for _ in 0..amount {
        out.push(stack.pop().expect("stack underflow already checked"));
    }
    Ok(out)
}

fn ensure_compatible(
    f: &MachineFunction,
    label: &str,
    got: StackValueType,
    expected: StackValueType,
    message: &str,
    detail: Option<&str>,
) -> Result<(), PinkerError> {
    if got != StackValueType::Unknown && got != expected {
        let inferred = format!(
            "{} [esperado={}, recebido={}]",
            message,
            render_stack_type(expected),
            render_stack_type(got)
        );
        return Err(err_ctx_with_detail(f, Some(label), &inferred, detail));
    }
    Ok(())
}

fn instr_name(i: &MachineInstr) -> &'static str {
    match i {
        MachineInstr::PushInt(_) => "push_int",
        MachineInstr::PushBool(_) => "push_bool",
        MachineInstr::LoadSlot(_) => "load_slot",
        MachineInstr::LoadGlobal(_) => "load_global",
        MachineInstr::StoreSlot(_) => "store_slot",
        MachineInstr::Neg => "neg",
        MachineInstr::Not => "not",
        MachineInstr::BitAnd => "bitand",
        MachineInstr::BitOr => "bitor",
        MachineInstr::BitXor => "bitxor",
        MachineInstr::Shl => "shl",
        MachineInstr::Shr => "shr",
        MachineInstr::Add => "add",
        MachineInstr::Sub => "sub",
        MachineInstr::Mul => "mul",
        MachineInstr::Div => "div",
        MachineInstr::Mod => "mod",
        MachineInstr::CmpEq => "cmp_eq",
        MachineInstr::CmpNe => "cmp_ne",
        MachineInstr::CmpLt => "cmp_lt",
        MachineInstr::CmpLe => "cmp_le",
        MachineInstr::CmpGt => "cmp_gt",
        MachineInstr::CmpGe => "cmp_ge",
        MachineInstr::Call { .. } => "call",
        MachineInstr::CallVoid { .. } => "call_void",
    }
}

fn render_stack_type(ty: StackValueType) -> &'static str {
    match ty {
        StackValueType::Bombom => "bombom",
        StackValueType::Logica => "lógica",
        StackValueType::Unknown => "unknown",
    }
}

fn type_to_stack(ty: TypeIR) -> StackValueType {
    match ty {
        TypeIR::Bombom
        | TypeIR::U8
        | TypeIR::U16
        | TypeIR::U32
        | TypeIR::U64
        | TypeIR::I8
        | TypeIR::I16
        | TypeIR::I32
        | TypeIR::I64 => StackValueType::Bombom,
        TypeIR::Logica => StackValueType::Logica,
        TypeIR::Nulo => StackValueType::Unknown,
    }
}

fn infer_operand_type(op: &crate::cfg_ir::OperandIR) -> StackValueType {
    match op {
        crate::cfg_ir::OperandIR::Int(_) => StackValueType::Bombom,
        crate::cfg_ir::OperandIR::Bool(_) => StackValueType::Logica,
        _ => StackValueType::Unknown,
    }
}

fn merge_stack_types(a: &[StackValueType], b: &[StackValueType]) -> Vec<StackValueType> {
    a.iter()
        .zip(b.iter())
        .map(|(lhs, rhs)| match (lhs, rhs) {
            (x, y) if x == y => *x,
            _ => StackValueType::Unknown,
        })
        .collect()
}

fn merge_types(a: StackValueType, b: StackValueType) -> StackValueType {
    match (a, b) {
        (x, y) if x == y => x,
        (StackValueType::Unknown, y) => y,
        (x, StackValueType::Unknown) => x,
        _ => StackValueType::Unknown,
    }
}

// Temporários gerados pelo lowering seguem o padrão `%t<N>` (e.g. `%t0`, `%t12`).
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
    err_ctx_with_detail(f, block, msg, None)
}

fn err_ctx_with_detail(
    f: &MachineFunction,
    block: Option<&str>,
    msg: &str,
    detail: Option<&str>,
) -> PinkerError {
    let prefix = if let Some(detail) = detail {
        format!("{} [{}]", msg, detail)
    } else {
        msg.to_string()
    };
    let scoped = if let Some(block) = block {
        format!("{} (função '{}', bloco '{}')", prefix, f.name, block)
    } else {
        format!("{} (função '{}')", prefix, f.name)
    };
    err(&scoped)
}
