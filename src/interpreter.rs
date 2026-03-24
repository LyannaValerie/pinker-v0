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
use std::env;
use std::fs;
use std::io;

const MAX_CALL_DEPTH: usize = 64;

// Truncamento de stack trace longo (Fase 27b):
// traces com mais de TRACE_TRUNC_THRESHOLD frames são resumidos mostrando
// os primeiros TRACE_HEAD e os últimos TRACE_TAIL, com linha de omissão.
const TRACE_TRUNC_THRESHOLD: usize = 10;
const TRACE_HEAD: usize = 5;
const TRACE_TAIL: usize = 5;

enum IntrinsicCall {
    NotIntrinsic,
    Done(Option<RuntimeValue>),
}

struct RuntimeIoState {
    open_files: HashMap<u64, RuntimeOpenFile>,
    next_file_handle: u64,
    closed_handles: std::collections::HashSet<u64>,
    cli_args: Vec<String>,
    exit_status: Option<i32>,
}

struct RuntimeOpenFile {
    path: String,
    content: String,
}

#[derive(Debug, Clone)]
struct RuntimeFrame {
    fn_name: String,
    block_label: Option<String>,
    current_instr: Option<&'static str>,
    future_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeValue {
    Int(u64),
    IntSigned(i64),
    Ptr(usize),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    pub return_value: Option<RuntimeValue>,
    pub exit_status: Option<i32>,
}

pub fn run_program(program: &MachineProgram) -> Result<Option<RuntimeValue>, PinkerError> {
    Ok(run_program_with_args(program, &[])?.return_value)
}

pub fn run_program_with_args(
    program: &MachineProgram,
    cli_args: &[String],
) -> Result<RunOutcome, PinkerError> {
    let globals = build_globals(program)?;
    let mut memory = build_memory(program, &globals)?;
    let mut io_state = RuntimeIoState {
        open_files: HashMap::new(),
        next_file_handle: 1,
        closed_handles: std::collections::HashSet::new(),
        cli_args: cli_args.to_vec(),
        exit_status: None,
    };
    let mut call_stack = Vec::new();
    let return_value = call_function(
        "principal",
        vec![],
        program,
        &globals,
        &mut memory,
        &mut io_state,
        &mut call_stack,
    )?;
    Ok(RunOutcome {
        return_value,
        exit_status: io_state.exit_status,
    })
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
                let value = globals
                    .get(&g.name)
                    .cloned()
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
    io_state: &mut RuntimeIoState,
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
                    instr, &mut slots, &mut stack, program, globals, memory, io_state, call_stack,
                )?;
                set_current_instr(call_stack, None);
                if io_state.exit_status.is_some() {
                    return Ok(None);
                }
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

#[allow(clippy::too_many_arguments)]
fn exec_instr(
    instr: &MachineInstr,
    slots: &mut HashMap<String, RuntimeValue>,
    stack: &mut Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
    memory: &mut HashMap<usize, RuntimeValue>,
    io_state: &mut RuntimeIoState,
    call_stack: &mut Vec<RuntimeFrame>,
) -> Result<(), PinkerError> {
    match instr {
        MachineInstr::PushInt(v) => stack.push(RuntimeValue::Int(*v)),
        MachineInstr::PushBool(v) => stack.push(RuntimeValue::Bool(*v)),
        MachineInstr::PushStr(v) => stack.push(RuntimeValue::Str(v.clone())),
        MachineInstr::LoadSlot(slot) => {
            let Some(value) = slots.get(slot).cloned() else {
                return Err(runtime_err("load_slot em slot não inicializado"));
            };
            stack.push(value);
        }
        MachineInstr::LoadGlobal(name) => {
            let Some(value) = globals.get(name).cloned() else {
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
                RuntimeValue::Str(_) => unreachable!("pop_numeric só retorna inteiro"),
            };
            stack.push(out);
        }
        MachineInstr::Not => {
            let value = pop_bool(stack, "not exige lógica no topo")?;
            stack.push(RuntimeValue::Bool(!value));
        }
        MachineInstr::BitNot => {
            let value = pop_numeric(stack, "bitnot exige inteiro no topo")?;
            let out = match value {
                RuntimeValue::Int(v) => RuntimeValue::Int(!v),
                RuntimeValue::IntSigned(v) => RuntimeValue::IntSigned(!v),
                RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Str(_) => unreachable!("pop_numeric só retorna inteiro"),
            };
            stack.push(out);
        }
        MachineInstr::DerefLoad { is_volatile, .. } => {
            let ptr = pop(stack, "deref_load exige ponteiro no topo")?;
            let RuntimeValue::Ptr(addr) = ptr else {
                return Err(runtime_err("deref_load exige ponteiro no topo"));
            };
            let loaded = if *is_volatile {
                deref_load_fragil(memory, addr)
            } else {
                deref_load_normal(memory, addr)
            };
            let Some(value) = loaded else {
                return Err(runtime_err(
                    "deref_load em endereço inválido ou não inicializado",
                ));
            };
            stack.push(value);
        }
        MachineInstr::DerefStore { ty, is_volatile } => {
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
            if *is_volatile {
                deref_store_fragil(memory, addr, coerced);
            } else {
                deref_store_normal(memory, addr, coerced);
            }
        }
        MachineInstr::Cast { ty } => {
            let value = pop(stack, "cast exige valor no topo")?;
            let casted = coerce_runtime_value_to_type(value, *ty)?;
            stack.push(casted);
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
            let result = match try_call_intrinsic(callee, &args, io_state)? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => {
                    call_function(callee, args, program, globals, memory, io_state, call_stack)?
                }
            };
            let Some(value) = result else {
                return Err(runtime_err("call exige função com retorno"));
            };
            stack.push(value);
        }
        MachineInstr::CallVoid { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = match try_call_intrinsic(callee, &args, io_state)? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => {
                    call_function(callee, args, program, globals, memory, io_state, call_stack)?
                }
            };
            if result.is_some() {
                return Err(runtime_err("call_void exige função sem retorno"));
            }
        }
        MachineInstr::PrintIntInline => {
            match pop_numeric(stack, "print_int_inline exige inteiro no topo")? {
                RuntimeValue::Int(v) => print!("{}", v),
                RuntimeValue::IntSigned(v) => print!("{}", v),
                RuntimeValue::Ptr(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Bool(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::Str(_) => unreachable!("pop_numeric só retorna inteiro"),
            }
        }
        MachineInstr::PrintBoolInline => {
            let v = pop_bool(stack, "print_bool_inline exige lógica no topo")?;
            print!("{}", if v { "verdade" } else { "falso" });
        }
        MachineInstr::PrintStrValueInline => {
            let s = pop_str(stack, "print_str_value_inline exige verso no topo")?;
            print!("{}", s);
        }
        MachineInstr::PrintStrInline(s) => {
            print!("{}", s);
        }
        MachineInstr::PrintSpace => {
            print!(" ");
        }
        MachineInstr::PrintNewline => {
            println!();
        }
    }

    Ok(())
}

fn try_call_intrinsic(
    callee: &str,
    args: &[RuntimeValue],
    io_state: &mut RuntimeIoState,
) -> Result<IntrinsicCall, PinkerError> {
    match callee {
        "ouvir" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'ouvir' exige 0 argumentos"));
            }
            let mut raw = String::new();
            io::stdin()
                .read_line(&mut raw)
                .map_err(|err| runtime_err(&format!("falha ao ler stdin em 'ouvir': {}", err)))?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(runtime_err(
                    "entrada inválida para 'ouvir': esperado inteiro bombom (u64), recebido vazio",
                ));
            }
            let parsed = trimmed.parse::<u64>().map_err(|_| {
                runtime_err(&format!(
                    "entrada inválida para 'ouvir': '{}' não é bombom válido",
                    trimmed
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "abrir" => {
            if args.len() != 1 {
                return Err(runtime_err("intrínseca 'abrir' exige 1 argumento (verso)"));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'abrir' exige caminho em verso"));
            };
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!("falha ao abrir arquivo em 'abrir': {}", err))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "criar_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'criar_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'criar_arquivo' exige caminho em verso",
                ));
            };
            fs::write(path, "").map_err(|err| {
                runtime_err(&format!(
                    "falha ao criar arquivo em 'criar_arquivo': {}",
                    err
                ))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content: String::new(),
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "ler_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'ler_arquivo' exige handle bombom"));
            };
            let Some(open_file) = io_state.open_files.get(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'ler_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'ler_arquivo'"));
            };
            let trimmed = open_file.content.trim();
            if trimmed.is_empty() {
                return Err(runtime_err(
                    "conteúdo inválido para 'ler_arquivo': esperado inteiro bombom (u64), recebido vazio",
                ));
            }
            let parsed = trimmed.parse::<u64>().map_err(|_| {
                runtime_err(&format!(
                    "conteúdo inválido para 'ler_arquivo': '{}' não é bombom válido",
                    trimmed
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(parsed))))
        }
        "ler_verso_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_verso_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_verso_arquivo' exige handle bombom",
                ));
            };
            let Some(open_file) = io_state.open_files.get(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'ler_verso_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'ler_verso_arquivo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                open_file.content.clone(),
            ))))
        }
        "escrever" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'escrever' exige 2 argumentos (handle, bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'escrever' exige handle bombom"));
            };
            let RuntimeValue::Int(value) = args[1] else {
                return Err(runtime_err("intrínseca 'escrever' exige valor bombom"));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'escrever'"));
                }
                return Err(runtime_err("handle inválido em 'escrever'"));
            };
            let next_content = value.to_string();
            fs::write(&open_file.path, &next_content).map_err(|err| {
                runtime_err(&format!("falha ao escrever arquivo em 'escrever': {}", err))
            })?;
            open_file.content = next_content;
            Ok(IntrinsicCall::Done(None))
        }
        "escrever_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige 2 argumentos (handle, verso)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige handle bombom",
                ));
            };
            let RuntimeValue::Str(value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'escrever_verso' exige valor em verso",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'escrever_verso'"));
                }
                return Err(runtime_err("handle inválido em 'escrever_verso'"));
            };
            fs::write(&open_file.path, value).map_err(|err| {
                runtime_err(&format!(
                    "falha ao escrever verso em arquivo em 'escrever_verso': {}",
                    err
                ))
            })?;
            open_file.content.clone_from(value);
            Ok(IntrinsicCall::Done(None))
        }
        "truncar_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'truncar_arquivo' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'truncar_arquivo' exige handle bombom",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'truncar_arquivo'"));
                }
                return Err(runtime_err("handle inválido em 'truncar_arquivo'"));
            };
            fs::write(&open_file.path, "").map_err(|err| {
                runtime_err(&format!(
                    "falha ao truncar arquivo em 'truncar_arquivo': {}",
                    err
                ))
            })?;
            open_file.content.clear();
            Ok(IntrinsicCall::Done(None))
        }
        "fechar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'fechar' exige 1 argumento (handle)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'fechar' exige handle bombom"));
            };
            if io_state.open_files.remove(&handle).is_none() {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'fechar'"));
                }
                return Err(runtime_err("handle inválido em 'fechar'"));
            }
            io_state.closed_handles.insert(handle);
            Ok(IntrinsicCall::Done(None))
        }
        "juntar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(lhs) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(rhs) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(format!(
                "{}{}",
                lhs, rhs
            )))))
        }
        "tamanho_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tamanho_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tamanho_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                value.chars().count() as u64,
            ))))
        }
        "indice_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige 2 argumentos (verso, bombom)",
                ));
            }
            let RuntimeValue::Str(value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso' exige segundo argumento em bombom",
                ));
            };
            let Some(ch) = value.chars().nth(index as usize) else {
                return Err(runtime_err(
                    "índice fora da faixa em 'indice_verso' para o verso informado",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(ch.to_string()))))
        }
        "contem_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(trecho) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'contem_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.contains(trecho),
            ))))
        }
        "comeca_com" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(prefixo) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'comeca_com' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.starts_with(prefixo),
            ))))
        }
        "termina_com" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sufixo) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'termina_com' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.ends_with(sufixo),
            ))))
        }
        "igual_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(lhs) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(rhs) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'igual_verso' exige segundo argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(lhs == rhs))))
        }
        "vazio_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'vazio_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'vazio_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                texto.is_empty(),
            ))))
        }
        "aparar_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aparar_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aparar_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.trim().to_string(),
            ))))
        }
        "minusculo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'minusculo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'minusculo_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.to_lowercase(),
            ))))
        }
        "maiusculo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'maiusculo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'maiusculo_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                texto.to_uppercase(),
            ))))
        }
        "argumento" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'argumento' exige 1 argumento (índice bombom)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err("intrínseca 'argumento' exige índice bombom"));
            };
            let Some(arg) = io_state.cli_args.get(index as usize) else {
                return Err(runtime_err("índice fora da faixa em 'argumento'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(arg.clone()))))
        }
        "argumento_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'argumento_ou' exige 2 argumentos (índice bombom, padrão verso)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err("intrínseca 'argumento_ou' exige índice bombom"));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'argumento_ou' exige valor padrão em verso",
                ));
            };
            let value = io_state
                .cli_args
                .get(index as usize)
                .cloned()
                .unwrap_or_else(|| default_value.clone());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        "ambiente_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'ambiente_ou' exige 2 argumentos (chave verso, padrão verso)",
                ));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err("intrínseca 'ambiente_ou' exige chave em verso"));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'ambiente_ou' exige valor padrão em verso",
                ));
            };
            let value = env::var(key).unwrap_or_else(|_| default_value.clone());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
        }
        "caminho_existe" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'caminho_existe' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'caminho_existe' exige caminho em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).exists(),
            ))))
        }
        "e_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'e_arquivo' exige caminho em verso"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).is_file(),
            ))))
        }
        "e_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'e_diretorio' exige caminho em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                std::path::Path::new(path).is_dir(),
            ))))
        }
        "juntar_caminho" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige 2 argumentos (base verso, trecho verso)",
                ));
            }
            let RuntimeValue::Str(base) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige base em verso",
                ));
            };
            let RuntimeValue::Str(child) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_caminho' exige trecho em verso",
                ));
            };
            let joined = std::path::PathBuf::from(base).join(child);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                joined.to_string_lossy().to_string(),
            ))))
        }
        "tamanho_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige caminho em verso",
                ));
            };
            let metadata = fs::metadata(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao obter metadados em 'tamanho_arquivo': {}",
                    err
                ))
            })?;
            if !metadata.is_file() {
                return Err(runtime_err(
                    "intrínseca 'tamanho_arquivo' exige caminho de arquivo regular",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(metadata.len()))))
        }
        "e_vazio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'e_vazio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err("intrínseca 'e_vazio' exige caminho em verso"));
            };
            let metadata = fs::metadata(path).map_err(|err| {
                runtime_err(&format!("falha ao obter metadados em 'e_vazio': {}", err))
            })?;
            if !metadata.is_file() {
                return Err(runtime_err(
                    "intrínseca 'e_vazio' exige caminho de arquivo regular",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                metadata.len() == 0,
            ))))
        }
        "criar_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'criar_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'criar_diretorio' exige caminho em verso",
                ));
            };
            fs::create_dir(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao criar diretório em 'criar_diretorio': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "remover_arquivo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'remover_arquivo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'remover_arquivo' exige caminho em verso",
                ));
            };
            fs::remove_file(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao remover arquivo em 'remover_arquivo': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "remover_diretorio" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'remover_diretorio' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'remover_diretorio' exige caminho em verso",
                ));
            };
            fs::remove_dir(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao remover diretório em 'remover_diretorio': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(None))
        }
        "diretorio_atual" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'diretorio_atual' exige 0 argumentos",
                ));
            }
            let value = env::current_dir().map_err(|err| {
                runtime_err(&format!(
                    "falha ao obter diretório atual em 'diretorio_atual': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                value.to_string_lossy().to_string(),
            ))))
        }
        "quantos_argumentos" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'quantos_argumentos' exige 0 argumentos",
                ));
            }
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                io_state.cli_args.len() as u64,
            ))))
        }
        "tem_argumento" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tem_argumento' exige 1 argumento (índice bombom)",
                ));
            }
            let RuntimeValue::Int(index) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'tem_argumento' exige índice bombom",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                io_state.cli_args.get(index as usize).is_some(),
            ))))
        }
        "sair" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'sair' exige 1 argumento (código bombom)",
                ));
            }
            let RuntimeValue::Int(code) = args[0] else {
                return Err(runtime_err("intrínseca 'sair' exige código bombom"));
            };
            io_state.exit_status = Some(code.min(i32::MAX as u64) as i32);
            Ok(IntrinsicCall::Done(None))
        }
        _ => Ok(IntrinsicCall::NotIntrinsic),
    }
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
        RuntimeValue::Str(_) => Err(runtime_err(msg)),
    }
}

