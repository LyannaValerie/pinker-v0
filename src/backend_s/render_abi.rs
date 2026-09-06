//! Renderização do `.s` textual da ABI mínima, movida de `src/backend_s.rs`
//! pela unidade BS-2 do inventário da #601 (Task #612).
//!
//! Só o arquivo mudou: as três regiões cartografadas, as treze funções e a
//! ordem em que decidem continuam exatamente como estavam. `super` mudou de
//! significado ao descer um nível, e o `use` abaixo devolve ao irmão o
//! vocabulário do pai — `BackendTextProgram`, os tipos da IR, `native_symbol`,
//! as constantes freestanding e os helpers `line`/`err` — sem promover nada:
//! um filho enxerga os itens privados do pai por privacidade de módulo, e este
//! `use` é privado.
//!
//! `render_program` já era `pub` antes do move. O pai a reexporta para que
//! `pinker_v0::backend_s::render_program` continue sendo o mesmo caminho
//! público; nenhum outro item ganhou visibilidade.

use super::*;

// @pinker-nav:start backend-s.renderizacao.abi-textual-programa
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary `render_program`: renderer do `.s` **textual** baseado em `BackendTextProgram` (caminho `emit_from_selected`), distinto do renderer montável. Emite cabeçalho, `module`, `mode` livre/hospedado, metadados `abi.*` **como comentários** (`; abi.func`/`abi.params`/`abi.ret`/`abi.frame`/`abi.prologue`/`abi.epilogue`), `.rodata` de globais e blocos. No modo freestanding embute `boot.entry`, o linker script e o kernel stub textuais e um loop `.Lpinker_hang`. **Não** é assembly GAS montável nem ABI SysV real: `mov $slot`/`unop`/`binop` e os `@arg`/`@ret` são convenções textuais, não reconhecíveis diretamente pelo assembler.
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
            &native_symbol::native_binding(NativeDefinition::Entrypoint)
                .directive(FREESTANDING_BOOT_ENTRY_SYMBOL),
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
            line(
                &mut out,
                0,
                &native_symbol::native_binding(NativeDefinition::UserGlobal)
                    .directive(&global.name),
            );
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
        let symbol = native_symbol::function_symbol(NativeSurface::TextualAbi, &function.name);
        let prologue = native_symbol::injective_local_label(&[&function.name, "prologue"]);
        let epilogue = native_symbol::injective_local_label(&[&function.name, "epilogue"]);
        line(
            &mut out,
            0,
            &format!("; abi.frame prologue={} epilogue={}", prologue, epilogue),
        );
        line(
            &mut out,
            0,
            &native_symbol::function_binding(&function.name).directive(&symbol),
        );
        line(&mut out, 0, &format!("{}:", symbol));
        line(&mut out, 1, &format!("{}:", prologue));
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
                &format!(
                    "{}:",
                    native_symbol::injective_local_label(&[&function.name, &block.label])
                ),
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
        line(&mut out, 1, &format!("{}:", epilogue));
        line(&mut out, 2, "; abi.epilogue (textual)");
    }

    out
}
// @pinker-nav:end backend-s.renderizacao.abi-textual-programa

