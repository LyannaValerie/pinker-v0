use crate::cfg_ir::OperandIR;
use crate::error::PinkerError;
use crate::instr_select::{SelectedInstr, SelectedProgram, SelectedTerminator};
use crate::ir::TypeIR;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet};

fn generic_map_intrinsic_ret_matches(callee: &str, ret_type: TypeIR) -> bool {
    match callee {
        "__pinker_internal_mapa_criar_chave_bombom" => matches!(
            ret_type,
            TypeIR::Map {
                key: crate::ir::MapKeyIR::Bombom,
                ..
            }
        ),
        "__pinker_internal_mapa_criar_chave_verso" => matches!(
            ret_type,
            TypeIR::Map {
                key: crate::ir::MapKeyIR::Verso,
                ..
            }
        ),
        "__pinker_internal_mapa_obter" => !matches!(ret_type, TypeIR::Nulo),
        "__pinker_internal_mapa_tem" => ret_type == TypeIR::Logica,
        "__pinker_internal_mapa_tamanho" | "__pinker_internal_mapa_iterador_criar" => {
            ret_type == TypeIR::Bombom
        }
        "__pinker_internal_mapa_iterador_proxima_chave_bombom" => ret_type == TypeIR::Bombom,
        "__pinker_internal_mapa_iterador_proxima_chave_verso" => ret_type == TypeIR::Verso,
        _ => false,
    }
}

fn generic_map_intrinsic_void(callee: &str) -> bool {
    matches!(
        callee,
        "__pinker_internal_mapa_definir" | "__pinker_internal_mapa_remover"
    )
}

