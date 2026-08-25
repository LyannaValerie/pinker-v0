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
// @pinker-nav:summary Contrato normativo único de canonicalização: `canonical_type_key` deriva a identidade de um tipo já resolvido; `canonical_type_graph_key` expande aliases em um DAG internado, canonicaliza uniões e serializa todos os bytes sem digest probabilístico nem expansão exponencial; `nominal_identity_of` expõe identidade nominal; `CanonicalUnionMemberKey`/`member_key`, `union_key` e `canonicalize_resolved_members` compartilham a mesma linhagem, achatando, deduplicando e ordenando membros. Semântica, projeção e lowering consomem estas funções; nenhuma camada reconstrói chave ou ordem por conta própria.
use crate::ast::Type;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

/// Categoria nominal de um tipo declarado pelo usuário.
///
/// Existe para que a identidade resolvida possa ser validada contra a
/// declaração de origem sem reintroduzir o nome textual como autoridade de
/// seleção.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NominalTypeKind {
    Ninho,
    Leque,
    OpaqueBuiltin,
}

impl NominalTypeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NominalTypeKind::Ninho => "ninho",
            NominalTypeKind::Leque => "leque",
            NominalTypeKind::OpaqueBuiltin => "handle opaco builtin",
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
        Type::OpaqueHandle { name, .. } => Some((NominalTypeKind::OpaqueBuiltin, name.clone())),
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

