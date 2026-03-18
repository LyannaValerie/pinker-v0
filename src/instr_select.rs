use crate::cfg_ir::{InstructionCfgIR, OperandIR, ProgramCfgIR, TerminatorIR};
use crate::error::PinkerError;
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgram {
    pub module_name: String,
    pub globals: Vec<SelectedGlobal>,
    pub functions: Vec<SelectedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedGlobal {
    pub name: String,
    pub value: OperandIR,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedFunction {
    pub name: String,
    pub ret_type: TypeIR,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub slot_types: HashMap<String, TypeIR>,
    pub blocks: Vec<SelectedBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedBlock {
    pub label: String,
    pub instructions: Vec<SelectedInstr>,
    pub terminator: SelectedTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstr {
    Mov {
        dest: String,
        src: OperandIR,
    },
    Neg {
        dest: crate::cfg_ir::TempIR,
        operand: OperandIR,
    },
    Not {
        dest: crate::cfg_ir::TempIR,
        operand: OperandIR,
    },
    BitAnd {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    BitOr {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    BitXor {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Shl {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Shr {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Add {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Sub {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Mul {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Div {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    CmpEq {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    CmpNe {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    CmpLt {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    CmpLe {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    CmpGt {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    CmpGe {
        dest: crate::cfg_ir::TempIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Call {
        dest: crate::cfg_ir::TempIR,
        callee: String,
        args: Vec<OperandIR>,
        ret_type: TypeIR,
    },
    CallVoid {
        callee: String,
        args: Vec<OperandIR>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTerminator {
    Jmp(String),
    Br {
        cond: OperandIR,
        then_label: String,
        else_label: String,
    },
    Ret(Option<OperandIR>),
}

pub fn lower_program(cfg: &ProgramCfgIR) -> Result<SelectedProgram, PinkerError> {
    let globals = cfg
        .consts
        .iter()
        .map(|g| SelectedGlobal {
            name: g.name.clone(),
            value: g.value.clone(),
        })
        .collect();

    let functions = cfg
        .functions
        .iter()
        .map(|f| {
            Ok(SelectedFunction {
                name: f.name.clone(),
                ret_type: f.ret_type,
                params: f.params.iter().map(|p| p.slot.clone()).collect(),
                locals: f.locals.iter().map(|l| l.slot.clone()).collect(),
                slot_types: f
                    .params
                    .iter()
                    .map(|p| (p.slot.clone(), p.ty))
                    .chain(f.locals.iter().map(|l| (l.slot.clone(), l.ty)))
                    .collect(),
                blocks: f
                    .blocks
                    .iter()
                    .map(|b| {
                        let instructions = b
                            .instructions
                            .iter()
                            .map(select_instruction)
                            .collect::<Result<Vec<_>, PinkerError>>()?;
                        let terminator = match &b.terminator {
                            TerminatorIR::Jump(t) => SelectedTerminator::Jmp(t.clone()),
                            TerminatorIR::Branch {
                                cond,
                                then_label,
                                else_label,
                            } => SelectedTerminator::Br {
                                cond: cond.clone(),
                                then_label: then_label.clone(),
                                else_label: else_label.clone(),
                            },
                            TerminatorIR::Return(v) => SelectedTerminator::Ret(v.clone()),
                        };
                        Ok(SelectedBlock {
                            label: b.label.clone(),
                            instructions,
                            terminator,
                        })
                    })
                    .collect::<Result<Vec<_>, PinkerError>>()?,
            })
        })
        .collect::<Result<Vec<_>, PinkerError>>()?;

    Ok(SelectedProgram {
        module_name: cfg.module_name.clone(),
        globals,
        functions,
    })
}

fn select_instruction(inst: &InstructionCfgIR) -> Result<SelectedInstr, PinkerError> {
    match inst {
        InstructionCfgIR::Let { slot, value } | InstructionCfgIR::Assign { slot, value } => {
            Ok(SelectedInstr::Mov {
                dest: slot.clone(),
                src: value.clone(),
            })
        }
        InstructionCfgIR::Unary { dest, op, operand } => Ok(match op {
            UnaryOpIR::Neg => SelectedInstr::Neg {
                dest: *dest,
                operand: operand.clone(),
            },
            UnaryOpIR::Not => SelectedInstr::Not {
                dest: *dest,
                operand: operand.clone(),
            },
        }),
        InstructionCfgIR::Binary { dest, op, lhs, rhs } => Ok(match op {
            BinaryOpIR::LogicalAnd | BinaryOpIR::LogicalOr => {
                return Err(PinkerError::Ir {
                    msg: "logical and/or deve ser resolvido no lowering de CFG com short-circuit"
                        .to_string(),
                    span: crate::token::Span::single(crate::token::Position::new(1, 1)),
                });
            }
            BinaryOpIR::BitAnd => SelectedInstr::BitAnd {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::BitOr => SelectedInstr::BitOr {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::BitXor => SelectedInstr::BitXor {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Shl => SelectedInstr::Shl {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Shr => SelectedInstr::Shr {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Add => SelectedInstr::Add {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Sub => SelectedInstr::Sub {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Mul => SelectedInstr::Mul {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Div => SelectedInstr::Div {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Eq => SelectedInstr::CmpEq {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Neq => SelectedInstr::CmpNe {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Lt => SelectedInstr::CmpLt {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Lte => SelectedInstr::CmpLe {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Gt => SelectedInstr::CmpGt {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
            BinaryOpIR::Gte => SelectedInstr::CmpGe {
                dest: *dest,
                lhs: lhs.clone(),
                rhs: rhs.clone(),
            },
        }),
        InstructionCfgIR::Call {
            dest,
            callee,
            args,
            ret_type,
        } => match (dest, ret_type) {
            (Some(dest), _) => Ok(SelectedInstr::Call {
                dest: *dest,
                callee: callee.clone(),
                args: args.clone(),
                ret_type: *ret_type,
            }),
            (None, TypeIR::Nulo) => Ok(SelectedInstr::CallVoid {
                callee: callee.clone(),
                args: args.clone(),
            }),
            (None, _) => Err(PinkerError::Ir {
                msg: "instruction selection encontrou call com retorno sem destino".to_string(),
                span: crate::token::Span::single(crate::token::Position::new(1, 1)),
            }),
        },
    }
}

pub fn render_program(program: &SelectedProgram) -> String {
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

    line(&mut out, 0, "selected:");
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
            for i in &b.instructions {
                line(&mut out, 3, &format!("isel {}", render_instr(i)));
            }
            line(&mut out, 3, &format!("term {}", render_term(&b.terminator)));
        }
    }

    out
}

fn render_instr(inst: &SelectedInstr) -> String {
    match inst {
        SelectedInstr::Mov { dest, src } => format!("mov {}, {}", dest, render_operand(src)),
        SelectedInstr::Neg { dest, operand } => {
            format!("neg {}, {}", render_temp(*dest), render_operand(operand))
        }
        SelectedInstr::Not { dest, operand } => {
            format!("not {}, {}", render_temp(*dest), render_operand(operand))
        }
        SelectedInstr::BitAnd { dest, lhs, rhs } => format!(
            "bitand {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::BitOr { dest, lhs, rhs } => format!(
            "bitor {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::BitXor { dest, lhs, rhs } => format!(
            "bitxor {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Shl { dest, lhs, rhs } => format!(
            "shl {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Shr { dest, lhs, rhs } => format!(
            "shr {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Add { dest, lhs, rhs } => format!(
            "add {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Sub { dest, lhs, rhs } => format!(
            "sub {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Mul { dest, lhs, rhs } => format!(
            "mul {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Div { dest, lhs, rhs } => format!(
            "div {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::CmpEq { dest, lhs, rhs } => format!(
            "cmp_eq {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::CmpNe { dest, lhs, rhs } => format!(
            "cmp_ne {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::CmpLt { dest, lhs, rhs } => format!(
            "cmp_lt {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::CmpLe { dest, lhs, rhs } => format!(
            "cmp_le {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::CmpGt { dest, lhs, rhs } => format!(
            "cmp_gt {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::CmpGe { dest, lhs, rhs } => format!(
            "cmp_ge {}, {}, {}",
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        SelectedInstr::Call {
            dest,
            callee,
            args,
            ret_type,
        } => format!(
            "call {}, {}({}), {}",
            render_temp(*dest),
            callee,
            args.iter()
                .map(render_operand)
                .collect::<Vec<_>>()
                .join(", "),
            ret_type.name()
        ),
        SelectedInstr::CallVoid { callee, args } => format!(
            "call_void {}({})",
            callee,
            args.iter()
                .map(render_operand)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_term(term: &SelectedTerminator) -> String {
    match term {
        SelectedTerminator::Jmp(l) => format!("jmp {}", l),
        SelectedTerminator::Br {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, {}, {}",
            render_operand(cond),
            then_label,
            else_label
        ),
        SelectedTerminator::Ret(Some(v)) => format!("ret {}", render_operand(v)),
        SelectedTerminator::Ret(None) => "ret".to_string(),
    }
}

fn render_operand(op: &OperandIR) -> String {
    match op {
        OperandIR::Local(s) => s.clone(),
        OperandIR::GlobalConst(g) => format!("@{}", g),
        OperandIR::Int(v) => v.to_string(),
        OperandIR::Bool(v) => {
            if *v {
                "verdade".to_string()
            } else {
                "falso".to_string()
            }
        }
        OperandIR::Temp(t) => render_temp(*t),
    }
}

fn render_temp(t: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", t.0)
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
