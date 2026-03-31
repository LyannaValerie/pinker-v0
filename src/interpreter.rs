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
use std::fs::OpenOptions;
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

enum NamedArgLookup<'a> {
    Missing,
    PresentWithoutValue,
    PresentValue(&'a str),
}

struct RuntimeIoState {
    open_files: HashMap<u64, RuntimeOpenFile>,
    next_file_handle: u64,
    closed_handles: std::collections::HashSet<u64>,
    cli_args: Vec<String>,
    exit_status: Option<i32>,
}

struct RuntimeListState {
    lists_bombom: HashMap<u64, Vec<u64>>,
    next_list_handle: u64,
}

struct RuntimeMapState {
    maps_verso_bombom: HashMap<u64, HashMap<String, u64>>,
    next_map_handle: u64,
    map_iters_verso_bombom: HashMap<u64, RuntimeMapVersoBombomIter>,
    next_map_iter_handle: u64,
}

struct RuntimeRandomState {
    generators: HashMap<u64, RuntimeRandomGenerator>,
    next_generator_handle: u64,
}

struct RuntimeRandomGenerator {
    state: u64,
}

struct RuntimeMapVersoBombomIter {
    keys_snapshot: Vec<String>,
    next_index: usize,
}

struct RuntimeOpenFile {
    path: String,
    content: String,
    append_enabled: bool,
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
    ListBombom(u64),
    MapVersoBombom(u64),
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
    let mut list_state = RuntimeListState {
        lists_bombom: HashMap::new(),
        next_list_handle: 1,
    };
    let mut map_state = RuntimeMapState {
        maps_verso_bombom: HashMap::new(),
        next_map_handle: 1,
        map_iters_verso_bombom: HashMap::new(),
        next_map_iter_handle: 1,
    };
    let mut random_state = RuntimeRandomState {
        generators: HashMap::new(),
        next_generator_handle: 1,
    };
    let mut call_stack = Vec::new();
    let return_value = call_function(
        "principal",
        vec![],
        program,
        &globals,
        &mut memory,
        &mut io_state,
        &mut list_state,
        &mut map_state,
        &mut random_state,
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
#[allow(clippy::too_many_arguments)]
fn call_function(
    fn_name: &str,
    args: Vec<RuntimeValue>,
    program: &MachineProgram,
    globals: &HashMap<String, RuntimeValue>,
    memory: &mut HashMap<usize, RuntimeValue>,
    io_state: &mut RuntimeIoState,
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
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
                    instr,
                    &mut slots,
                    &mut stack,
                    program,
                    globals,
                    memory,
                    io_state,
                    list_state,
                    map_state,
                    random_state,
                    call_stack,
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
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
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
                RuntimeValue::ListBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
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
                RuntimeValue::ListBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
            };
            stack.push(out);
        }
        MachineInstr::DerefLoad {
            ty, is_volatile, ..
        } => {
            let ptr = pop(stack, "deref_load exige ponteiro no topo")?;
            let RuntimeValue::Ptr(addr) = ptr else {
                return Err(runtime_err("deref_load exige ponteiro no topo"));
            };
            if matches!(ty, crate::ir::TypeIR::FixedArray { .. }) {
                stack.push(RuntimeValue::Ptr(addr));
                return Ok(());
            }
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
            let result = match try_call_intrinsic(
                callee,
                &args,
                io_state,
                list_state,
                map_state,
                random_state,
            )? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => call_function(
                    callee,
                    args,
                    program,
                    globals,
                    memory,
                    io_state,
                    list_state,
                    map_state,
                    random_state,
                    call_stack,
                )?,
            };
            let Some(value) = result else {
                return Err(runtime_err("call exige função com retorno"));
            };
            stack.push(value);
        }
        MachineInstr::CallVoid { callee, argc } => {
            let args = pop_args(stack, *argc)?;
            let result = match try_call_intrinsic(
                callee,
                &args,
                io_state,
                list_state,
                map_state,
                random_state,
            )? {
                IntrinsicCall::Done(value) => value,
                IntrinsicCall::NotIntrinsic => call_function(
                    callee,
                    args,
                    program,
                    globals,
                    memory,
                    io_state,
                    list_state,
                    map_state,
                    random_state,
                    call_stack,
                )?,
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
                RuntimeValue::ListBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
                RuntimeValue::MapVersoBombom(_) => unreachable!("pop_numeric só retorna inteiro"),
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
    list_state: &mut RuntimeListState,
    map_state: &mut RuntimeMapState,
    random_state: &mut RuntimeRandomState,
) -> Result<IntrinsicCall, PinkerError> {
    match callee {
        "aleatorio_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_criar' exige 1 argumento (semente bombom)",
                ));
            }
            let RuntimeValue::Int(seed) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_criar' exige semente bombom",
                ));
            };
            let handle = random_state.next_generator_handle;
            random_state.next_generator_handle =
                random_state.next_generator_handle.saturating_add(1);
            random_state
                .generators
                .insert(handle, RuntimeRandomGenerator { state: seed });
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "aleatorio_proximo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_proximo' exige 1 argumento (gerador bombom)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'aleatorio_proximo' exige gerador bombom",
                ));
            };
            let Some(generator) = random_state.generators.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de aleatoriedade inválido em 'aleatorio_proximo'",
                ));
            };
            let next = advance_random_generator(&mut generator.state);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(next))))
        }
        "lista_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = list_state.next_list_handle;
            list_state.next_list_handle = list_state.next_list_handle.saturating_add(1);
            list_state.lists_bombom.insert(handle, Vec::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListBombom(handle))))
        }
        "lista_bombom_anexar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige 2 argumentos (lista<bombom>, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_anexar' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            lista.push(value);
            Ok(IntrinsicCall::Done(None))
        }
        "lista_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige 2 argumentos (lista<bombom>, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_obter' exige bombom no segundo argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(value) = lista.get(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_bombom_obter'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "lista_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tamanho' exige 1 argumento (lista<bombom>)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tamanho' exige lista<bombom> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                lista.len() as u64
            ))))
        }
        "lista_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige 3 argumentos (lista<bombom>, bombom, bombom)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Int(index) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige bombom no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(slot) = lista.get_mut(index as usize) else {
                return Err(runtime_err(
                    "índice fora do intervalo em 'lista_bombom_definir'",
                ));
            };
            *slot = value;
            Ok(IntrinsicCall::Done(None))
        }
        "lista_bombom_tirar_ultimo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tirar_ultimo' exige 1 argumento (lista<bombom>)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'lista_bombom_tirar_ultimo' exige lista<bombom> no argumento",
                ));
            };
            let Some(lista) = list_state.lists_bombom.get_mut(&handle) else {
                return Err(runtime_err("handle de lista<bombom> inválido em runtime"));
            };
            let Some(value) = lista.pop() else {
                return Err(runtime_err("lista vazia em 'lista_bombom_tirar_ultimo'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(value))))
        }
        "mapa_verso_bombom_criar" => {
            if !args.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_criar' exige 0 argumentos",
                ));
            }
            let handle = map_state.next_map_handle;
            map_state.next_map_handle = map_state.next_map_handle.saturating_add(1);
            map_state.maps_verso_bombom.insert(handle, HashMap::new());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoBombom(
                handle,
            ))))
        }
        "mapa_verso_bombom_definir" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige 3 argumentos (mapa<verso,bombom>, verso, bombom)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige verso no segundo argumento",
                ));
            };
            let RuntimeValue::Int(value) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_definir' exige bombom no terceiro argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get_mut(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_definir'",
                ));
            };
            mapa.insert(key.clone(), value);
            Ok(IntrinsicCall::Done(None))
        }
        "mapa_verso_bombom_obter" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige 2 argumentos (mapa<verso,bombom>, verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_obter' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_obter'",
                ));
            };
            let Some(value) = mapa.get(key) else {
                return Err(runtime_err("chave ausente em 'mapa_verso_bombom_obter'"));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(*value))))
        }
        "mapa_verso_bombom_tem" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige 2 argumentos (mapa<verso,bombom>, verso)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige mapa<verso,bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(ref key) = args[1] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tem' exige verso no segundo argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_tem'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                mapa.contains_key(key),
            ))))
        }
        "mapa_verso_bombom_tamanho" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tamanho' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'mapa_verso_bombom_tamanho' exige mapa<verso,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'mapa_verso_bombom_tamanho'",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(
                mapa.len() as u64
            ))))
        }
        "__pinker_internal_mapa_verso_bombom_iterador_criar" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_criar' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_criar' exige mapa<verso,bombom> no argumento",
                ));
            };
            let handle = *handle;
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em '__pinker_internal_mapa_verso_bombom_iterador_criar'",
                ));
            };
            let iter_handle = map_state.next_map_iter_handle;
            map_state.next_map_iter_handle = map_state.next_map_iter_handle.saturating_add(1);
            map_state.map_iters_verso_bombom.insert(
                iter_handle,
                RuntimeMapVersoBombomIter {
                    keys_snapshot: mapa.keys().cloned().collect(),
                    next_index: 0,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(iter_handle))))
        }
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave' exige 1 argumento (cursor)",
                ));
            };
            let RuntimeValue::Int(iter_handle) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca interna '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave' exige cursor 'bombom'",
                ));
            };
            let Some(iter) = map_state.map_iters_verso_bombom.get_mut(iter_handle) else {
                return Err(runtime_err(
                    "cursor interno de mapa inválido em '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave'",
                ));
            };
            let key = iter.keys_snapshot.get(iter.next_index).ok_or_else(|| {
                runtime_err(
                    "cursor interno de mapa esgotado em '__pinker_internal_mapa_verso_bombom_iterador_proxima_chave'",
                )
            })?;
            iter.next_index = iter.next_index.saturating_add(1);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(key.clone()))))
        }
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
        "ouvir_verso" => {
            if !args.is_empty() {
                return Err(runtime_err("intrínseca 'ouvir_verso' exige 0 argumentos"));
            }
            let maybe_line = read_stdin_line_minima("ouvir_verso")?;
            let Some(line) = maybe_line else {
                return Err(runtime_err(
                    "falha ao ler stdin em 'ouvir_verso': EOF imediato sem linha disponível",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                trim_final_newline_minimo(line),
            ))))
        }
        "ouvir_verso_ou" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ouvir_verso_ou' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(default_value) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ouvir_verso_ou' exige valor padrão em verso",
                ));
            };
            match read_stdin_line_minima("ouvir_verso_ou") {
                Ok(Some(line)) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    trim_final_newline_minimo(line),
                )))),
                Ok(None) | Err(_) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
            }
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
                    append_enabled: false,
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
                    append_enabled: false,
                },
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(handle))))
        }
        "abrir_anexo" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'abrir_anexo' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'abrir_anexo' exige caminho em verso",
                ));
            };
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .map_err(|err| {
                    runtime_err(&format!("falha ao abrir arquivo em 'abrir_anexo': {}", err))
                })?;
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao carregar conteúdo em 'abrir_anexo': {}",
                    err
                ))
            })?;
            let handle = io_state.next_file_handle;
            io_state.next_file_handle = io_state.next_file_handle.saturating_add(1);
            io_state.open_files.insert(
                handle,
                RuntimeOpenFile {
                    path: path.clone(),
                    content,
                    append_enabled: true,
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
        "ler_arquivo_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_arquivo_verso' exige caminho em verso",
                ));
            };
            let content = fs::read_to_string(path).map_err(|err| {
                runtime_err(&format!(
                    "falha ao ler arquivo em 'ler_arquivo_verso': {}",
                    err
                ))
            })?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(content))))
        }
        "arquivo_ou" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(path) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige caminho em verso",
                ));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'arquivo_ou' exige valor padrão em verso",
                ));
            };
            match fs::read_to_string(path) {
                Ok(content) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(content)))),
                Err(_) => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
            }
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
        "anexar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'anexar_verso' exige 2 argumentos (handle, verso)",
                ));
            }
            let RuntimeValue::Int(handle) = args[0] else {
                return Err(runtime_err("intrínseca 'anexar_verso' exige handle bombom"));
            };
            let RuntimeValue::Str(value) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'anexar_verso' exige valor em verso",
                ));
            };
            let Some(open_file) = io_state.open_files.get_mut(&handle) else {
                if io_state.closed_handles.contains(&handle) {
                    return Err(runtime_err("handle já fechado em 'anexar_verso'"));
                }
                return Err(runtime_err("handle inválido em 'anexar_verso'"));
            };
            if !open_file.append_enabled {
                return Err(runtime_err(
                    "handle não foi aberto com 'abrir_anexo' em 'anexar_verso'",
                ));
            }
            let mut file = OpenOptions::new()
                .append(true)
                .open(&open_file.path)
                .map_err(|err| {
                    runtime_err(&format!(
                        "falha ao anexar verso em arquivo em 'anexar_verso': {}",
                        err
                    ))
                })?;
            use std::io::Write as _;
            file.write_all(value.as_bytes()).map_err(|err| {
                runtime_err(&format!(
                    "falha ao anexar verso em arquivo em 'anexar_verso': {}",
                    err
                ))
            })?;
            open_file.content.push_str(value);
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
        "indice_verso_em" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(trecho) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'indice_verso_em' exige segundo argumento em verso",
                ));
            };
            let pos = texto.find(trecho).map_or(u64::MAX, |v| v as u64);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(pos))))
        }
        // Fase 140 — buscar_verso(texto, padrao) -> bombom
        "buscar_verso" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(padrao) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' exige segundo argumento em verso",
                ));
            };
            if padrao.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'buscar_verso' não aceita padrão vazio",
                ));
            }
            let pos = texto.find(padrao).map_or(u64::MAX, |v| v as u64);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(pos))))
        }
        "nao_vazio_verso" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'nao_vazio_verso' exige 1 argumento (verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'nao_vazio_verso' exige argumento em verso",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(
                !texto.is_empty(),
            ))))
        }
        // Fase 137 — dividir_verso_em(texto, sep, indice) -> verso
        "dividir_verso_em" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige 3 argumentos (verso, verso, bombom)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Int(indice) = args[2] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' exige terceiro argumento em bombom",
                ));
            };
            if sep.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_em' não aceita separador vazio",
                ));
            }
            let partes: Vec<&str> = texto.split(sep.as_str()).collect();
            let Some(parte) = partes.get(indice as usize) else {
                return Err(runtime_err(
                    "índice fora da faixa em 'dividir_verso_em' para o verso informado",
                ));
            };
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                parte.to_string(),
            ))))
        }
        // Fase 137 — dividir_verso_contar(texto, sep) -> bombom
        "dividir_verso_contar" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige 2 argumentos (verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' exige segundo argumento em verso",
                ));
            };
            if sep.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'dividir_verso_contar' não aceita separador vazio",
                ));
            }
            let count = texto.split(sep.as_str()).count() as u64;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Int(count))))
        }
        // Fase 138 — substituir_verso(texto, de, para) -> verso
        "substituir_verso" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige 3 argumentos (verso, verso, verso)",
                ));
            }
            let RuntimeValue::Str(texto) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(de) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Str(para) = &args[2] else {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' exige terceiro argumento em verso",
                ));
            };
            if de.is_empty() {
                return Err(runtime_err(
                    "intrínseca 'substituir_verso' não aceita padrão vazio",
                ));
            }
            let resultado = texto.replace(de.as_str(), para.as_str());
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        // Fase 139 — juntar_verso_com(a, sep, b) -> verso
        "juntar_verso_com" => {
            if args.len() != 3 {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige 3 argumentos (verso, verso, verso)",
                ));
            }
            let RuntimeValue::Str(a) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige primeiro argumento em verso",
                ));
            };
            let RuntimeValue::Str(sep) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige segundo argumento em verso",
                ));
            };
            let RuntimeValue::Str(b) = &args[2] else {
                return Err(runtime_err(
                    "intrínseca 'juntar_verso_com' exige terceiro argumento em verso",
                ));
            };
            let resultado = format!("{}{}{}", a, sep, b);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        "formatar_verso" => {
            if !(args.len() == 2 || args.len() == 3) {
                return Err(runtime_err(
                    "intrínseca 'formatar_verso' exige 2 ou 3 argumentos (modelo verso, bombom/verso[, bombom/verso])",
                ));
            }
            let RuntimeValue::Str(modelo) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'formatar_verso' exige modelo em verso",
                ));
            };
            let resultado = formatar_verso_runtime(modelo, &args[1..])?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(resultado))))
        }
        "ler_linha_csv_bombom" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige 2 argumentos (linha verso, separador verso)",
                ));
            }
            let RuntimeValue::Str(linha) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige linha em verso",
                ));
            };
            let RuntimeValue::Str(separador) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'ler_linha_csv_bombom' exige separador em verso",
                ));
            };
            let separador = validar_separador_csv("ler_linha_csv_bombom", separador)?;
            if linha.contains('\n') || linha.contains('\r') {
                return Err(runtime_err(
                    "linha inválida em 'ler_linha_csv_bombom': multiline fora do recorte",
                ));
            }
            if linha.contains('"') {
                return Err(runtime_err(
                    "linha inválida em 'ler_linha_csv_bombom': quoting fora do recorte",
                ));
            }

            let handle = list_state.next_list_handle;
            list_state.next_list_handle += 1;
            let mut itens = Vec::new();
            for campo in linha.split(separador) {
                let Ok(valor) = campo.parse::<u64>() else {
                    return Err(runtime_err(
                        "campo inválido em 'ler_linha_csv_bombom': esperado bombom simples sem quoting",
                    ));
                };
                itens.push(valor);
            }
            list_state.lists_bombom.insert(handle, itens);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::ListBombom(handle))))
        }
        "emitir_linha_csv_bombom" => {
            if args.len() != 2 {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige 2 argumentos (lista<bombom>, separador verso)",
                ));
            }
            let RuntimeValue::ListBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige lista<bombom> no primeiro argumento",
                ));
            };
            let RuntimeValue::Str(separador) = &args[1] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_linha_csv_bombom' exige separador em verso no segundo argumento",
                ));
            };
            let separador = validar_separador_csv("emitir_linha_csv_bombom", separador)?;
            let Some(itens) = list_state.lists_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de lista inválido em 'emitir_linha_csv_bombom'",
                ));
            };
            let linha = itens
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(separador);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(linha))))
        }
        "ler_json_plano_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'ler_json_plano_bombom' exige 1 argumento (json verso)",
                ));
            }
            let RuntimeValue::Str(json) = &args[0] else {
                return Err(runtime_err(
                    "intrínseca 'ler_json_plano_bombom' exige json em verso",
                ));
            };
            let handle = map_state.next_map_handle;
            map_state.next_map_handle += 1;
            let mapa = parse_json_plano_bombom(json)?;
            map_state.maps_verso_bombom.insert(handle, mapa);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::MapVersoBombom(
                handle,
            ))))
        }
        "emitir_json_plano_bombom" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'emitir_json_plano_bombom' exige 1 argumento (mapa<verso,bombom>)",
                ));
            }
            let RuntimeValue::MapVersoBombom(handle) = args[0] else {
                return Err(runtime_err(
                    "intrínseca 'emitir_json_plano_bombom' exige mapa<verso,bombom> no argumento",
                ));
            };
            let Some(mapa) = map_state.maps_verso_bombom.get(&handle) else {
                return Err(runtime_err(
                    "handle de mapa<verso,bombom> inválido em 'emitir_json_plano_bombom'",
                ));
            };
            let json = emit_json_plano_bombom(mapa)?;
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(json))))
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
        intrinsic_name @ ("tem_chave" | "tem_argumento_nomeado") => {
            if args.len() != 1 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 1 argumento (chave verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, key)?;
            let found = matches!(
                find_named_cli_argument(&io_state.cli_args, key),
                NamedArgLookup::PresentValue(_)
            );
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(found))))
        }
        intrinsic_name @ ("pedir_argumento" | "argumento_nomeado_ou") => {
            if args.len() != 2 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 2 argumentos (chave verso, padrão verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(default_value) = &args[1] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige valor padrão em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, key)?;
            match find_named_cli_argument(&io_state.cli_args, key) {
                NamedArgLookup::Missing => Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(
                    default_value.clone(),
                )))),
                NamedArgLookup::PresentValue(value) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(value.to_string()),
                ))),
                NamedArgLookup::PresentWithoutValue => Err(runtime_err(&format!(
                    "intrínseca '{}' encontrou chave '{}' sem valor na forma '--chave valor'",
                    intrinsic_name, key
                ))),
            }
        }
        "tem_flag" => {
            if args.len() != 1 {
                return Err(runtime_err(
                    "intrínseca 'tem_flag' exige 1 argumento (chave verso)",
                ));
            }
            let RuntimeValue::Str(key) = &args[0] else {
                return Err(runtime_err("intrínseca 'tem_flag' exige chave em verso"));
            };
            ensure_named_arg_key_valid("tem_flag", key)?;
            let found = io_state.cli_args.iter().any(|a| a == key);
            Ok(IntrinsicCall::Done(Some(RuntimeValue::Bool(found))))
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
        intrinsic_name @ ("buscar_contexto" | "argumento_nomeado_ou_ambiente_ou") => {
            if args.len() != 3 {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige 3 argumentos (chave_arg verso, chave_env verso, padrão verso)",
                    intrinsic_name
                )));
            }
            let RuntimeValue::Str(arg_key) = &args[0] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave_arg em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(env_key) = &args[1] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige chave_env em verso",
                    intrinsic_name
                )));
            };
            let RuntimeValue::Str(default_value) = &args[2] else {
                return Err(runtime_err(&format!(
                    "intrínseca '{}' exige valor padrão em verso",
                    intrinsic_name
                )));
            };
            ensure_named_arg_key_valid(intrinsic_name, arg_key)?;
            ensure_env_key_valid(intrinsic_name, env_key)?;
            match find_named_cli_argument(&io_state.cli_args, arg_key) {
                NamedArgLookup::PresentValue(value) => Ok(IntrinsicCall::Done(Some(
                    RuntimeValue::Str(value.to_string()),
                ))),
                NamedArgLookup::PresentWithoutValue => Err(runtime_err(&format!(
                    "intrínseca '{}' encontrou chave '{}' sem valor na forma '--chave valor'",
                    intrinsic_name, arg_key
                ))),
                NamedArgLookup::Missing => {
                    let value = env::var(env_key).unwrap_or_else(|_| default_value.clone());
                    Ok(IntrinsicCall::Done(Some(RuntimeValue::Str(value))))
                }
            }
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

