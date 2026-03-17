//! Máquina abstrata de pilha — camada `--machine` do pipeline.
//!
//! `MachineProgram` é o resultado do lowering de `SelectedProgram` para um conjunto explícito
//! de instruções de pilha (`MachineInstr`) e terminadores (`MachineTerminator`).
//! Cada instrução opera sobre uma pilha implícita de valores; nenhuma instrução referencia
//! registradores — apenas slots nomeados e a pilha de operandos.
//!
//! A representação é validada por `abstract_machine_validate` antes de ser interpretada
//! ou emitida como pseudo-assembly via `backend_text`.
//!
//! Posição no pipeline:
//!   `instr_select` → **`abstract_machine`** → `abstract_machine_validate` → `interpreter` / `backend_text`

use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::TypeIR;
use std::collections::HashMap;

/// Programa completo na representação de máquina abstrata.
/// Contém globals (constantes somente-leitura) e funções com blocos de instruções de pilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProgram {
    pub module_name: String,
    pub globals: Vec<MachineGlobal>,
    pub functions: Vec<MachineFunction>,
}

/// Constante global somente-leitura. O runtime acessa via `LoadGlobal`; escrita não é suportada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineGlobal {
    pub name: String,
    pub value: OperandIR,
}

/// Função na Machine. `params` e `locals` listam os nomes dos slots nomeados;
/// `slot_types` mapeia cada slot ao seu tipo para uso pelo validador de pilha.
/// Temporários (`%tN`) são gerados durante o lowering e também registrados em `slot_types`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFunction {
    pub name: String,
    pub ret_type: TypeIR,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub slot_types: HashMap<String, TypeIR>,
    pub blocks: Vec<MachineBlock>,
}

/// Bloco básico da Machine: sequência linear de instruções de pilha seguida de um terminador.
/// A invariante é que `terminator` sempre está presente — não existe bloco sem saída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineBlock {
    pub label: String,
    pub code: Vec<MachineInstr>,
    pub terminator: MachineTerminator,
}

/// Instruções de pilha. Convenções:
/// - `PushInt`/`PushBool`: empilha literal.
/// - `LoadSlot`/`StoreSlot`: lê/escreve slot nomeado (params, locals ou temporário `%tN`).
/// - `LoadGlobal`: lê constante global pelo nome; não existe `StoreGlobal` nesta versão.
/// - Operações aritméticas/lógicas/comparação: consomem topo(s) da pilha e empilham resultado.
/// - `Call`/`CallVoid`: empilha `argc` argumentos antes da instrução; `Call` empilha o retorno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineInstr {
    PushInt(u64),
    PushBool(bool),
    LoadSlot(String),
    LoadGlobal(String),
    StoreSlot(String),
    Neg,
    Not,
    Add,
    Sub,
    Mul,
    Div,
    CmpEq,
    CmpNe,
    CmpLt,
    CmpLe,
    CmpGt,
    CmpGe,
    Call { callee: String, argc: usize },
    CallVoid { callee: String, argc: usize },
}

/// Terminadores de bloco. `BrTrue` consome o topo da pilha (deve ser `lógica`).
/// `Ret` consome o único valor da pilha como retorno; `RetVoid` exige pilha vazia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineTerminator {
    Jmp(String),
    BrTrue {
        then_label: String,
        else_label: String,
    },
    Ret,
    RetVoid,
}

pub fn lower_program(selected: &SelectedProgram) -> Result<MachineProgram, PinkerError> {
    let globals = selected
        .globals
        .iter()
        .map(|g| MachineGlobal {
            name: g.name.clone(),
            value: g.value.clone(),
        })
        .collect();

    let functions = selected
        .functions
        .iter()
        .map(|f| {
            let blocks = f
                .blocks
                .iter()
                .map(|b| {
                    let mut code = Vec::new();
                    for i in &b.instructions {
                        lower_instr(i, &mut code);
                    }
                    let terminator = lower_term(&b.terminator, &mut code);
                    MachineBlock {
                        label: b.label.clone(),
                        code,
                        terminator,
                    }
                })
                .collect();

            MachineFunction {
                name: f.name.clone(),
                ret_type: f.ret_type,
                params: f.params.clone(),
                locals: f.locals.clone(),
                slot_types: f.slot_types.clone(),
                blocks,
            }
        })
        .collect();

    Ok(MachineProgram {
        module_name: selected.module_name.clone(),
        globals,
        functions,
    })
}