// @pinker-nav:start backend-s.renderizacao.abi-textual-instrucoes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary `render_instruction` e `render_terminator` do `.s` textual: formatam cada `BackendTextInstruction` (`mov`, `unop`, `binop`, `call ; abi.call ... -> ...` com ramo defensivo de call inválida, `falar` com pares `valor:tipo`) e cada `BackendTextTerminator` (`jmp`, `br`, `ret @ret`, `ret_void`). Convenções textuais anotadas — não emitem instruções x86 reais.
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
        crate::backend_text::BackendTextInstruction::PointerOffset {
            dest,
            pointer,
            offset,
            element_size,
            element_align,
        } => format!(
            "pointer_offset {}, {}, {}, size={}, align={}",
            render_temp(*dest),
            render_operand(pointer),
            render_operand(offset),
            element_size,
            element_align
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
        crate::backend_text::BackendTextInstruction::CallRaw {
            dest,
            callee,
            args,
            param_types,
            ret_type,
        } => {
            let call = format!(
                "call_raw {}({}) : ({}) -> {}",
                render_operand(callee),
                args.iter()
                    .map(render_operand)
                    .collect::<Vec<_>>()
                    .join(", "),
                param_types
                    .iter()
                    .map(TypeIR::render_name)
                    .collect::<Vec<_>>()
                    .join(", "),
                ret_type.render_name()
            );
            match dest {
                Some(dest) => format!("{} -> {}", call, render_temp(*dest)),
                None => call,
            }
        }
        crate::backend_text::BackendTextInstruction::MakeTraitObject {
            dest,
            value,
            trait_name,
            concrete_type_name,
            concrete_size,
            vtable_methods,
            ..
        } => format!(
            "make_trait_object {} <- {} as trato<{}> snapshot={}({}) vtable=[{}]",
            render_temp(*dest),
            render_operand(value),
            trait_name,
            concrete_size,
            concrete_type_name,
            vtable_methods.join(", ")
        ),
        crate::backend_text::BackendTextInstruction::TraitCall {
            dest,
            object,
            trait_name,
            method_name,
            method_slot,
            args,
            ret_type,
            ..
        } => {
            let call_site = format!(
                "trait_call trato<{}>.{}[{}]({})",
                trait_name,
                method_name,
                method_slot,
                args.iter()
                    .map(render_operand)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            match (dest, ret_type) {
                (Some(dest), _) => format!(
                    "{} {} ; abi.trait_call object={} -> {}",
                    call_site,
                    ret_type.name(),
                    render_operand(object),
                    render_temp(*dest)
                ),
                (None, TypeIR::Nulo) => format!(
                    "{} void ; abi.trait_call object={}",
                    call_site,
                    render_operand(object)
                ),
                (None, _) => format!("; trait_call inválida: {}", call_site),
            }
        }
        crate::backend_text::BackendTextInstruction::Falar { args } => format!(
            "falar {}",
            args.iter()
                .map(|arg| format!("{}:{}", render_operand(&arg.value), arg.ty.name()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        crate::backend_text::BackendTextInstruction::InlineAsm {
            chunks,
            operands,
            clobbers,
        } => {
            format!(
                "inline_asm {:?} operands={} clobbers={:?}",
                chunks,
                operands.len(),
                clobbers
            )
        }
        crate::backend_text::BackendTextInstruction::UnionInject {
            dest,
            value,
            union_type_id,
            tag,
        } => format!(
            "union_inject %{} #{} tag={} {}",
            dest.0,
            union_type_id.0,
            tag,
            render_operand(value)
        ),
        crate::backend_text::BackendTextInstruction::UnionTag {
            dest,
            value,
            union_type_id,
        } => format!(
            "union_tag %{} #{} {}",
            dest.0,
            union_type_id.0,
            render_operand(value)
        ),
        crate::backend_text::BackendTextInstruction::UnionExtract {
            dest,
            value,
            union_type_id,
            tag,
            canonical_member_key,
            payload_type,
        } => format!(
            "union_extract %{} #{} tag={} key={} {} -> {}",
            dest.0,
            union_type_id.0,
            tag,
            canonical_member_key,
            render_operand(value),
            payload_type.name()
        ),
    }
}

fn render_terminator(
    term: &crate::backend_text::BackendTextTerminator,
    function_name: &str,
) -> String {
    match term {
        crate::backend_text::BackendTextTerminator::Jump(label) => {
            format!(
                "jmp {}",
                native_symbol::injective_local_label(&[function_name, label])
            )
        }
        crate::backend_text::BackendTextTerminator::Branch {
            cond,
            then_label,
            else_label,
        } => format!(
            "br {}, {}, {}",
            render_operand(cond),
            native_symbol::injective_local_label(&[function_name, then_label]),
            native_symbol::injective_local_label(&[function_name, else_label])
        ),
        crate::backend_text::BackendTextTerminator::Return(Some(value)) => {
            format!("ret @ret, {}", render_operand(value))
        }
        crate::backend_text::BackendTextTerminator::Return(None) => "ret_void".to_string(),
    }
}
// @pinker-nav:end backend-s.renderizacao.abi-textual-instrucoes

// @pinker-nav:start backend-s.renderizacao.abi-textual-componentes
// @pinker-nav:domain renderizacao
// @pinker-nav:layer backend-s
// @pinker-nav:summary Componentes do renderer `.s` textual: `render_unary`/`render_binop` (nomes de operador), `render_operand` (locais `$slot`, globais `@nome(%rip)`, inteiros, `1`/`0`, strings entre aspas **sem escape**, temporários `%tN`), `render_temp`, `render_slot`, `join_or_empty` e os helpers de metadado `render_abi_params`/`render_abi_return`/`render_call_site`/`render_abi_call_args` (`@arg`/`@ret`, comentários). Serializam elementos individuais da representação textual; não produzem código nativo.
fn render_unary(op: UnaryOpIR) -> &'static str {
    match op {
        UnaryOpIR::Neg => "neg",
        UnaryOpIR::Not => "not",
        UnaryOpIR::BitNot => "bitnot",
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
        crate::cfg_ir::OperandIR::FunctionRef(name) => format!("fnref({})", name),
        crate::cfg_ir::OperandIR::RawFunctionRef(name) => format!("raw_fnref({})", name),
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
// @pinker-nav:end backend-s.renderizacao.abi-textual-componentes