fn read_stdin_line_minima(intrinsic_name: &str) -> Result<Option<String>, PinkerError> {
    let mut raw = String::new();
    let bytes = io::stdin().read_line(&mut raw).map_err(|err| {
        runtime_err(&format!(
            "falha ao ler stdin em '{}': {}",
            intrinsic_name, err
        ))
    })?;
    if bytes == 0 {
        return Ok(None);
    }
    Ok(Some(raw))
}

fn advance_random_generator(state: &mut u64) -> u64 {
    // LCG mínimo e determinístico em u64, suficiente para o recorte auditável da fase.
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn ensure_named_arg_key_valid(intrinsic_name: &str, key: &str) -> Result<(), PinkerError> {
    if key.is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige chave não vazia",
            intrinsic_name
        )));
    }
    Ok(())
}

fn ensure_env_key_valid(intrinsic_name: &str, key: &str) -> Result<(), PinkerError> {
    if key.is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige chave de ambiente não vazia",
            intrinsic_name
        )));
    }
    Ok(())
}

fn formatar_verso_runtime(modelo: &str, args: &[RuntimeValue]) -> Result<String, PinkerError> {
    let mut saida = String::new();
    let mut ultimo_idx = 0usize;
    let mut arg_idx = 0usize;
    let mut chars = modelo.char_indices().peekable();

    while let Some((idx, ch)) = chars.next() {
        match ch {
            '{' => {
                saida.push_str(&modelo[ultimo_idx..idx]);
                let Some((close_idx, next_ch)) = chars.next() else {
                    return Err(runtime_err(
                        "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                    ));
                };
                if next_ch != '}' {
                    return Err(runtime_err(
                        "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                    ));
                }
                let Some(arg) = args.get(arg_idx) else {
                    return Err(runtime_err(
                        "quantidade de placeholders '{}' em 'formatar_verso' difere da quantidade de argumentos",
                    ));
                };
                saida.push_str(&formatar_verso_argumento(arg)?);
                arg_idx += 1;
                ultimo_idx = close_idx + next_ch.len_utf8();
            }
            '}' => {
                return Err(runtime_err(
                    "modelo inválido em 'formatar_verso': placeholders devem ser apenas '{}'",
                ));
            }
            _ => {}
        }
    }

    saida.push_str(&modelo[ultimo_idx..]);
    if arg_idx != args.len() {
        return Err(runtime_err(
            "quantidade de placeholders '{}' em 'formatar_verso' difere da quantidade de argumentos",
        ));
    }
    Ok(saida)
}

fn validar_separador_csv<'a>(
    intrinsic_name: &str,
    separador: &'a str,
) -> Result<&'a str, PinkerError> {
    if separador.is_empty() {
        return Err(runtime_err(&format!(
            "intrínseca '{}' não aceita separador vazio",
            intrinsic_name
        )));
    }
    if separador.chars().count() != 1 {
        return Err(runtime_err(&format!(
            "intrínseca '{}' exige separador de 1 caractere",
            intrinsic_name
        )));
    }
    if matches!(separador, "\"" | "\n" | "\r") {
        return Err(runtime_err(&format!(
            "intrínseca '{}' rejeita separador fora do recorte mínimo de CSV",
            intrinsic_name
        )));
    }
    Ok(separador)
}

fn parse_json_plano_bombom(json: &str) -> Result<HashMap<String, u64>, PinkerError> {
    let mut cursor = JsonPlanoCursor::new(json);
    cursor.skip_ws();
    cursor.expect_char('{')?;
    cursor.skip_ws();

    let mut mapa = HashMap::new();
    if cursor.consume_char('}') {
        cursor.skip_ws();
        cursor.ensure_eof()?;
        return Ok(mapa);
    }

    loop {
        cursor.skip_ws();
        let chave = cursor.parse_key()?;
        if mapa.contains_key(&chave) {
            return Err(runtime_err(
                "json inválido em 'ler_json_plano_bombom': chave duplicada fora do recorte auditável",
            ));
        }
        cursor.skip_ws();
        cursor.expect_char(':')?;
        cursor.skip_ws();
        let valor = cursor.parse_u64()?;
        mapa.insert(chave, valor);
        cursor.skip_ws();
        if cursor.consume_char('}') {
            cursor.skip_ws();
            cursor.ensure_eof()?;
            return Ok(mapa);
        }
        cursor.expect_char(',')?;
        cursor.skip_ws();
    }
}

fn emit_json_plano_bombom(mapa: &HashMap<String, u64>) -> Result<String, PinkerError> {
    let mut chaves = mapa.keys().cloned().collect::<Vec<_>>();
    chaves.sort();
    let mut partes = Vec::with_capacity(chaves.len());
    for chave in chaves {
        validar_chave_json_plana(&chave, "emitir_json_plano_bombom")?;
        let valor = mapa
            .get(&chave)
            .ok_or_else(|| runtime_err("mapa inconsistente em 'emitir_json_plano_bombom'"))?;
        partes.push(format!("\"{}\":{}", chave, valor));
    }
    Ok(format!("{{{}}}", partes.join(",")))
}

fn validar_chave_json_plana(chave: &str, nome: &str) -> Result<(), PinkerError> {
    if chave.contains('"') || chave.contains('\\') {
        return Err(runtime_err(&format!(
            "json inválido em '{}': chave exige escape fora do recorte",
            nome
        )));
    }
    if chave.chars().any(|ch| ch.is_control()) {
        return Err(runtime_err(&format!(
            "json inválido em '{}': chave contém controle fora do recorte",
            nome
        )));
    }
    Ok(())
}

struct JsonPlanoCursor<'a> {
    src: &'a str,
    idx: usize,
}

impl<'a> JsonPlanoCursor<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, idx: 0 }
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_whitespace() {
                self.idx += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.src[self.idx..].chars().next()
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.idx += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), PinkerError> {
        if self.consume_char(expected) {
            Ok(())
        } else {
            Err(runtime_err(&format!(
                "json inválido em 'ler_json_plano_bombom': esperado '{}'",
                expected
            )))
        }
    }

    fn parse_key(&mut self) -> Result<String, PinkerError> {
        self.expect_char('"')?;
        let inicio = self.idx;
        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    let chave = self.src[inicio..self.idx].to_string();
                    self.idx += 1;
                    validar_chave_json_plana(&chave, "ler_json_plano_bombom")?;
                    return Ok(chave);
                }
                '\\' => {
                    return Err(runtime_err(
                        "json inválido em 'ler_json_plano_bombom': escapes em chave fora do recorte",
                    ));
                }
                _ if ch.is_control() => {
                    return Err(runtime_err(
                        "json inválido em 'ler_json_plano_bombom': controle em chave fora do recorte",
                    ));
                }
                _ => {
                    self.idx += ch.len_utf8();
                }
            }
        }
        Err(runtime_err(
            "json inválido em 'ler_json_plano_bombom': string de chave não terminada",
        ))
    }

    fn parse_u64(&mut self) -> Result<u64, PinkerError> {
        let inicio = self.idx;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.idx += ch.len_utf8();
            } else {
                break;
            }
        }
        if inicio == self.idx {
            return Err(runtime_err(
                "json inválido em 'ler_json_plano_bombom': valor deve ser bombom sem sinal",
            ));
        }
        self.src[inicio..self.idx].parse::<u64>().map_err(|_| {
            runtime_err("json inválido em 'ler_json_plano_bombom': bombom fora da faixa")
        })
    }

    fn ensure_eof(&self) -> Result<(), PinkerError> {
        if self.idx == self.src.len() {
            Ok(())
        } else {
            Err(runtime_err(
                "json inválido em 'ler_json_plano_bombom': conteúdo extra após objeto",
            ))
        }
    }
}

