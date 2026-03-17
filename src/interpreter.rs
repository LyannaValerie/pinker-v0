use crate::abstract_machine::{MachineFunction, MachineInstr, MachineProgram, MachineTerminator};
use crate::error::PinkerError;
use crate::token::{Position, Span};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValue {
    Int(u64),
    Bool(bool),
}

pub fn run_program(program: &MachineProgram) -> Result<Option<RuntimeValue>, PinkerError> {
    if !program.globals.is_empty() {
        return Err(runtime_err("runtime mínimo não suporta globals"));
    }
    if program.functions.len() != 1 {
        return Err(runtime_err(
            "runtime mínimo suporta apenas programa com função principal",
        ));
    }

    let function = &program.functions[0];
    if function.name != "principal" {
        return Err(runtime_err(
            "runtime mínimo exige função principal como única função",
        ));
    }

    run_function(function)
}

fn run_function(function: &MachineFunction) -> Result<Option<RuntimeValue>, PinkerError> {
    let mut labels = HashMap::new();
    for (idx, block) in function.blocks.iter().enumerate() {
        labels.insert(block.label.clone(), idx);
    }

    let mut slots: HashMap<String, RuntimeValue> = HashMap::new();
    let mut stack: Vec<RuntimeValue> = Vec::new();
    let mut current_label = "entry".to_string();

    loop {
        let Some(&block_idx) = labels.get(&current_label) else {
            return Err(runtime_err("label de execução inexistente"));
        };
        let block = &function.blocks[block_idx];

        for instr in &block.code {
            exec_instr(instr, &mut slots, &mut stack)?;
        }

        match &block.terminator {
            MachineTerminator::Jmp(target) => {
                current_label = target.clone();
            }
            MachineTerminator::BrTrue {
                then_label,
                else_label,
            } => {
                let cond = pop_bool(&mut stack, "br_true requer bool no topo")?;
                current_label = if cond {
                    then_label.clone()
                } else {
                    else_label.clone()
                };
            }
            MachineTerminator::Ret => {
                if stack.len() != 1 {
                    return Err(runtime_err("ret inválido: pilha deve ter 1 valor"));
                }
                return Ok(Some(stack.pop().expect("len checked")));
            }
            MachineTerminator::RetVoid => {
                if !stack.is_empty() {
                    return Err(runtime_err("ret_void inválido: pilha deve estar vazia"));
                }
                return Ok(None);
            }
        }
    }
}

fn exec_instr(
    instr: &MachineInstr,
    slots: &mut HashMap<String, RuntimeValue>,
    stack: &mut Vec<RuntimeValue>,
) -> Result<(), PinkerError> {
    match instr {
        MachineInstr::PushInt(v) => stack.push(RuntimeValue::Int(*v)),
        MachineInstr::PushBool(v) => stack.push(RuntimeValue::Bool(*v)),
        MachineInstr::LoadSlot(slot) => {
            let Some(value) = slots.get(slot).copied() else {
                return Err(runtime_err("load_slot em slot não inicializado"));
            };
            stack.push(value);
        }
        MachineInstr::StoreSlot(slot) => {
            let value = pop(stack, "store_slot exige valor na pilha")?;
            slots.insert(slot.clone(), value);
        }
        MachineInstr::Neg => {
            let value = pop_int(stack, "neg exige bombom no topo")?;
            stack.push(RuntimeValue::Int((0u64).wrapping_sub(value)));
        }
        MachineInstr::Not => {
            let value = pop_bool(stack, "not exige lógica no topo")?;
            stack.push(RuntimeValue::Bool(!value));
        }
        MachineInstr::Add => {
            let (lhs, rhs) = pop_bin_int(stack, "add exige dois bombons")?;
            stack.push(RuntimeValue::Int(lhs.wrapping_add(rhs)));
        }
        MachineInstr::Sub => {
            let (lhs, rhs) = pop_bin_int(stack, "sub exige dois bombons")?;
            stack.push(RuntimeValue::Int(lhs.wrapping_sub(rhs)));
        }
        MachineInstr::Mul => {
            let (lhs, rhs) = pop_bin_int(stack, "mul exige dois bombons")?;
            stack.push(RuntimeValue::Int(lhs.wrapping_mul(rhs)));
        }
        MachineInstr::Div => {
            let (lhs, rhs) = pop_bin_int(stack, "div exige dois bombons")?;
            if rhs == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            stack.push(RuntimeValue::Int(lhs / rhs));
        }
        MachineInstr::CmpEq => {
            let (lhs, rhs) = pop_bin_int(stack, "cmp_eq exige dois bombons")?;
            stack.push(RuntimeValue::Bool(lhs == rhs));
        }
        MachineInstr::CmpNe => {
            let (lhs, rhs) = pop_bin_int(stack, "cmp_ne exige dois bombons")?;
            stack.push(RuntimeValue::Bool(lhs != rhs));
        }
        MachineInstr::CmpLt => {
            let (lhs, rhs) = pop_bin_int(stack, "cmp_lt exige dois bombons")?;
            stack.push(RuntimeValue::Bool(lhs < rhs));
        }
        MachineInstr::CmpLe => {
            let (lhs, rhs) = pop_bin_int(stack, "cmp_le exige dois bombons")?;
            stack.push(RuntimeValue::Bool(lhs <= rhs));
        }
        MachineInstr::CmpGt => {
            let (lhs, rhs) = pop_bin_int(stack, "cmp_gt exige dois bombons")?;
            stack.push(RuntimeValue::Bool(lhs > rhs));
        }
        MachineInstr::CmpGe => {
            let (lhs, rhs) = pop_bin_int(stack, "cmp_ge exige dois bombons")?;
            stack.push(RuntimeValue::Bool(lhs >= rhs));
        }
        MachineInstr::LoadGlobal(_) => {
            return Err(runtime_err("runtime mínimo não suporta globals"));
        }
        MachineInstr::Call { .. } => {
            return Err(runtime_err("runtime mínimo não suporta call"));
        }
        MachineInstr::CallVoid { .. } => {
            return Err(runtime_err("runtime mínimo não suporta call_void"));
        }
    }

    Ok(())
}

fn pop(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<RuntimeValue, PinkerError> {
    stack.pop().ok_or_else(|| runtime_err(msg))
}

fn pop_int(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<u64, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Int(v) => Ok(v),
        RuntimeValue::Bool(_) => Err(runtime_err(msg)),
    }
}

fn pop_bool(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<bool, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Bool(v) => Ok(v),
        RuntimeValue::Int(_) => Err(runtime_err(msg)),
    }
}

fn pop_bin_int(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<(u64, u64), PinkerError> {
    let rhs = pop_int(stack, msg)?;
    let lhs = pop_int(stack, msg)?;
    Ok((lhs, rhs))
}

fn runtime_err(msg: &str) -> PinkerError {
    PinkerError::Runtime {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}
