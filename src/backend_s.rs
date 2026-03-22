use crate::backend_text;
use crate::backend_text::BackendTextProgram;
use crate::boot::{
    freestanding_kernel_stub, freestanding_linker_script, FREESTANDING_BOOT_ENTRY_FUNCTION,
    FREESTANDING_BOOT_ENTRY_SYMBOL,
};
use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::{BinaryOpIR, TypeIR, UnaryOpIR};
use crate::token::{Position, Span};
use std::collections::{BTreeSet, HashMap};

pub fn emit_from_selected(selected: &SelectedProgram) -> Result<String, PinkerError> {
    validate_supported_subset(selected)?;
    let lowered = backend_text::lower_selected_program(selected)?;
    Ok(render_program(&lowered))
}

/// Emite um `.s` mínimo montável por toolchain externa (assembler+linker do sistema).
///
/// Escopo deliberadamente mínimo para a Fase 83:
/// - target assumido: Linux x86_64 (SysV) hospedado;
/// - subset aceito: funções `-> bombom` com bloco único linear, incluindo `call` direta;
/// - disciplina mínima de registradores/frame: `%rax` (retorno/acumulador), `%rdi` (arg0), `%rsi` (arg1), `%r10` (temporário volátil), slots em frame `%rbp`;
/// - memória mínima real garantida: load/store em slots de frame via `movq -off(%rbp), %reg` e `movq %reg, -off(%rbp)`;
/// - sem globais, sem fluxo de controle e sem ABI completa.
///
/// O resultado mapeia `principal` para o símbolo `main`, para permitir linkedição
/// via driver C (`cc`/`gcc`/`clang`) sem runtime próprio.
pub fn emit_external_toolchain_subset(selected: &SelectedProgram) -> Result<String, PinkerError> {
    let program = extract_external_callconv_program(selected)?;
    Ok(render_external_x86_64_linux_callconv(&program))
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

struct ExternalCallConvProgram {
    functions: Vec<ExternalCallConvFunction>,
}

struct ExternalCallConvFunction {
    name: String,
    stack_size: u32,
    slot_offsets: HashMap<String, u32>,
    body: Vec<String>,
    ret: OperandIR,
    params: Vec<String>,
}

const REG_RET: &str = "%rax";
const ARG_REGS: [&str; 2] = ["%rdi", "%rsi"];
const REG_TMP: &str = "%r10";

fn extract_external_callconv_program(
    selected: &SelectedProgram,
) -> Result<ExternalCallConvProgram, PinkerError> {
    if !selected.globals.is_empty() {
        return Err(err(
            "subset externo montável (Fase 83) não suporta globais; esperado programa sem `eterno`",
        ));
    }

    let has_main = selected.functions.iter().any(|f| f.name == "principal");
    if !has_main {
        return Err(err(
            "subset externo montável (Fase 83) exige função `principal`",
        ));
    }

    let mut functions = Vec::new();
    for function in &selected.functions {
        if function.ret_type != TypeIR::Bombom {
            return Err(err(
                "subset externo montável (Fase 83) exige retorno `bombom` em todas as funções",
            ));
        }
        if function.name == "principal" && !function.params.is_empty() {
            return Err(err(
                "subset externo montável (Fase 83) exige `principal()` sem parâmetros",
            ));
        }
        if function.params.len() > ARG_REGS.len() {
            return Err(err(
                "subset externo montável (Fase 83) recusa explicitamente 3+ parâmetros por função; limite garantido: até 2 parâmetros `bombom`",
            ));
        }
        for param in &function.params {
            let Some(ty) = function.slot_types.get(param) else {
                return Err(err(
                    "subset externo montável (Fase 83) encontrou parâmetro sem tipo",
                ));
            };
            if *ty != TypeIR::Bombom {
                return Err(err(
                    "subset externo montável (Fase 83) aceita somente parâmetro `bombom`",
                ));
            }
        }
        for local in &function.locals {
            let Some(ty) = function.slot_types.get(local) else {
                return Err(err(
                    "subset externo montável (Fase 83) encontrou local sem tipo",
                ));
            };
            if *ty != TypeIR::Bombom {
                return Err(err(&format!(
                    "subset externo montável (Fase 83) só aceita local `bombom`; '{}' é '{}'",
                    local,
                    ty.name()
                )));
            }
        }
        if has_control_flow_talvez_senao(function) {
            return Err(err(
                "subset externo montável (Fase 83) recusa explicitamente `talvez/senao`; controle de fluxo geral continua fora do subset externo",
            ));
        }
        if function.blocks.len() != 1 {
            return Err(err(
                "subset externo montável (Fase 83) exige bloco único por função",
            ));
        }

        let block = &function.blocks[0];
        let ret = match &block.terminator {
            SelectedTerminator::Ret(Some(value)) => value.clone(),
            _ => {
                return Err(err(
                    "subset externo montável (Fase 83) exige `mimo <valor>;` em cada função",
                ))
            }
        };

        let temp_ids = collect_temp_ids(function, &ret);
        let mut slot_offsets = HashMap::new();
        let mut slot_index = 1u32;
        for param in &function.params {
            slot_offsets.insert(param.clone(), slot_index * 8);
            slot_index += 1;
        }
        for local in &function.locals {
            slot_offsets.insert(local.clone(), slot_index * 8);
            slot_index += 1;
        }
        for temp in temp_ids {
            slot_offsets.insert(temp, slot_index * 8);
            slot_index += 1;
        }
        let raw_stack = (slot_index.saturating_sub(1)) * 8;
        let stack_size = if raw_stack == 0 {
            0
        } else {
            raw_stack.div_ceil(16) * 16
        };

        let mut body = Vec::new();
        for inst in &block.instructions {
            match inst {
                SelectedInstr::Mov { dest, src } => {
                    ensure_dest_is_local_or_param(dest, function)?;
                    body.extend(load_operand(REG_RET, src, &slot_offsets)?);
                    body.push(format!("movq {}, -{}(%rbp)", REG_RET, slot_offsets[dest]));
                }
                SelectedInstr::Add { dest, lhs, rhs } => {
                    body.extend(lower_linear_binop("addq", *dest, lhs, rhs, &slot_offsets)?);
                }
                SelectedInstr::Sub { dest, lhs, rhs } => {
                    body.extend(lower_linear_binop("subq", *dest, lhs, rhs, &slot_offsets)?);
                }
                SelectedInstr::Mul { dest, lhs, rhs } => {
                    body.extend(lower_linear_binop("imulq", *dest, lhs, rhs, &slot_offsets)?);
                }
                SelectedInstr::Call {
                    dest,
                    callee,
                    args,
                    ret_type,
                } => {
                    if *ret_type != TypeIR::Bombom {
                        return Err(err(
                            "subset externo montável (Fase 83) só aceita call com retorno `bombom`",
                        ));
                    }
                    if !selected.functions.iter().any(|f| &f.name == callee) {
                        return Err(err(
                            "subset externo montável (Fase 83) encontrou call para função inexistente",
                        ));
                    }
                    if callee == &function.name {
                        return Err(err(
                            "subset externo montável (Fase 83) não suporta recursão externa",
                        ));
                    }
                    if args.len() > ARG_REGS.len() {
                        return Err(err(
                            "subset externo montável (Fase 83) recusa explicitamente call com 3+ argumentos; limite garantido: até 2 argumentos `bombom`",
                        ));
                    }
                    for (idx, arg) in args.iter().enumerate() {
                        body.extend(load_operand(ARG_REGS[idx], arg, &slot_offsets)?);
                    }
                    body.push(format!("call {}", callee));
                    body.push(format!(
                        "movq {}, -{}(%rbp)",
                        REG_RET,
                        slot_offsets[&temp_key(*dest)]
                    ));
                }
                _ => {
                    return Err(err(
                        "subset externo montável (Fase 83) aceita apenas atribuição, aritmética linear (+,-,*), call direta com até 2 argumentos `bombom` e load/store em slots de frame",
                    ));
                }
            }
        }

        functions.push(ExternalCallConvFunction {
            name: function.name.clone(),
            stack_size,
            slot_offsets,
            body,
            ret,
            params: function.params.clone(),
        });
    }

    Ok(ExternalCallConvProgram { functions })
}

fn render_external_x86_64_linux_callconv(program: &ExternalCallConvProgram) -> String {
    let mut out = String::new();
    line(
        &mut out,
        0,
        "# pinker v0 external toolchain subset (fase 83, linux x86_64, frame/reg + memoria minima + recusa 3+ params + recusa talvez/senao)",
    );
    line(&mut out, 0, ".text");

    for function in &program.functions {
        let symbol = if function.name == "principal" {
            "main".to_string()
        } else {
            function.name.clone()
        };
        line(&mut out, 0, &format!(".globl {}", symbol));
        line(&mut out, 0, &format!("{}:", symbol));
        line(&mut out, 1, "pushq %rbp");
        line(&mut out, 1, "movq %rsp, %rbp");
        if function.stack_size > 0 {
            line(&mut out, 1, &format!("subq ${}, %rsp", function.stack_size));
        }
        for (idx, param) in function.params.iter().enumerate() {
            line(
                &mut out,
                1,
                &format!(
                    "movq {}, -{}(%rbp)",
                    ARG_REGS[idx], function.slot_offsets[param]
                ),
            );
        }
        if function.stack_size > 0 {
            line(
                &mut out,
                1,
                &format!(
                    "# frame: %rbp base + {} bytes de slots",
                    function.stack_size
                ),
            );
        }
        for stmt in &function.body {
            line(&mut out, 1, stmt);
        }
        for stmt in load_operand(REG_RET, &function.ret, &function.slot_offsets)
            .expect("retorno deve ser carregável")
        {
            line(&mut out, 1, &stmt);
        }
        line(&mut out, 1, "leave");
        line(&mut out, 1, "ret");
    }
    out
}

fn ensure_dest_is_local_or_param(
    dest: &str,
    function: &crate::instr_select::SelectedFunction,
) -> Result<(), PinkerError> {
    if function.locals.iter().any(|local| local == dest)
        || function.params.iter().any(|param| param == dest)
    {
        Ok(())
    } else {
        Err(err(
            "subset externo montável (Fase 83) só aceita escrita em parâmetros ou variáveis locais declaradas",
        ))
    }
}

fn lower_linear_binop(
    opcode: &str,
    dest: crate::cfg_ir::TempIR,
    lhs: &OperandIR,
    rhs: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut body = Vec::new();
    body.extend(load_operand(REG_RET, lhs, slot_offsets)?);
    body.extend(load_operand(REG_TMP, rhs, slot_offsets)?);
    body.push(format!("{} {}, {}", opcode, REG_TMP, REG_RET));
    body.push(format!(
        "movq {}, -{}(%rbp)",
        REG_RET,
        slot_offsets[&temp_key(dest)]
    ));
    Ok(body)
}

fn collect_temp_ids(
    function: &crate::instr_select::SelectedFunction,
    ret: &OperandIR,
) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for block in &function.blocks {
        for inst in &block.instructions {
            match inst {
                SelectedInstr::Neg { dest, .. }
                | SelectedInstr::Not { dest, .. }
                | SelectedInstr::DerefLoad { dest, .. }
                | SelectedInstr::Cast { dest, .. }
                | SelectedInstr::BitAnd { dest, .. }
                | SelectedInstr::BitOr { dest, .. }
                | SelectedInstr::BitXor { dest, .. }
                | SelectedInstr::Shl { dest, .. }
                | SelectedInstr::Shr { dest, .. }
                | SelectedInstr::Add { dest, .. }
                | SelectedInstr::Sub { dest, .. }
                | SelectedInstr::Mul { dest, .. }
                | SelectedInstr::Div { dest, .. }
                | SelectedInstr::Mod { dest, .. }
                | SelectedInstr::CmpEq { dest, .. }
                | SelectedInstr::CmpNe { dest, .. }
                | SelectedInstr::CmpLt { dest, .. }
                | SelectedInstr::CmpLe { dest, .. }
                | SelectedInstr::CmpGt { dest, .. }
                | SelectedInstr::CmpGe { dest, .. }
                | SelectedInstr::Call { dest, .. } => {
                    ids.insert(temp_key(*dest));
                }
                _ => {}
            }
        }
    }
    if let OperandIR::Temp(temp) = ret {
        ids.insert(temp_key(*temp));
    }
    ids
}