fn pop_bool(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<bool, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Bool(v) => Ok(v),
        RuntimeValue::Int(_) => Err(runtime_err(msg)),
        RuntimeValue::IntSigned(_) => Err(runtime_err(msg)),
        RuntimeValue::Ptr(_) => Err(runtime_err(msg)),
        RuntimeValue::Str(_) => Err(runtime_err(msg)),
    }
}

fn pop_str(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<String, PinkerError> {
    let value = pop(stack, msg)?;
    match value {
        RuntimeValue::Str(v) => Ok(v),
        _ => Err(runtime_err(msg)),
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
            (RuntimeValue::Ptr(v), true) => Ok(RuntimeValue::IntSigned(v as i64)),
            (RuntimeValue::Ptr(v), false) => Ok(RuntimeValue::Int(v as u64)),
            (RuntimeValue::Str(_), _) => Err(runtime_err("cast inteiro não aceita verso")),
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
            RuntimeValue::Str(_) => Err(runtime_err(
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
    match (&lhs, &rhs) {
        (RuntimeValue::Int(_), RuntimeValue::Int(_))
        | (RuntimeValue::IntSigned(_), RuntimeValue::IntSigned(_)) => Ok((lhs, rhs)),
        // lhs signed, rhs unsigned: converte rhs para signed preservando ordem
        (RuntimeValue::IntSigned(a), RuntimeValue::Int(b)) => {
            if *b > i64::MAX as u64 {
                return Err(runtime_err(
                    "mistura signed/unsigned fora de faixa no runtime (sem coerção implícita)",
                ));
            }
            Ok((
                RuntimeValue::IntSigned(*a),
                RuntimeValue::IntSigned(*b as i64),
            ))
        }
        // lhs unsigned, rhs signed: converte lhs para signed preservando ordem
        (RuntimeValue::Int(a), RuntimeValue::IntSigned(b)) => {
            if *a > i64::MAX as u64 {
                return Err(runtime_err(
                    "mistura signed/unsigned fora de faixa no runtime (sem coerção implícita)",
                ));
            }
            Ok((
                RuntimeValue::IntSigned(*a as i64),
                RuntimeValue::IntSigned(*b),
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

fn deref_load_normal(memory: &HashMap<usize, RuntimeValue>, addr: usize) -> Option<RuntimeValue> {
    memory.get(&addr).cloned()
}

fn deref_load_fragil(memory: &HashMap<usize, RuntimeValue>, addr: usize) -> Option<RuntimeValue> {
    memory.get(&addr).cloned()
}

fn deref_store_normal(memory: &mut HashMap<usize, RuntimeValue>, addr: usize, value: RuntimeValue) {
    memory.insert(addr, value);
}

fn deref_store_fragil(memory: &mut HashMap<usize, RuntimeValue>, addr: usize, value: RuntimeValue) {
    memory.insert(addr, value);
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
    } else if msg.contains("handle já fechado") {
        (
            "handle_ja_fechado",
            Some("o handle já foi fechado com 'fechar'; abra novamente com 'abrir' ou 'criar_arquivo' se necessário"),
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
        MachineInstr::PushStr(_) => "push_str",
        MachineInstr::LoadSlot(_) => "load_slot",
        MachineInstr::LoadGlobal(_) => "load_global",
        MachineInstr::StoreSlot(_) => "store_slot",
        MachineInstr::Neg => "neg",
        MachineInstr::Not => "not",
        MachineInstr::BitNot => "bitnot",
        MachineInstr::DerefLoad { is_volatile, .. } => {
            if *is_volatile {
                "deref_load_fragil"
            } else {
                "deref_load"
            }
        }
        MachineInstr::DerefStore { is_volatile, .. } => {
            if *is_volatile {
                "deref_store_fragil"
            } else {
                "deref_store"
            }
        }
        MachineInstr::Cast { .. } => "cast",
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
        MachineInstr::PrintIntInline => "print_int_inline",
        MachineInstr::PrintBoolInline => "print_bool_inline",
        MachineInstr::PrintStrValueInline => "print_str_value_inline",
        MachineInstr::PrintStrInline(_) => "print_str_inline",
        MachineInstr::PrintSpace => "print_space",
        MachineInstr::PrintNewline => "print_newline",
    }
}
