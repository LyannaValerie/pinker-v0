//! Interpretador da Machine abstrata de pilha do Pinker.
//!
//! Executa um `MachineProgram` validado chamando `principal` com frame
//! próprio de slots e pilha de operandos. Suporta chamadas entre funções,
//! recursão, globals literais e stack trace simples em erros de runtime.
//!
//! Ponto de entrada: [`run_program`].

use crate::abstract_machine::{
    MachineFunction, MachineGlobal, MachineInstr, MachineProgram, MachineTerminator,
};
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::token::Span;
use std::collections::HashMap;

const MAX_CALL_DEPTH: usize = 64;

// Truncamento de stack trace longo (Fase 27b):
// traces com mais de TRACE_TRUNC_THRESHOLD frames são resumidos mostrando
// os primeiros TRACE_HEAD e os últimos TRACE_TAIL, com linha de omissão.
const TRACE_TRUNC_THRESHOLD: usize = 10;
const TRACE_HEAD: usize = 5;
const TRACE_TAIL: usize = 5;

#[derive(Debug, Clone)]
struct RuntimeFrame {
    fn_name: String,
    block_label: Option<String>,
    current_instr: Option<&'static str>,
    future_span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValue {
    Int(u64),
    IntSigned(i64),
    Ptr(usize),
    Bool(bool),
}

pub fn run_program(program: &MachineProgram) -> Result<Option<RuntimeValue>, PinkerError> {
    let globals = build_globals(program)?;
    let mut memory = build_memory(program, &globals)?;
    let mut call_stack = Vec::new();
    call_function(
        "principal",
        vec![],
        program,
        &globals,
        &mut memory,
        &mut call_stack,
    )
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
    match (&g.value, g.ty) {
        (OperandIR::Int(v), crate::ir::TypeIR::Pointer { .. }) => {
            Ok(RuntimeValue::Ptr(*v as usize))
        }
        (OperandIR::Int(v), ty) if ty.is_signed() => Ok(RuntimeValue::IntSigned(*v as i64)),
        (OperandIR::Int(v), _) => Ok(RuntimeValue::Int(*v)),
        (OperandIR::Bool(v), _) => Ok(RuntimeValue::Bool(*v)),
        _ => Err(runtime_err("valor global não suportado em runtime")),
    }
}

fn build_memory(
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
) -> Result<HashMap<usize, RuntimeValue>, PinkerError> {
    let mut memory = HashMap::new();
    let mut next_addr: usize = 1;
    for g in &program.globals {
        match g.ty {
            crate::ir::TypeIR::Bombom
            | crate::ir::TypeIR::U8
            | crate::ir::TypeIR::U16
            | crate::ir::TypeIR::U32
            | crate::ir::TypeIR::U64
            | crate::ir::TypeIR::I8
            | crate::ir::TypeIR::I16
            | crate::ir::TypeIR::I32
            | crate::ir::TypeIR::I64
            | crate::ir::TypeIR::Logica => {
                let value = *globals
                    .get(&g.name)
                    .ok_or_else(|| runtime_err("global inexistente em runtime"))?;
                memory.insert(next_addr, value);
                next_addr = next_addr.saturating_add(1);
            }
            _ => {}
        }
    }
    Ok(memory)
}

// Executa uma função pelo nome com os argumentos fornecidos.
// O call_stack acumula os nomes ativos para montar o stack trace em erros.
// Retorna `None` para funções void, `Some(valor)` caso contrário.
fn call_function(
    fn_name: &str,
    args: Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
    memory: &mut HashMap<usize, RuntimeValue>,
    call_stack: &mut Vec<RuntimeFrame>,
) -> Result<Option<RuntimeValue>, PinkerError> {
    if call_stack.len() >= MAX_CALL_DEPTH {
        return Err(runtime_err(&format!(
            "limite preventivo de recursão excedido: profundidade máxima de chamadas ({MAX_CALL_DEPTH}) atingida ao entrar em '{fn_name}'"
        )));
    }

    call_stack.push(RuntimeFrame {
        fn_name: fn_name.to_string(),
        block_label: None,
        current_instr: None,
        future_span: None,
    });

    // Encapsula a execução numa closure para poder anexar o trace no retorno.
    let result = (|| {
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
            let coerced = if let Some(ty) = function.slot_types.get(&slot) {
                coerce_runtime_value_to_type(value, *ty)?
            } else {
                value
            };
            slots.insert(slot, coerced);
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
            if let Some(frame) = call_stack.last_mut() {
                frame.block_label = Some(block.label.clone());
            }

            for instr in &block.code {
                set_current_instr(call_stack, Some(machine_instr_name(instr)));
                exec_instr(
                    instr, &mut slots, &mut stack, program, globals, memory, call_stack,
                )?;
                set_current_instr(call_stack, None);
            }

            match &block.terminator {
                MachineTerminator::Jmp(target) => {
                    current_label.clone_from(target);
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
    })();

    let result = result.map_err(|err| attach_runtime_trace(err, call_stack));
    let _ = call_stack.pop();
    result
}

fn exec_instr(
    instr: &MachineInstr,
    slots: &mut HashMap<String, RuntimeValue>,
    stack: &mut Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
    memory: &mut HashMap<usize, RuntimeValue>,
    call_stack: &mut Vec<RuntimeFrame>,
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
            let coerced =
                if let Some(ty) = current_function(program, call_stack)?.slot_types.get(slot) {
                    coerce_runtime_value_to_type(value, *ty)?
                } else {
                    value
                };
            slots.insert(slot.clone(), coerced);
        }
        MachineInstr::Neg => {
            let value = pop_numeric(stack, "neg exige inteiro no topo")?;
            let out = match value {
                RuntimeValue::Int(v) => RuntimeValue::Int((0u64).wrapping_sub(v)),
                RuntimeValue::IntSigned(v) => RuntimeValue::IntSigned(v.wrapping_neg()),
                RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
            };
            stack.push(out);
        }
        MachineInstr::Not => {
            let value = pop_bool(stack, "not exige lógica no topo")?;
            stack.push(RuntimeValue::Bool(!value));
        }
        MachineInstr::DerefLoad { .. } => {
            let ptr = pop(stack, "deref_load exige ponteiro no topo")?;
            let RuntimeValue::Ptr(addr) = ptr else {
                return Err(runtime_err("deref_load exige ponteiro no topo"));
            };
            let Some(value) = memory.get(&addr).copied() else {
                return Err(runtime_err(
                    "deref_load em endereço inválido ou não inicializado",
                ));
            };
            stack.push(value);
        }
        MachineInstr::DerefStore { ty } => {
            let value = pop(stack, "deref_store exige valor no topo")?;
            let ptr = pop(stack, "deref_store exige ponteiro abaixo do valor")?;
            let RuntimeValue::Ptr(addr) = ptr else {
                return Err(runtime_err(
                    "deref_store exige ponteiro abaixo do valor no topo",
                ));
            };
            if !memory.contains_key(&addr) {
                return Err(runtime_err(
                    "deref_store em endereço inválido ou não inicializado",
                ));
            }
            let coerced = coerce_runtime_value_to_type(value, *ty)?;
            memory.insert(addr, coerced);
        }
        MachineInstr::BitAnd => {
            let (lhs, rhs) = pop_bin_numeric(stack, "bitand exige dois inteiros")?;
            stack.push(bin_int(lhs, rhs, |a, b| a & b, |a, b| a & b)?);
        }
        MachineInstr::BitOr => {
            let (lhs, rhs) = pop_bin_numeric(stack, "bitor exige dois inteiros")?;
            stack.push(bin_int(lhs, rhs, |a, b| a | b, |a, b| a | b)?);
        }
        MachineInstr::BitXor => {
            let (lhs, rhs) = pop_bin_numeric(stack, "bitxor exige dois inteiros")?;
            stack.push(bin_int(lhs, rhs, |a, b| a ^ b, |a, b| a ^ b)?);
        }
        MachineInstr::Shl => {
            let (lhs, rhs) = pop_bin_numeric(stack, "shl exige dois inteiros")?;
            stack.push(bin_int(
                lhs,
                rhs,
                |a, b| a.wrapping_shl(b as u32),
                |a, b| a.wrapping_shl(b as u32),
            )?);
        }
        MachineInstr::Shr => {
            let (lhs, rhs) = pop_bin_numeric(stack, "shr exige dois inteiros")?;
            stack.push(bin_int(
                lhs,
                rhs,
                |a, b| a.wrapping_shr(b as u32),
                |a, b| a.wrapping_shr(b as u32),
            )?);
        }
        MachineInstr::Add => {
            let rhs = pop(stack, "underflow em add")?;
            let lhs = pop(stack, "underflow em add")?;
            stack.push(eval_add(lhs, rhs)?);
        }
        MachineInstr::Sub => {
            let rhs = pop(stack, "underflow em sub")?;
            let lhs = pop(stack, "underflow em sub")?;
            stack.push(eval_sub(lhs, rhs)?);
        }
        MachineInstr::Mul => {
            let (lhs, rhs) = pop_bin_numeric(stack, "mul exige dois inteiros")?;
            stack.push(bin_int(
                lhs,
                rhs,
                |a, b| a.wrapping_mul(b),
                |a, b| a.wrapping_mul(b),
            )?);
        }
        MachineInstr::Div => {
            let (lhs, rhs) = pop_bin_numeric(stack, "div exige dois inteiros")?;
            stack.push(bin_int_checked_div(lhs, rhs)?);
        }
        MachineInstr::Mod => {
            let (lhs, rhs) = pop_bin_numeric(stack, "mod exige dois inteiros")?;
            stack.push(bin_int_checked_mod(lhs, rhs)?);
        }
        MachineInstr::CmpEq => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_eq exige dois inteiros")?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a == b,
                |a, b| a == b,
            )?));
        }
        MachineInstr::CmpNe => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_ne exige dois inteiros")?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a != b,
                |a, b| a != b,
            )?));
        }
        MachineInstr::CmpLt => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_lt exige dois inteiros")?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a < b,
                |a, b| a < b,
            )?));
        }
        MachineInstr::CmpLe => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_le exige dois inteiros")?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a <= b,
                |a, b| a <= b,
            )?));
        }
        MachineInstr::CmpGt => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_gt exige dois inteiros")?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a > b,
                |a, b| a > b,
            )?));
        }
        MachineInstr::CmpGe => {
            let (lhs, rhs) = pop_bin_numeric(stack, "cmp_ge exige dois inteiros")?;
            stack.push(RuntimeValue::Bool(cmp_int(
                lhs,
                rhs,
                |a, b| a >= b,
                |a, b| a >= b,
            )?));
        }
        MachineInstr::Call { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = call_function(callee, args, program, globals, memory, call_stack)?;
            let Some(value) = result else {
                return Err(runtime_err("call exige função com retorno"));
            };
            stack.push(value);
        }
        MachineInstr::CallVoid { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = call_function(callee, args, program, globals, memory, call_stack)?;
            if result.is_some() {
                return Err(runtime_err("call_void exige função sem retorno"));
            }
        }
        MachineInstr::PrintInt => match pop_numeric(stack, "print_int exige inteiro no topo")? {
            RuntimeValue::Int(v) => println!("{}", v),
            RuntimeValue::IntSigned(v) => println!("{}", v),
            RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
            RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
        },
        MachineInstr::PrintBool => {
            let v = pop_bool(stack, "print_bool exige lógica no topo")?;
            println!("{}", if v { "verdade" } else { "falso" });
        }
        MachineInstr::PrintStr(s) => {
            println!("{}", s);
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

// Desempilha `argc` argumentos e reverte a ordem para corresponder à
// declaração da função (pilha é LIFO, mas args foram empilhados left-to-right).
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

fn pop_numeric(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<RuntimeValue, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Int(v) => Ok(RuntimeValue::Int(v)),
        RuntimeValue::IntSigned(v) => Ok(RuntimeValue::IntSigned(v)),
        RuntimeValue::Ptr(_) => Err(runtime_err(msg)),
        RuntimeValue::Bool(_) => Err(runtime_err(msg)),
    }
}

