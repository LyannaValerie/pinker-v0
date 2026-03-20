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
    pub is_freestanding: bool,
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
    Falar {
        value: OperandIR,
        ty: TypeIR,
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
    Str(String),
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
    next_logical_slot: u32,
    logical_locals: Vec<crate::ir::LocalIR>,
    loop_exit_stack: Vec<String>,
    loop_continue_stack: Vec<String>,
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
        is_freestanding: program.is_freestanding,
        consts,
        functions,
    })
}

pub fn render_program(program: &ProgramCfgIR) -> String {
    let mut out = String::new();
    line(&mut out, 0, &format!("module {}", program.module_name));
    line(
        &mut out,
        0,
        &format!(
            "mode {}",
            if program.is_freestanding {
                "livre"
            } else {
                "hospedado"
            }
        ),
    );
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
        next_logical_slot: 0,
        logical_locals: Vec::new(),
        loop_exit_stack: Vec::new(),
        loop_continue_stack: Vec::new(),
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

    let mut locals = function.locals.clone();
    locals.extend(lowerer.logical_locals);

    Ok(FunctionCfgIR {
        name: function.name.clone(),
        params: function.params.clone(),
        locals,
        ret_type: function.ret_type,
        entry: "entry".to_string(),
        blocks,
        span: function.span,
    })
}

