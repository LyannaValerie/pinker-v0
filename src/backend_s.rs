use crate::backend_text;
use crate::backend_text::BackendTextProgram;
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};
use crate::token::{Position, Span};

pub fn emit_from_selected(selected: &SelectedProgram) -> Result<String, PinkerError> {
    validate_supported_subset(selected)?;
    let lowered = backend_text::lower_selected_program(selected)?;
    Ok(render_program(&lowered))
}

/// Emite um `.s` mínimo montável por toolchain externa (assembler+linker do sistema).
///
/// Escopo deliberadamente mínimo para a Fase 55:
/// - target assumido: Linux x86_64 (SysV) hospedado;
/// - subset aceito: apenas `principal() -> bombom` com retorno inteiro constante;
/// - sem globals, sem parâmetros, sem fluxo de controle e sem chamadas.
///
/// O resultado mapeia `principal` para o símbolo `main`, para permitir linkedição
/// via driver C (`cc`/`gcc`/`clang`) sem runtime próprio.
pub fn emit_external_toolchain_subset(selected: &SelectedProgram) -> Result<String, PinkerError> {
    let const_ret = extract_external_constant_return(selected)?;
    Ok(render_external_x86_64_linux_main(const_ret))
}

fn validate_supported_subset(selected: &SelectedProgram) -> Result<(), PinkerError> {
    for function in &selected.functions {
        if !is_supported_type(function.ret_type) {
            return Err(err(&format!(
                "backend .s textual da Fase 54 ainda não suporta retorno '{}' em '{}'",
                function.ret_type.name(),
                function.name
            )));
        }

        for (slot, ty) in &function.slot_types {
            if !is_supported_type(*ty) {
                return Err(err(&format!(
                    "backend .s textual da Fase 54 ainda não suporta slot '{}' do tipo '{}' em '{}'",
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
                            "backend .s textual da Fase 54 ainda não suporta call com retorno '{}'",
                            ret_type.name()
                        )));
                    }
                }
            }
        }
    }

    Ok(())
}

fn extract_external_constant_return(selected: &SelectedProgram) -> Result<u64, PinkerError> {
    if !selected.globals.is_empty() {
        return Err(err(
            "integração externa mínima (Fase 55) não suporta globais; esperado programa sem `eterno`",
        ));
    }
    if selected.functions.len() != 1 {
        return Err(err(
            "integração externa mínima (Fase 55) exige exatamente uma função: `principal`",
        ));
    }

    let function = &selected.functions[0];
    if function.name != "principal" {
        return Err(err(
            "integração externa mínima (Fase 55) exige função única chamada `principal`",
        ));
    }
    if function.ret_type != TypeIR::Bombom {
        return Err(err(
            "integração externa mínima (Fase 55) exige `principal() -> bombom`",
        ));
    }
    if !function.params.is_empty() || !function.locals.is_empty() {
        return Err(err(
            "integração externa mínima (Fase 55) não suporta parâmetros/locais em `principal`",
        ));
    }
    if function.blocks.len() != 1 {
        return Err(err(
            "integração externa mínima (Fase 55) exige bloco único em `principal`",
        ));
    }

    let block = &function.blocks[0];
    if !block.instructions.is_empty() {
        return Err(err(
            "integração externa mínima (Fase 55) exige `principal` sem instruções intermediárias",
        ));
    }

    match &block.terminator {
        SelectedTerminator::Ret(Some(OperandIR::Int(v))) => Ok(*v),
        _ => Err(err(
            "integração externa mínima (Fase 55) exige `mimo <inteiro_constante>;` em `principal`",
        )),
    }
}

fn render_external_x86_64_linux_main(ret: u64) -> String {
    let mut out = String::new();
    line(
        &mut out,
        0,
        "# pinker v0 external toolchain subset (fase 55, linux x86_64)",
    );
    line(&mut out, 0, ".text");
    line(&mut out, 0, ".globl main");
    line(&mut out, 0, "main:");
    line(&mut out, 1, &format!("movq ${}, %rax", ret));
    line(&mut out, 1, "ret");
    out
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
        "; pinker v0 textual .s (fase 54, abi textual minima, derivado de --selected)",
    );
    line(&mut out, 0, &format!("; module {}", program.module_name));
    line(
        &mut out,
        0,
        &format!(
            "; mode {}",
            if program.is_freestanding {
                "livre (freestanding intent)"
            } else {
                "hospedado"
            }
        ),
    );
    line(&mut out, 0, "; abi pinker.text.v0");
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
        line(&mut out, 0, &format!("; abi.func {}", function.name));
        line(
            &mut out,
            0,
            &format!("; abi.params {}", render_abi_params(function)),
        );
        line(
            &mut out,
            0,
            &format!("; abi.ret {}", render_abi_return(function.ret_type)),
        );
        line(
            &mut out,
            0,
            &format!(
                "; abi.frame prologue=.L{}_prologue epilogue=.L{}_epilogue",
                function.name, function.name
            ),
        );
        line(&mut out, 0, &format!(".globl {}", function.name));
        line(&mut out, 0, &format!("{}:", function.name));
        line(&mut out, 1, &format!(".L{}_prologue:", function.name));
        line(&mut out, 2, "; abi.prologue (textual)");
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
        line(&mut out, 1, &format!(".L{}_epilogue:", function.name));
        line(&mut out, 2, "; abi.epilogue (textual)");
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
            let call_site = render_call_site(callee, args);
            let abi_args = render_abi_call_args(args);

            match (dest, ret_type) {
                (Some(dest), _) => format!(
                    "{} ; abi.call {} -> {}",
                    call_site,
                    abi_args,
                    render_temp(*dest)
                ),
                (None, TypeIR::Nulo) => format!("{} ; abi.call {} -> void", call_site, abi_args),
                (None, _) => format!("; call inválida: {} {}", callee, abi_args),
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
            format!("ret @ret, {}", render_operand(value))
        }
        crate::backend_text::BackendTextTerminator::Return(None) => "ret_void".to_string(),
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

fn render_abi_params(function: &crate::backend_text::BackendTextFunction) -> String {
    if function.params.is_empty() {
        return "[]".to_string();
    }

    let rendered = function
        .params
        .iter()
        .enumerate()
        .map(|(idx, slot)| format!("@arg{}={}", idx, render_slot(slot)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", rendered)
}

fn render_abi_return(ret_type: TypeIR) -> String {
    if ret_type == TypeIR::Nulo {
        "void".to_string()
    } else {
        "@ret".to_string()
    }
}

fn render_call_site(callee: &str, args: &[crate::cfg_ir::OperandIR]) -> String {
    if args.is_empty() {
        format!("call {}", callee)
    } else {
        let args = args
            .iter()
            .map(render_operand)
            .collect::<Vec<_>>()
            .join(", ");
        format!("call {}, {}", callee, args)
    }
}

fn render_abi_call_args(args: &[crate::cfg_ir::OperandIR]) -> String {
    if args.is_empty() {
        "[]".to_string()
    } else {
        let args = args
            .iter()
            .enumerate()
            .map(|(idx, operand)| format!("@arg{}={}", idx, render_operand(operand)))
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{}]", args)
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