fn pop_bool(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<bool, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Bool(v) => Ok(v),
        RuntimeValue::Int(_) => Err(runtime_err(msg)),
        RuntimeValue::IntSigned(_) => Err(runtime_err(msg)),
        RuntimeValue::Ptr(_) => Err(runtime_err(msg)),
    }
}

fn pop_bin_numeric(
    stack: &mut Vec<RuntimeValue>,
    msg: &str,
) -> Result<(RuntimeValue, RuntimeValue), PinkerError> {
    let rhs = pop_numeric(stack, msg)?;
    let lhs = pop_numeric(stack, msg)?;
    Ok((lhs, rhs))
}

fn coerce_runtime_value_to_type(
    value: RuntimeValue,
    ty: crate::ir::TypeIR,
) -> Result<RuntimeValue, PinkerError> {
    if ty.is_integer() {
        return match (value, ty.is_signed()) {
            (RuntimeValue::Int(v), true) => Ok(RuntimeValue::IntSigned(v as i64)),
            (RuntimeValue::IntSigned(v), false) => Ok(RuntimeValue::Int(v as u64)),
            (v, _) => Ok(v),
        };
    }

    if matches!(ty, crate::ir::TypeIR::Pointer { .. }) {
        return match value {
            RuntimeValue::Int(v) => Ok(RuntimeValue::Ptr(v as usize)),
            RuntimeValue::IntSigned(v) if v < 0 => Err(runtime_err(
                "endereço de ponteiro inválido em runtime: valor negativo",
            )),
            RuntimeValue::IntSigned(v) => Ok(RuntimeValue::Ptr(v as usize)),
            RuntimeValue::Ptr(v) => Ok(RuntimeValue::Ptr(v)),
            RuntimeValue::Bool(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
        };
    }

    Ok(value)
}

fn current_function<'a>(
    program: &'a MachineProgram,
    call_stack: &[RuntimeFrame],
) -> Result<&'a MachineFunction, PinkerError> {
    let fn_name = call_stack
        .last()
        .map(|frame| frame.fn_name.as_str())
        .ok_or_else(|| runtime_err("pilha de chamadas vazia"))?;
    find_function(fn_name, program)
}

