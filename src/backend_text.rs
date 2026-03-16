use crate::cfg_ir::{InstructionCfgIR, OperandIR, ProgramCfgIR, TerminatorIR};
use crate::error::PinkerError;
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextProgram {
    pub module_name: String,
    pub globals: Vec<BackendTextGlobal>,
    pub functions: Vec<BackendTextFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextGlobal {
    pub name: String,
    pub value: OperandIR,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextFunction {
    pub name: String,
    pub ret_type: TypeIR,
    pub params: Vec<String>,
    pub locals: Vec<String>,
    pub blocks: Vec<BackendTextBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTextBlock {
    pub label: String,
    pub instructions: Vec<BackendTextInstruction>,
    pub terminator: BackendTextTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendTextInstruction {
    Mov {
        dest: String,
        src: OperandIR,
    },
    Unary {
        dest: crate::cfg_ir::TempIR,
        op: UnaryOpIR,
        operand: OperandIR,
    },
    Binary {
        dest: crate::cfg_ir::TempIR,
        op: BinaryOpIR,
        lhs: OperandIR,
        rhs: OperandIR,
    },
    Call {
        dest: Option<crate::cfg_ir::TempIR>,
        callee: String,
        args: Vec<OperandIR>,
        ret_type: TypeIR,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendTextTerminator {
    Jump(String),
    Branch {
        cond: OperandIR,
        then_label: String,
        else_label: String,
    },
    Return(Option<OperandIR>),
}

pub fn lower_program(program: &ProgramCfgIR) -> Result<BackendTextProgram, PinkerError> {
    let globals = program
        .consts
        .iter()
        .map(|g| BackendTextGlobal {
            name: g.name.clone(),
            value: g.value.clone(),
        })
        .collect();

    let functions = program
        .functions
        .iter()
        .map(|f| BackendTextFunction {
            name: f.name.clone(),
            ret_type: f.ret_type,
            params: f.params.iter().map(|p| p.slot.clone()).collect(),
            locals: f.locals.iter().map(|l| l.slot.clone()).collect(),
            blocks: f
                .blocks
                .iter()
                .map(|b| BackendTextBlock {
                    label: b.label.clone(),
                    instructions: b
                        .instructions
                        .iter()
                        .map(|i| match i {
                            InstructionCfgIR::Let { slot, value }
                            | InstructionCfgIR::Assign { slot, value } => {
                                BackendTextInstruction::Mov {
                                    dest: slot.clone(),
                                    src: value.clone(),
                                }
                            }
                            InstructionCfgIR::Unary { dest, op, operand } => {
                                BackendTextInstruction::Unary {
                                    dest: *dest,
                                    op: *op,
                                    operand: operand.clone(),
                                }
                            }
                            InstructionCfgIR::Binary { dest, op, lhs, rhs } => {
                                BackendTextInstruction::Binary {
                                    dest: *dest,
                                    op: *op,
                                    lhs: lhs.clone(),
                                    rhs: rhs.clone(),
                                }
                            }
                            InstructionCfgIR::Call {
                                dest,
                                callee,
                                args,
                                ret_type,
                            } => BackendTextInstruction::Call {
                                dest: *dest,
                                callee: callee.clone(),
                                args: args.clone(),
                                ret_type: *ret_type,
                            },
                        })
                        .collect(),
                    terminator: match &b.terminator {
                        TerminatorIR::Jump(label) => BackendTextTerminator::Jump(label.clone()),
                        TerminatorIR::Branch {
                            cond,
                            then_label,
                            else_label,
                        } => BackendTextTerminator::Branch {
                            cond: cond.clone(),
                            then_label: then_label.clone(),
                            else_label: else_label.clone(),
                        },
                        TerminatorIR::Return(v) => BackendTextTerminator::Return(v.clone()),
                    },
                })
                .collect(),
        })
        .collect();

    Ok(BackendTextProgram {
        module_name: program.module_name.clone(),
        globals,
        functions,
    })
}

pub fn emit_program(program: &ProgramCfgIR) -> Result<String, PinkerError> {
    let lowered = lower_program(program)?;
    crate::backend_text_validate::validate_program(&lowered)?;
    Ok(render_program(&lowered))
}

pub fn render_program(program: &BackendTextProgram) -> String {
    let mut out = String::new();

    line(&mut out, 0, &format!("module {}", program.module_name));
    line(&mut out, 0, "globals:");
    if program.globals.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for global in &program.globals {
            line(
                &mut out,
                1,
                &format!(
                    "global @{} = {}",
                    global.name,
                    render_operand(&global.value)
                ),
            );
        }
    }

    line(&mut out, 0, "text:");
    for function in &program.functions {
        line(&mut out, 1, &format!("func {}:", function.name));
        line(
            &mut out,
            2,
            &format!(
                "params {}",
                if function.params.is_empty() {
                    "[]".to_string()
                } else {
                    function.params.join(", ")
                }
            ),
        );
        line(
            &mut out,
            2,
            &format!(
                "locals {}",
                if function.locals.is_empty() {
                    "[]".to_string()
                } else {
                    function.locals.join(", ")
                }
            ),
        );
        for block in &function.blocks {
            line(&mut out, 2, &format!("{}:", block.label));
            for instruction in &block.instructions {
                line(
                    &mut out,
                    3,
                    &format!("ins {}", render_instruction(instruction)),
                );
            }
            line(
                &mut out,
                3,
                &format!("term {}", render_terminator(&block.terminator)),
            );
        }
    }

    out
}

fn render_instruction(inst: &BackendTextInstruction) -> String {
    match inst {
        BackendTextInstruction::Mov { dest, src } => {
            format!("mov {}, {}", dest, render_operand(src))
        }
        BackendTextInstruction::Unary { dest, op, operand } => {
            format!(
                "unop {}, {}, {}",
                render_temp(*dest),
                op_name(*op),
                render_operand(operand)
            )
        }
        BackendTextInstruction::Binary { dest, op, lhs, rhs } => format!(
            "binop {}, {}, {}, {}",
            render_temp(*dest),
            binop_name(*op),
            render_operand(lhs),
            render_operand(rhs)
        ),
        BackendTextInstruction::Call {
            dest,
            callee,
            args,
            ret_type,
        } => {
            let args = args
                .iter()
                .map(render_operand)
                .collect::<Vec<_>>()
                .join(", ");
            match (dest, ret_type) {
                (Some(dest), _) => {
                    format!(
                        "call {}, {}({}), {}",
                        render_temp(*dest),
                        callee,
                        args,
                        ret_type.name()
                    )
                }
                (None, TypeIR::Nulo) => format!("call_void {}({})", callee, args),
                (None, _) => format!("call {}, {}({}), {}", "_", callee, args, ret_type.name()),
            }
        }
    }
}

fn render_terminator(term: &BackendTextTerminator) -> String {
    match term {
        BackendTextTerminator::Jump(label) => format!("jmp {}", label),
        BackendTextTerminator::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, {}, {}",
            render_operand(cond),
            then_label,
            else_label
        ),
        BackendTextTerminator::Return(Some(value)) => format!("ret {}", render_operand(value)),
        BackendTextTerminator::Return(None) => "ret".to_string(),
    }
}

fn render_operand(operand: &OperandIR) -> String {
    match operand {
        OperandIR::Local(slot) => slot.clone(),
        OperandIR::GlobalConst(name) => format!("@{}", name),
        OperandIR::Int(value) => value.to_string(),
        OperandIR::Bool(value) => {
            if *value {
                "verdade".to_string()
            } else {
                "falso".to_string()
            }
        }
        OperandIR::Temp(temp) => render_temp(*temp),
    }
}

fn render_temp(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}

fn op_name(op: UnaryOpIR) -> &'static str {
    match op {
        UnaryOpIR::Neg => "neg",
        UnaryOpIR::Not => "not",
    }
}

fn binop_name(op: BinaryOpIR) -> &'static str {
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

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
