//! Canonicalização normativa de tipos resolvidos e de uniões estruturais.
//!
//! Este módulo é a **única** definição das chaves canônicas de identidade
//! semântica de um tipo e da ordem canônica dos membros de uma união.
//! `semantic` e `ir` consomem exatamente este contrato, de modo que a
//! identidade de um membro é sempre a mesma linhagem:
//!
//! ```text
//! tipo AST
//! → resolução integral de apelidos
//! → chave canônica compartilhada (identidade semântica)
//! → ResolvedTypeId internado
//! → UnionTypeIR internado
//! → membro exato do registry
//! → tag do registry
//! ```
//!
//! A identidade de um tipo nunca depende do nome textual do apelido, da
//! posição do braço de `encaixe`, do span, do índice de declaração, da ordem
//! de iteração de mapas nem de qualquer texto de diagnóstico. Em particular
//! ela **não** é derivável de `TypeIR`, que representa apenas a categoria
//! operacional do valor: `ninho Alfa` e `ninho Beta` compartilham
//! `TypeIR::Struct`, dois `leque` distintos compartilham a representação
//! escalar, e assinaturas ou ponteiros diferentes compartilham
//! `TypeIR::Function`/`TypeIR::Pointer`.

// @pinker-nav:start union.unioes.canonicalizacao
// @pinker-nav:domain unioes
// @pinker-nav:layer union
// @pinker-nav:summary Contrato normativo único de canonicalização: `canonical_type_key` deriva a identidade semântica completa de um tipo já resolvido (injetiva por prefixo de comprimento, independente de apelido, span e ordem de mapa), `nominal_identity_of` expõe a identidade nominal de `ninho`/`leque`, `CanonicalUnionMemberKey`/`member_key` reaproveitam a mesma chave para membros de união, `union_key` deriva a chave canônica da união internada e `canonicalize_resolved_members` achata uniões aninhadas, remove duplicatas canônicas e fixa a ordem dos membros. A semântica e o lowering consomem estas funções; nenhuma camada reconstrói chave ou ordem por conta própria.
use crate::ast::Type;
use std::collections::BTreeMap;

/// Categoria nominal de um tipo declarado pelo usuário.
///
/// Existe para que a identidade resolvida possa ser validada contra a
/// declaração de origem sem reintroduzir o nome textual como autoridade de
/// seleção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NominalTypeKind {
    Ninho,
    Leque,
}

impl NominalTypeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NominalTypeKind::Ninho => "ninho",
            NominalTypeKind::Leque => "leque",
        }
    }
}

/// Identidade nominal de um tipo já resolvido, quando existir.
///
/// `apelido` é transparente: o chamador deve resolver apelidos antes, exatamente
/// como faz para a chave canônica.
pub fn nominal_identity_of(ty: &Type) -> Option<(NominalTypeKind, String)> {
    match ty {
        Type::Struct { name, .. } => Some((NominalTypeKind::Ninho, name.clone())),
        Type::Enum { name, .. } => Some((NominalTypeKind::Leque, name.clone())),
        _ => None,
    }
}

/// Chave canônica da identidade semântica completa de um tipo já resolvido.
///
/// É a mesma função que define a chave de um membro de união — a identidade de
/// um valor e a identidade de um membro precisam ser comparáveis por igualdade
/// exata, e não por categoria operacional.
pub fn canonical_type_key(ty: &Type) -> String {
    member_key_text(ty)
}

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
        Type::ListBombom(_) => "lista<bombom>".to_string(),
        Type::ListVerso(_) => "lista<verso>".to_string(),
        Type::MapVersoBombom(_) => "mapa<verso,bombom>".to_string(),
        Type::MapVersoVerso(_) => "mapa<verso,verso>".to_string(),
        Type::MapBombomBombom(_) => "mapa<bombom,bombom>".to_string(),
        Type::MapBombomVerso(_) => "mapa<bombom,verso>".to_string(),
        Type::Map { key, value, .. } => {
            format!("mapa<{},{}>", member_key_text(key), member_key_text(value))
        }
        Type::Struct { name, .. } => format!("struct:{}:{name}", name.len()),
        Type::Enum { name, .. } => format!("enum:{}:{name}", name.len()),
        Type::Pointer {
            base, is_volatile, ..
        } => {
            format!("ptr:{}:{}", u8::from(*is_volatile), member_key_text(base))
        }
        Type::Function { params, ret, .. } => {
            // Cada componente é prefixado por comprimento: duas assinaturas
            // diferentes nunca podem produzir a mesma chave por concatenação.
            let ret = member_key_text(ret);
            format!(
                "fn({})->{}:{ret}",
                params
                    .iter()
                    .map(member_key_text)
                    .map(|key| format!("{}:{key}", key.len()))
                    .collect::<Vec<_>>()
                    .join(","),
                ret.len()
            )
        }
        Type::FixedArray { element, size, .. } => {
            let element = member_key_text(element);
            format!("array:{size}:{}:{element}", element.len())
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
        // `lista<Leque>` é nominal: duas listas de leques diferentes não podem
        // colapsar na mesma identidade só porque a representação é a mesma.
        Type::ListEnum { element, .. } => {
            format!("lista<leque>:{}:{element}", element.len())
        }
        // Tipos aplicados (`trato<Nome>`) preservam nome e argumentos. O
        // argumento de `trato<...>` é um **nome de trato**, que é nominal e não
        // um apelido de tipo a resolver: `trato<Falante>` e `trato<Somavel>` têm
        // identidades distintas exatamente por esse nome.
        Type::Applied { name, args, .. } => format!(
            "aplicado:{}:{name}[{}]",
            name.len(),
            args.iter()
                .map(|arg| match arg {
                    Type::Alias { name, .. } => format!("nome:{}:{name}", name.len()),
                    other => member_key_text(other),
                })
                .map(|key| format!("{}:{key}", key.len()))
                .collect::<Vec<_>>()
                .join(",")
        ),
        // Um apelido só chega aqui se o chamador esqueceu de resolvê-lo. A
        // chave resultante é deliberadamente impossível de casar com qualquer
        // membro de união, para que a perda de resolução seja um erro visível
        // e nunca uma identidade aproximada.
        Type::Alias { name, .. } => format!("?apelido-nao-resolvido:{}:{name}", name.len()),
        Type::Nulo(_) => "nulo".to_string(),
    }
}

/// Verdadeiro quando a chave é um marcador de identidade perdida.
///
/// Nenhum membro de união pode carregar uma chave envenenada; a igualdade
/// exata da injeção usa esta função apenas para escolher o diagnóstico.
pub fn is_poisoned_key(key: &str) -> bool {
    key.starts_with('?')
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