// Padrão de lowering para instruções binárias/unárias: carrega operandos na pilha,
// emite a operação, depois armazena o resultado em um slot temporário `%tN`.
fn lower_instr(inst: &SelectedInstr, code: &mut Vec<MachineInstr>) {
    match inst {
        SelectedInstr::Mov { dest, src } => {
            emit_load(src, code);
            code.push(MachineInstr::StoreSlot(dest.clone()));
        }
        SelectedInstr::Neg { dest, operand } => {
            emit_load(operand, code);
            code.push(MachineInstr::Neg);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Not { dest, operand } => {
            emit_load(operand, code);
            code.push(MachineInstr::Not);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Add { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Add);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Sub { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Sub);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Mul { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Mul);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Div { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::Div);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpEq { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpEq);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpNe { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpNe);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpLt { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpLt);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpLe { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpLe);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpGt { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpGt);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CmpGe { dest, lhs, rhs } => {
            emit_load(lhs, code);
            emit_load(rhs, code);
            code.push(MachineInstr::CmpGe);
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::Call {
            dest, callee, args, ..
        } => {
            for arg in args {
                emit_load(arg, code);
            }
            code.push(MachineInstr::Call {
                callee: callee.clone(),
                argc: args.len(),
            });
            code.push(MachineInstr::StoreSlot(temp_name(*dest)));
        }
        SelectedInstr::CallVoid { callee, args } => {
            for arg in args {
                emit_load(arg, code);
            }
            code.push(MachineInstr::CallVoid {
                callee: callee.clone(),
                argc: args.len(),
            });
        }
    }
}

fn lower_term(term: &SelectedTerminator, code: &mut Vec<MachineInstr>) -> MachineTerminator {
    match term {
        SelectedTerminator::Jmp(label) => MachineTerminator::Jmp(label.clone()),
        SelectedTerminator::Br {
            cond,
            then_label,
            else_label,
        } => {
            emit_load(cond, code);
            MachineTerminator::BrTrue {
                then_label: then_label.clone(),
                else_label: else_label.clone(),
            }
        }
        SelectedTerminator::Ret(Some(v)) => {
            emit_load(v, code);
            MachineTerminator::Ret
        }
        SelectedTerminator::Ret(None) => MachineTerminator::RetVoid,
    }
}

fn emit_load(op: &OperandIR, code: &mut Vec<MachineInstr>) {
    match op {
        OperandIR::Int(v) => code.push(MachineInstr::PushInt(*v)),
        OperandIR::Bool(v) => code.push(MachineInstr::PushBool(*v)),
        OperandIR::Local(s) => code.push(MachineInstr::LoadSlot(s.clone())),
        OperandIR::GlobalConst(g) => code.push(MachineInstr::LoadGlobal(g.clone())),
        OperandIR::Temp(t) => code.push(MachineInstr::LoadSlot(temp_name(*t))),
    }
}

// Temporários recebem o nome canônico `%tN` (N = índice do TempIR).
// Esse padrão é reconhecido pelo validador em `abstract_machine_validate::is_temp_slot`.
fn temp_name(t: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", t.0)
}

pub fn render_program(program: &MachineProgram) -> String {
    let mut out = String::new();
    line(&mut out, 0, &format!("module {}", program.module_name));
    line(&mut out, 0, "globals:");
    if program.globals.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for g in &program.globals {
            line(
                &mut out,
                1,
                &format!("global @{} = {}", g.name, render_operand(&g.value)),
            );
        }
    }

    line(&mut out, 0, "machine:");
    for f in &program.functions {
        line(&mut out, 1, &format!("func {}:", f.name));
        line(
            &mut out,
            2,
            &format!(
                "params {}",
                if f.params.is_empty() {
                    "[]".to_string()
                } else {
                    f.params.join(", ")
                }
            ),
        );
        line(
            &mut out,
            2,
            &format!(
                "locals {}",
                if f.locals.is_empty() {
                    "[]".to_string()
                } else {
                    f.locals.join(", ")
                }
            ),
        );
        for b in &f.blocks {
            line(&mut out, 2, &format!("{}:", b.label));
            for i in &b.code {
                line(&mut out, 3, &format!("vm {}", render_instr(i)));
            }
            line(&mut out, 3, &format!("term {}", render_term(&b.terminator)));
        }
    }

    out
}

fn render_instr(i: &MachineInstr) -> String {
    match i {
        MachineInstr::PushInt(v) => format!("push_int {}", v),
        MachineInstr::PushBool(v) => {
            format!("push_bool {}", if *v { "verdade" } else { "falso" })
        }
        MachineInstr::LoadSlot(s) => format!("load_slot {}", s),
        MachineInstr::LoadGlobal(g) => format!("load_global @{}", g),
        MachineInstr::StoreSlot(s) => format!("store_slot {}", s),
        MachineInstr::Neg => "neg".to_string(),
        MachineInstr::Not => "not".to_string(),
        MachineInstr::Add => "add".to_string(),
        MachineInstr::Sub => "sub".to_string(),
        MachineInstr::Mul => "mul".to_string(),
        MachineInstr::Div => "div".to_string(),
        MachineInstr::CmpEq => "cmp_eq".to_string(),
        MachineInstr::CmpNe => "cmp_ne".to_string(),
        MachineInstr::CmpLt => "cmp_lt".to_string(),
        MachineInstr::CmpLe => "cmp_le".to_string(),
        MachineInstr::CmpGt => "cmp_gt".to_string(),
        MachineInstr::CmpGe => "cmp_ge".to_string(),
        MachineInstr::Call { callee, argc } => format!("call {}, {}", callee, argc),
        MachineInstr::CallVoid { callee, argc } => format!("call_void {}, {}", callee, argc),
    }
}

fn render_term(t: &MachineTerminator) -> String {
    match t {
        MachineTerminator::Jmp(l) => format!("jmp {}", l),
        MachineTerminator::BrTrue {
            then_label,
            else_label,
        } => format!("br_true {}, {}", then_label, else_label),
        MachineTerminator::Ret => "ret".to_string(),
        MachineTerminator::RetVoid => "ret_void".to_string(),
    }
}

fn render_operand(op: &OperandIR) -> String {
    match op {
        OperandIR::Int(v) => v.to_string(),
        OperandIR::Bool(v) => {
            if *v {
                "verdade".to_string()
            } else {
                "falso".to_string()
            }
        }
        OperandIR::Local(s) => s.clone(),
        OperandIR::GlobalConst(g) => format!("@{}", g),
        OperandIR::Temp(t) => format!("%t{}", t.0),
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