/// Chave exata da identidade semântica de um tipo com apelidos transparentes.
///
/// A chave é uma serialização injetiva de um DAG internado, não um digest. Isso
/// preserva custo proporcional ao grafo de apelidos mesmo para diamantes como
/// `An = mapa<An-1, An-1>`, sem promover uma colisão probabilística a igualdade
/// estrutural. Uniões são achatadas, deduplicadas e ordenadas aqui, pela mesma
/// autoridade que [`canonicalize_resolved_members`].
pub fn canonical_type_graph_key(ty: &Type, aliases: &HashMap<String, Type>) -> String {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    enum Node {
        Atom(&'static str),
        Nominal(String),
        Opaque(String),
        List(Rc<Node>),
        Map(Rc<Node>, Rc<Node>),
        FixedArray(Rc<Node>, u64),
        Pointer(bool, Rc<Node>),
        Function(Vec<Rc<Node>>, Rc<Node>),
        Applied(Rc<Node>, Vec<Rc<Node>>),
        Union(Vec<Rc<Node>>),
        Cycle,
    }

    struct Builder<'a> {
        aliases: &'a HashMap<String, Type>,
        visiting: Vec<String>,
        aliases_done: HashMap<String, Rc<Node>>,
        interned: BTreeMap<Node, Rc<Node>>,
    }

    impl Builder<'_> {
        fn intern(&mut self, node: Node) -> Rc<Node> {
            if let Some(existing) = self.interned.get(&node) {
                return Rc::clone(existing);
            }
            let canonical = Rc::new(node.clone());
            self.interned.insert(node, Rc::clone(&canonical));
            canonical
        }

        fn atom(&mut self, name: &'static str) -> Rc<Node> {
            self.intern(Node::Atom(name))
        }

        fn nominal(&mut self, name: &str) -> Rc<Node> {
            let Some(target) = self.aliases.get(name).cloned() else {
                return self.intern(Node::Nominal(name.to_string()));
            };
            if let Some(done) = self.aliases_done.get(name) {
                return Rc::clone(done);
            }
            if self.visiting.iter().any(|visiting| visiting == name) {
                return self.intern(Node::Cycle);
            }
            self.visiting.push(name.to_string());
            let resolved = self.build(&target);
            self.visiting.pop();
            self.aliases_done
                .insert(name.to_string(), Rc::clone(&resolved));
            resolved
        }

        fn build(&mut self, ty: &Type) -> Rc<Node> {
            match ty {
                Type::Bombom(_) => self.atom("bombom"),
                Type::U8(_) => self.atom("u8"),
                Type::U16(_) => self.atom("u16"),
                Type::U32(_) => self.atom("u32"),
                Type::U64(_) => self.atom("u64"),
                Type::I8(_) => self.atom("i8"),
                Type::I16(_) => self.atom("i16"),
                Type::I32(_) => self.atom("i32"),
                Type::I64(_) => self.atom("i64"),
                Type::Logica(_) => self.atom("logica"),
                Type::Verso(_) => self.atom("verso"),
                Type::Nulo(_) => self.atom("nulo"),
                Type::ListBombom(_) => {
                    let element = self.atom("bombom");
                    self.intern(Node::List(element))
                }
                Type::ListVerso(_) => {
                    let element = self.atom("verso");
                    self.intern(Node::List(element))
                }
                Type::MapVersoBombom(_) => {
                    let key = self.atom("verso");
                    let value = self.atom("bombom");
                    self.intern(Node::Map(key, value))
                }
                Type::MapVersoVerso(_) => {
                    let key = self.atom("verso");
                    let value = self.atom("verso");
                    self.intern(Node::Map(key, value))
                }
                Type::MapBombomBombom(_) => {
                    let key = self.atom("bombom");
                    let value = self.atom("bombom");
                    self.intern(Node::Map(key, value))
                }
                Type::MapBombomVerso(_) => {
                    let key = self.atom("bombom");
                    let value = self.atom("verso");
                    self.intern(Node::Map(key, value))
                }
                Type::OpaqueHandle { name, .. } => self.intern(Node::Opaque(name.clone())),
                Type::ListEnum { element, .. } => {
                    let element = self.nominal(element);
                    self.intern(Node::List(element))
                }
                Type::Alias { name, .. } | Type::Struct { name, .. } | Type::Enum { name, .. } => {
                    self.nominal(name)
                }
                Type::Applied { name, args, .. } => {
                    let base = self.nominal(name);
                    let args = args.iter().map(|arg| self.build(arg)).collect();
                    self.intern(Node::Applied(base, args))
                }
                Type::Map { key, value, .. } => {
                    let key = self.build(key);
                    let value = self.build(value);
                    self.intern(Node::Map(key, value))
                }
                Type::FixedArray { element, size, .. } => {
                    let element = self.build(element);
                    self.intern(Node::FixedArray(element, *size))
                }
                Type::Pointer {
                    base, is_volatile, ..
                } => {
                    let base = self.build(base);
                    self.intern(Node::Pointer(*is_volatile, base))
                }
                Type::Function { params, ret, .. } => {
                    let params = params.iter().map(|param| self.build(param)).collect();
                    let ret = self.build(ret);
                    self.intern(Node::Function(params, ret))
                }
                Type::Union { members, .. } => {
                    let mut canonical = BTreeSet::<Rc<Node>>::new();
                    for member in members {
                        let member = self.build(member);
                        match member.as_ref() {
                            Node::Union(nested) => canonical.extend(nested.iter().cloned()),
                            _ => {
                                canonical.insert(member);
                            }
                        }
                    }
                    self.intern(Node::Union(canonical.into_iter().collect()))
                }
            }
        }
    }

    fn encode_text(out: &mut Vec<u8>, text: &str) {
        out.extend_from_slice(&(text.len() as u64).to_be_bytes());
        out.extend_from_slice(text.as_bytes());
    }

    fn emit(node: &Rc<Node>, ids: &mut HashMap<*const Node, u64>, defs: &mut Vec<Vec<u8>>) -> u64 {
        let pointer = Rc::as_ptr(node);
        if let Some(id) = ids.get(&pointer) {
            return *id;
        }

        let mut definition = Vec::new();
        match node.as_ref() {
            Node::Atom(name) => {
                definition.push(0x01);
                encode_text(&mut definition, name);
            }
            Node::Nominal(name) => {
                definition.push(0x02);
                encode_text(&mut definition, name);
            }
            Node::Opaque(name) => {
                definition.push(0x03);
                encode_text(&mut definition, name);
            }
            Node::List(element) => {
                definition.push(0x10);
                definition.extend_from_slice(&emit(element, ids, defs).to_be_bytes());
            }
            Node::Map(key, value) => {
                definition.push(0x11);
                definition.extend_from_slice(&emit(key, ids, defs).to_be_bytes());
                definition.extend_from_slice(&emit(value, ids, defs).to_be_bytes());
            }
            Node::FixedArray(element, size) => {
                definition.push(0x12);
                definition.extend_from_slice(&emit(element, ids, defs).to_be_bytes());
                definition.extend_from_slice(&size.to_be_bytes());
            }
            Node::Pointer(is_volatile, base) => {
                definition.push(0x13);
                definition.push(u8::from(*is_volatile));
                definition.extend_from_slice(&emit(base, ids, defs).to_be_bytes());
            }
            Node::Function(params, ret) => {
                definition.push(0x14);
                definition.extend_from_slice(&(params.len() as u64).to_be_bytes());
                for param in params {
                    definition.extend_from_slice(&emit(param, ids, defs).to_be_bytes());
                }
                definition.extend_from_slice(&emit(ret, ids, defs).to_be_bytes());
            }
            Node::Applied(base, args) => {
                definition.push(0x15);
                definition.extend_from_slice(&emit(base, ids, defs).to_be_bytes());
                definition.extend_from_slice(&(args.len() as u64).to_be_bytes());
                for arg in args {
                    definition.extend_from_slice(&emit(arg, ids, defs).to_be_bytes());
                }
            }
            Node::Union(members) => {
                definition.push(0x16);
                definition.extend_from_slice(&(members.len() as u64).to_be_bytes());
                for member in members {
                    definition.extend_from_slice(&emit(member, ids, defs).to_be_bytes());
                }
            }
            Node::Cycle => definition.push(0x7f),
        }

        let id = defs.len() as u64;
        defs.push(definition);
        ids.insert(pointer, id);
        id
    }

    let mut builder = Builder {
        aliases,
        visiting: Vec::new(),
        aliases_done: HashMap::new(),
        interned: BTreeMap::new(),
    };
    let root = builder.build(ty);
    let mut ids = HashMap::new();
    let mut defs = Vec::new();
    let root_id = emit(&root, &mut ids, &mut defs);

    let mut bytes = b"pinker-type-dag-v1".to_vec();
    bytes.extend_from_slice(&(defs.len() as u64).to_be_bytes());
    for definition in defs {
        bytes.extend_from_slice(&(definition.len() as u64).to_be_bytes());
        bytes.extend_from_slice(&definition);
    }
    bytes.extend_from_slice(&root_id.to_be_bytes());

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(char::from(HEX[usize::from(byte >> 4)]));
        rendered.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    rendered
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
        Type::OpaqueHandle { name, .. } => format!("opaque:{}:{name}", name.len()),
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
