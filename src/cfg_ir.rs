//! CFG IR — controle de fluxo explícito, camada `--cfg-ir` do pipeline.
//!
//! Converte a IR estruturada (`ir.rs`) para blocos básicos com terminadores explícitos
//! (`Jump`, `Branch`, `Return`). Após este passo, não existem mais `if`/`else` aninhados
//! na representação — apenas saltos entre labels.
//!
//! Temporários (`TempIR`) são introduzidos aqui para resultados intermediários de expressões.
//! Cada temporário tem escopo de função (não por bloco); o validador `cfg_ir_validate`
//! impõe que temporários usados sejam definidos antes do uso no mesmo bloco.
//!
//! Posição no pipeline:
//!   `ir` → **`cfg_ir`** → `cfg_ir_validate` → `instr_select`

use crate::error::PinkerError;
use crate::ir::{BinaryOpIR, FunctionIR, InstructionIR, ProgramIR, TypeIR, UnaryOpIR, ValueIR};
use crate::token::Span;

/// Programa na CFG IR: módulo com constantes globais e funções em forma de blocos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramCfgIR {
    pub module_name: String,
    pub consts: Vec<GlobalConstCfgIR>,
    pub functions: Vec<FunctionCfgIR>,
}

/// Constante global na CFG IR. O valor é restrito a literais ou referências a outras globais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConstCfgIR {
    pub name: String,
    pub ty: TypeIR,
    pub value: ValueCfgIR,
}

/// Função na CFG IR. `entry` nomeia o label do bloco de entrada (sempre `"entry"`).
/// `params` e `locals` preservam os metadados originais (nome-fonte, tipo, mutabilidade).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCfgIR {
    pub name: String,
    pub params: Vec<crate::ir::BindingIR>,
    pub locals: Vec<crate::ir::LocalIR>,
    pub ret_type: TypeIR,
    pub entry: String,
    pub blocks: Vec<BasicBlockIR>,
    pub span: Span,
}

/// Bloco básico: sequência linear de instruções sem salto, terminada por um `TerminatorIR`.
/// Todo bloco possui exatamente um terminador; não existe bloco sem saída.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlockIR {
    pub label: String,
    pub instructions: Vec<InstructionCfgIR>,
    pub terminator: TerminatorIR,
}

/// Instruções dentro de um bloco básico. `Let`/`Assign` operam sobre slots nomeados;
/// `Unary`/`Binary`/`Call` produzem um resultado em um `TempIR`.
/// `Call` com `dest: None` descarta o retorno (chamadas `nulo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionCfgIR {
    Let {
        slot: String,
        value: OperandIR,
    },
    Assign {
        slot: String,
        value: OperandIR,
    },
    Unary {
        dest: TempIR,
        op: UnaryOpIR,
        operand: OperandIR,
    },
    Binary {
        dest: TempIR,
        op: BinaryOpIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Call {
        dest: Option<TempIR>,
        callee: String,
        args: Vec<OperandIR>,
        ret_type: TypeIR,
    },
}

/// Terminador de bloco. `Branch` consome um operando booleano como condição.
/// `Return(None)` corresponde a funções `nulo`; `Return(Some(_))` a funções com retorno.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminatorIR {
    Jump(String),
    Branch {
        cond: OperandIR,
        then_label: String,
        else_label: String,
    },
    Return(Option<OperandIR>),
}

/// Índice de temporário gerado durante o lowering. Escopo de função; único por função.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TempIR(pub u32);

/// Operando: literal, referência a slot local, referência a constante global ou temporário.
/// `GlobalConst` referencia apenas `eterno` (somente-leitura); não existe operando mutável global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandIR {
    Local(String),
    GlobalConst(String),
    Int(u64),
    Bool(bool),
    Temp(TempIR),
}

pub type ValueCfgIR = OperandIR;

