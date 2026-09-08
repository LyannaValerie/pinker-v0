//! Renderização textual auditável da IR estruturada, movida de `src/ir.rs` pela
//! unidade IR-4 do inventário da #601 (Task #626).
//!
//! Só o arquivo mudou: a região cartografada `ir.renderizacao.textual` — os
//! seis corpos `render_function`, `render_block`, `render_instruction`,
//! `render_enum_pattern`, `render_value` e o helper `line` — chega aqui na
//! mesma ordem, com os mesmos corpos e o mesmo texto produzido. `super` mudou
//! de significado ao descer um nível, e o `use` abaixo devolve ao irmão o
//! vocabulário do pai — `FunctionIR`, `BlockIR`, `InstructionIR`,
//! `EnumPatternIR`, `ValueIR`, `TypeIR` — sem promover nada: um filho enxerga
//! os itens privados do pai por privacidade de módulo, e este `use` é privado.
//!
//! A entrada pública `render_program` não é da unidade e continua no pai, junto
//! da orquestração: ela recebe a `ProgramIR` pronta, imprime módulo, modo e
//! constantes, e delega a estas funções. Este arquivo é implementação física da
//! mesma renderização, não uma camada nova.
//!
//! Nada aqui altera a IR, valida invariantes, executa ou gera assembly. A
//! validação da IR continua em `src/ir_validate.rs` e a fronteira de CFG
//! continua em `src/cfg_ir.rs`; nada disso desceu com o corte, e a seleção de
//! método (`crate::method_dispatch`, C2) e o registry declarativo de
//! intrínsecas (`crate::intrinsics::registry`, C1) não são consultados por
//! renderização nenhuma — nem antes nem agora.
//!
//! Nenhum item do corte era `pub` antes do move e nenhum é agora.
//! `render_function`, `render_value` e `line` são os três símbolos que o pai
//! chama de `render_program` e, por isso, os únicos que passaram de privados a
//! `pub(super)` — exatamente os três `exports` que o `unit_costs.json` da #601
//! nomeia para a IR-4.

use super::*;

// @pinker-nav:start ir.renderizacao.textual
// @pinker-nav:domain renderizacao
// @pinker-nav:layer ir
// @pinker-nav:summary Renderização textual auditável da IR já construída: `render_function`/`render_block`/`render_instruction`/`render_value` (com o helper `line`) percorrem `FunctionIR`/`BlockIR`/`InstructionIR`/`ValueIR` e produzem a forma legível consumida por depuração e testes. Recebe uma `ProgramIR` pronta (a entrada pública `render_program` fica junto à orquestração e delega a estas funções); não modifica a IR, não valida invariantes, não executa e não gera assembly.
pub(super) fn render_function(function: &FunctionIR, indent: usize, out: &mut String) {
    line(
        out,
        indent,
        &format!(
            "func {} -> {}",
            function.name,
            function.ret_type.render_name()
        ),
    );

    if function.params.is_empty() {
        line(out, indent + 1, "params: []");
    } else {
        line(out, indent + 1, "params:");
        for param in &function.params {
            line(
                out,
                indent + 2,
                &format!("{}: {}", param.slot, param.ty.render_name()),
            );
        }
    }

    if function.locals.is_empty() {
        line(out, indent + 1, "locals: []");
    } else {
        line(out, indent + 1, "locals:");
        for local in &function.locals {
            let mutability = if local.is_mut { " muda" } else { "" };
            line(
                out,
                indent + 2,
                &format!("{}: {}{}", local.slot, local.ty.render_name(), mutability),
            );
        }
    }

    render_block(&function.entry, indent + 1, out);
}

fn render_block(block: &BlockIR, indent: usize, out: &mut String) {
    line(out, indent, &format!("block {}:", block.label));
    for instruction in &block.instructions {
        render_instruction(instruction, indent + 1, out);
    }
}

