//! Extração do programa de convenção de chamada externa, movida de
//! `src/backend_s.rs` pela unidade BS-1 do inventário da #601 (Task #615).
//!
//! Só o arquivo mudou: as sete regiões cartografadas, a ordem em que decidem e
//! cada validação continuam exatamente como estavam. `super` mudou de
//! significado ao descer um nível, e o `use` abaixo devolve ao irmão o
//! vocabulário do pai — o modelo `ExternalCallConv*`, os tipos da IR, os
//! predicados de tipo, `load_operand`, `resolver_rota_de_chamada`,
//! `native_symbol` e os helpers `line`/`err` — sem promover nada: um filho
//! enxerga os itens privados do pai por privacidade de módulo, e este `use` é
//! privado.
//!
//! A rota de chamada e o símbolo de runtime continuam sendo decididos fora
//! daqui: esta função consome `CalleeIdentity` e `resolver_rota_de_chamada`, e
//! não guarda tabela de grafia, censo de intrínsecas nem registro paralelo. A
//! autoridade C1 permanece em `src/intrinsics/**`.
//!
//! `extract_external_callconv_program` era privada ao módulo `backend_s` antes
//! do move e continua sendo: `pub(super)` é o mínimo que devolve ao pai a
//! função que ele chama nas duas entradas públicas do caminho montável.

use super::*;