// `FunctionLowerer` mantém estado mutable durante o lowering de uma função:
// - `blocks`: blocos acumulados em ordem de criação (índice = posição no vetor).
// - `next_block`: contador de sufixo para labels gerados (`join_N`, `dead_N`).
// - `next_temp`: contador global de temporários por função.
struct FunctionLowerer {
    blocks: Vec<BlockBuilder>,
    next_block: usize,
    next_temp: u32,
}

// `BlockBuilder` é um bloco em construção. `terminator: None` indica bloco ainda aberto.
// `is_terminated()` é consultado antes de adicionar instruções para evitar código morto.
struct BlockBuilder {
    label: String,
    instructions: Vec<InstructionCfgIR>,
    terminator: Option<TerminatorIR>,
}

pub fn lower_program(program: &ProgramIR) -> Result<ProgramCfgIR, PinkerError> {
    let consts = program
        .consts
        .iter()
        .map(|c| {
            let value = lower_constant_value(&c.value, c.span)?;
            Ok(GlobalConstCfgIR {
                name: c.name.clone(),
                ty: c.ty,
                value,
            })
        })
        .collect::<Result<Vec<_>, PinkerError>>()?;

    let functions = program
        .functions
        .iter()
        .map(lower_function)
        .collect::<Result<Vec<_>, PinkerError>>()?;

    Ok(ProgramCfgIR {
        module_name: program.module_name.clone(),
        consts,
        functions,
    })
}

pub fn render_program(program: &ProgramCfgIR) -> String {
    let mut out = String::new();
    line(&mut out, 0, &format!("module {}", program.module_name));
    line(&mut out, 0, "consts:");
    if program.consts.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for konst in &program.consts {
            line(
                &mut out,
                1,
                &format!(
                    "const @{}: {} = {}",
                    konst.name,
                    konst.ty.name(),
                    render_operand(&konst.value)
                ),
            );
        }
    }

    line(&mut out, 0, "functions:");
    for function in &program.functions {
        line(
            &mut out,
            1,
            &format!("func {} -> {}", function.name, function.ret_type.name()),
        );
        if function.params.is_empty() {
            line(&mut out, 2, "params: []");
        } else {
            line(&mut out, 2, "params:");
            for p in &function.params {
                line(&mut out, 3, &format!("{}: {}", p.slot, p.ty.name()));
            }
        }
        if function.locals.is_empty() {
            line(&mut out, 2, "locals: []");
        } else {
            line(&mut out, 2, "locals:");
            for l in &function.locals {
                let mutability = if l.is_mut { " mut" } else { "" };
                line(
                    &mut out,
                    3,
                    &format!("{}: {}{}", l.slot, l.ty.name(), mutability),
                );
            }
        }

        for block in &function.blocks {
            line(&mut out, 2, &format!("block {}:", block.label));
            for inst in &block.instructions {
                line(&mut out, 3, &render_instruction(inst));
            }
            line(&mut out, 3, &render_terminator(&block.terminator));
        }
    }

    out
}

fn lower_function(function: &FunctionIR) -> Result<FunctionCfgIR, PinkerError> {
    let mut lowerer = FunctionLowerer {
        blocks: vec![BlockBuilder::new("entry".to_string())],
        next_block: 0,
        next_temp: 0,
    };

    let mut current = 0;
    for instruction in &function.entry.instructions {
        if lowerer.blocks[current].is_terminated() {
            let dead = lowerer.next_label("dead");
            current = lowerer.fresh_block(dead);
        }
        current = lowerer.lower_instruction(instruction, current, function.ret_type)?;
    }

    if !lowerer.blocks[current].is_terminated() {
        if function.ret_type == TypeIR::Nulo {
            lowerer.blocks[current].terminator = Some(TerminatorIR::Return(None));
        } else {
            return Err(PinkerError::Ir {
                msg: "lowering CFG IR encontrou bloco sem terminador e função não-nulo".to_string(),
                span: function.span,
            });
        }
    }

    let blocks = lowerer
        .blocks
        .into_iter()
        .map(|b| BasicBlockIR {
            label: b.label,
            instructions: b.instructions,
            terminator: b.terminator.expect("terminador obrigatório"),
        })
        .collect();

    Ok(FunctionCfgIR {
        name: function.name.clone(),
        params: function.params.clone(),
        locals: function.locals.clone(),
        ret_type: function.ret_type,
        entry: "entry".to_string(),
        blocks,
        span: function.span,
    })
}