fn bin_int(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
    op_u: fn(u64, u64) -> u64,
    op_s: fn(i64, i64) -> i64,
) -> Result<RuntimeValue, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => Ok(RuntimeValue::Int(op_u(a, b))),
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => {
            Ok(RuntimeValue::IntSigned(op_s(a, b)))
        }
        _ => Err(runtime_err("operação inteira inválida em runtime")),
    }
}

fn eval_add(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match (lhs, rhs) {
        (RuntimeValue::Ptr(base), RuntimeValue::Int(offset)) => {
            Ok(RuntimeValue::Ptr(base.wrapping_add(offset as usize)))
        }
        (lhs, rhs) => bin_int(lhs, rhs, |a, b| a.wrapping_add(b), |a, b| a.wrapping_add(b))
            .map_err(|_| runtime_err("add exige inteiros ou 'seta<bombom> + bombom'")),
    }
}

fn eval_sub(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match (lhs, rhs) {
        (RuntimeValue::Ptr(base), RuntimeValue::Int(offset)) => {
            Ok(RuntimeValue::Ptr(base.wrapping_sub(offset as usize)))
        }
        (lhs, rhs) => bin_int(lhs, rhs, |a, b| a.wrapping_sub(b), |a, b| a.wrapping_sub(b))
            .map_err(|_| runtime_err("sub exige inteiros ou 'seta<bombom> - bombom'")),
    }
}

