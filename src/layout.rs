use crate::ast::{StructDecl, Type};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size: u64,
    pub align: u64,
}

pub const POINTER_SIZE: u64 = 8;
pub const POINTER_ALIGN: u64 = 8;

pub fn layout_of_type(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    structs: &HashMap<String, StructDecl>,
) -> Result<TypeLayout, String> {
    layout_of_type_inner(ty, aliases, structs, &mut Vec::new(), &mut Vec::new())
}

pub fn struct_field_offsets(
    struct_name: &str,
    aliases: &HashMap<String, Type>,
    structs: &HashMap<String, StructDecl>,
) -> Result<HashMap<String, u64>, String> {
    struct_field_offsets_inner(
        struct_name,
        aliases,
        structs,
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

fn layout_of_type_inner(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    structs: &HashMap<String, StructDecl>,
    resolving_aliases: &mut Vec<String>,
    resolving_structs: &mut Vec<String>,
) -> Result<TypeLayout, String> {
    match ty {
        Type::Bombom(_) | Type::U64(_) | Type::I64(_) => Ok(TypeLayout { size: 8, align: 8 }),
        Type::U32(_) | Type::I32(_) => Ok(TypeLayout { size: 4, align: 4 }),
        Type::U16(_) | Type::I16(_) => Ok(TypeLayout { size: 2, align: 2 }),
        Type::U8(_) | Type::I8(_) | Type::Logica(_) => Ok(TypeLayout { size: 1, align: 1 }),
        Type::Verso(_) => {
            Err("tipo 'verso' ainda não possui layout estático nesta fase".to_string())
        }
        Type::Pointer { .. } => Ok(TypeLayout {
            size: POINTER_SIZE,
            align: POINTER_ALIGN,
        }),
        Type::FixedArray { element, size, .. } => {
            let element_layout = layout_of_type_inner(
                element,
                aliases,
                structs,
                resolving_aliases,
                resolving_structs,
            )?;
            let total_size = element_layout
                .size
                .checked_mul(*size)
                .ok_or_else(|| "overflow ao calcular tamanho de array fixo".to_string())?;
            Ok(TypeLayout {
                size: total_size,
                align: element_layout.align,
            })
        }
        Type::Struct { name, .. } => {
            let struct_decl = structs
                .get(name)
                .ok_or_else(|| format!("tipo de struct '{}' não existe", name))?;
            if resolving_structs.iter().any(|entry| entry == name) {
                return Err(format!(
                    "layout recursivo de struct '{}' não é suportado nesta fase",
                    name
                ));
            }
            resolving_structs.push(name.clone());
            let mut offset = 0_u64;
            let mut max_align = 1_u64;
            for field in &struct_decl.fields {
                let field_layout = layout_of_type_inner(
                    &field.ty,
                    aliases,
                    structs,
                    resolving_aliases,
                    resolving_structs,
                )?;
                offset = round_up(offset, field_layout.align)?;
                offset = offset
                    .checked_add(field_layout.size)
                    .ok_or_else(|| "overflow ao calcular tamanho de struct".to_string())?;
                max_align = max_align.max(field_layout.align);
            }
            let final_size = round_up(offset, max_align)?;
            resolving_structs.pop();
            Ok(TypeLayout {
                size: final_size,
                align: max_align,
            })
        }
        Type::Alias { name, .. } => {
            if structs.contains_key(name) {
                return layout_of_type_inner(
                    &Type::Struct {
                        name: name.clone(),
                        span: ty.span(),
                    },
                    aliases,
                    structs,
                    resolving_aliases,
                    resolving_structs,
                );
            }
            if resolving_aliases.iter().any(|entry| entry == name) {
                return Err(format!("alias de tipo recursivo detectado em '{}'", name));
            }
            let target = aliases
                .get(name)
                .ok_or_else(|| format!("tipo '{}' não existe", name))?;
            resolving_aliases.push(name.clone());
            let layout = layout_of_type_inner(
                target,
                aliases,
                structs,
                resolving_aliases,
                resolving_structs,
            );
            resolving_aliases.pop();
            layout
        }
        Type::Nulo(_) => Err("tipo 'nulo' não tem layout de memória".to_string()),
    }
}

fn struct_field_offsets_inner(
    struct_name: &str,
    aliases: &HashMap<String, Type>,
    structs: &HashMap<String, StructDecl>,
    resolving_aliases: &mut Vec<String>,
    resolving_structs: &mut Vec<String>,
) -> Result<HashMap<String, u64>, String> {
    let struct_decl = structs
        .get(struct_name)
        .ok_or_else(|| format!("tipo de struct '{}' não existe", struct_name))?;
    if resolving_structs.iter().any(|entry| entry == struct_name) {
        return Err(format!(
            "layout recursivo de struct '{}' não é suportado nesta fase",
            struct_name
        ));
    }

    resolving_structs.push(struct_name.to_string());
    let mut offset = 0_u64;
    let mut offsets = HashMap::new();
    for field in &struct_decl.fields {
        let field_layout = layout_of_type_inner(
            &field.ty,
            aliases,
            structs,
            resolving_aliases,
            resolving_structs,
        )?;
        offset = round_up(offset, field_layout.align)?;
        offsets.insert(field.name.clone(), offset);
        offset = offset
            .checked_add(field_layout.size)
            .ok_or_else(|| "overflow ao calcular tamanho de struct".to_string())?;
    }
    resolving_structs.pop();

    Ok(offsets)
}

fn round_up(value: u64, align: u64) -> Result<u64, String> {
    let addend = align
        .checked_sub(1)
        .ok_or_else(|| "alinhamento inválido".to_string())?;
    let with_add = value
        .checked_add(addend)
        .ok_or_else(|| "overflow ao arredondar alinhamento".to_string())?;
    Ok((with_add / align) * align)
}