// @pinker-nav:start backend-s.lowering.globais-rodata
// @pinker-nav:domain lowering
// @pinker-nav:layer backend-s
// @pinker-nav:summary `extract_external_callconv_program` (início): deduplicação de símbolos globais (recusa duplicados), aceitação apenas de globais estáticas `bombom`/`logica` com inicializador literal inteiro/lógico (`OperandIR::Int`/`Bool`), montagem de `rodata_globals`, e a exigência de função `principal`. Primeira responsabilidade contígua da extração para `ExternalCallConvProgram`.
pub(super) fn extract_external_callconv_program(
    selected: &SelectedProgram,
    native_runtime: bool,
) -> Result<ExternalCallConvProgram, PinkerError> {
    let mut seen_globals = HashSet::new();
    let mut rodata_globals = Vec::new();
    for global in &selected.globals {
        if !seen_globals.insert(global.name.clone()) {
            return Err(err(
                "subset externo montável (Fase 114) encontrou símbolo global duplicado",
            ));
        }
        if global.ty != TypeIR::Bombom && global.ty != TypeIR::Logica {
            return Err(err(
                "subset externo montável (Fase 114) aceita apenas globais estáticas `bombom`/`logica`",
            ));
        }
        let value = match &global.value {
            OperandIR::Int(v) => *v,
            OperandIR::Bool(v) => u64::from(*v),
            _ => {
                return Err(err(
                    "subset externo montável (Fase 114) aceita apenas inicialização literal inteira/lógica em globais estáticas",
                ));
            }
        };
        rodata_globals.push(ExternalCallConvGlobal {
            name: global.name.clone(),
            value,
        });
    }

    let has_main = selected
        .functions
        .iter()
        .any(|f| native_symbol::is_entrypoint(&f.name));
    if !has_main {
        return Err(err(
            "subset externo montável (Fase 84) exige função `principal`",
        ));
    }
    // @pinker-nav:end backend-s.lowering.globais-rodata

    // @pinker-nav:start backend-s.lowering.funcoes-frames
    // @pinker-nav:domain lowering
    // @pinker-nav:layer backend-s
    // @pinker-nav:summary Validação e enquadramento por função no caminho montável: recusa de retorno fora de `is_external_ret_type`, `principal` sem parâmetros, tipos de parâmetro/local fora de `is_external_param_type`/`is_external_local_type`, exigência de ao menos um bloco e `validate_external_block_labels`. Em seguida constrói `slot_offsets` alocando 8 bytes por slot na ordem parâmetros → locais → temporários (`collect_temp_ids`), calcula `raw_stack` e arredonda `stack_size` para múltiplo de 16 (0 quando não há slots). Tipos menores ainda ocupam slot de 8 bytes.
    let mut functions = Vec::new();
    let mut rodata_string_labels = HashMap::new();
    let mut rodata_strings = Vec::new();
    let mut trait_vtables = BTreeMap::<String, ExternalTraitVtable>::new();
    let mut trait_adapters = BTreeMap::<String, ExternalTraitAdapter>::new();
    for function in &selected.functions {
        if !is_external_ret_type(&function.ret_type) {
            return Err(err(
                "subset externo montável (Fase 215) aceita retorno `bombom`, `verso` ou `logica` em funções",
            ));
        }
        if native_symbol::is_entrypoint(&function.name) && !function.params.is_empty() {
            return Err(err(
                "subset externo montável (Fase 84) exige `principal()` sem parâmetros",
            ));
        }
        for (param_index, param) in function.params.iter().enumerate() {
            let Some(ty) = function.slot_types.get(param) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou parâmetro sem tipo",
                ));
            };
            let is_trait_receiver = param_index == 0
                && function.name.starts_with("__impl_")
                && is_external_trait_receiver_type(ty);
            let is_trait_method_word =
                function.name.starts_with("__impl_") && ty.is_native_abi_word();
            let supported_param = if native_runtime {
                is_external_param_type(ty) || is_external_scalar_param_type(ty)
            } else {
                is_external_param_type(ty)
            };
            if !supported_param && !is_trait_receiver && !is_trait_method_word {
                return Err(err(
                    "subset externo montável aceita parâmetro `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>` no recorte conservador",
                ));
            }
        }
        for local in &function.locals {
            let Some(ty) = function.slot_types.get(local) else {
                return Err(err(
                    "subset externo montável (Fase 84) encontrou local sem tipo",
                ));
            };
            let is_trait_snapshot_source = function.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        SelectedInstr::MakeTraitObject {
                            value: OperandIR::Local(slot),
                            concrete_type,
                            ..
                        } if slot == local && concrete_type == ty
                    )
                })
            });
            let is_trait_method_word =
                function.name.starts_with("__impl_") && ty.is_native_abi_word();
            if !(is_external_local_type(ty)
                || is_trait_method_word
                || is_trait_snapshot_source && is_external_trait_receiver_type(ty))
            {
                return Err(err(&format!(
                    "subset externo montável só aceita local `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`; '{}' é '{}'",
                    local,
                    ty.name()
                )));
            }
        }
        if function.blocks.is_empty() {
            return Err(err(
                "subset externo montável (Fase 111) exige ao menos um bloco por função",
            ));
        }
        validate_external_block_labels(function)?;

        let temp_ids = collect_temp_ids(function);
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
        // HR3: as operações de união deixam de presumir que todo storage ocupa
        // oito bytes. Cada injeção recebe um scratch do tamanho real do payload
        // e cada extração recebe storage próprio para o binding, ambos
        // alinhados. Os offsets são múltiplos de 16, o maior alinhamento
        // suportado por `MAX_UNION_PAYLOAD_ALIGN`, e crescem com aritmética
        // checada.
        let mut union_storage_offsets: HashMap<UnionStorageKey, u32> = HashMap::new();
        let mut frame_top = (slot_index.saturating_sub(1)) * 8;
        frame_top = frame_top.div_ceil(16) * 16;
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instr_index, inst) in block.instructions.iter().enumerate() {
                let layout = match inst {
                    SelectedInstr::UnionInject { payload_layout, .. }
                    | SelectedInstr::UnionExtract { payload_layout, .. } => *payload_layout,
                    _ => continue,
                };
                if !layout.is_well_formed() {
                    return Err(err(
                        "subset externo montável recusa layout de payload de união mal formado",
                    ));
                }
                let bytes = u32::try_from(layout.size).map_err(|_| {
                    err("subset externo montável recusa payload de união acima da plataforma")
                })?;
                let reserved = bytes.div_ceil(16).saturating_mul(16);
                frame_top = frame_top.checked_add(reserved).ok_or_else(|| {
                    err("overflow no frame do subset externo montável ao reservar storage de união")
                })?;
                union_storage_offsets.insert(
                    UnionStorageKey {
                        block: block_index,
                        instr: instr_index,
                    },
                    frame_top,
                );
            }
        }
        let raw_stack = frame_top;
        let stack_size = if raw_stack == 0 {
            0
        } else {
            raw_stack.div_ceil(16) * 16
        };
        // @pinker-nav:end backend-s.lowering.funcoes-frames

        // @pinker-nav:start backend-s.lowering.blocos-terminadores
        // @pinker-nav:domain lowering
        // @pinker-nav:layer backend-s
        // @pinker-nav:summary Abertura do laço de blocos e seleção do terminador de cada bloco: `SelectedTerminator::Jmp` → `ExternalCallConvTerminator::Jmp`; `Ret(Some(value))` materializa literais `verso` de retorno em `.rodata` (`register_rodata_strings_for_operand`) e vira `Ret`; `Ret(None)` vira `RetVoid` para funções/métodos `nulo`; `Br` copia condição e rótulos. Constrói o `terminator` antes do corpo do bloco.
        let mut blocks = Vec::new();
        // Identificador determinístico de envelope de `sussurro` dentro da função.
        let mut inline_asm_envelopes = 0_u32;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let terminator = match &block.terminator {
                SelectedTerminator::Jmp(target) => ExternalCallConvTerminator::Jmp(target.clone()),
                SelectedTerminator::Ret(Some(value)) => {
                    // Literais `verso` também podem aparecer direto no retorno
                    // (`mimo "texto";`); materializa o rodata aqui (Fase 215/B4).
                    register_rodata_strings_for_operand(
                        value,
                        &mut rodata_string_labels,
                        &mut rodata_strings,
                    );
                    ExternalCallConvTerminator::Ret(value.clone())
                }
                SelectedTerminator::Ret(None) => ExternalCallConvTerminator::RetVoid,
                SelectedTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => ExternalCallConvTerminator::Br {
                    cond: cond.clone(),
                    then_label: then_label.clone(),
                    else_label: else_label.clone(),
                },
            };
            // @pinker-nav:end backend-s.lowering.blocos-terminadores

            // @pinker-nav:start backend-s.lowering.operacoes-memoria
            // @pinker-nav:domain lowering
            // @pinker-nav:layer backend-s
            // @pinker-nav:summary Lowering externo de dados/memória: `Mov`; aritmética `Add`/`Sub`/`Mul`, com validação nativa de derivação quando o resultado preserva tipo ponteiro; comparações, incluindo condições assinadas inferidas dos produtores; `DerefLoad`/`DerefStore` por largura e sinal para todos os escalares públicos, precedidos por validação de região; e casts de uma palavra. O caminho hospedado legado mantém seu subconjunto conservador.
            let mut body = Vec::new();
            for (instr_index, inst) in block.instructions.iter().enumerate() {
                match inst {
                    SelectedInstr::Mov { dest, src } => {
                        ensure_dest_is_local_or_param(dest, function)?;
                        register_rodata_strings_for_operand(
                            src,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(REG_RET, src, &slot_offsets, &rodata_strings)?);
                        body.push(format!("movq {}, -{}(%rbp)", REG_RET, slot_offsets[dest]));
                    }
                    SelectedInstr::Neg { dest, operand, ty } => {
                        register_rodata_strings_for_operand(
                            operand,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(
                            REG_RET,
                            operand,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(format!("negq {}", REG_RET));
                        body.extend(normalize_rax(*ty));
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    SelectedInstr::PointerOffset {
                        dest,
                        pointer,
                        offset,
                        element_size,
                        element_align,
                        ..
                    } => {
                        body.extend(lower_typed_pointer_offset(
                            function,
                            *dest,
                            pointer,
                            offset,
                            (*element_size, *element_align),
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                    }
                    SelectedInstr::Add { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "addq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                        body.extend(lower_public_pointer_derivation(
                            function,
                            *dest,
                            lhs,
                            rhs,
                            false,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                    }
                    SelectedInstr::Sub { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "subq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                        body.extend(lower_public_pointer_derivation(
                            function,
                            *dest,
                            lhs,
                            rhs,
                            true,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                    }
                    SelectedInstr::Mul { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "imulq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::BitAnd { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "andq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::BitOr { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "orq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::BitXor { dest, lhs, rhs, ty } => {
                        body.extend(lower_linear_binop(
                            "xorq",
                            *dest,
                            (lhs, rhs),
                            *ty,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::Shl { dest, lhs, rhs, ty }
                    | SelectedInstr::Shr { dest, lhs, rhs, ty } => {
                        body.extend(lower_shift(
                            matches!(inst, SelectedInstr::Shr { .. }),
                            *dest,
                            lhs,
                            rhs,
                            *ty,
                            &function.name,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::Div { dest, lhs, rhs, ty }
                    | SelectedInstr::Mod { dest, lhs, rhs, ty } => {
                        body.extend(lower_div_mod(
                            matches!(inst, SelectedInstr::Mod { .. }),
                            *dest,
                            lhs,
                            rhs,
                            *ty,
                            &function.name,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpEq { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_eq(
                            *dest,
                            lhs,
                            rhs,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpNe { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_ne(
                            *dest,
                            lhs,
                            rhs,
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpLt { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_lt(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpGt { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_gt(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpLe { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_le(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::CmpGe { dest, lhs, rhs, .. } => {
                        body.extend(lower_cmp_ge(
                            *dest,
                            lhs,
                            rhs,
                            selected_comparison_is_signed(function, lhs, rhs),
                            &slot_offsets,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        )?);
                    }
                    SelectedInstr::DerefLoad {
                        dest,
                        ptr,
                        ty,
                        is_volatile,
                    } => {
                        // HR3: um agregado é representado **pelo endereço** da
                        // sua representação completa. Abrir `*ptr` de um array
                        // fixo ou de um `ninho` não lê memória: produz o mesmo
                        // endereço, que é o que a injeção de união entrega ao
                        // runtime para a cópia integral.
                        if matches!(ty, TypeIR::FixedArray { .. } | TypeIR::Struct) {
                            if *is_volatile {
                                return Err(err(
                                    "subset externo montável não suporta caminho `fragil` em agregado",
                                ));
                            }
                            register_rodata_strings_for_operand(
                                ptr,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                            body.extend(load_operand(
                                REG_RET,
                                ptr,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        if !(if native_runtime {
                            is_external_deref_load_type(ty)
                        } else {
                            is_external_legacy_deref_load_type(ty)
                        }) {
                            return Err(err(
                                "subset externo montável (Fase 134) aceita `deref_load` apenas no recorte mínimo `bombom`/`u32`/`u64` (camada 4 conservadora de `ninho` heterogêneo + legado homogêneo)",
                            ));
                        }
                        if *is_volatile {
                            return Err(err(
                                "subset externo montável (Fase 134) ainda não suporta caminho `fragil` no acesso indireto externo",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            ptr,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        let mut visiting_temps = HashSet::new();
                        let mut visiting_slots = HashSet::new();
                        if selected_operand_is_public_pointer(
                            function,
                            ptr,
                            &mut visiting_temps,
                            &mut visiting_slots,
                        ) {
                            body.push(format!("movq {}, %rdi", REG_RET));
                            body.push(format!("movq ${}, %rsi", external_memory_width(*ty)));
                            body.push(format!("movq ${}, %rdx", external_memory_alignment(*ty)));
                            body.push("call pinker_publico_validar_acesso".to_string());
                        }
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        body.push(if native_runtime {
                            match ty {
                                TypeIR::U8 | TypeIR::Logica => {
                                    format!("movzbq ({}), {}", REG_RET, REG_RET)
                                }
                                TypeIR::I8 => format!("movsbq ({}), {}", REG_RET, REG_RET),
                                TypeIR::U16 => format!("movzwq ({}), {}", REG_RET, REG_RET),
                                TypeIR::I16 => format!("movswq ({}), {}", REG_RET, REG_RET),
                                TypeIR::U32 => format!("movl ({}), %eax", REG_RET),
                                TypeIR::I32 => format!("movslq ({}), {}", REG_RET, REG_RET),
                                _ => format!("movq ({}), {}", REG_RET, REG_RET),
                            }
                        } else {
                            format!("movq ({}), {}", REG_RET, REG_RET)
                        });
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    SelectedInstr::DerefStore {
                        ptr,
                        value,
                        ty,
                        is_volatile,
                    } => {
                        if !(if native_runtime {
                            is_external_deref_store_type(ty)
                        } else {
                            is_external_legacy_deref_store_type(ty)
                        }) {
                            return Err(err(
                                "subset externo montável (Fase 134) aceita `deref_store` apenas no recorte mínimo `bombom`/`u32`/`u64` (camada 4 conservadora de `ninho` heterogêneo + legado homogêneo)",
                            ));
                        }
                        if *is_volatile {
                            return Err(err(
                                "subset externo montável (Fase 134) ainda não suporta caminho `fragil` no acesso indireto externo",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            ptr,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        let mut visiting_temps = HashSet::new();
                        let mut visiting_slots = HashSet::new();
                        if selected_operand_is_public_pointer(
                            function,
                            ptr,
                            &mut visiting_temps,
                            &mut visiting_slots,
                        ) {
                            body.push(format!("movq {}, %rdi", REG_RET));
                            body.push(format!("movq ${}, %rsi", external_memory_width(*ty)));
                            body.push(format!("movq ${}, %rdx", external_memory_alignment(*ty)));
                            body.push("call pinker_publico_validar_acesso".to_string());
                        }
                        body.extend(load_operand(REG_RET, ptr, &slot_offsets, &rodata_strings)?);
                        body.extend(load_operand(
                            REG_TMP,
                            value,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(if native_runtime {
                            match ty {
                                TypeIR::U8 | TypeIR::I8 | TypeIR::Logica => {
                                    format!("movb %r10b, ({})", REG_RET)
                                }
                                TypeIR::U16 | TypeIR::I16 => {
                                    format!("movw %r10w, ({})", REG_RET)
                                }
                                TypeIR::U32 | TypeIR::I32 => {
                                    format!("movl %r10d, ({})", REG_RET)
                                }
                                _ => format!("movq {}, ({})", REG_TMP, REG_RET),
                            }
                        } else {
                            format!("movq {}, ({})", REG_TMP, REG_RET)
                        });
                    }
                    SelectedInstr::Cast {
                        dest,
                        value,
                        target_type,
                    } => {
                        if matches!(target_type, TypeIR::Pointer { .. }) {
                            register_rodata_strings_for_operand(
                                value,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                            body.extend(load_operand(
                                REG_RET,
                                value,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        if !target_type.is_integer() {
                            return Err(err(
                                "backend nativo aceita `virar` escalar apenas para inteiro ou ponteiro",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand(
                            REG_RET,
                            value,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.extend(normalize_rax(*target_type));
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // @pinker-nav:end backend-s.lowering.operacoes-memoria

                    // @pinker-nav:start backend-s.lowering.chamadas-sysv
                    // @pinker-nav:domain lowering
                    // @pinker-nav:layer backend-s
                    // @pinker-nav:summary Lowering de chamadas no corpo do bloco (ABI SysV): `Call` com destino trata `__ternario` puro por `cmoveq`; `formatar_verso` materializa um pack contíguo de handles `verso` na pilha e chama a autoridade única `pinker_formatar_verso_pack(modelo,count,entries)`; passagem dos 6 primeiros argumentos em `ARG_REGS`, empilhamento do 7º+ do último ao primeiro com padding de alinhamento e cleanup após o `call`. `Call` e `CallVoid` delegam a escolha do destino a `resolver_rota_de_chamada` e diferem apenas em `CallVoid` não guardar `%rax`. `CallVoid` passou a consultar também o despacho por aridade, que antes só o `Call` consultava: é por aí que `afirmar` alcança `pinker_afirmar_1`/`pinker_afirmar_2`, e para as demais intrínsecas de aridade variável o ramo é inalcançável porque a seleção só emite `CallVoid` para retorno `Nulo`. Aridade fora do recorte e callee desconhecido continuam recusados por esta camada.
                    SelectedInstr::Call {
                        dest,
                        callee,
                        args,
                        ret_type,
                        identidade,
                    } => {
                        if !is_external_call_ret_type(ret_type) {
                            return Err(err(
                                "subset externo montável (Fase 216) só aceita call com retorno `bombom`, `verso`, `logica`, lista ou `nulo`",
                            ));
                        }
                        // A CFG conserva a pseudo-chamada somente quando os
                        // dois braços são valores trivialmente puros. Braços
                        // com chamadas, alocações ou outros efeitos já foram
                        // separados em blocos lazy antes da seleção.
                        //
                        // #532: os dois desvios por grafia abaixo acontecem
                        // ANTES de `resolver_rota_de_chamada` e fazem
                        // `continue`, então o portão de identidade daquela
                        // autoridade não os alcança. Eles precisam do portão
                        // aqui, ou a grafia volta a ser autoridade executiva
                        // neste emissor — e só neste.
                        if identidade.dispatches_as_builtin()
                            && crate::internal_operations::e_ternaria(callee)
                        {
                            if Some(args.len()) != crate::internal_operations::aridade(callee) {
                                return Err(err(
                                    "subset externo montável (Fase 214) exige `__ternario` com 3 argumentos",
                                ));
                            }
                            for arg in args {
                                register_rodata_strings_for_operand(
                                    arg,
                                    &mut rodata_string_labels,
                                    &mut rodata_strings,
                                );
                            }
                            body.extend(load_operand(
                                REG_RET,
                                &args[1],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(load_operand(
                                "%r10",
                                &args[2],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(load_operand(
                                "%r11",
                                &args[0],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push("cmpq $0, %r11".to_string());
                            body.push(format!("cmoveq %r10, {}", REG_RET));
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        // D7: a IR já converte todos os argumentos de
                        // substituição para handles `verso`. Materializamos
                        // um pack contíguo e passamos modelo/count/entries à
                        // autoridade única do runtime, independentemente da
                        // quantidade de argumentos.
                        if identidade.dispatches_as_builtin() && callee == "formatar_verso" {
                            if args.len() < 2 {
                                return Err(err(
                                    "subset externo montável exige ao menos uma substituição em formatar_verso",
                                ));
                            }
                            for arg in args {
                                register_rodata_strings_for_operand(
                                    arg,
                                    &mut rodata_string_labels,
                                    &mut rodata_strings,
                                );
                            }
                            let substitutions = args.len() - 1;
                            let pack_bytes = substitutions.checked_mul(8).ok_or_else(|| {
                                err("pack de formatar_verso excede a representação da plataforma")
                            })?;
                            if pack_bytes > isize::MAX as usize {
                                return Err(err(
                                    "pack de formatar_verso excede a representação da plataforma",
                                ));
                            }
                            let pad_words = substitutions % 2;
                            if pad_words == 1 {
                                body.push("subq $8, %rsp".to_string());
                            }
                            for arg in args.iter().skip(1).rev() {
                                body.extend(load_operand(
                                    REG_TMP,
                                    arg,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push(format!("pushq {}", REG_TMP));
                            }
                            body.extend(load_operand(
                                ARG_REGS[0],
                                &args[0],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!("movq ${}, {}", substitutions, ARG_REGS[1]));
                            body.push(format!("movq %rsp, {}", ARG_REGS[2]));
                            body.push("call pinker_formatar_verso_pack".to_string());
                            let cleanup_bytes = pack_bytes
                                .checked_add(pad_words * 8)
                                .filter(|bytes| *bytes <= isize::MAX as usize)
                                .ok_or_else(|| {
                                    err("pack de formatar_verso excede a representação da plataforma")
                                })?;
                            if cleanup_bytes > 0 {
                                body.push(format!("addq ${}, %rsp", cleanup_bytes));
                            }
                            body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            ));
                            continue;
                        }
                        // Intrínsecas de aridade variável usam wrappers por
                        // aridade no runtime (Fases 219/B8 e 221/B10).
                        let call_target = match resolver_rota_de_chamada(
                            *identidade,
                            callee,
                            args.len(),
                            || selected.functions.iter().any(|f| &f.name == callee),
                        ) {
                            RotaDeChamada::Runtime(simbolo) => simbolo,
                            RotaDeChamada::FuncaoPinker(simbolo) => simbolo,
                            RotaDeChamada::AridadeForaDoRecorte => {
                                return Err(err(
                                    "subset externo montável (Fase 221) recusa aridade fora do recorte da intrínseca de runtime",
                                ));
                            }
                            RotaDeChamada::CalleeDesconhecido => {
                                return Err(err(
                                    "subset externo montável (Fase 84) encontrou call para função inexistente",
                                ));
                            }
                        };
                        for arg in args.iter() {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        // ABI SysV completa (Fase 213/B2): 7º argumento em
                        // diante viaja pela pilha, empilhado do último para o
                        // primeiro; padding mantém o alinhamento de 16 no call.
                        let stack_args = args.len().saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for arg in args.iter().skip(ARG_REGS.len()).rev() {
                            body.extend(load_operand("%r10", arg, &slot_offsets, &rodata_strings)?);
                            body.push("pushq %r10".to_string());
                        }
                        for (idx, arg) in args.iter().take(ARG_REGS.len()).enumerate() {
                            body.extend(load_operand(
                                ARG_REGS[idx],
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                        }
                        body.push(format!("call {}", call_target));
                        if stack_args > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // Fase 242: chamada indireta real. `callee` é um operando
                    // (handle callable: endereço do descritor estático ou
                    // heap {code_ptr, env_ptr}), não um símbolo. Mesma ABI de
                    // argumentos do usuário do `call` direto; o código é lido
                    // do descritor em tempo de execução e chamado via
                    // registrador (`call *reg`). `env_ptr` (offset 8) é
                    // reservado/ignorado nesta fase (sem captura).
                    SelectedInstr::CallIndirect {
                        dest,
                        callee,
                        args,
                        ret_type,
                    } => {
                        if !is_external_call_ret_type(ret_type) {
                            return Err(err(
                                "subset externo montável (Fase 242) só aceita call_indirect com retorno `bombom`, `verso`, `logica`, lista ou carinho",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            callee,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        for arg in args.iter() {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        // Fase 243: `__env` é sempre o argumento real final
                        // (trailing) — uniforme para toda função
                        // indiretamente chamável, capturante ou não (ver
                        // `ir.rs::lower_closure_function`/`ensure_fnref_wrapper`).
                        // O índice virtual `args.len()` é sempre `__env`,
                        // extraído do descritor (offset 8) em vez de vir de
                        // um operando real.
                        let total_args = args.len() + 1;
                        let stack_args = total_args.saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for index in (ARG_REGS.len()..total_args).rev() {
                            if index == args.len() {
                                body.extend(load_operand(
                                    REG_TMP,
                                    callee,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push(format!("movq 8({0}), {0}", REG_TMP));
                                body.push(format!("pushq {}", REG_TMP));
                            } else {
                                body.extend(load_operand(
                                    "%r11",
                                    &args[index],
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push("pushq %r11".to_string());
                            }
                        }
                        for index in 0..total_args.min(ARG_REGS.len()) {
                            if index == args.len() {
                                body.extend(load_operand(
                                    REG_TMP,
                                    callee,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.push(format!("movq 8({}), {}", REG_TMP, ARG_REGS[index]));
                            } else {
                                body.extend(load_operand(
                                    ARG_REGS[index],
                                    &args[index],
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                            }
                        }
                        // code_ptr por último: recarrega o handle (slot
                        // estável, seguro reler) — não conflita com o uso
                        // anterior de REG_TMP para extrair env_ptr, já
                        // consumido acima nos ramos de pilha/registrador.
                        body.extend(load_operand(
                            REG_TMP,
                            callee,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(format!("movq ({}), {}", REG_TMP, REG_TMP));
                        body.push(format!("call *{}", REG_TMP));
                        if stack_args > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                        body.push(format!(
                            "movq {}, -{}(%rbp)",
                            REG_RET,
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // Fase 245: endereço cru de código em uma palavra. A ABI
                    // contém apenas os argumentos declarados, sem descritor e
                    // sem o argumento implícito `__env`.
                    SelectedInstr::CallRaw {
                        dest,
                        callee,
                        args,
                        param_types,
                        ret_type,
                    } => {
                        if args.len() != param_types.len()
                            || !param_types.iter().all(is_external_raw_call_type)
                            || !is_external_raw_call_ret_type(ret_type)
                        {
                            return Err(err(
                                "subset externo montável encontrou assinatura ABI inválida em call_raw",
                            ));
                        }
                        for arg in args {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        body.extend(load_operand(
                            "%rdi",
                            callee,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push("call pinker_publico_validar_ponteiro_funcao".to_string());
                        let stack_args = args.len().saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for arg in args.iter().skip(ARG_REGS.len()).rev() {
                            body.extend(load_operand(
                                REG_TMP,
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!("pushq {}", REG_TMP));
                        }
                        for (index, arg) in args.iter().take(ARG_REGS.len()).enumerate() {
                            body.extend(load_operand(
                                ARG_REGS[index],
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                        }
                        body.extend(load_operand(
                            REG_TMP,
                            callee,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push(format!("call *{}", REG_TMP));
                        if stack_args > 0 || pad > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                        match (dest, ret_type) {
                            (Some(_), TypeIR::Nulo) => {
                                return Err(err("call_raw nulo não pode ter destino"));
                            }
                            (Some(dest), _) => body.push(format!(
                                "movq {}, -{}(%rbp)",
                                REG_RET,
                                slot_offsets[&temp_key(*dest)]
                            )),
                            (None, TypeIR::Nulo) => {}
                            (None, _) => {
                                return Err(err("call_raw com retorno exige destino"));
                            }
                        }
                    }
                    // Fase 243/D3: materializa uma closure em uma única
                    // alocação possuída pelo descritor dinâmico
                    // {code_ptr, env_ptr}. O ambiente, quando existe, ocupa o
                    // storage trailing (uma palavra por captura), eliminando
                    // a falha parcial entre ambiente e descritor. Continua
                    // distinto do descritor ESTÁTICO em `.rodata` de
                    // `FunctionRef`.
                    SelectedInstr::MakeClosure {
                        dest,
                        function_name,
                        captures,
                    } => {
                        if !selected.functions.iter().any(|f| &f.name == function_name) {
                            return Err(err(
                                "subset externo montável (Fase 243) encontrou make_closure para função inexistente",
                            ));
                        }
                        for capture in captures.iter() {
                            register_rodata_strings_for_operand(
                                capture,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        let dest_offset = slot_offsets[&temp_key(*dest)];
                        body.push(format!("movabsq ${}, %rdi", captures.len()));
                        body.push("call pinker_callable_alocar".to_string());
                        // O slot já recebe a identidade final do descritor;
                        // `8(descriptor)` contém o ambiente possuído.
                        body.push(format!("movq %rax, -{}(%rbp)", dest_offset));
                        for (index, capture) in captures.iter().enumerate() {
                            body.push(format!("movq -{}(%rbp), %r10", dest_offset));
                            body.push("movq 8(%r10), %r10".to_string());
                            body.extend(load_operand(
                                "%r11",
                                capture,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.push(format!("movq %r11, {}(%r10)", index * 8));
                        }
                        body.push(format!("movq -{}(%rbp), %r11", dest_offset));
                        body.push(format!("leaq {}(%rip), %r10", function_name));
                        body.push("movq %r10, (%r11)".to_string());
                    }
                    // Call sem destino (intrínsecas de efeito, Fase 216/B5):
                    // mesma ABI do call comum, sem o movq de retorno.
                    SelectedInstr::CallVoid {
                        callee,
                        args,
                        identidade,
                    } => {
                        let call_target = match resolver_rota_de_chamada(
                            *identidade,
                            callee,
                            args.len(),
                            || selected.functions.iter().any(|f| &f.name == callee),
                        ) {
                            RotaDeChamada::Runtime(simbolo) => simbolo,
                            RotaDeChamada::FuncaoPinker(simbolo) => simbolo,
                            RotaDeChamada::AridadeForaDoRecorte => {
                                return Err(err(
                                    "subset externo montável (Fase 221) recusa aridade fora do recorte da intrínseca de runtime",
                                ));
                            }
                            RotaDeChamada::CalleeDesconhecido => {
                                return Err(err(
                                    "subset externo montável (Fase 84) encontrou call para função inexistente",
                                ));
                            }
                        };
                        for arg in args.iter() {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }
                        let stack_args = args.len().saturating_sub(ARG_REGS.len());
                        let pad = stack_args % 2;
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for arg in args.iter().skip(ARG_REGS.len()).rev() {
                            body.extend(load_operand("%r10", arg, &slot_offsets, &rodata_strings)?);
                            body.push("pushq %r10".to_string());
                        }
                        for (idx, arg) in args.iter().take(ARG_REGS.len()).enumerate() {
                            body.extend(load_operand(
                                ARG_REGS[idx],
                                arg,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                        }
                        body.push(format!("call {}", call_target));
                        if stack_args > 0 {
                            body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad)));
                        }
                    }
                    // @pinker-nav:end backend-s.lowering.chamadas-sysv
                    // @pinker-nav:start backend-s.lowering.objetos-trato-nativos
                    // @pinker-nav:domain lowering
                    // @pinker-nav:layer backend-s
                    // @pinker-nav:summary Materialização nativa de `trato<T>` e despacho por vtable: `MakeTraitObject` avalia o operando uma vez, aloca/copia o snapshot pelo tamanho concreto exato, aloca o descritor `{data_ptr,vtable_ptr}` de 16 bytes e guarda seu endereço no destino. `TraitCall` carrega descritor, snapshot, vtable e slot, posiciona receiver + argumentos pela ABI SysV (incluindo spill/padding) e executa `call *%r11`, sem `__env`; retorno `nulo` não grava destino.
                    SelectedInstr::MakeTraitObject {
                        dest,
                        value,
                        trait_name,
                        concrete_type,
                        concrete_type_name,
                        concrete_size,
                        vtable_methods,
                    } => {
                        let vtable_symbol = register_trait_vtable(
                            selected,
                            trait_name,
                            concrete_type_name,
                            *concrete_type,
                            vtable_methods,
                            &mut trait_vtables,
                            &mut trait_adapters,
                        )?;
                        body.extend(load_operand(
                            REG_RET,
                            value,
                            &slot_offsets,
                            &rodata_strings,
                        )?);

                        // Preserva o operando através da primeira alocação sem
                        // depender de registrador caller-saved e mantém %rsp
                        // alinhado a 16 bytes antes do call.
                        body.push("pushq %rax".to_string());
                        body.push("subq $8, %rsp".to_string());
                        body.push(format!("movabsq ${}, %rdi", concrete_size));
                        body.push("call pinker_alocar".to_string());
                        body.push("addq $8, %rsp".to_string());
                        body.push("popq %r10".to_string());
                        body.extend(lower_trait_snapshot_copy(*concrete_type, *concrete_size)?);

                        let dest_offset = slot_offsets[&temp_key(*dest)];
                        body.push(format!("movq %rax, -{}(%rbp)", dest_offset));
                        body.push("movabsq $16, %rdi".to_string());
                        body.push("call pinker_alocar".to_string());
                        body.push(format!("movq -{}(%rbp), %r10", dest_offset));
                        body.push("movq %r10, 0(%rax)".to_string());
                        body.push(format!("leaq {}(%rip), %r10", vtable_symbol));
                        body.push("movq %r10, 8(%rax)".to_string());
                        body.push(format!("movq %rax, -{}(%rbp)", dest_offset));
                    }
                    SelectedInstr::TraitCall {
                        dest,
                        object,
                        trait_name: _,
                        method_name: _,
                        method_slot,
                        method_count,
                        args,
                        param_types,
                        ret_type,
                    } => {
                        if *method_count == 0 || *method_slot >= *method_count {
                            return Err(err(
                                "backend nativo encontrou slot de chamada dinâmica fora da vtable",
                            ));
                        }
                        if param_types.len() != args.len()
                            || param_types.iter().any(|ty| !ty.is_native_abi_word())
                        {
                            return Err(err(
                                "backend nativo encontrou assinatura de chamada dinâmica fora do subset SysV",
                            ));
                        }
                        if *ret_type != TypeIR::Nulo && !is_external_call_ret_type(ret_type) {
                            return Err(err(
                                "backend nativo encontrou retorno de chamada dinâmica fora do subset SysV",
                            ));
                        }
                        register_rodata_strings_for_operand(
                            object,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        for arg in args {
                            register_rodata_strings_for_operand(
                                arg,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                        }

                        // O handle é carregado exatamente uma vez. Método e
                        // data_ptr ficam em dois spills internos, abaixo dos
                        // argumentos SysV escritos pelo usuário.
                        body.extend(load_operand(
                            "%r10",
                            object,
                            &slot_offsets,
                            &rodata_strings,
                        )?);
                        body.push("movq 0(%r10), %r11".to_string());
                        body.push("movq 8(%r10), %r10".to_string());
                        body.push(format!("movq {}(%r10), %r10", method_slot * 8));
                        body.push("pushq %r10".to_string());
                        body.push("pushq %r11".to_string());

                        let total_args = args.len() + 1;
                        let (stack_args, pad) = sysv_stack_layout(total_args);
                        if pad == 1 {
                            body.push("subq $8, %rsp".to_string());
                        }
                        for virtual_index in (ARG_REGS.len()..total_args).rev() {
                            let param_type = param_types[virtual_index - 1];
                            body.extend(load_operand(
                                "%r11",
                                &args[virtual_index - 1],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(normalize_sysv_scalar_argument("%r11", param_type)?);
                            body.push("pushq %r11".to_string());
                        }

                        let data_offset = 8 * (stack_args + pad);
                        body.push(format!("movq {}(%rsp), %rdi", data_offset));
                        for virtual_index in 1..total_args.min(ARG_REGS.len()) {
                            let param_type = param_types[virtual_index - 1];
                            body.extend(load_operand(
                                ARG_REGS[virtual_index],
                                &args[virtual_index - 1],
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            body.extend(normalize_sysv_scalar_argument(
                                ARG_REGS[virtual_index],
                                param_type,
                            )?);
                        }
                        body.push(format!("movq {}(%rsp), %r11", data_offset + 8));
                        body.push("call *%r11".to_string());
                        body.push(format!("addq ${}, %rsp", 8 * (stack_args + pad + 2)));
                        if let Some(dest) = dest {
                            body.push(format!(
                                "movq %rax, -{}(%rbp)",
                                slot_offsets[&temp_key(*dest)]
                            ));
                        }
                    }
                    // @pinker-nav:end backend-s.lowering.objetos-trato-nativos

                    // @pinker-nav:start backend-s.lowering.falar-runtime
                    // @pinker-nav:domain lowering
                    // @pinker-nav:layer backend-s
                    // @pinker-nav:summary Lowering de `falar` no corpo do bloco: cada pedaço vira uma chamada ao runtime conforme o tipo (`pinker_falar_pedaco_verso`/`_logica`/`_bombom`), com `pinker_falar_espaco` como separador entre pedaços e `pinker_falar_fim` ao final — espelhando `PrintInt`/`PrintBool`/`PrintStr` do interpretador. Inclui o braço catch-all do `match` que recusa instruções fora do subset montável. `falar` continua instrução própria (não intrínseca); mesmo o caminho hospedado (não nativo) emite referências a esses símbolos de `pinker_rt` quando o programa usa `falar` ou intrínsecas.
                    // `falar` nativo (Fase 215/B4): cada pedaço vira uma
                    // chamada ao runtime conforme o tipo, com separador entre
                    // pedaços e quebra de linha ao final — espelhando as
                    // instruções PrintInt/PrintBool/PrintStr do interpretador.
                    SelectedInstr::Falar { args } => {
                        for (idx, arg) in args.iter().enumerate() {
                            if idx > 0 {
                                body.push("call pinker_falar_espaco".to_string());
                            }
                            register_rodata_strings_for_operand(
                                &arg.value,
                                &mut rodata_string_labels,
                                &mut rodata_strings,
                            );
                            body.extend(load_operand(
                                ARG_REGS[0],
                                &arg.value,
                                &slot_offsets,
                                &rodata_strings,
                            )?);
                            let pedaco = match arg.ty {
                                TypeIR::Verso => "pinker_falar_pedaco_verso",
                                TypeIR::Logica => "pinker_falar_pedaco_logica",
                                TypeIR::I8 | TypeIR::I16 | TypeIR::I32 | TypeIR::I64 => {
                                    "pinker_falar_pedaco_inteiro"
                                }
                                _ => "pinker_falar_pedaco_bombom",
                            };
                            body.push(format!("call {}", pedaco));
                        }
                        body.push("call pinker_falar_fim".to_string());
                    }
                    SelectedInstr::InlineAsm {
                        chunks,
                        operands,
                        clobbers,
                        ..
                    } => {
                        let constraint_specs = operands
                            .iter()
                            .map(|operand| match operand {
                                crate::cfg_ir::InlineAsmOperandCfgIR::Input {
                                    name,
                                    constraint,
                                    ..
                                }
                                | crate::cfg_ir::InlineAsmOperandCfgIR::Output {
                                    name,
                                    constraint,
                                    ..
                                } => (name.clone(), *constraint),
                            })
                            .collect::<Vec<_>>();
                        let bindings =
                            crate::inline_asm::allocate_registers(&constraint_specs, clobbers)
                                .map_err(|error| err(&error.to_string()))?;
                        for operand in operands {
                            if let crate::cfg_ir::InlineAsmOperandCfgIR::Input {
                                name,
                                value,
                                ty,
                                ..
                            } = operand
                            {
                                let register = bindings[name].att();
                                body.push(format!(
                                    "# pinker:sussurro input {name} -> {}",
                                    bindings[name].intel()
                                ));
                                body.extend(load_operand(
                                    register,
                                    value,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                body.extend(normalize_sysv_scalar_argument(register, *ty)?);
                            }
                        }
                        // As sentinelas do envelope são geradas pelo compilador;
                        // nenhum texto delas vem da fonte. O identificador é
                        // determinístico por função e ordem de bloco.
                        inline_asm_envelopes += 1;
                        let envelope_id = format!("{}#{}", function.name, inline_asm_envelopes);
                        body.push(format!(
                            "{}{envelope_id}",
                            crate::inline_asm::SENTINEL_BEGIN_PREFIX
                        ));
                        body.push(crate::inline_asm::INTEL_SYNTAX_WRAPPER.to_string());
                        for (index, chunk) in chunks.iter().enumerate() {
                            body.push(format!("# pinker:sussurro chunk={index}"));
                            let parts = crate::inline_asm::parse_template(chunk)
                                .map_err(|error| err(&error.to_string()))?;
                            let rendered = crate::inline_asm::render_template(&parts, &bindings)
                                .map_err(|error| err(&error.to_string()))?;
                            body.extend(rendered.lines().map(str::to_string));
                        }
                        body.push(crate::inline_asm::ATT_SYNTAX_WRAPPER.to_string());
                        body.push(format!(
                            "{}{envelope_id}",
                            crate::inline_asm::SENTINEL_END_PREFIX
                        ));
                        for operand in operands {
                            if let crate::cfg_ir::InlineAsmOperandCfgIR::Output {
                                name,
                                slot,
                                ty,
                                ..
                            } = operand
                            {
                                let register = bindings[name].att();
                                body.extend(normalize_inline_asm_output(register, *ty)?);
                                body.push(format!(
                                    "movq {}, -{}(%rbp)",
                                    register, slot_offsets[slot]
                                ));
                                body.push(format!(
                                    "# pinker:sussurro output {name} <- {}",
                                    bindings[name].intel()
                                ));
                            }
                        }
                    }
                    SelectedInstr::UnionInject {
                        dest,
                        value,
                        union_type_id,
                        tag,
                        payload_layout,
                        ..
                    } => {
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        let storage = union_storage_offsets
                            .get(&UnionStorageKey {
                                block: block_index,
                                instr: instr_index,
                            })
                            .copied()
                            .ok_or_else(|| {
                                err("storage de união ausente no frame do subset externo montável")
                            })?;
                        // A ABI de criação recebe **endereço**, nunca o payload
                        // reempacotado em `u64`. Escalares e handles são
                        // materializados num scratch do tamanho real; agregados
                        // já são representados por endereço e o próprio
                        // endereço é passado, e o runtime copia imediatamente.
                        match payload_layout.representation {
                            crate::union_payload::UnionPayloadRepresentation::Scalar
                            | crate::union_payload::UnionPayloadRepresentation::OpaqueHandle => {
                                body.extend(load_operand(
                                    "%rax",
                                    value,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                                // O scratch é zerado antes da escrita para que
                                // um payload estreito não vaze bytes anteriores
                                // do frame para dentro do snapshot.
                                body.push(format!("movq $0, -{storage}(%rbp)"));
                                body.extend(store_union_scratch_word(
                                    "%rax",
                                    storage,
                                    payload_layout.size,
                                )?);
                                body.push(format!("leaq -{storage}(%rbp), %r8"));
                            }
                            crate::union_payload::UnionPayloadRepresentation::Aggregate => {
                                body.extend(load_operand(
                                    "%r8",
                                    value,
                                    &slot_offsets,
                                    &rodata_strings,
                                )?);
                            }
                        }
                        body.push(format!("movq ${}, %rdi", union_type_id.0));
                        body.push(format!("movq ${tag}, %rsi"));
                        body.push(format!("movq ${}, %rdx", payload_layout.size));
                        body.push(format!("movq ${}, %rcx", payload_layout.align));
                        body.push("call pinker_uniao_criar".to_string());
                        body.push(format!(
                            "movq %rax, -{}(%rbp)",
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    // A escolha do símbolo interno de ABI acontece **aqui**, no
                    // backend: a AST e a IR não carregam nome de runtime.
                    SelectedInstr::UnionTag {
                        dest,
                        value,
                        union_type_id,
                    } => {
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        body.extend(load_operand("%rdi", value, &slot_offsets, &rodata_strings)?);
                        // A leitura de tag valida também a identidade da união:
                        // um handle de outra união não devolve tag alguma.
                        body.push(format!("movq ${}, %rsi", union_type_id.0));
                        body.push("call pinker_uniao_tag".to_string());
                        body.push(format!(
                            "movq %rax, -{}(%rbp)",
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    SelectedInstr::UnionExtract {
                        dest,
                        value,
                        union_type_id,
                        tag,
                        payload_layout,
                        ..
                    } => {
                        register_rodata_strings_for_operand(
                            value,
                            &mut rodata_string_labels,
                            &mut rodata_strings,
                        );
                        let storage = union_storage_offsets
                            .get(&UnionStorageKey {
                                block: block_index,
                                instr: instr_index,
                            })
                            .copied()
                            .ok_or_else(|| {
                                err("storage de união ausente no frame do subset externo montável")
                            })?;
                        // A extração copia para storage novo do binding. O
                        // ponteiro interno do descritor nunca é devolvido.
                        body.extend(load_operand("%rdi", value, &slot_offsets, &rodata_strings)?);
                        body.push(format!("movq ${}, %rsi", union_type_id.0));
                        body.push(format!("movq ${tag}, %rdx"));
                        body.push(format!("movq ${}, %rcx", payload_layout.size));
                        body.push(format!("movq ${}, %r8", payload_layout.align));
                        body.push(format!("leaq -{storage}(%rbp), %r9"));
                        body.push("call pinker_uniao_copiar_payload".to_string());
                        match payload_layout.representation {
                            crate::union_payload::UnionPayloadRepresentation::Scalar
                            | crate::union_payload::UnionPayloadRepresentation::OpaqueHandle => {
                                body.extend(load_union_scratch_word(
                                    "%rax",
                                    storage,
                                    payload_layout.size,
                                )?);
                            }
                            crate::union_payload::UnionPayloadRepresentation::Aggregate => {
                                body.push(format!("leaq -{storage}(%rbp), %rax"));
                            }
                        }
                        body.push(format!(
                            "movq %rax, -{}(%rbp)",
                            slot_offsets[&temp_key(*dest)]
                        ));
                    }
                    _ => {
                        return Err(err(
                            "subset externo montável (Fase 135) aceita apenas atribuição, aritmética linear (+,-,*), comparações mínimas (`==`, `!=`, `<`, `>`, `<=` e `>=`), `virar` mínimo explícito (`u32` slot -> `u64` e `u64` slot -> `u32`), call direta com N argumentos (`bombom`/`u32`/`u64`/`verso` opaco/`seta<T>`; ABI SysV completa, Fase 213/B2), `deref_store` mínimo em `bombom`/`u32`/`u64` (incluindo escrita heterogênea de campo de `ninho` via offset explícito), `deref_load` mínimo em `bombom`/`u32`/`u64` (incluindo campo heterogêneo de `ninho` via offset explícito), literal `verso` estático mínimo em `.rodata` carregado por endereço e tráfego opaco por slot/parâmetro, composição heterogênea mínima auditável no mesmo `ninho` (`u32` + `u64` por offset) e load/store em slots de frame, preservando recorte conservador de `quebrar`/`continuar` em `sempre que` via saltos já materializados (até três níveis de laço aninhado)",
                        ));
                    }
                }
            }
            // @pinker-nav:end backend-s.lowering.falar-runtime
            blocks.push(ExternalCallConvBlock {
                label: block.label.clone(),
                body,
                terminator,
            });
        }

        functions.push(ExternalCallConvFunction {
            name: function.name.clone(),
            stack_size,
            slot_offsets,
            blocks,
            params: function.params.clone(),
        });
    }

    let mut function_refs = std::collections::BTreeSet::new();
    for function in &selected.functions {
        collect_function_refs_in_function(function, &mut function_refs);
    }
    for name in &function_refs {
        if !selected.functions.iter().any(|f| &f.name == name) {
            return Err(err(
                "subset externo montável (Fase 242) encontrou referência a função inexistente como valor",
            ));
        }
    }

    Ok(ExternalCallConvProgram {
        rodata_globals,
        rodata_strings,
        rodata_function_refs: function_refs.into_iter().collect(),
        trait_vtables: trait_vtables.into_values().collect(),
        trait_adapters: trait_adapters.into_values().collect(),
        functions,
    })
}