impl FunctionLowerer {
    fn lower_instruction(
        &mut self,
        instruction: &InstructionIR,
        current: usize,
        function_ret: TypeIR,
    ) -> Result<usize, PinkerError> {
        match instruction {
            InstructionIR::Let { slot, value, span } => {
                let operand = self.lower_value_operand(value, current, *span)?;
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Let {
                        slot: slot.clone(),
                        value: operand,
                    });
                Ok(current)
            }
            InstructionIR::Assign { slot, value, span } => {
                let operand = self.lower_value_operand(value, current, *span)?;
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Assign {
                        slot: slot.clone(),
                        value: operand,
                    });
                Ok(current)
            }
            InstructionIR::Expr { value, span } => {
                self.lower_expr_stmt(value, current, *span)?;
                Ok(current)
            }
            InstructionIR::Return { value, span } => {
                let ret = value
                    .as_ref()
                    .map(|v| self.lower_value_operand(v, current, *span))
                    .transpose()?;
                self.blocks[current].terminator = Some(TerminatorIR::Return(ret));
                Ok(current)
            }
            InstructionIR::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                let cond = self.lower_value_operand(condition, current, *span)?;
                let then_idx = self.fresh_block(then_block.label.clone());
                let else_idx = else_block
                    .as_ref()
                    .map(|b| self.fresh_block(b.label.clone()));

                // Se não há `senão`, o label de else aponta para o bloco de junção (join).
                // Ambas as arestas do Branch precisam de destino válido.
                let else_label = else_idx
                    .map(|idx| self.blocks[idx].label.clone())
                    .unwrap_or_else(|| self.next_label("join"));

                self.blocks[current].terminator = Some(TerminatorIR::Branch {
                    cond,
                    then_label: self.blocks[then_idx].label.clone(),
                    else_label: else_label.clone(),
                });

                let mut then_current = then_idx;
                for inst in &then_block.instructions {
                    then_current = self.lower_instruction(inst, then_current, function_ret)?;
                }
                let mut then_falls_through = !self.blocks[then_current].is_terminated();

                let mut else_falls_through = else_idx.is_none();
                if let (Some(else_block), Some(else_idx)) = (else_block, else_idx) {
                    let mut else_current = else_idx;
                    for inst in &else_block.instructions {
                        else_current = self.lower_instruction(inst, else_current, function_ret)?;
                    }
                    else_falls_through = !self.blocks[else_current].is_terminated();
                    if else_falls_through {
                        let join_idx = self.fresh_block(else_label.clone());
                        self.blocks[else_current].terminator =
                            Some(TerminatorIR::Jump(self.blocks[join_idx].label.clone()));
                    }
                }