// @pinker-nav:start select.validacao.invariantes
// @pinker-nav:domain validacao
// @pinker-nav:layer select
// @pinker-nav:summary Valida a camada de seleção de instruções: operandos e destinos bem formados, uso coerente de temporários e conformidade das instruções selecionadas antes de descer à máquina abstrata.
pub fn validate_program(program: &SelectedProgram) -> Result<(), PinkerError> {
    crate::ir::validate_union_registry(&program.union_types).map_err(|message| err(&message))?;
    let mut globals = HashSet::new();
    for g in &program.globals {
        globals.insert(g.name.clone());
    }

    let mut sigs_usuario = HashMap::new();
    let mut sigs_intrinsecas = HashMap::new();
    for f in &program.functions {
        sigs_usuario.insert(f.name.clone(), f.ret_type);
    }
    // #442/C1 — assinaturas históricas vindas do registry declarativo.
    //
    // Esta tabela era uma das sete enumerações independentes do mesmo fato.
    // Agora ela é consumidora: a autoridade responde retorno e parâmetros, e o
    // validador só decide o que fazer com eles.
    for entrada in crate::intrinsics::registry::HISTORICAL {
        let Some((ret_type, _)) = entrada.assinatura_ir() else {
            continue;
        };
        sigs_intrinsecas.insert(entrada.spelling.to_string(), ret_type);
    }
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_bombom_iterador_criar".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave".to_string(),
        TypeIR::Verso,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
        TypeIR::Verso,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_criar_0".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_anexar_b".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_anexar_v".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert("__pinker_internal_leque_tag".to_string(), TypeIR::Bombom);
    sigs_intrinsecas.insert(
        "__pinker_internal_leque_carga_b".to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert("__pinker_internal_leque_carga_v".to_string(), TypeIR::Verso);
    // D1: cargas de lista reutilizam o caminho de uma palavra.
    sigs_intrinsecas.insert(
        crate::enum_payload::ANEXAR_LISTA_BOMBOM.to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::ANEXAR_LISTA_VERSO.to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::CARGA_LISTA_BOMBOM.to_string(),
        TypeIR::ListBombom,
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::CARGA_LISTA_VERSO.to_string(),
        TypeIR::ListVerso,
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::ANEXAR_SAIDA_PROCESSO.to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        crate::enum_payload::CARGA_SAIDA_PROCESSO.to_string(),
        TypeIR::OpaqueWordHandle,
    );
    // União não tem intrínseca chamável: `UnionTag`/`UnionExtract` são
    // operações internas tipadas da seleção.
    // Parte B: leque com carga é handle de uma palavra na seleção.
    for nome in crate::falha_operacional::nomes() {
        sigs_intrinsecas.insert(nome.to_string(), TypeIR::Bombom);
    }
    for nome in crate::valor_json::ACESSORES {
        let (retorno, _) = crate::valor_json::assinatura_ir(nome)
            .expect("acessor JSON sem assinatura na autoridade");
        sigs_intrinsecas.insert(nome.to_string(), retorno);
    }
    for nome in crate::sha256::ACESSORES {
        let (retorno, _) = crate::sha256::assinatura_ir(nome)
            .expect("acessor SHA-256 sem assinatura na autoridade");
        sigs_intrinsecas.insert(nome.to_string(), retorno);
    }
    sigs_intrinsecas.insert(
        crate::saida_processo::ACESSOR_CODIGO.to_string(),
        TypeIR::Bombom,
    );
    sigs_intrinsecas.insert(
        crate::saida_processo::ACESSOR_SAIDA.to_string(),
        TypeIR::Verso,
    );
    sigs_intrinsecas.insert(
        crate::saida_processo::ACESSOR_ERRO.to_string(),
        TypeIR::Verso,
    );
    // Fase 140
    // Fase 137
    // Fase 138
    // Fase 139
    // Fase 158

    // #532: a assinatura consultada depende da identidade decidida na
    // resolução, não da chave textual compartilhada.
    let sigs = crate::intrinsics::identity::TabelaPorIdentidade {
        usuario: sigs_usuario,
        intrinsecas: sigs_intrinsecas,
    };
    for f in &program.functions {
        if f.blocks.is_empty() {
            return Err(err("selected function sem blocos"));
        }
        let mut labels = HashSet::new();
        let mut entry_count = 0usize;
        for b in &f.blocks {
            if !labels.insert(b.label.clone()) {
                return Err(err("selected label duplicado"));
            }
            if b.label == "entry" {
                entry_count += 1;
            }
        }
        if entry_count != 1 {
            return Err(err("selected function deve conter entry único"));
        }

        let mut slots = HashSet::new();
        for p in &f.params {
            slots.insert(p.clone());
        }
        for l in &f.locals {
            slots.insert(l.clone());
        }

        for b in &f.blocks {
            let mut temps = HashSet::new();
            for i in &b.instructions {
                match i {
                    SelectedInstr::Mov { dest, src } => {
                        if !slots.contains(dest) {
                            return Err(err("selected mov para slot inexistente"));
                        }
                        check_operand(src, &slots, &temps, &globals)?;
                    }
                    SelectedInstr::Neg { dest, operand, .. }
                    | SelectedInstr::Not { dest, operand }
                    | SelectedInstr::BitNot { dest, operand, .. } => {
                        check_operand(operand, &slots, &temps, &globals)?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::DerefLoad { dest, ptr, ty, .. } => {
                        check_operand(ptr, &slots, &temps, &globals)?;
                        if *ty == TypeIR::Nulo {
                            return Err(err("selected deref_load não pode retornar nulo"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::UnionInject {
                        dest,
                        value,
                        union_type_id,
                        tag,
                        resolved_member_type_id,
                        canonical_member_key,
                        payload_type,
                        payload_layout,
                    } => {
                        check_operand(value, &slots, &temps, &globals)?;
                        if !payload_layout.is_well_formed() {
                            return Err(err("selected union_inject inválido"));
                        }
                        // A injeção selecionada tem de continuar apontando para o
                        // membro exato decidido no lowering: tag, chave canônica
                        // e identidade resolvida precisam concordar entre si.
                        crate::ir::validate_union_member_reference(
                            &program.union_types,
                            *union_type_id,
                            *tag,
                            canonical_member_key,
                            *payload_type,
                            *payload_layout,
                        )
                        .map_err(|message| err(&message))?;
                        crate::ir::validate_union_member_identity(
                            &program.union_types,
                            *union_type_id,
                            *tag,
                            *resolved_member_type_id,
                        )
                        .map_err(|message| err(&message))?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::UnionTag {
                        dest,
                        value,
                        union_type_id,
                    } => {
                        check_operand(value, &slots, &temps, &globals)?;
                        crate::ir::validate_union_reference(&program.union_types, *union_type_id)
                            .map_err(|message| err(&message))?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::UnionExtract {
                        dest,
                        value,
                        union_type_id,
                        tag,
                        resolved_member_type_id,
                        canonical_member_key,
                        payload_type,
                        payload_layout,
                    } => {
                        check_operand(value, &slots, &temps, &globals)?;
                        crate::ir::validate_union_member_reference(
                            &program.union_types,
                            *union_type_id,
                            *tag,
                            canonical_member_key,
                            *payload_type,
                            *payload_layout,
                        )
                        .map_err(|message| err(&message))?;
                        crate::ir::validate_union_member_identity(
                            &program.union_types,
                            *union_type_id,
                            *tag,
                            *resolved_member_type_id,
                        )
                        .map_err(|message| err(&message))?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::DerefStore { ptr, value, ty, .. } => {
                        check_operand(ptr, &slots, &temps, &globals)?;
                        check_operand(value, &slots, &temps, &globals)?;
                        if *ty == TypeIR::Nulo {
                            return Err(err("selected deref_store não pode receber nulo"));
                        }
                    }
                    SelectedInstr::Cast {
                        dest,
                        value,
                        target_type,
                    } => {
                        check_operand(value, &slots, &temps, &globals)?;
                        if *target_type == TypeIR::Nulo {
                            return Err(err("selected cast não pode ter alvo nulo"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::Add { dest, lhs, rhs, .. }
                    | SelectedInstr::BitAnd { dest, lhs, rhs, .. }
                    | SelectedInstr::BitOr { dest, lhs, rhs, .. }
                    | SelectedInstr::BitXor { dest, lhs, rhs, .. }
                    | SelectedInstr::Shl { dest, lhs, rhs, .. }
                    | SelectedInstr::Shr { dest, lhs, rhs, .. }
                    | SelectedInstr::Sub { dest, lhs, rhs, .. }
                    | SelectedInstr::Mul { dest, lhs, rhs, .. }
                    | SelectedInstr::Div { dest, lhs, rhs, .. }
                    | SelectedInstr::Mod { dest, lhs, rhs, .. }
                    | SelectedInstr::CmpEq { dest, lhs, rhs, .. }
                    | SelectedInstr::CmpNe { dest, lhs, rhs, .. }
                    | SelectedInstr::CmpLt { dest, lhs, rhs, .. }
                    | SelectedInstr::CmpLe { dest, lhs, rhs, .. }
                    | SelectedInstr::CmpGt { dest, lhs, rhs, .. }
                    | SelectedInstr::CmpGe { dest, lhs, rhs, .. } => {
                        check_operand(lhs, &slots, &temps, &globals)?;
                        check_operand(rhs, &slots, &temps, &globals)?;
                        temps.insert(*dest);
                    }
                    SelectedInstr::PointerOffset {
                        dest,
                        pointer,
                        offset,
                        pointer_type,
                        element_size,
                        element_align,
                    } => {
                        check_operand(pointer, &slots, &temps, &globals)?;
                        check_operand(offset, &slots, &temps, &globals)?;
                        if !matches!(pointer_type, TypeIR::Pointer { .. })
                            || *element_size == 0
                            || *element_align == 0
                            || !element_align.is_power_of_two()
                            || *element_size % *element_align != 0
                        {
                            return Err(err("selected pointer_offset possui layout inválido"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::Call {
                        dest,
                        callee,
                        args,
                        ret_type,
                        identidade,
                    } => {
                        for a in args {
                            check_operand(a, &slots, &temps, &globals)?;
                        }
                        if !(identidade.dispatches_as_builtin()
                            && (callee == "__ternario"
                                || generic_map_intrinsic_ret_matches(callee, *ret_type)))
                        {
                            let Some(sig) = sigs.resolver(*identidade, callee) else {
                                return Err(err("selected call para função inexistente"));
                            };
                            if !sig.is_compatible_with(*ret_type) {
                                return Err(err("selected call com ret_type inválido"));
                            }
                        }
                        if *ret_type == TypeIR::Nulo {
                            return Err(err("selected call nulo não pode ter destino"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::MakeTraitObject {
                        dest,
                        value,
                        trait_name,
                        concrete_type,
                        concrete_type_name,
                        concrete_size,
                        vtable_methods,
                    } => {
                        check_operand(value, &slots, &temps, &globals)?;

                        if trait_name.trim().is_empty() {
                            return Err(err("selected make_trait_object sem nome de trato"));
                        }

                        if concrete_type_name.trim().is_empty() {
                            return Err(err("selected make_trait_object sem nome concreto"));
                        }

                        if *concrete_size == 0 {
                            return Err(err("selected make_trait_object com snapshot zero"));
                        }

                        if vtable_methods.is_empty()
                            || vtable_methods.iter().any(|method| method.trim().is_empty())
                        {
                            return Err(err("selected make_trait_object exige vtable não vazia"));
                        }

                        if matches!(*concrete_type, TypeIR::TraitObject | TypeIR::Nulo) {
                            return Err(err(
                                "selected make_trait_object com tipo concreto inválido",
                            ));
                        }

                        temps.insert(*dest);
                    }
                    SelectedInstr::TraitCall {
                        dest,
                        object,
                        trait_name,
                        method_name,
                        method_slot,
                        method_count,
                        args,
                        param_types,
                        ret_type,
                    } => {
                        check_operand(object, &slots, &temps, &globals)?;

                        for arg in args {
                            check_operand(arg, &slots, &temps, &globals)?;
                        }

                        if trait_name.trim().is_empty() || method_name.trim().is_empty() {
                            return Err(err("selected trait_call sem identidade nominal completa"));
                        }
                        if *method_count == 0 || *method_slot >= *method_count {
                            return Err(err("selected trait_call referencia slot fora da vtable"));
                        }

                        if args.len() != param_types.len() {
                            return Err(err("selected trait_call com aridade inconsistente"));
                        }

                        match (*dest, *ret_type) {
                            (Some(_), TypeIR::Nulo) => {
                                return Err(err("selected trait_call nulo não pode ter destino"));
                            }
                            (None, TypeIR::Nulo) => {}
                            (Some(dest), _) => {
                                temps.insert(dest);
                            }
                            (None, _) => {
                                return Err(err("selected trait_call com retorno exige destino"));
                            }
                        }
                    }
                    SelectedInstr::CallIndirect {
                        dest,
                        callee,
                        args,
                        ret_type,
                    } => {
                        check_operand(callee, &slots, &temps, &globals)?;
                        for a in args {
                            check_operand(a, &slots, &temps, &globals)?;
                        }
                        if *ret_type == TypeIR::Nulo {
                            return Err(err("selected call_indirect nulo não pode ter destino"));
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::CallRaw {
                        dest,
                        callee,
                        args,
                        param_types,
                        ret_type,
                    } => {
                        check_operand(callee, &slots, &temps, &globals)?;
                        if args.len() != param_types.len() {
                            return Err(err("selected call_raw com aridade inconsistente"));
                        }
                        for arg in args {
                            check_operand(arg, &slots, &temps, &globals)?;
                        }
                        match (dest, *ret_type) {
                            (Some(_), TypeIR::Nulo) => {
                                return Err(err("selected call_raw nulo não pode ter destino"));
                            }
                            (Some(dest), _) => {
                                temps.insert(*dest);
                            }
                            (None, TypeIR::Nulo) => {}
                            (None, _) => {
                                return Err(err("selected call_raw com retorno exige destino"));
                            }
                        }
                    }
                    SelectedInstr::CallVoid {
                        callee,
                        args,
                        identidade,
                    } => {
                        for a in args {
                            check_operand(a, &slots, &temps, &globals)?;
                        }
                        if identidade.dispatches_as_builtin() && generic_map_intrinsic_void(callee)
                        {
                            continue;
                        }
                        let Some(sig) = sigs.resolver(*identidade, callee) else {
                            return Err(err("selected call_void para função inexistente"));
                        };
                        if !sig.is_compatible_with(TypeIR::Nulo) {
                            return Err(err("selected call_void exige função nulo"));
                        }
                    }
                    SelectedInstr::MakeClosure {
                        dest,
                        function_name,
                        captures,
                    } => {
                        if !sigs.contem_grafia(function_name) {
                            return Err(err("selected make_closure para função inexistente"));
                        }
                        for capture in captures {
                            check_operand(capture, &slots, &temps, &globals)?;
                        }
                        temps.insert(*dest);
                    }
                    SelectedInstr::Falar { args: _ } => {}
                    SelectedInstr::InlineAsm {
                        chunks,
                        operands,
                        clobbers,
                        ..
                    } => {
                        if chunks.is_empty() || chunks.iter().any(|chunk| chunk.trim().is_empty()) {
                            return Err(err("selected inline_asm exige chunks não vazios"));
                        }
                        let mut specs = Vec::new();
                        for operand in operands {
                            match operand {
                                crate::cfg_ir::InlineAsmOperandCfgIR::Input {
                                    name,
                                    constraint,
                                    value,
                                    ..
                                } => {
                                    check_operand(value, &slots, &temps, &globals)?;
                                    specs.push((name.clone(), *constraint));
                                }
                                crate::cfg_ir::InlineAsmOperandCfgIR::Output {
                                    name,
                                    constraint,
                                    slot,
                                    ..
                                } => {
                                    if !slots.contains(slot) {
                                        return Err(err(
                                            "selected inline_asm aponta para output inexistente",
                                        ));
                                    }
                                    specs.push((name.clone(), *constraint));
                                }
                            }
                        }
                        crate::inline_asm::validate_bound_operands(chunks, &specs, clobbers)
                            .map_err(|failure| err(&failure.to_string()))?;
                    }
                }
            }

            match &b.terminator {
                SelectedTerminator::Jmp(t) => {
                    if !labels.contains(t) {
                        return Err(err("selected jmp para label inexistente"));
                    }
                }
                SelectedTerminator::Br {
                    cond,
                    then_label,
                    else_label,
                } => {
                    check_operand(cond, &slots, &temps, &globals)?;
                    if !labels.contains(then_label) || !labels.contains(else_label) {
                        return Err(err("selected br para label inexistente"));
                    }
                }
                SelectedTerminator::Ret(v) => match (f.ret_type, v) {
                    (TypeIR::Nulo, Some(_)) => {
                        return Err(err("selected ret com valor em função nulo"));
                    }
                    (TypeIR::Nulo, None) => {}
                    (_, None) => return Err(err("selected ret vazio em função com retorno")),
                    (_, Some(v)) => {
                        check_operand(v, &slots, &temps, &globals)?;
                    }
                },
            }
        }
    }

    Ok(())
}

fn check_operand(
    op: &OperandIR,
    slots: &HashSet<String>,
    temps: &HashSet<crate::cfg_ir::TempIR>,
    globals: &HashSet<String>,
) -> Result<(), PinkerError> {
    match op {
        OperandIR::Local(s) if !slots.contains(s) => Err(err("selected operand local inexistente")),
        OperandIR::GlobalConst(g) if !globals.contains(g) => {
            Err(err("selected operand global inexistente"))
        }
        OperandIR::Temp(t) if !temps.contains(t) => Err(err("selected operand temp inexistente")),
        _ => Ok(()),
    }
}

fn err(msg: &str) -> PinkerError {
    PinkerError::InstrSelectValidation {
        msg: msg.to_string(),
        span: Span::single(Position::new(1, 1)),
    }
}

// @pinker-nav:end select.validacao.invariantes
