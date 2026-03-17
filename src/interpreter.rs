use crate::abstract_machine::{
    MachineFunction, MachineGlobal, MachineInstr, MachineProgram, MachineTerminator,
};
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::token::{Position, Span};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValue {
    Int(u64),
    Bool(bool),
}

pub fn run_program(program: &MachineProgram) -> Result<Option<RuntimeValue>, PinkerError> {
    let globals = build_globals(program)?;
    call_function("principal", vec![], program, &globals)
}

fn build_globals(program: &MachineProgram) -> Result<HashMap<String, RuntimeValue>, PinkerError> {
    let mut globals = HashMap::new();
    for g in &program.globals {
        let value = eval_global_value(g)?;
        globals.insert(g.name.clone(), value);
    }
    Ok(globals)
}

fn eval_global_value(g: &MachineGlobal) -> Result<RuntimeValue, PinkerError> {
    match &g.value {
        OperandIR::Int(v) => Ok(RuntimeValue::Int(*v)),
        OperandIR::Bool(v) => Ok(RuntimeValue::Bool(*v)),
        _ => Err(runtime_err("valor global não suportado em runtime")),
    }
}

fn call_function(
    fn_name: &str,
    args: Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
) -> Result<Option<RuntimeValue>, PinkerError> {
    let function = find_function(fn_name, program)?;

    if function.params.len() != args.len() {
        return Err(runtime_err(&format!(
            "[{}] chamada com aridade inválida",
            fn_name
        )));
    }

    let mut labels = HashMap::new();
    for (idx, block) in function.blocks.iter().enumerate() {
        labels.insert(block.label.clone(), idx);
    }

    let mut slots: HashMap<String, RuntimeValue> = HashMap::new();
    for (slot, value) in function.params.iter().cloned().zip(args.into_iter()) {
        slots.insert(slot, value);
    }

    let mut stack: Vec<RuntimeValue> = Vec::new();
    let mut current_label = "entry".to_string();

    loop {
        let Some(&block_idx) = labels.get(&current_label) else {
            return Err(runtime_err(&format!(
                "[{}] label de execução inexistente: {}",
                fn_name, current_label
            )));
        };
        let block = &function.blocks[block_idx];

        for instr in &block.code {
            exec_instr(instr, &mut slots, &mut stack, program, globals)?;
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
                    return Err(runtime_err(&format!(
                        "[{}] ret inválido: pilha deve ter 1 valor",
                        fn_name
                    )));
                }
                return Ok(Some(stack.pop().expect("len checked")));
            }
            MachineTerminator::RetVoid => {
                if !stack.is_empty() {
                    return Err(runtime_err(&format!(
                        "[{}] ret_void inválido: pilha deve estar vazia",
                        fn_name
                    )));
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
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
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
        MachineInstr::LoadGlobal(name) => {
            let Some(value) = globals.get(name).copied() else {
                return Err(runtime_err("global inexistente em runtime"));
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
        MachineInstr::Call { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = call_function(callee, args, program, globals)?;
            let Some(value) = result else {
                return Err(runtime_err("call exige função com retorno"));
            };
            stack.push(value);
        }
        MachineInstr::CallVoid { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = call_function(callee, args, program, globals)?;
            if result.is_some() {
                return Err(runtime_err("call_void exige função sem retorno"));
            }
        }
    }

    Ok(())
}

fn find_function<'a>(
    name: &str,
    program: &'a MachineProgram,
) -> Result<&'a MachineFunction, PinkerError> {
    program
        .functions
        .iter()
        .find(|f| f.name == name)
        .ok_or_else(|| runtime_err("função chamada inexistente"))
}

fn pop_args(stack: &mut Vec<RuntimeValue>, argc: usize) -> Result<Vec<RuntimeValue>, PinkerError> {
    let mut args = Vec::with_capacity(argc);
    for _ in 0..argc {
        args.push(pop(stack, "underflow em argumentos de chamada")?);
    }
    args.reverse();
    Ok(args)
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
