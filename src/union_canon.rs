//! Canonicalização normativa de uniões estruturais.
//!
//! Este módulo é a **única** definição das chaves e da ordem canônicas de uma
//! união. `semantic` e `ir` consomem exatamente este contrato, de modo que a
//! identidade de um membro é sempre a mesma linhagem:
//!
//! ```text
//! tipo resolvido do membro
//! → chave canônica compartilhada
//! → UnionTypeIR internado
//! → membro exato do registry
//! → tag do registry
//! ```
//!
//! A identidade de um membro nunca depende do nome textual do apelido, da
//! posição do braço de `encaixe`, do span, do índice de declaração nem de
//! qualquer texto de diagnóstico.

// @pinker-nav:start union.unioes.canonicalizacao
// @pinker-nav:domain unioes
// @pinker-nav:layer union
// @pinker-nav:summary Contrato normativo único de canonicalização de uniões estruturais: `CanonicalUnionMemberKey`/`member_key` derivam a chave canônica de um membro já resolvido, `union_key` deriva a chave canônica da união internada e `canonicalize_resolved_members` achata uniões aninhadas, remove duplicatas canônicas e fixa a ordem dos membros. A semântica e o lowering consomem estas funções; nenhuma camada reconstrói chave ou ordem por conta própria.
use crate::ast::Type;
use std::collections::BTreeMap;

/// Identidade canônica de um membro de união.
///
/// É derivada exclusivamente do **tipo resolvido** do membro. Aliases são
/// transparentes: `apelido aa = u8` e `u8` produzem a mesma chave. Tipos
/// canônicos distintos produzem chaves distintas.
///
/// A representação permanece textual e opaca de propósito: a futura identidade
/// nominal (`NominalTypeId`) poderá ser acrescentada como metadado adicional
/// sem invalidar as chaves já gravadas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalUnionMemberKey {
    pub canonical_type_key: String,
}

impl CanonicalUnionMemberKey {
    pub fn as_str(&self) -> &str {
        &self.canonical_type_key
    }
}

/// Chave canônica de um membro já resolvido.
pub fn member_key(ty: &Type) -> CanonicalUnionMemberKey {
    CanonicalUnionMemberKey {
        canonical_type_key: member_key_text(ty),
    }
}

/// Forma textual da chave canônica de membro.
///
/// Os componentes compostos são prefixados por comprimento para que a chave
/// seja injetiva: nomes nominais não podem forjar a chave de outro membro.
pub fn member_key_text(ty: &Type) -> String {
    match ty {
        Type::Bombom(_) => "bombom".to_string(),
        Type::U8(_) => "u8".to_string(),
        Type::U16(_) => "u16".to_string(),
        Type::U32(_) => "u32".to_string(),
        Type::U64(_) => "u64".to_string(),
        Type::I8(_) => "i8".to_string(),
        Type::I16(_) => "i16".to_string(),
        Type::I32(_) => "i32".to_string(),
        Type::I64(_) => "i64".to_string(),
        Type::Logica(_) => "logica".to_string(),
        Type::Verso(_) => "verso".to_string(),
        Type::Struct { name, .. } => format!("struct:{}:{name}", name.len()),
        Type::Enum { name, .. } => format!("enum:{}:{name}", name.len()),
        Type::Pointer {
            base, is_volatile, ..
        } => {
            format!("ptr:{}:{}", u8::from(*is_volatile), member_key_text(base))
        }
        Type::Function { params, ret, .. } => format!(
            "fn({})->{}",
            params
                .iter()
                .map(member_key_text)
                .collect::<Vec<_>>()
                .join(","),
            member_key_text(ret)
        ),
        Type::FixedArray { element, size, .. } => {
            format!("array:{size}:{}", member_key_text(element))
        }
        Type::Union { members, .. } => format!(
            "union:[{}]",
            members
                .iter()
                .map(member_key_text)
                .map(|key| format!("{}:{key}", key.len()))
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => ty.name().to_string(),
    }
}

/// Chave canônica da união internada, derivada dos membros já canonicalizados.
pub fn union_key(members: &[Type]) -> String {
    format!(
        "pinker-union-v1[{}]",
        members
            .iter()
            .map(member_key_text)
            .map(|key| format!("{}:{key}", key.len()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Achata uniões aninhadas, remove duplicatas canônicas e fixa a ordem.
///
/// `resolved` deve conter os membros com apelidos já resolvidos pelo chamador —
/// a resolução depende das tabelas locais de cada camada, mas o achatamento, a
/// deduplicação e a **ordem** são definidos aqui e em nenhum outro lugar.
///
/// A ordem resultante é a ordem crescente de [`member_key_text`] em bytes; a
/// ordem textual da declaração da união não a influencia.
pub fn canonicalize_resolved_members(resolved: Vec<Type>) -> Vec<Type> {
    let mut canonical = BTreeMap::<String, Type>::new();
    for member in resolved {
        match member {
            Type::Union { members, .. } => {
                for nested in members {
                    canonical.insert(member_key_text(&nested), nested);
                }
            }
            other => {
                canonical.insert(member_key_text(&other), other);
            }
        }
    }
    canonical.into_values().collect()
}

/// Localiza o índice canônico de um membro pela chave, sem varredura textual
/// de nomes crus e sem escolha por primeira ocorrência aproximada.
pub fn canonical_member_index(members: &[Type], key: &CanonicalUnionMemberKey) -> Option<usize> {
    members
        .iter()
        .position(|member| member_key_text(member) == key.canonical_type_key)
}
// @pinker-nav:end union.unioes.canonicalizacao