fn render_instruction(instruction: &InstructionIR, indent: usize, out: &mut String) {
    match instruction {
        InstructionIR::Let { slot, value, .. } => {
            line(
                out,
                indent,
                &format!("let {} = {}", slot, render_value(value)),
            );
        }
        InstructionIR::Assign { slot, value, .. } => {
            line(
                out,
                indent,
                &format!("assign {} = {}", slot, render_value(value)),
            );
        }
        InstructionIR::StoreIndirect { ptr, value, .. } => {
            line(
                out,
                indent,
                &format!(
                    "store_indirect {} <- {}",
                    render_value(ptr),
                    render_value(value)
                ),
            );
        }
        InstructionIR::StoreIndexed {
            base, index, value, ..
        } => {
            line(
                out,
                indent,
                &format!(
                    "store_indexed {}[{}] <- {}",
                    render_value(base),
                    render_value(index),
                    render_value(value)
                ),
            );
        }
        InstructionIR::StoreFieldIndirect {
            base,
            field,
            field_offset,
            value,
            ..
        } => {
            line(
                out,
                indent,
                &format!(
                    "store_field_indirect {}.{}/*+{}*/ <- {}",
                    render_value(base),
                    field,
                    field_offset,
                    render_value(value)
                ),
            );
        }
        InstructionIR::Expr { value, .. } => {
            line(out, indent, &format!("expr {}", render_value(value)));
        }
        InstructionIR::Return { value, .. } => match value {
            Some(value) => line(out, indent, &format!("return {}", render_value(value))),
            None => line(out, indent, "return"),
        },
        InstructionIR::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            line(out, indent, &format!("if {}", render_value(condition)));
            render_block(then_block, indent + 1, out);
            if let Some(else_block) = else_block {
                render_block(else_block, indent + 1, out);
            }
        }
        InstructionIR::While {
            condition,
            body_block,
            ..
        } => {
            line(out, indent, &format!("while {}", render_value(condition)));
            render_block(body_block, indent + 1, out);
        }
        InstructionIR::Break {
            loop_exit_label, ..
        } => {
            line(out, indent, &format!("break {}", loop_exit_label));
        }
        InstructionIR::Continue {
            loop_continue_label,
            ..
        } => {
            line(out, indent, &format!("continue {}", loop_continue_label));
        }
        InstructionIR::Falar { args, .. } => {
            let rendered_args = args
                .iter()
                .map(|arg| format!("{}:{}", render_value(&arg.value), arg.ty.name()))
                .collect::<Vec<_>>()
                .join(", ");
            line(out, indent, &format!("falar {}", rendered_args));
        }
        InstructionIR::InlineAsm {
            chunks,
            operands,
            clobbers,
            ..
        } => {
            line(
                out,
                indent,
                &format!(
                    "inline_asm [{}] operands={} clobbers={:?}",
                    chunks.join(" | "),
                    operands.len(),
                    clobbers
                ),
            );
        }
        InstructionIR::EnumMatch(enum_match) => {
            line(
                out,
                indent,
                &format!(
                    "enum_match alvo={} {}",
                    enum_match.scrutinee_binding.slot,
                    render_value(&enum_match.scrutinee)
                ),
            );
            for arm in &enum_match.arms {
                render_enum_pattern(&arm.pattern, indent + 1, out);
                render_block(&arm.body, indent + 2, out);
            }
            if let Some(otherwise) = &enum_match.otherwise {
                line(out, indent + 1, "otherwise");
                render_block(otherwise, indent + 2, out);
            }
        }
        InstructionIR::UnionMatch(union_match) => {
            line(
                out,
                indent,
                &format!(
                    "union_match #{} alvo={} tag={} {}",
                    union_match.union_type_id.0,
                    union_match.scrutinee_binding.slot,
                    union_match.tag_binding.slot,
                    render_value(&union_match.scrutinee)
                ),
            );
            for arm in &union_match.arms {
                line(
                    out,
                    indent + 1,
                    &format!(
                        "arm tag={} key={} {} : {}",
                        arm.tag,
                        arm.canonical_member_key,
                        arm.binding.slot,
                        arm.payload_type.render_name()
                    ),
                );
                render_block(&arm.body, indent + 2, out);
            }
        }
    }
}

fn render_enum_pattern(pattern: &EnumPatternIR, indent: usize, out: &mut String) {
    match pattern {
        EnumPatternIR::Binding { binding, .. } => {
            line(out, indent, &format!("bind {}", binding.slot));
        }
        EnumPatternIR::Variant {
            enum_name,
            variant_name,
            discriminant,
            payloads,
            ..
        } => {
            line(
                out,
                indent,
                &format!(
                    "pattern {}.{} tag={}",
                    enum_name, variant_name, discriminant
                ),
            );
            for payload in payloads {
                line(
                    out,
                    indent + 1,
                    &format!(
                        "payload {} {} {} via {}",
                        payload.index,
                        payload.operational_type.render_name(),
                        payload.canonical_key,
                        payload.extract_intrinsic
                    ),
                );
                render_enum_pattern(&payload.pattern, indent + 2, out);
            }
        }
    }
}

