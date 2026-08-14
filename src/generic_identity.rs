//! Identidade injetiva estreita de especializações genéricas.
//!
//! A AST ainda possui algumas representações históricas equivalentes. Este
//! módulo normaliza somente as equivalências comprovadas no ponto em que a
//! monomorfização ocorre e enquadra todas as distinções restantes da
//! representação disponível nesse estágio. Não representa o quociente
//! semântico completo após resolução de aliases. A renderização hexadecimal é
//! integral: não é digest e não perde informação.

use crate::ast::Type;

// @pinker-nav:start genericos.identidade-canonica
// @pinker-nav:domain genericos
// @pinker-nav:layer identidade
// @pinker-nav:summary Autoridade única da identidade do estágio atual de monomorfização: preserva apenas equivalências AST já exigidas nesse estágio, enquadra kind/origem/nome/argumentos e tipos recursivos sem fingir resolução semântica de aliases, e renderiza o fluxo completo como hexadecimal ASCII montável.

const FORMAT_MAGIC: &[u8] = b"pinker-generic-specialization-v1";

/// Espécie do template especializado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenericKind {
    Function,
    Enum,
}

impl GenericKind {
    fn tag(self) -> u8 {
        match self {
            Self::Function => 1,
            Self::Enum => 2,
        }
    }

    fn symbol_prefix(self) -> &'static str {
        match self {
            Self::Function => "__gen_",
            Self::Enum => "__gen_leque_",
        }
    }
}

/// Origem canônica já possuída pelo loader.
///
/// `Module` recebe a chave textual de `ImportDecl::module`, a mesma usada pelo
/// loader para ciclo, deduplicação e lookup. Caminho físico, cwd e worktree não
/// participam da identidade.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GenericOrigin {
    Root,
    Module(String),
}

impl GenericOrigin {
    pub fn module(module_key: impl Into<String>) -> Self {
        Self::Module(module_key.into())
    }
}

#[derive(Default)]
struct MonomorphizationBytes {
    bytes: Vec<u8>,
}

impl MonomorphizationBytes {
    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn count(&mut self, value: usize) {
        self.u64(u64::try_from(value).expect("contagem canônica cabe em u64"));
    }

    fn raw_bytes(&mut self, value: &[u8]) {
        self.count(value.len());
        self.bytes.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) {
        self.raw_bytes(value.as_bytes());
    }

    fn nominal(&mut self, name: &str) {
        self.byte(0x30);
        self.text(name);
    }

    fn list_of_nominal(&mut self, name: &str) {
        self.byte(0x20);
        self.nominal(name);
    }

    fn map(&mut self, key: &Type, value: &Type) {
        self.byte(0x21);
        self.ty(key);
        self.ty(value);
    }

