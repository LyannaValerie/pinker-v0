use crate::backend_text;
use crate::backend_text::BackendTextProgram;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram};
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};
use crate::token::{Position, Span};

pub fn emit_from_selected(selected: &SelectedProgram) -> Result<String, PinkerError> {
    validate_supported_subset(selected)?;
    let lowered = backend_text::lower_selected_program(selected)?;
    Ok(render_program(&lowered))
}

fn validate_supported_subset(selected: &SelectedProgram) -> Result<(), PinkerError> {
    for function in &selected.functions {
        if !is_supported_type(function.ret_type) {
            return Err(err(&format!(
                "backend .s textual da Fase 53 ainda não suporta retorno '{}' em '{}'",
                function.ret_type.name(),
                function.name
            )));
        }

        for (slot, ty) in &function.slot_types {
            if !is_supported_type(*ty) {
                return Err(err(&format!(
                    "backend .s textual da Fase 53 ainda não suporta slot '{}' do tipo '{}' em '{}'",
                    slot,
                    ty.name(),
                    function.name
                )));
            }
        }

        for block in &function.blocks {
            for inst in &block.instructions {
                if let SelectedInstr::Call { ret_type, .. } = inst {
                    if !is_supported_type(*ret_type) {
                        return Err(err(&format!(
                            "backend .s textual da Fase 53 ainda não suporta call com retorno '{}'",
                            ret_type.name()
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}

fn is_supported_type(ty: TypeIR) -> bool {
    matches!(
        ty,
        TypeIR::Bombom
            | TypeIR::U8
            | TypeIR::U16
            | TypeIR::U32
            | TypeIR::U64
            | TypeIR::I8
            | TypeIR::I16
            | TypeIR::I32
            | TypeIR::I64
            | TypeIR::Logica
            | TypeIR::Nulo
    )
}

pub fn render_program(program: &BackendTextProgram) -> String {
    let mut out = String::new();

    line(
        &mut out,
        0,
        "; pinker v0 textual .s (fase 53, derivado de --selected)",
    );
    line(&mut out, 0, &format!("; module {}", program.module_name));
    line(&mut out, 0, ".text");

    if !program.globals.is_empty() {
        line(&mut out, 0, ".section .rodata");
        for global in &program.globals {
            line(&mut out, 0, &format!(".globl {}", global.name));
            line(&mut out, 0, &format!("{}:", global.name));
            line(
                &mut out,
                1,
                &format!(".quad {}", render_operand(&global.value)),
            );
        }
        line(&mut out, 0, ".text");
    }

    for function in &program.functions {
        line(&mut out, 0, &format!(".globl {}", function.name));
        line(&mut out, 0, &format!("{}:", function.name));
        line(
            &mut out,
            1,
            &format!(
                "; slots params={} locals={}",
                join_or_empty(&function.params),
                join_or_empty(&function.locals)
            ),
        );

        for block in &function.blocks {
            line(
                &mut out,
                1,
                &format!(".L{}_{}:", function.name, block.label),
            );
            for instruction in &block.instructions {
                line(&mut out, 2, &render_instruction(instruction));
            }
            line(
                &mut out,
                2,
                &render_terminator(&block.terminator, &function.name),
            );
        }
    }

    out
}

fn render_instruction(inst: &crate::backend_text::BackendTextInstruction) -> String {
    match inst {
        crate::backend_text::BackendTextInstruction::Mov { dest, src } => {
            format!("mov {}, {}", render_slot(dest), render_operand(src))
        }
        crate::backend_text::BackendTextInstruction::Unary { dest, op, operand } => {
            format!(
                "{} {}, {}",
                render_unary(*op),
                render_temp(*dest),
                render_operand(operand)
            )
        }
        crate::backend_text::BackendTextInstruction::Binary { dest, op, lhs, rhs } => format!(
            "{} {}, {}, {}",
            render_binop(*op),
            render_temp(*dest),
            render_operand(lhs),
            render_operand(rhs)
        ),
        crate::backend_text::BackendTextInstruction::Call {
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
                (Some(dest), _) => format!("call {}, {} ; -> {}", callee, args, render_temp(*dest)),
                (None, TypeIR::Nulo) => format!("call {}, {} ; void", callee, args),
                (None, _) => format!("; call inválida: {}({})", callee, args),
            }
        }
    }
}

fn render_terminator(
    term: &crate::backend_text::BackendTextTerminator,
    function_name: &str,
) -> String {
    match term {
        crate::backend_text::BackendTextTerminator::Jump(label) => {
            format!("jmp .L{}_{}", function_name, label)
        }
        crate::backend_text::BackendTextTerminator::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, .L{}_{}, .L{}_{}",
            render_operand(cond),
            function_name,
            then_label,
            function_name,
            else_label
        ),
        crate::backend_text::BackendTextTerminator::Return(Some(value)) => {
            format!("ret {}", render_operand(value))
        }
        crate::backend_text::BackendTextTerminator::Return(None) => "ret".to_string(),
    }
}

fn render_unary(op: UnaryOpIR) -> &'static str {
    match op {
        UnaryOpIR::Neg => "neg",
        UnaryOpIR::Not => "not",
    }
}

fn render_binop(op: BinaryOpIR) -> &'static str {
    match op {
        BinaryOpIR::LogicalAnd => "and",
        BinaryOpIR::LogicalOr => "or",
        BinaryOpIR::BitAnd => "and",
        BinaryOpIR::BitOr => "or",
        BinaryOpIR::BitXor => "xor",
        BinaryOpIR::Shl => "shl",
        BinaryOpIR::Shr => "shr",
        BinaryOpIR::Add => "add",
        BinaryOpIR::Sub => "sub",
        BinaryOpIR::Mul => "mul",
        BinaryOpIR::Div => "div",
        BinaryOpIR::Mod => "mod",
        BinaryOpIR::Eq => "cmp_eq",
        BinaryOpIR::Neq => "cmp_ne",
        BinaryOpIR::Lt => "cmp_lt",
        BinaryOpIR::Lte => "cmp_le",
        BinaryOpIR::Gt => "cmp_gt",
        BinaryOpIR::Gte => "cmp_ge",
    }
}

fn render_operand(op: &crate::cfg_ir::OperandIR) -> String {
    match op {
        crate::cfg_ir::OperandIR::Local(slot) => render_slot(slot),
        crate::cfg_ir::OperandIR::GlobalConst(name) => format!("{}(%rip)", name),
        crate::cfg_ir::OperandIR::Int(v) => v.to_string(),
        crate::cfg_ir::OperandIR::Bool(v) => {
            if *v {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        crate::cfg_ir::OperandIR::Temp(temp) => render_temp(*temp),
    }
}

fn render_temp(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}

fn render_slot(slot: &str) -> String {
    format!("${}", slot)
}

fn join_or_empty(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        values.join(", ")
    }
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}

fn err(msg: &str) -> PinkerError {
    PinkerError::BackendTextValidation {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}