pub(super) fn render_value(value: &ValueIR) -> String {
    match value {
        ValueIR::Local(slot) => slot.clone(),
        ValueIR::GlobalConst(name) => format!("@{}", name),
        ValueIR::Int(value) => format!("{}:bombom", value),
        ValueIR::Bool(value) => format!("{}:logica", if *value { "verdade" } else { "falso" }),
        ValueIR::String(value) => format!("\"{}\":verso", value),
        ValueIR::Unary { op, operand, ty } => {
            format!("{}<{}>({})", op.name(), ty.name(), render_value(operand))
        }
        ValueIR::Deref {
            ptr, is_volatile, ..
        } => {
            if *is_volatile {
                format!("deref_fragil({})", render_value(ptr))
            } else {
                format!("deref({})", render_value(ptr))
            }
        }
        ValueIR::Binary { op, lhs, rhs, ty } => {
            format!(
                "{}<{}>({}, {})",
                op.name(),
                ty.name(),
                render_value(lhs),
                render_value(rhs)
            )
        }
        ValueIR::PointerOffset {
            pointer,
            offset,
            element_size,
            element_align,
            ..
        } => format!(
            "pointer_offset<size={},align={}>({}, {})",
            element_size,
            element_align,
            render_value(pointer),
            render_value(offset)
        ),
        ValueIR::Call {
            callee,
            args,
            ret_type,
            // #532: a impressão da IR mostra a chamada como o usuário a lê. A
            // identidade é interna e não vaza para a superfície textual.
            identidade: _,
        } => format!(
            "call {}({}) -> {}",
            callee,
            args.iter().map(render_value).collect::<Vec<_>>().join(", "),
            ret_type.render_name()
        ),
        ValueIR::FunctionRef(name) => format!("fnref({})", name),
        ValueIR::RawFunctionRef(name) => format!("raw_fnref({})", name),
        ValueIR::MakeClosure {
            function_name,
            captures,
        } => format!(
            "make_closure {}[{}]",
            function_name,
            captures
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ValueIR::MakeTraitObject {
            value,
            trait_name,
            concrete_type,
            concrete_type_name,
            concrete_size,
            vtable_methods,
        } => format!(
            "make_trait_object trato<{}> from {} as {}:{} size={} vtable=[{}]",
            trait_name,
            render_value(value),
            concrete_type_name,
            concrete_type.render_name(),
            concrete_size,
            vtable_methods.join(", ")
        ),
        ValueIR::TraitCall {
            object,
            trait_name,
            method_name,
            method_slot,
            method_count,
            args,
            param_types: _,
            ret_type,
        } => format!(
            "trait_call trato<{}>.{}#{}/{} {}({}) -> {}",
            trait_name,
            method_name,
            method_slot,
            method_count,
            render_value(object),
            args.iter().map(render_value).collect::<Vec<_>>().join(", "),
            ret_type.render_name()
        ),
        ValueIR::CallIndirect {
            callee,
            args,
            ret_type,
        } => format!(
            "call_indirect {}({}) -> {}",
            render_value(callee),
            args.iter().map(render_value).collect::<Vec<_>>().join(", "),
            ret_type.render_name()
        ),
        ValueIR::CallRaw {
            callee,
            args,
            param_types,
            ret_type,
        } => format!(
            "call_raw {}({}) : ({}) -> {}",
            render_value(callee),
            args.iter().map(render_value).collect::<Vec<_>>().join(", "),
            param_types
                .iter()
                .map(TypeIR::render_name)
                .collect::<Vec<_>>()
                .join(", "),
            ret_type.render_name()
        ),
        ValueIR::FieldAccess {
            base,
            field,
            field_offset,
            ..
        } => {
            format!("{}.{}/*+{}*/", render_value(base), field, field_offset)
        }
        ValueIR::Index { base, index, .. } => {
            format!("{}[{}]", render_value(base), render_value(index))
        }
        ValueIR::Cast { value, target_type } => {
            format!(
                "{} virar {}",
                render_value(value),
                target_type.render_name()
            )
        }
        ValueIR::UnionInject {
            value,
            union_type_id,
            tag,
            ..
        } => format!(
            "union_inject #{} tag={} ({})",
            union_type_id.0,
            tag,
            render_value(value)
        ),
        ValueIR::UnionTag {
            value,
            union_type_id,
        } => format!("union_tag #{} ({})", union_type_id.0, render_value(value)),
        ValueIR::UnionExtract {
            value,
            union_type_id,
            tag,
            canonical_member_key,
            ..
        } => format!(
            "union_extract #{} tag={} key={} ({})",
            union_type_id.0,
            tag,
            canonical_member_key,
            render_value(value)
        ),
    }
}

pub(super) fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
// @pinker-nav:end ir.renderizacao.textual