fn cmp_int(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
    op_u: fn(u64, u64) -> bool,
    op_s: fn(i64, i64) -> bool,
) -> Result<bool, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => Ok(op_u(a, b)),
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => Ok(op_s(a, b)),
        _ => Err(runtime_err("comparação inteira inválida em runtime")),
    }
}

fn bin_int_checked_div(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            Ok(RuntimeValue::Int(a / b))
        }
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            Ok(RuntimeValue::IntSigned(a / b))
        }
        _ => Err(runtime_err("divisão inteira inválida em runtime")),
    }
}

fn bin_int_checked_mod(lhs: RuntimeValue, rhs: RuntimeValue) -> Result<RuntimeValue, PinkerError> {
    match normalize_numeric_pair(lhs, rhs)? {
        (RuntimeValue::Int(a), RuntimeValue::Int(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            Ok(RuntimeValue::Int(a % b))
        }
        (RuntimeValue::IntSigned(a), RuntimeValue::IntSigned(b)) => {
            if b == 0 {
                return Err(runtime_err("divisão por zero"));
            }
            Ok(RuntimeValue::IntSigned(a % b))
        }
        _ => Err(runtime_err("módulo inteiro inválido em runtime")),
    }
}

fn normalize_numeric_pair(
    lhs: RuntimeValue,
    rhs: RuntimeValue,
) -> Result<(RuntimeValue, RuntimeValue), PinkerError> {
    match (lhs, rhs) {
        (RuntimeValue::Int(_), RuntimeValue::Int(_))
        | (RuntimeValue::IntSigned(_), RuntimeValue::IntSigned(_)) => Ok((lhs, rhs)),
        // lhs signed, rhs unsigned: converte rhs para signed preservando ordem
        (RuntimeValue::IntSigned(a), RuntimeValue::Int(b)) => {
            if b > i64::MAX as u64 {
                return Err(runtime_err(
                    "mistura signed/unsigned fora de faixa no runtime (sem coerção implícita)",
                ));
            }
            Ok((
                RuntimeValue::IntSigned(a),
                RuntimeValue::IntSigned(b as i64),
            ))
        }
        // lhs unsigned, rhs signed: converte lhs para signed preservando ordem
        (RuntimeValue::Int(a), RuntimeValue::IntSigned(b)) => {
            if a > i64::MAX as u64 {
                return Err(runtime_err(
                    "mistura signed/unsigned fora de faixa no runtime (sem coerção implícita)",
                ));
            }
            Ok((
                RuntimeValue::IntSigned(a as i64),
                RuntimeValue::IntSigned(b),
            ))
        }
        _ => Err(runtime_err("operação inteira exige valores inteiros")),
    }
}

fn runtime_err(msg: &str) -> PinkerError {
    PinkerError::Runtime {
        msg: enrich_runtime_msg(msg),
        span: None,
    }
}

fn enrich_runtime_msg(msg: &str) -> String {
    let (kind, hint) = classify_runtime_msg(msg);
    format!(
        "[runtime::{kind}] {msg}{}",
        hint.map(|h| format!(" | dica: {h}")).unwrap_or_default()
    )
}

fn classify_runtime_msg(msg: &str) -> (&'static str, Option<&'static str>) {
    if msg.contains("limite preventivo de recursão excedido") {
        (
            "limite_recursao_excedido",
            Some(
                "revise o caso-base da função recursiva para garantir término antes do limite interno",
            ),
        )
    } else if msg.contains("divisão por zero") {
        (
            "divisao_por_zero",
            Some("verifique se o divisor é diferente de 0 antes da operação '/'"),
        )
    } else if msg.contains("slot não inicializado") {
        (
            "slot_nao_inicializado",
            Some("inicialize o slot antes de fazer load_slot"),
        )
    } else if msg.contains("função chamada inexistente") {
        (
            "funcao_inexistente",
            Some("confira se o nome da função e a assinatura existem no programa"),
        )
    } else if msg.contains("aridade inválida") {
        (
            "aridade_invalida",
            Some("confira a quantidade de argumentos passados na chamada"),
        )
    } else if msg.contains("global inexistente") {
        (
            "global_inexistente",
            Some("use apenas globals declaradas em `eterno`"),
        )
    } else if msg.contains("deref_load")
        || msg.contains("deref_store")
        || msg.contains("endereço inválido")
        || msg.contains("ponteiro no topo")
    {
        (
            "acesso_invalido_ptr",
            Some("verifique se o endereço do ponteiro está mapeado (global escalar declarada)"),
        )
    } else {
        ("erro", None)
    }
}

// Adiciona o stack trace textual à mensagem de erro, se ainda não tiver sido
// adicionado (evita duplicação quando o erro borbulha por múltiplos frames).
fn attach_runtime_trace(err: PinkerError, call_stack: &[RuntimeFrame]) -> PinkerError {
    match err {
        PinkerError::Runtime { msg, span } => {
            if msg.contains("\nstack trace:\n") {
                PinkerError::Runtime { msg, span }
            } else {
                let mut traced = msg;
                traced.push_str(&render_runtime_trace(call_stack));
                PinkerError::Runtime { msg: traced, span }
            }
        }
        _ => err,
    }
}

fn render_frame(frame: &RuntimeFrame, out: &mut String) {
    out.push_str("  at ");
    out.push_str(&frame.fn_name);
    if let Some(label) = &frame.block_label {
        out.push_str(" [bloco: ");
        out.push_str(label);
        out.push(']');
    }
    if let Some(instr) = frame.current_instr {
        out.push_str(" [instr: ");
        out.push_str(instr);
        out.push(']');
    }
    if let Some(span) = frame.future_span {
        out.push_str(" [span: ");
        out.push_str(&span.to_string());
        out.push(']');
    }
    out.push('\n');
}

fn render_runtime_trace(call_stack: &[RuntimeFrame]) -> String {
    let mut out = String::from("\nstack trace:\n");
    let n = call_stack.len();
    if n <= TRACE_TRUNC_THRESHOLD {
        for frame in call_stack {
            render_frame(frame, &mut out);
        }
    } else {
        for frame in &call_stack[..TRACE_HEAD] {
            render_frame(frame, &mut out);
        }
        let omitted = n - TRACE_HEAD - TRACE_TAIL;
        out.push_str(&format!("  ... {omitted} frames omitidos ...\n"));
        for frame in &call_stack[n - TRACE_TAIL..] {
            render_frame(frame, &mut out);
        }
    }
    out
}

fn set_current_instr(call_stack: &mut [RuntimeFrame], instr_name: Option<&'static str>) {
    if let Some(frame) = call_stack.last_mut() {
        frame.current_instr = instr_name;
    }
}

fn machine_instr_name(instr: &MachineInstr) -> &'static str {
    match instr {
        MachineInstr::PushInt(_) => "push_int",
        MachineInstr::PushBool(_) => "push_bool",
        MachineInstr::LoadSlot(_) => "load_slot",
        MachineInstr::LoadGlobal(_) => "load_global",
        MachineInstr::StoreSlot(_) => "store_slot",
        MachineInstr::Neg => "neg",
        MachineInstr::Not => "not",
        MachineInstr::DerefLoad { .. } => "deref_load",
        MachineInstr::DerefStore { .. } => "deref_store",
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
        MachineInstr::PrintInt => "print_int",
        MachineInstr::PrintBool => "print_bool",
        MachineInstr::PrintStr(_) => "print_str",
    }
}