    fn ty(&mut self, ty: &Type) {
        match ty {
            Type::Bombom(_) => self.byte(0x01),
            Type::U8(_) => self.byte(0x02),
            Type::U16(_) => self.byte(0x03),
            Type::U32(_) => self.byte(0x04),
            Type::U64(_) => self.byte(0x05),
            Type::I8(_) => self.byte(0x06),
            Type::I16(_) => self.byte(0x07),
            Type::I32(_) => self.byte(0x08),
            Type::I64(_) => self.byte(0x09),
            Type::Logica(_) => self.byte(0x0a),
            Type::Verso(_) => self.byte(0x0b),
            Type::Nulo(_) => self.byte(0x0c),

            Type::ListBombom(_) => {
                self.byte(0x20);
                self.byte(0x01);
            }
            Type::ListVerso(_) => {
                self.byte(0x20);
                self.byte(0x0b);
            }
            Type::ListEnum { element, .. } => self.list_of_nominal(element),

            // Gate A1: as quatro variantes históricas são apenas entrypoints
            // compatíveis da mesma autoridade adulta `Type::Map`.
            Type::MapVersoBombom(span) => {
                self.map(&Type::Verso(*span), &Type::Bombom(*span));
            }
            Type::MapVersoVerso(span) => {
                self.map(&Type::Verso(*span), &Type::Verso(*span));
            }
            Type::MapBombomBombom(span) => {
                self.map(&Type::Bombom(*span), &Type::Bombom(*span));
            }
            Type::MapBombomVerso(span) => {
                self.map(&Type::Bombom(*span), &Type::Verso(*span));
            }
            Type::Map { key, value, .. } => self.map(key, value),

            Type::FixedArray { element, size, .. } => {
                self.byte(0x22);
                self.ty(element);
                self.u64(*size);
            }
            Type::Pointer {
                base, is_volatile, ..
            } => {
                self.byte(0x23);
                self.byte(u8::from(*is_volatile));
                self.ty(base);
            }
            Type::Function { params, ret, .. } => {
                self.byte(0x24);
                self.count(params.len());
                for param in params {
                    self.ty(param);
                }
                self.ty(ret);
            }

            // Gate A2: antes da resolução semântica, a mesma identidade
            // nominal pode chegar como Alias (fonte), Enum/Struct (inferência
            // local ou materialização). A inferência vigente já os compara por
            // nome. OpaqueHandle não participa dessa equivalência.
            Type::Alias { name, .. } | Type::Struct { name, .. } | Type::Enum { name, .. } => {
                self.nominal(name)
            }
            Type::OpaqueHandle { name, .. } => {
                self.byte(0x31);
                self.text(name);
            }
            Type::Applied { name, args, .. } => {
                self.byte(0x32);
                self.text(name);
                self.count(args.len());
                for arg in args {
                    self.ty(arg);
                }
            }
            Type::Union { members, .. } => {
                self.byte(0x33);
                // Este estágio ainda não possui membros semanticamente
                // resolvidos, pré-condição de `union_canon`. Portanto a
                // identidade preserva a representação estrutural disponível:
                // contagem, ordem e identidade de estágio de cada membro.
                self.count(members.len());
                for member in members {
                    self.ty(member);
                }
            }
        }
    }
}

/// Bytes da identidade de tipo disponível no estágio atual de monomorfização.
pub fn monomorphization_type_bytes(ty: &Type) -> Vec<u8> {
    let mut encoder = MonomorphizationBytes::default();
    encoder.ty(ty);
    encoder.bytes
}

/// Bytes completos da identidade de uma especialização no estágio atual.
pub fn monomorphization_specialization_bytes(
    kind: GenericKind,
    origin: &GenericOrigin,
    local_generic_name: &str,
    type_arguments: &[Type],
) -> Vec<u8> {
    let mut encoder = MonomorphizationBytes::default();
    encoder.raw_bytes(FORMAT_MAGIC);
    encoder.byte(kind.tag());
    match origin {
        GenericOrigin::Root => encoder.byte(0),
        GenericOrigin::Module(module_key) => {
            encoder.byte(1);
            encoder.text(module_key);
        }
    }
    encoder.text(local_generic_name);
    encoder.count(type_arguments.len());
    for argument in type_arguments {
        encoder.ty(argument);
    }
    encoder.bytes
}

fn full_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(char::from(HEX[usize::from(byte >> 4)]));
        rendered.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    rendered
}

/// Símbolo textual injetivo e seguro para o assembler vigente.
pub fn specialization_name(
    kind: GenericKind,
    origin: &GenericOrigin,
    local_generic_name: &str,
    type_arguments: &[Type],
) -> String {
    let bytes =
        monomorphization_specialization_bytes(kind, origin, local_generic_name, type_arguments);
    format!("{}{}", kind.symbol_prefix(), full_hex(&bytes))
}

