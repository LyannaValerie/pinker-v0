use crate::cfg_ir::{FunctionCfgIR, InstructionCfgIR, OperandIR, ProgramCfgIR, TerminatorIR};
use crate::error::PinkerError;

pub fn emit_program(program: &ProgramCfgIR) -> Result<String, PinkerError> {
    let mut out = String::new();

    line(&mut out, 0, &format!("module {}", program.module_name));
    line(&mut out, 0, "globals:");
    if program.consts.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for konst in &program.consts {
            line(
                &mut out,
                1,
                &format!("global @{} = {}", konst.name, render_operand(&konst.value)),
            );
        }
    }

    line(&mut out, 0, "text:");
    for function in &program.functions {
        emit_function(function, &mut out)?;
    }

    Ok(out)
}

fn emit_function(function: &FunctionCfgIR, out: &mut String) -> Result<(), PinkerError> {
    line(out, 1, &format!("func {}:", function.name));

    if function.params.is_empty() {
        line(out, 2, "params []");
    } else {
        line(
            out,
            2,
            &format!(
                "params {}",
                function
                    .params
                    .iter()
                    .map(|p| p.slot.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }

    if function.locals.is_empty() {
        line(out, 2, "locals []");
    } else {
        line(
            out,
            2,
            &format!(
                "locals {}",
                function
                    .locals
                    .iter()
                    .map(|l| l.slot.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }

    for block in &function.blocks {
        line(out, 2, &format!("{}:", block.label));
        for instruction in &block.instructions {
            line(out, 3, &emit_instruction(instruction));
        }
        line(out, 3, &emit_terminator(&block.terminator));
    }

    Ok(())
}

fn emit_instruction(inst: &InstructionCfgIR) -> String {
    match inst {
        InstructionCfgIR::Let { slot, value } => {
            format!("mov {}, {}", slot, render_operand(value))
        }
        InstructionCfgIR::Assign { slot, value } => {
            format!("mov {}, {}", slot, render_operand(value))
        }
        InstructionCfgIR::Unary { dest, op, operand } => format!(
            "{} = {} {}",
            render_temp(*dest),
            render_unary_op(*op),
            render_operand(operand)
        ),
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

fn emit_terminator(term: &TerminatorIR) -> String {
    match term {
        TerminatorIR::Jump(label) => format!("jmp {}", label),
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

fn render_unary_op(op: crate::ir::UnaryOpIR) -> &'static str {
    match op {
        crate::ir::UnaryOpIR::Neg => "neg",
        crate::ir::UnaryOpIR::Not => "not",
    }
}

fn render_binary_op(op: crate::ir::BinaryOpIR) -> &'static str {
    match op {
        crate::ir::BinaryOpIR::Add => "add",
        crate::ir::BinaryOpIR::Sub => "sub",
        crate::ir::BinaryOpIR::Mul => "mul",
        crate::ir::BinaryOpIR::Div => "div",
        crate::ir::BinaryOpIR::Eq => "eq",
        crate::ir::BinaryOpIR::Neq => "neq",
        crate::ir::BinaryOpIR::Lt => "lt",
        crate::ir::BinaryOpIR::Lte => "lte",
        crate::ir::BinaryOpIR::Gt => "gt",
        crate::ir::BinaryOpIR::Gte => "gte",
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