impl FunctionLowerer {
    #[allow(clippy::only_used_in_recursion)]
    fn lower_instruction(
        &mut self,
        instruction: &InstructionIR,
        current: usize,
        function_ret: TypeIR,
    ) -> Result<usize, PinkerError> {
        match instruction {
            InstructionIR::Let { slot, value, span } => {
                let (operand, next_current) = self.lower_value_operand(value, current, *span)?;
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Let {
                        slot: slot.clone(),
                        value: operand,
                    });
                Ok(next_current)
            }
            InstructionIR::Assign { slot, value, span } => {
                let (operand, next_current) = self.lower_value_operand(value, current, *span)?;
                self.blocks[current]
                    .instructions
                    .push(InstructionCfgIR::Assign {
                        slot: slot.clone(),
                        value: operand,
                    });
                Ok(next_current)
            }
            InstructionIR::Expr { value, span } => self.lower_expr_stmt(value, current, *span),
            InstructionIR::Return { value, span } => {
                let (ret, next_current) = match value {
                    Some(v) => {
                        let (ret, next_current) = self.lower_value_operand(v, current, *span)?;
                        (Some(ret), next_current)
                    }
                    None => (None, current),
                };
                self.blocks[next_current].terminator = Some(TerminatorIR::Return(ret));
                Ok(next_current)
            }
            InstructionIR::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                let (cond, cond_current) = self.lower_value_operand(condition, current, *span)?;
                let then_idx = self.fresh_block(then_block.label.clone());
                let else_idx = else_block
                    .as_ref()
                    .map(|b| self.fresh_block(b.label.clone()));

                // Se não há `senão`, o label de else aponta para o bloco de junção (join).
                // Ambas as arestas do Branch precisam de destino válido.
                let else_label = else_idx
                    .map(|idx| self.blocks[idx].label.clone())
                    .unwrap_or_else(|| self.next_label("join"));

                self.blocks[cond_current].terminator = Some(TerminatorIR::Branch {
                    cond,
                    then_label: self.blocks[then_idx].label.clone(),
                    else_label: else_label.clone(),
                });

                let mut then_current = then_idx;
                for inst in &then_block.instructions {
                    then_current = self.lower_instruction(inst, then_current, function_ret)?;
                }
                let then_falls_through = !self.blocks[then_current].is_terminated();

                // Para sem-else: else "cai" implicitamente via Branch → join.
                let mut else_falls_through = else_idx.is_none();
                let mut else_end: Option<usize> = None;

                if let (Some(else_block), Some(else_idx)) = (else_block, else_idx) {
                    let mut else_current = else_idx;
                    for inst in &else_block.instructions {
                        else_current = self.lower_instruction(inst, else_current, function_ret)?;
                    }
                    else_falls_through = !self.blocks[else_current].is_terminated();
                    else_end = Some(else_current);
                }

                if then_falls_through || else_falls_through {
                    // Para sem-else: join label já é else_label (Branch aponta para ele).
                    // Para com-else: precisamos de um join label novo para não duplicar o
                    // label do bloco else.
                    let join_label = if else_end.is_some() {
                        self.next_label("join")
                    } else {
                        else_label
                    };

                    let join_idx =
                        if let Some(idx) = self.blocks.iter().position(|b| b.label == join_label) {
                            idx
                        } else {
                            self.fresh_block(join_label)
                        };

                    if then_falls_through {
                        self.blocks[then_current].terminator =
                            Some(TerminatorIR::Jump(self.blocks[join_idx].label.clone()));
                    }
                    if let Some(ec) = else_end {
                        if else_falls_through {
                            self.blocks[ec].terminator =
                                Some(TerminatorIR::Jump(self.blocks[join_idx].label.clone()));
                        }
                    }
                    Ok(join_idx)
                } else {
                    Ok(cond_current)
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

                let (cond, cond_end_idx) = self.lower_value_operand(condition, cond_idx, *span)?;
                let body_idx = self.fresh_block(body_block.label.clone());
                let join_label = self.next_label("loop_join");
                let join_idx = self.fresh_block(join_label);
                self.blocks[cond_end_idx].terminator = Some(TerminatorIR::Branch {
                    cond,
                    then_label: self.blocks[body_idx].label.clone(),
                    else_label: self.blocks[join_idx].label.clone(),
                });

                self.loop_exit_stack
                    .push(self.blocks[join_idx].label.clone());
                self.loop_continue_stack
                    .push(self.blocks[cond_end_idx].label.clone());
                let mut body_current = body_idx;
                for inst in &body_block.instructions {
                    body_current = self.lower_instruction(inst, body_current, function_ret)?;
                }
                self.loop_continue_stack.pop();
                self.loop_exit_stack.pop();

                if !self.blocks[body_current].is_terminated() {
                    self.blocks[body_current].terminator =
                        Some(TerminatorIR::Jump(self.blocks[cond_end_idx].label.clone()));
                }

                Ok(join_idx)
            }
            InstructionIR::Break { span, .. } => {
                let Some(loop_exit_label) = self.loop_exit_stack.last().cloned() else {
                    return Err(PinkerError::Ir {
                        msg: "lowering CFG IR encontrou break fora de loop".to_string(),
                        span: *span,
                    });
                };

                let cont_label = self.next_label("loop_break_cont");
                let cont_idx = self.fresh_block(cont_label);
                self.blocks[current].terminator = Some(TerminatorIR::Branch {
                    cond: OperandIR::Bool(true),
                    then_label: loop_exit_label,
                    else_label: self.blocks[cont_idx].label.clone(),
                });
                Ok(cont_idx)
            }
            InstructionIR::Falar { value, ty, span } => {
                let (operand, next_current) = self.lower_falar_operand(value, current, *span)?;
                self.blocks[next_current]
                    .instructions
                    .push(InstructionCfgIR::Falar {
                        value: operand,
                        ty: *ty,
                    });
                Ok(next_current)
            }
            InstructionIR::InlineAsm { span, .. } => Err(PinkerError::Ir {
                msg: "CFG IR ainda não lowera inline asm ('sussurro') nesta fase".to_string(),
                span: *span,
            }),
            InstructionIR::Continue { span, .. } => {
                let Some(loop_continue_label) = self.loop_continue_stack.last().cloned() else {
                    return Err(PinkerError::Ir {
                        msg: "lowering CFG IR encontrou continue fora de loop".to_string(),
                        span: *span,
                    });
                };

                let cont_label = self.next_label("loop_continue_cont");
                let cont_idx = self.fresh_block(cont_label);
                self.blocks[current].terminator = Some(TerminatorIR::Branch {
                    cond: OperandIR::Bool(true),
                    then_label: loop_continue_label,
                    else_label: self.blocks[cont_idx].label.clone(),
                });
                Ok(cont_idx)
            }
        }
    }

    fn lower_expr_stmt(
        &mut self,
        value: &ValueIR,
        current: usize,
        span: Span,
    ) -> Result<usize, PinkerError> {
        match value {
            ValueIR::Call {
                callee,
                args,
                ret_type,
            } if *ret_type == TypeIR::Nulo => {
                let lowered_args =
                    args.iter()
                        .try_fold((Vec::new(), current), |(mut acc, cur), arg| {
                            let (lowered, next_cur) = self.lower_value_operand(arg, cur, span)?;
                            acc.push(lowered);
                            Ok::<_, PinkerError>((acc, next_cur))
                        })?;
                self.blocks[lowered_args.1]
                    .instructions
                    .push(InstructionCfgIR::Call {
                        dest: None,
                        callee: callee.clone(),
                        args: lowered_args.0,
                        ret_type: *ret_type,
                    });
                Ok(lowered_args.1)
            }
            _ => {
                let (_, next_current) = self.lower_value_operand(value, current, span)?;
                Ok(next_current)
            }
        }
    }

    fn lower_value_operand(
        &mut self,
        value: &ValueIR,
        current: usize,
        span: Span,
    ) -> Result<(OperandIR, usize), PinkerError> {
        match value {
            ValueIR::Local(slot) => Ok((OperandIR::Local(slot.clone()), current)),
            ValueIR::GlobalConst(name) => Ok((OperandIR::GlobalConst(name.clone()), current)),
            ValueIR::Int(v) => Ok((OperandIR::Int(*v), current)),
            ValueIR::Bool(v) => Ok((OperandIR::Bool(*v), current)),
            ValueIR::String(_) => Err(PinkerError::Ir {
                msg: "CFG IR ainda não lowera valores 'verso' nesta fase".to_string(),
                span,
            }),
            ValueIR::Unary { op, operand } => {
                let (operand, next_current) = self.lower_value_operand(operand, current, span)?;
                let dest = self.next_temp();
                self.blocks[next_current]
                    .instructions
                    .push(InstructionCfgIR::Unary {
                        dest,
                        op: *op,
                        operand,
                    });
                Ok((OperandIR::Temp(dest), next_current))
            }
            ValueIR::Binary { op, lhs, rhs } => match op {
                BinaryOpIR::LogicalAnd | BinaryOpIR::LogicalOr => {
                    self.lower_short_circuit_value(*op, lhs, rhs, current, span)
                }
                _ => {
                    let (lhs, lhs_current) = self.lower_value_operand(lhs, current, span)?;
                    let (rhs, rhs_current) = self.lower_value_operand(rhs, lhs_current, span)?;
                    let dest = self.next_temp();
                    self.blocks[rhs_current]
                        .instructions
                        .push(InstructionCfgIR::Binary {
                            dest,
                            op: *op,
                            lhs,
                            rhs,
                        });
                    Ok((OperandIR::Temp(dest), rhs_current))
                }
            },
            ValueIR::Call {
                callee,
                args,
                ret_type,
            } => {
                let lowered_args =
                    args.iter()
                        .try_fold((Vec::new(), current), |(mut acc, cur), arg| {
                            let (lowered, next_cur) = self.lower_value_operand(arg, cur, span)?;
                            acc.push(lowered);
                            Ok::<_, PinkerError>((acc, next_cur))
                        })?;
                if *ret_type == TypeIR::Nulo {
                    return Err(PinkerError::Ir {
                        msg: "chamada nulo usada como valor na CFG IR".to_string(),
                        span,
                    });
                }
                let dest = self.next_temp();
                self.blocks[lowered_args.1]
                    .instructions
                    .push(InstructionCfgIR::Call {
                        dest: Some(dest),
                        callee: callee.clone(),
                        args: lowered_args.0,
                        ret_type: *ret_type,
                    });
                Ok((OperandIR::Temp(dest), lowered_args.1))
            }
            ValueIR::FieldAccess { .. } | ValueIR::Index { .. } | ValueIR::Cast { .. } => {
                Err(PinkerError::Ir {
                    msg: "CFG IR ainda não lowera acesso a campo/indexação/cast nesta fase"
                        .to_string(),
                    span,
                })
            }
        }
    }

    /// Like `lower_value_operand` but also handles `ValueIR::String` for `falar`.
    fn lower_falar_operand(
        &mut self,
        value: &ValueIR,
        current: usize,
        span: Span,
    ) -> Result<(OperandIR, usize), PinkerError> {
        if let ValueIR::String(s) = value {
            return Ok((OperandIR::Str(s.clone()), current));
        }
        self.lower_value_operand(value, current, span)
    }

    fn lower_short_circuit_value(
        &mut self,
        op: BinaryOpIR,
        lhs: &ValueIR,
        rhs: &ValueIR,
        current: usize,
        span: Span,
    ) -> Result<(OperandIR, usize), PinkerError> {
        let (lhs_cond, lhs_current) = self.lower_value_operand(lhs, current, span)?;
        let rhs_label = self.next_label("logic_rhs");
        let short_label = self.next_label("logic_short");
        let join_label = self.next_label("logic_join");
        let rhs_idx = self.fresh_block(rhs_label.clone());
        let short_idx = self.fresh_block(short_label.clone());
        let join_idx = self.fresh_block(join_label.clone());

        let (then_label, else_label) = if op == BinaryOpIR::LogicalAnd {
            (rhs_label, short_label)
        } else {
            (short_label, rhs_label)
        };

        self.blocks[lhs_current].terminator = Some(TerminatorIR::Branch {
            cond: lhs_cond,
            then_label,
            else_label,
        });

        let logical_slot = self.next_logical_slot();
        let short_value = if op == BinaryOpIR::LogicalAnd {
            OperandIR::Bool(false)
        } else {
            OperandIR::Bool(true)
        };
        self.blocks[short_idx]
            .instructions
            .push(InstructionCfgIR::Let {
                slot: logical_slot.clone(),
                value: short_value,
            });
        self.blocks[short_idx].terminator = Some(TerminatorIR::Jump(join_label.clone()));

        let (rhs_value, rhs_end_idx) = self.lower_value_operand(rhs, rhs_idx, span)?;
        self.blocks[rhs_end_idx]
            .instructions
            .push(InstructionCfgIR::Let {
                slot: logical_slot.clone(),
                value: rhs_value,
            });
        self.blocks[rhs_end_idx].terminator = Some(TerminatorIR::Jump(join_label));

        Ok((OperandIR::Local(logical_slot), join_idx))
    }

    fn next_logical_slot(&mut self) -> String {
        let index = self.next_logical_slot;
        self.next_logical_slot += 1;
        let slot = format!("%logic#{}", index);
        self.logical_locals.push(crate::ir::LocalIR {
            source_name: format!("$logic_{}", index),
            slot: slot.clone(),
            ty: TypeIR::Logica,
            is_mut: true,
        });
        slot
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
        ValueIR::String(_) => Err(PinkerError::Ir {
            msg: "constante global 'verso' ainda não é lowerada na CFG IR nesta fase".to_string(),
            span,
        }),
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
        InstructionCfgIR::Falar { value, ty } => {
            format!("falar {}:{}", render_operand(value), ty.name())
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
        BinaryOpIR::LogicalAnd => "and",
        BinaryOpIR::LogicalOr => "or",
        BinaryOpIR::BitAnd => "bitand",
        BinaryOpIR::BitOr => "bitor",
        BinaryOpIR::BitXor => "bitxor",
        BinaryOpIR::Shl => "shl",
        BinaryOpIR::Shr => "shr",
        BinaryOpIR::Add => "add",
        BinaryOpIR::Sub => "sub",
        BinaryOpIR::Mul => "mul",
        BinaryOpIR::Div => "div",
        BinaryOpIR::Mod => "mod",
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
        OperandIR::Str(s) => format!("\"{}\":verso", s),
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