// @pinker-nav:end genericos.identidade-canonica

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{Position, Span};

    fn span() -> Span {
        Span::single(Position::new(1, 1))
    }

    fn alias(name: &str) -> Type {
        Type::Alias {
            name: name.to_string(),
            span: span(),
        }
    }

    fn applied(name: &str, args: Vec<Type>) -> Type {
        Type::Applied {
            name: name.to_string(),
            args,
            span: span(),
        }
    }

    fn identity(name: &str, args: Vec<Type>) -> String {
        specialization_name(GenericKind::Enum, &GenericOrigin::Root, name, &args)
    }

    #[test]
    fn gate_a1_mapas_historicos_normalizam_para_a_autoridade_adulta() {
        let cases = [
            (
                Type::MapVersoBombom(span()),
                Type::Map {
                    key: Box::new(Type::Verso(span())),
                    value: Box::new(Type::Bombom(span())),
                    span: span(),
                },
            ),
            (
                Type::MapVersoVerso(span()),
                Type::Map {
                    key: Box::new(Type::Verso(span())),
                    value: Box::new(Type::Verso(span())),
                    span: span(),
                },
            ),
            (
                Type::MapBombomBombom(span()),
                Type::Map {
                    key: Box::new(Type::Bombom(span())),
                    value: Box::new(Type::Bombom(span())),
                    span: span(),
                },
            ),
            (
                Type::MapBombomVerso(span()),
                Type::Map {
                    key: Box::new(Type::Bombom(span())),
                    value: Box::new(Type::Verso(span())),
                    span: span(),
                },
            ),
        ];
        for (historical, adult) in cases {
            assert_eq!(
                monomorphization_type_bytes(&historical),
                monomorphization_type_bytes(&adult)
            );
        }
    }

    #[test]
    fn gate_a2_fases_nominais_convergem_e_handle_opaco_permanece_distinto() {
        let unresolved = alias("Cor");
        let enum_phase = Type::Enum {
            name: "Cor".to_string(),
            span: span(),
        };
        let struct_phase = Type::Struct {
            name: "Cor".to_string(),
            span: span(),
        };
        let opaque = Type::OpaqueHandle {
            name: "Cor".to_string(),
            span: span(),
        };
        assert_eq!(
            monomorphization_type_bytes(&unresolved),
            monomorphization_type_bytes(&enum_phase)
        );
        assert_eq!(
            monomorphization_type_bytes(&unresolved),
            monomorphization_type_bytes(&struct_phase)
        );
        assert_ne!(
            monomorphization_type_bytes(&unresolved),
            monomorphization_type_bytes(&opaque)
        );

        let mixed_phases = Type::Union {
            members: vec![
                alias("A"),
                Type::Enum {
                    name: "B".to_string(),
                    span: span(),
                },
            ],
            span: span(),
        };
        let same_order_other_phases = Type::Union {
            members: vec![
                Type::Enum {
                    name: "A".to_string(),
                    span: span(),
                },
                alias("B"),
            ],
            span: span(),
        };
        assert_eq!(
            monomorphization_type_bytes(&mixed_phases),
            monomorphization_type_bytes(&same_order_other_phases)
        );
    }

    #[test]
    fn c1_nome_do_template_e_fronteira_do_argumento_nao_colidem() {
        assert_ne!(
            identity("A_B", vec![alias("C")]),
            identity("A", vec![alias("B_C")])
        );
    }

    #[test]
    fn c2_nominal_e_estrutura_que_antes_tinham_o_mesmo_texto_nao_colidem() {
        assert_ne!(
            identity("G", vec![alias("lista_verso")]),
            identity("G", vec![Type::ListVerso(span())])
        );
    }

    #[test]
    fn c3_fronteiras_de_argumentos_sao_injetivas() {
        assert_ne!(
            identity("G", vec![alias("A_B"), alias("C")]),
            identity("G", vec![alias("A"), alias("B_C")])
        );
    }

    #[test]
    fn c4_nesting_preserva_nome_aridade_e_argumentos_aplicados() {
        assert_ne!(
            identity(
                "G",
                vec![applied("F", vec![alias("A"), alias("B")]), alias("C")]
            ),
            identity("G", vec![applied("F_A", vec![alias("B")]), alias("C")])
        );
    }

    #[test]
    fn c5_leques_puramente_do_usuario_nao_colidem() {
        assert_ne!(
            identity("Caixa_Doce", vec![alias("Sabor")]),
            identity("Caixa", vec![alias("Doce_Sabor")])
        );
    }

    #[test]
    fn c6_origem_de_modulo_participa_da_identidade() {
        let a = specialization_name(
            GenericKind::Enum,
            &GenericOrigin::module("mod_a"),
            "G",
            &[Type::Verso(span())],
        );
        let b = specialization_name(
            GenericKind::Enum,
            &GenericOrigin::module("mod_b"),
            "G",
            &[Type::Verso(span())],
        );
        assert_ne!(a, b);
    }

    #[test]
    fn matriz_de_identidade_preserva_todos_os_campos_do_estagio() {
        assert_ne!(
            monomorphization_type_bytes(&applied("Foo", vec![alias("A")])),
            monomorphization_type_bytes(&applied("Foo", vec![alias("B")]))
        );
        assert_ne!(
            monomorphization_type_bytes(&Type::FixedArray {
                element: Box::new(Type::U8(span())),
                size: 3,
                span: span(),
            }),
            monomorphization_type_bytes(&Type::FixedArray {
                element: Box::new(Type::U8(span())),
                size: 4,
                span: span(),
            })
        );
        assert_ne!(
            monomorphization_type_bytes(&Type::Pointer {
                base: Box::new(Type::U8(span())),
                is_volatile: false,
                span: span(),
            }),
            monomorphization_type_bytes(&Type::Pointer {
                base: Box::new(Type::U8(span())),
                is_volatile: true,
                span: span(),
            })
        );

        let function = |params: Vec<Type>, ret: Type| Type::Function {
            params,
            ret: Box::new(ret),
            span: span(),
        };
        assert_ne!(
            monomorphization_type_bytes(&function(vec![alias("A")], alias("B"))),
            monomorphization_type_bytes(&function(vec![alias("A"), alias("C")], alias("B")))
        );
        assert_ne!(
            monomorphization_type_bytes(&function(vec![alias("A")], alias("B"))),
            monomorphization_type_bytes(&function(vec![alias("A")], alias("C")))
        );

        let union_ab = Type::Union {
            members: vec![alias("A"), alias("B")],
            span: span(),
        };
        let union_ba = Type::Union {
            members: vec![alias("B"), alias("A")],
            span: span(),
        };
        assert_ne!(
            monomorphization_type_bytes(&union_ab),
            monomorphization_type_bytes(&union_ba),
            "ordem da representação não resolvida pertence à identidade deste estágio"
        );
        assert_ne!(
            monomorphization_type_bytes(&alias("A_B")),
            monomorphization_type_bytes(&union_ab)
        );
        assert_ne!(
            monomorphization_type_bytes(&alias("Café")),
            monomorphization_type_bytes(&alias("Cafe\u{301}"))
        );
    }

    #[test]
    fn union_nao_resolvida_preserva_fronteiras_estruturais() {
        let left = Type::Union {
            members: vec![alias("A_B"), alias("C")],
            span: span(),
        };
        let right = Type::Union {
            members: vec![alias("A"), alias("B_C")],
            span: span(),
        };
        let nested_left = Type::Union {
            members: vec![left.clone(), alias("D")],
            span: span(),
        };
        let nested_right = Type::Union {
            members: vec![right.clone(), alias("D")],
            span: span(),
        };

        assert_ne!(
            monomorphization_type_bytes(&left),
            monomorphization_type_bytes(&right)
        );
        assert_ne!(
            monomorphization_type_bytes(&nested_left),
            monomorphization_type_bytes(&nested_right)
        );
    }

    #[test]
    fn full_alias_canonicalization_permanece_deferred_para_477() {
        // O encoder recebe spelling, não o alvo semântico do alias. Resolver
        // `AA -> A` pertence à auditoria arquitetural #477.
        assert_ne!(
            monomorphization_type_bytes(&alias("AA")),
            monomorphization_type_bytes(&alias("A"))
        );
    }

    #[test]
    fn renderer_e_deterministico_hex_integral_e_montavel() {
        let first = identity("G", vec![alias("Café")]);
        let second = identity("G", vec![alias("Café")]);
        assert_eq!(first, second);
        assert!(first.starts_with("__gen_leque_"));
        assert!(first
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
        let encoded = first.strip_prefix("__gen_leque_").unwrap();
        assert_eq!(encoded.len() % 2, 0);
        assert!(encoded.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn framing_publica_contagem_de_argumentos_em_u64_big_endian() {
        let bytes = monomorphization_specialization_bytes(
            GenericKind::Function,
            &GenericOrigin::Root,
            "G",
            &[alias("A"), alias("B")],
        );
        let count_offset = 8 + FORMAT_MAGIC.len() + 1 + 1 + 8 + "G".len();
        assert_eq!(&bytes[count_offset..count_offset + 8], &2_u64.to_be_bytes());
    }
}