fn load_operand(
    reg: &str,
    operand: &OperandIR,
    slot_offsets: &HashMap<String, u32>,
) -> Result<Vec<String>, PinkerError> {
    let mut lines = Vec::new();
    match operand {
        OperandIR::Int(v) => lines.push(format!("movabsq ${}, {}", v, reg)),
        OperandIR::Local(slot) => {
            let Some(offset) = slot_offsets.get(slot) else {
                return Err(err(
                    "subset externo montável (Fase 83) encontrou slot sem offset",
                ));
            };
            lines.push(format!("movq -{}(%rbp), {}", offset, reg));
        }
        OperandIR::Temp(temp) => {
            let key = temp_key(*temp);
            let Some(offset) = slot_offsets.get(&key) else {
                return Err(err(
                    "subset externo montável (Fase 83) encontrou temporário sem offset",
                ));
            };
            lines.push(format!("movq -{}(%rbp), {}", offset, reg));
        }
        _ => {
            return Err(err(
                "subset externo montável (Fase 83) só aceita operandos inteiros, locais e temporários",
            ));
        }
    }
    Ok(lines)
}

fn temp_key(temp: crate::cfg_ir::TempIR) -> String {
    format!("%t{}", temp.0)
}

fn has_control_flow_talvez_senao(function: &crate::instr_select::SelectedFunction) -> bool {
    function
        .blocks
        .iter()
        .any(|block| block.label.starts_with("then_") || block.label.starts_with("else_"))
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
    if program.is_freestanding {
        line(
            &mut out,
            0,
            &format!(
                "; boot.entry {} -> {}",
                FREESTANDING_BOOT_ENTRY_FUNCTION, FREESTANDING_BOOT_ENTRY_SYMBOL
            ),
        );
        line(&mut out, 0, "; linker.script.v0 (textual, mínimo):");
        for script_line in freestanding_linker_script().lines() {
            line(&mut out, 0, &format!(";   {}", script_line));
        }
        line(&mut out, 0, "; kernel.stub.v0 (experimental):");
        for stub_line in freestanding_kernel_stub().lines() {
            line(&mut out, 0, &format!(";   {}", stub_line));
        }
    }
    line(&mut out, 0, ".text");

    if program.is_freestanding {
        line(
            &mut out,
            0,
            &format!(".globl {}", FREESTANDING_BOOT_ENTRY_SYMBOL),
        );
        line(&mut out, 0, &format!("{}:", FREESTANDING_BOOT_ENTRY_SYMBOL));
        line(
            &mut out,
            1,
            &format!("call {}", FREESTANDING_BOOT_ENTRY_FUNCTION),
        );
        line(&mut out, 0, ".Lpinker_hang:");
        line(&mut out, 1, "jmp .Lpinker_hang");
    }

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
        crate::backend_text::BackendTextInstruction::Falar { value, ty } => {
            format!("falar {}:{}", render_operand(value), ty.name())
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
        UnaryOpIR::Deref => "deref",
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
        crate::cfg_ir::OperandIR::Str(s) => format!("\"{}\"", s),
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