                if then_falls_through || else_falls_through {
                    let join_idx =
                        if let Some(idx) = self.blocks.iter().position(|b| b.label == else_label) {
                            idx
                        } else {
                            self.fresh_block(else_label)
                        };
                    if then_falls_through {
                        self.blocks[then_current].terminator =
                            Some(TerminatorIR::Jump(self.blocks[join_idx].label.clone()));
                        then_falls_through = false;
                    }
                    let _ = then_falls_through;
                    Ok(join_idx)
                } else {
                    Ok(current)
                }
            }
            InstructionIR::While {
                condition,
                body_block,
                span,
            } => {
                let cond_label = self.next_label("loop_cond");
                let cond_idx = self.fresh_block(cond_label);
                self.blocks[current].terminator =
                    Some(TerminatorIR::Jump(self.blocks[cond_idx].label.clone()));

                let cond = self.lower_value_operand(condition, cond_idx, *span)?;
                let body_idx = self.fresh_block(body_block.label.clone());
                let join_label = self.next_label("loop_join");
                let join_idx = self.fresh_block(join_label);
                self.blocks[cond_idx].terminator = Some(TerminatorIR::Branch {
                    cond,
                    then_label: self.blocks[body_idx].label.clone(),
                    else_label: self.blocks[join_idx].label.clone(),
                });

                let mut body_current = body_idx;
                for inst in &body_block.instructions {
                    body_current = self.lower_instruction(inst, body_current, function_ret)?;
                }

                if !self.blocks[body_current].is_terminated() {
                    self.blocks[body_current].terminator =
                        Some(TerminatorIR::Jump(self.blocks[cond_idx].label.clone()));
                }

                Ok(join_idx)
            }
        }
    }

    fn lower_expr_stmt(
        &mut self,
        value: &ValueIR,
        current: usize,
        span: Span,
    ) -> Result<(), PinkerError> {
        match value {
            ValueIR::Call {
                callee,
                args,
                ret_type,
            } if *ret_type == TypeIR::Nulo => {
                let lowered_args = args
                    .iter()
                    .map(|arg| self.lower_value_operand(arg, current, span))
                    .collect::<Result<Vec<_>, PinkerError>>()?;
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Call {
                        dest: None,
                        callee: callee.clone(),
                        args: lowered_args,
                        ret_type: *ret_type,
                    });
                Ok(())
            }
            _ => {
                let _ = self.lower_value_operand(value, current, span)?;
                Ok(())
            }
        }
    }

    fn lower_value_operand(
        &mut self,
        value: &ValueIR,
        current: usize,
        span: Span,
    ) -> Result<OperandIR, PinkerError> {
        match value {
            ValueIR::Local(slot) => Ok(OperandIR::Local(slot.clone())),
            ValueIR::GlobalConst(name) => Ok(OperandIR::GlobalConst(name.clone())),
            ValueIR::Int(v) => Ok(OperandIR::Int(*v)),
            ValueIR::Bool(v) => Ok(OperandIR::Bool(*v)),
            ValueIR::Unary { op, operand } => {
                let operand = self.lower_value_operand(operand, current, span)?;
                let dest = self.next_temp();
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Unary {
                        dest,
                        op: *op,
                        operand,
                    });
                Ok(OperandIR::Temp(dest))
            }
            ValueIR::Binary { op, lhs, rhs } => {
                let lhs = self.lower_value_operand(lhs, current, span)?;
                let rhs = self.lower_value_operand(rhs, current, span)?;
                let dest = self.next_temp();
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Binary {
                        dest,
                        op: *op,
                        lhs,
                        rhs,
                    });
                Ok(OperandIR::Temp(dest))
            }
            ValueIR::Call {
                callee,
                args,
                ret_type,
            } => {
                let lowered_args = args
                    .iter()
                    .map(|arg| self.lower_value_operand(arg, current, span))
                    .collect::<Result<Vec<_>, PinkerError>>()?;
                if *ret_type == TypeIR::Nulo {
                    return Err(PinkerError::Ir {
                        msg: "chamada nulo usada como valor na CFG IR".to_string(),
                        span,
                    });
                }
                let dest = self.next_temp();
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Call {
                        dest: Some(dest),
                        callee: callee.clone(),
                        args: lowered_args,
                        ret_type: *ret_type,
                    });
                Ok(OperandIR::Temp(dest))
            }
        }
    }

    fn fresh_block(&mut self, label: String) -> usize {
        let idx = self.blocks.len();
        self.blocks.push(BlockBuilder::new(label));
        idx
    }

    fn next_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.next_block);
        self.next_block += 1;
        label
    }

    fn next_temp(&mut self) -> TempIR {
        let temp = TempIR(self.next_temp);
        self.next_temp += 1;
        temp
    }
}