fn formatar_verso_argumento(arg: &RuntimeValue) -> Result<String, PinkerError> {
    match arg {
        RuntimeValue::Int(value) => Ok(value.to_string()),
        RuntimeValue::Str(value) => Ok(value.clone()),
        _ => Err(runtime_err(
            "intrínseca 'formatar_verso' exige argumentos de substituição em bombom ou verso",
        )),
    }
}

fn find_named_cli_argument<'a>(args: &'a [String], key: &str) -> NamedArgLookup<'a> {
    let key_eq = format!("{key}=");
    for (index, arg) in args.iter().enumerate() {
        if arg == key {
            return match args.get(index + 1) {
                Some(value) => NamedArgLookup::PresentValue(value),
                None => NamedArgLookup::PresentWithoutValue,
            };
        }
        if let Some(value) = arg.strip_prefix(&key_eq) {
            return NamedArgLookup::PresentValue(value);
        }
    }
    NamedArgLookup::Missing
}

fn trim_final_newline_minimo(mut line: String) -> String {
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    line
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
        RuntimeValue::ListBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::MapVersoBombom(_) => Err(runtime_err(msg)),
    }
}

fn pop_bool(stack: &mut Vec<RuntimeValue>, msg: &str) -> Result<bool, PinkerError> {
    match pop(stack, msg)? {
        RuntimeValue::Bool(v) => Ok(v),
        RuntimeValue::Int(_) => Err(runtime_err(msg)),
        RuntimeValue::IntSigned(_) => Err(runtime_err(msg)),
        RuntimeValue::Ptr(_) => Err(runtime_err(msg)),
        RuntimeValue::Str(_) => Err(runtime_err(msg)),
        RuntimeValue::ListBombom(_) => Err(runtime_err(msg)),
        RuntimeValue::MapVersoBombom(_) => Err(runtime_err(msg)),
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
            (RuntimeValue::ListBombom(_), _) => {
                Err(runtime_err("cast inteiro não aceita lista<bombom>"))
            }
            (RuntimeValue::MapVersoBombom(_), _) => {
                Err(runtime_err("cast inteiro não aceita mapa<verso,bombom>"))
            }
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
            RuntimeValue::ListBombom(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
            RuntimeValue::MapVersoBombom(_) => Err(runtime_err(
                "ponteiro em runtime requer valor inteiro de endereço",
            )),
        };
    }

    if matches!(ty, crate::ir::TypeIR::ListBombom) {
        return match value {
            RuntimeValue::ListBombom(handle) => Ok(RuntimeValue::ListBombom(handle)),
            _ => Err(runtime_err("valor incompatível: esperado lista<bombom>")),
        };
    }
    if matches!(ty, crate::ir::TypeIR::MapVersoBombom) {
        return match value {
            RuntimeValue::MapVersoBombom(handle) => Ok(RuntimeValue::MapVersoBombom(handle)),
            _ => Err(runtime_err(
                "valor incompatível: esperado mapa<verso,bombom>",
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
            Some("o handle já foi fechado com 'fechar'; abra novamente com 'abrir', 'criar_arquivo' ou 'abrir_anexo' se necessário"),
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