impl BlockBuilder {
    fn new(label: String) -> Self {
        Self {
            label,
            instructions: Vec::new(),
            terminator: None,
        }
    }

    fn is_terminated(&self) -> bool {
        self.terminator.is_some()
    }
}

fn lower_constant_value(value: &ValueIR, span: Span) -> Result<ValueCfgIR, PinkerError> {
    match value {
        ValueIR::Int(v) => Ok(OperandIR::Int(*v)),
        ValueIR::Bool(v) => Ok(OperandIR::Bool(*v)),
        ValueIR::GlobalConst(name) => Ok(OperandIR::GlobalConst(name.clone())),
        ValueIR::Local(slot) => Ok(OperandIR::Local(slot.clone())),
        _ => Err(PinkerError::Ir {
            msg: "constante global com valor não-literal fora do escopo da CFG IR".to_string(),
            span,
        }),
    }
}

fn render_instruction(inst: &InstructionCfgIR) -> String {
    match inst {
        InstructionCfgIR::Let { slot, value } => {
            format!("let {} = {}", slot, render_operand(value))
        }
        InstructionCfgIR::Assign { slot, value } => {
            format!("assign {} = {}", slot, render_operand(value))
        }
        InstructionCfgIR::Unary { dest, op, operand } => {
            format!(
                "{} = {} {}",
                render_temp(*dest),
                render_unary_op(*op),
                render_operand(operand)
            )
        }
        InstructionCfgIR::Binary { dest, op, lhs, rhs } => format!(
            "{} = {} {}, {}",
            render_temp(*dest),
            render_binary_op(*op),
            render_operand(lhs),
            render_operand(rhs)
        ),
        InstructionCfgIR::Call {
            dest,
            callee,
            args,
            ret_type,
        } => {
            let call = format!(
                "call {}({}) -> {}",
                callee,
                args.iter()
                    .map(render_operand)
                    .collect::<Vec<_>>()
                    .join(", "),
                ret_type.name()
            );
            match dest {
                Some(dest) => format!("{} = {}", render_temp(*dest), call),
                None => call,
            }
        }
    }
}

fn render_unary_op(op: UnaryOpIR) -> &'static str {
    match op {
        UnaryOpIR::Neg => "neg",
        UnaryOpIR::Not => "not",
    }
}

fn render_binary_op(op: BinaryOpIR) -> &'static str {
    match op {
        BinaryOpIR::Add => "add",
        BinaryOpIR::Sub => "sub",
        BinaryOpIR::Mul => "mul",
        BinaryOpIR::Div => "div",
        BinaryOpIR::Eq => "eq",
        BinaryOpIR::Neq => "neq",
        BinaryOpIR::Lt => "lt",
        BinaryOpIR::Lte => "lte",
        BinaryOpIR::Gt => "gt",
        BinaryOpIR::Gte => "gte",
    }
}

fn render_terminator(term: &TerminatorIR) -> String {
    match term {
        TerminatorIR::Jump(target) => format!("jmp {}", target),
        TerminatorIR::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, {}, {}",
            render_operand(cond),
            then_label,
            else_label
        ),
        TerminatorIR::Return(Some(value)) => format!("ret {}", render_operand(value)),
        TerminatorIR::Return(None) => "ret".to_string(),
    }
}

fn render_operand(op: &OperandIR) -> String {
    match op {
        OperandIR::Local(slot) => slot.clone(),
        OperandIR::GlobalConst(name) => format!("@{}", name),
        OperandIR::Int(v) => format!("{}:bombom", v),
        OperandIR::Bool(v) => format!("{}:logica", if *v { "verdade" } else { "falso" }),
        OperandIR::Temp(t) => render_temp(*t),
    }
}

fn render_temp(temp: TempIR) -> String {
    format!("%t{}", temp.0)
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
