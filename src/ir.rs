//! IR estruturada — primeira representação interna após a análise semântica.
//!
//! Preserva a estrutura do programa (funções, blocos, `if/else` aninhados) porém substitui
//! referências de nome por slots normalizados e explicita tipos em cada nó.
//! Esta camada ainda não divide o fluxo de controle em blocos básicos — isso ocorre em `cfg_ir`.
//!
//! Convenção de nomes de slots: `%nome#N`, onde `N` é um contador por nome-fonte.
//! Isso permite múltiplas declarações do mesmo nome em escopos distintos sem colisão.
//!
//! Posição no pipeline:
//!   `semantic` → **`ir`** → `ir_validate` → `cfg_ir`

use crate::ast::{
    transitive_free_identifiers_in_function, AssignTarget, BinaryOp, Block, BreakStmt, ConstDecl,
    ContinueStmt, ElseBlock, EnumMatchStmt, EnumPattern, Expr, ExprKind, FalarStmt, FunctionDecl,
    IfStmt, InlineAsmStmt, Item, LetStmt, Program, ReturnStmt, Stmt, StructDecl, Type, UnaryOp,
    UnionMatchStmt, WhileStmt,
};
use crate::error::PinkerError;
use crate::layout;
use crate::token::{Position, Span};
use crate::union_canon;
use std::collections::{HashMap, HashSet};

// @pinker-nav:start ir.modelo.representacao
// @pinker-nav:domain modelo
// @pinker-nav:layer ir
// @pinker-nav:summary Modelo de dados da IR estruturada: programa, constantes, funções, blocos, instruções, valores, tipos (`TypeIR`/`ScalarTypeIR`) e operadores — a representação com slots normalizados e tipos explícitos produzida após a semântica.
/// Programa completo na IR estruturada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramIR {
    pub module_name: String,
    pub is_freestanding: bool,
    /// Tabela internada de identidades semânticas resolvidas, em ordem canônica
    /// por chave. É a única autoridade de identidade de tipo do programa.
    pub resolved_types: Vec<ResolvedTypeIR>,
    pub union_types: Vec<UnionTypeIR>,
    /// Metadata das variantes de `leque`, em ordem estável por leque e por
    /// discriminante. Cada carga carrega representação operacional **e**
    /// identidade semântica resolvida; nenhuma camada posterior reconstrói uma
    /// a partir da outra.
    pub enum_variants: Vec<EnumVariantMetaIR>,
    pub consts: Vec<ConstIR>,
    pub functions: Vec<FunctionIR>,
}

/// Constante global (`eterno`). `value` é sempre um literal ou referência a outra global.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstIR {
    pub name: String,
    pub ty: TypeIR,
    pub value: ValueIR,
    pub span: Span,
}

/// Função na IR estruturada. `entry` contém o único bloco da função (ainda não dividido em CFG).
/// `params` lista os parâmetros como bindings; `locals` lista variáveis locais declaradas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionIR {
    pub name: String,
    pub params: Vec<BindingIR>,
    pub locals: Vec<LocalIR>,
    pub ret_type: TypeIR,
    pub entry: BlockIR,
    pub span: Span,
}

/// Parâmetro ou binding de escopo. `source_name` é o nome original; `slot` é o nome normalizado.
///
/// `ty` é a categoria operacional e `resolved` é a identidade semântica
/// completa. As duas viajam juntas: nenhuma camada posterior pode reconstruir a
/// identidade a partir de `ty`, porque tipos nominais distintos compartilham a
/// mesma representação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingIR {
    pub source_name: String,
    pub slot: String,
    pub ty: TypeIR,
    /// Identidade semântica do parâmetro/slot. Mesma convenção de
    /// [`LocalIR::resolved`]: `None` significa que a representação já é a
    /// identidade completa, nunca que a identidade foi descartada.
    pub resolved: Option<ResolvedTypeId>,
}

impl BindingIR {
    pub fn type_ref(&self) -> Option<TypeRefIR> {
        self.resolved
            .map(|resolved| TypeRefIR::new(self.ty, resolved))
    }
}

/// Variável local declarada por `nova`. `is_mut` reflete a palavra-chave `muda`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIR {
    pub source_name: String,
    pub slot: String,
    pub ty: TypeIR,
    /// Identidade semântica do slot.
    ///
    /// `None` apenas nos temporários fabricados por camadas posteriores
    /// (`%logic#N`, `%ternary#N`), que nunca são fonte de injeção de união e cuja
    /// identidade é a própria representação. Todo local originado de uma
    /// declaração do usuário carrega `Some`.
    pub resolved: Option<ResolvedTypeId>,
    pub is_mut: bool,
}

impl LocalIR {
    pub fn type_ref(&self) -> Option<TypeRefIR> {
        self.resolved
            .map(|resolved| TypeRefIR::new(self.ty, resolved))
    }
}

/// Bloco de instruções com label e span. Na IR estruturada, `if/else` é uma instrução,
/// não um conjunto de blocos — a divisão em blocos básicos ocorre em `cfg_ir`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIR {
    pub label: String,
    pub instructions: Vec<InstructionIR>,
    pub span: Span,
}

/// Instrução da IR estruturada. `If` preserva o bloco `then` e o bloco `else` como filhos diretos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionIR {
    Let {
        slot: String,
        value: ValueIR,
        span: Span,
    },
    Assign {
        slot: String,
        value: ValueIR,
        span: Span,
    },
    StoreIndirect {
        ptr: ValueIR,
        value: ValueIR,
        value_type: TypeIR,
        is_volatile: bool,
        span: Span,
    },
    StoreFieldIndirect {
        base: ValueIR,
        field: String,
        field_offset: u64,
        value: ValueIR,
        value_type: TypeIR,
        is_volatile: bool,
        span: Span,
    },
    StoreIndexed {
        base: ValueIR,
        index: ValueIR,
        value: ValueIR,
        element_type: TypeIR,
        span: Span,
    },
    Expr {
        value: ValueIR,
        span: Span,
    },
    Return {
        value: Option<ValueIR>,
        span: Span,
    },
    If {
        condition: ValueIR,
        then_block: BlockIR,
        else_block: Option<BlockIR>,
        span: Span,
    },
    While {
        condition: ValueIR,
        body_block: BlockIR,
        span: Span,
    },
    Break {
        loop_exit_label: String,
        span: Span,
    },
    Continue {
        loop_continue_label: String,
        span: Span,
    },
    Falar {
        args: Vec<FalarArgIR>,
        span: Span,
    },
    InlineAsm {
        chunks: Vec<String>,
        operands: Vec<InlineAsmOperandIR>,
        clobbers: Vec<crate::inline_asm::AsmClobber>,
        span: Span,
    },
    /// `encaixe` de leque preservando a árvore recursiva de patterns.
    EnumMatch(EnumMatchIR),
    /// `encaixe` de união já associado ao registry canônico.
    UnionMatch(UnionMatchIR),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMatchIR {
    pub scrutinee: ValueIR,
    pub scrutinee_binding: BindingIR,
    pub arms: Vec<EnumMatchArmIR>,
    pub otherwise: Option<BlockIR>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMatchArmIR {
    pub pattern: EnumPatternIR,
    pub body: BlockIR,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumPatternIR {
    Binding {
        binding: BindingIR,
        span: Span,
    },
    Variant {
        enum_name: String,
        expected_type_id: ResolvedTypeId,
        variant_name: String,
        discriminant: u64,
        has_payload: bool,
        payloads: Vec<EnumPatternPayloadIR>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumPatternPayloadIR {
    pub index: u64,
    pub operational_type: TypeIR,
    pub class: crate::enum_payload::EnumPayloadClass,
    pub canonical_key: String,
    pub resolved_type_id: ResolvedTypeId,
    pub extract_intrinsic: String,
    /// Slot interno de staging. A extração pode ocorrer após o pai casar, mas
    /// o binding de fonte só é materializado quando a árvore inteira casar.
    pub extracted_binding: BindingIR,
    pub pattern: Box<EnumPatternIR>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineAsmOperandIR {
    Input {
        name: String,
        constraint: crate::inline_asm::AsmConstraint,
        value: ValueIR,
        ty: TypeIR,
    },
    Output {
        name: String,
        constraint: crate::inline_asm::AsmConstraint,
        slot: String,
        ty: TypeIR,
    },
}

/// Match de união na IR estruturada.
///
/// O scrutinee é abaixado uma única vez. Cada braço carrega a tag **copiada**
/// do `UnionTypeIR` internado — nunca derivada da posição do braço, da ordem
/// textual da união, do nome do apelido ou de um `TypeIR` isolado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionMatchIR {
    pub scrutinee: ValueIR,
    /// Slot que guarda o scrutinee já avaliado. Existe para que o valor seja
    /// avaliado **uma única vez** e permaneça legível em todos os blocos do
    /// match, cujos temporários têm escopo por bloco.
    pub scrutinee_binding: BindingIR,
    /// Slot que guarda a tag lida uma única vez do valor de união.
    pub tag_binding: BindingIR,
    pub union_type_id: UnionTypeId,
    pub arms: Vec<UnionMatchArmIR>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionMatchArmIR {
    pub tag: u64,
    pub canonical_member_key: String,
    /// Identidade semântica do membro coberto pelo braço. Obrigatória: um braço
    /// de `encaixe` existe exatamente por causa de um membro exato do registry.
    pub resolved_member_type_id: ResolvedTypeId,
    pub binding: BindingIR,
    pub payload_type: TypeIR,
    pub payload_layout: crate::union_payload::UnionPayloadLayout,
    pub body: BlockIR,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FalarArgIR {
    pub value: ValueIR,
    pub ty: TypeIR,
}

/// Expressão na IR. `Call` carrega `ret_type` explicitamente para que camadas posteriores
/// não precisem consultar a tabela de funções — o tipo está embutido no nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueIR {
    Local(String),
    GlobalConst(String),
    Int(u64),
    Bool(bool),
    String(String),
    Unary {
        op: UnaryOpIR,
        operand: Box<ValueIR>,
        ty: TypeIR,
    },
    Deref {
        ptr: Box<ValueIR>,
        result_type: TypeIR,
        is_volatile: bool,
    },
    Binary {
        op: BinaryOpIR,
        lhs: Box<ValueIR>,
        rhs: Box<ValueIR>,
        ty: TypeIR,
    },
    /// Derivação tipada de ponteiro. `offset` está em elementos; tamanho e
    /// alinhamento vêm exclusivamente de `layout::layout_of_type`.
    PointerOffset {
        pointer: Box<ValueIR>,
        offset: Box<ValueIR>,
        pointer_type: TypeIR,
        element_size: u64,
        element_align: u64,
    },
    Call {
        callee: String,
        args: Vec<ValueIR>,
        ret_type: TypeIR,
    },
    // Fase 242: referência a função top-level como valor (materializa o
    // descritor callable {code_ptr, env_ptr}; env_ptr nulo/estático aqui).
    FunctionRef(String),
    // Fase 245: endereço cru de uma função top-level. É uma palavra contendo
    // diretamente o endereço do código, sem descritor e sem `__env`.
    RawFunctionRef(String),
    // Fase 243: cria uma closure — aloca em heap (via `pinker_alocar`) um
    // ambiente com os valores de `captures` (snapshot por valor, na ordem
    // dada) e materializa o descritor callable {code_ptr, env_ptr} apontando
    // para ele. `captures` vazio equivale a `FunctionRef` (env_ptr nulo).
    MakeClosure {
        function_name: String,
        captures: Vec<ValueIR>,
    },
    // Fase 244: materialização explícita de um objeto de trato.
    //
    // `value` é o receiver concreto antes da cópia. `concrete_size` informa
    // quantos bytes formam o snapshot. `vtable_methods` preserva, em ordem de
    // declaração do trato, os símbolos dos métodos do impl correspondente.
    MakeTraitObject {
        value: Box<ValueIR>,
        trait_name: String,
        concrete_type: TypeIR,
        concrete_type_name: String,
        concrete_size: u64,
        vtable_methods: Vec<String>,
    },
    // Fase 244: chamada por slot em objeto de trato.
    //
    // `param_types` exclui o receiver contextual `si`; o receiver é o próprio
    // `object`. O retorno pode ser `Nulo`, ao contrário de `CallIndirect`.
    TraitCall {
        object: Box<ValueIR>,
        trait_name: String,
        method_name: String,
        method_slot: u64,
        method_count: u64,
        args: Vec<ValueIR>,
        param_types: Vec<TypeIR>,
        ret_type: TypeIR,
    },
    // Fase 242: chamada indireta — `callee` é um valor (variável/parâmetro
    // de tipo função), não um nome resolvido em tempo de parse. O tipo
    // função público sempre declara retorno não-nulo (semantic.rs), então
    // `ret_type` nunca é `Nulo` aqui.
    CallIndirect {
        callee: Box<ValueIR>,
        args: Vec<ValueIR>,
        ret_type: TypeIR,
    },
    // Fase 245: chamada por endereço cru. `param_types` preserva a assinatura
    // concreta para validadores e ABI; `ret_type` pode ser `Nulo`.
    CallRaw {
        callee: Box<ValueIR>,
        args: Vec<ValueIR>,
        param_types: Vec<TypeIR>,
        ret_type: TypeIR,
    },
    FieldAccess {
        base: Box<ValueIR>,
        field: String,
        field_offset: u64,
        result_type: TypeIR,
    },
    Index {
        base: Box<ValueIR>,
        index: Box<ValueIR>,
        element_type: TypeIR,
    },
    Cast {
        value: Box<ValueIR>,
        target_type: TypeIR,
    },
    /// Injeção em união já **decidida** pela identidade semântica exata.
    ///
    /// A decisão de tag acontece uma única vez, no lowering, comparando o
    /// `ResolvedTypeId` do valor de origem com o `ResolvedTypeId` do membro.
    /// Nenhuma camada posterior escolhe membro, e em particular nenhuma escolhe
    /// pela primeira ocorrência de um mesmo `TypeIR`.
    UnionInject {
        value: Box<ValueIR>,
        union_type_id: UnionTypeId,
        resolved_member_type_id: ResolvedTypeId,
        canonical_member_key: String,
        tag: u64,
        payload_type: TypeIR,
        payload_layout: crate::union_payload::UnionPayloadLayout,
    },
    // Operações internas tipadas de união (HR1/HR5). Não possuem nome textual
    // chamável, não passam pela resolução comum de função, nunca aparecem como
    // `Call` e não podem ser construídas pelo parser. Os símbolos de runtime
    // correspondentes são um detalhe do backend.
    /// Lê a tag corrente de um valor de união validado.
    UnionTag {
        value: Box<ValueIR>,
        union_type_id: UnionTypeId,
    },
    /// Extrai o payload de um membro já validado contra o registry.
    ///
    /// Os metadados de layout viajam no nó para que HR3 possa estender a
    /// extração a payloads multi-palavra sem reconstruir o match.
    UnionExtract {
        value: Box<ValueIR>,
        union_type_id: UnionTypeId,
        resolved_member_type_id: ResolvedTypeId,
        tag: u64,
        canonical_member_key: String,
        payload_type: TypeIR,
        payload_layout: crate::union_payload::UnionPayloadLayout,
    },
}

/// Tipos do sistema de tipos da v0. `Nulo` representa ausência de retorno (funções sem `-> tipo`);
/// não é exposto como tipo de usuário — apenas interno ao pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKeyIR {
    Bombom,
    Verso,
}

impl MapKeyIR {
    pub(crate) fn type_ir(self) -> TypeIR {
        match self {
            Self::Bombom => TypeIR::Bombom,
            Self::Verso => TypeIR::Verso,
        }
    }
}

pub(crate) fn is_generic_map_intrinsic(name: &str) -> bool {
    matches!(
        name,
        "__pinker_internal_mapa_criar_chave_bombom"
            | "__pinker_internal_mapa_criar_chave_verso"
            | "__pinker_internal_mapa_definir"
            | "__pinker_internal_mapa_obter"
            | "__pinker_internal_mapa_tem"
            | "__pinker_internal_mapa_tamanho"
            | "__pinker_internal_mapa_remover"
            | "__pinker_internal_mapa_iterador_criar"
            | "__pinker_internal_mapa_iterador_proxima_chave_bombom"
            | "__pinker_internal_mapa_iterador_proxima_chave_verso"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapValueIR {
    Bombom,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Logica,
    Verso,
}

impl MapValueIR {
    pub(crate) fn type_ir(self) -> TypeIR {
        match self {
            Self::Bombom => TypeIR::Bombom,
            Self::U8 => TypeIR::U8,
            Self::U16 => TypeIR::U16,
            Self::U32 => TypeIR::U32,
            Self::U64 => TypeIR::U64,
            Self::I8 => TypeIR::I8,
            Self::I16 => TypeIR::I16,
            Self::I32 => TypeIR::I32,
            Self::I64 => TypeIR::I64,
            Self::Logica => TypeIR::Logica,
            Self::Verso => TypeIR::Verso,
        }
    }

    fn from_type_ir(ty: TypeIR) -> Option<Self> {
        Some(match ty {
            TypeIR::Bombom => Self::Bombom,
            TypeIR::U8 => Self::U8,
            TypeIR::U16 => Self::U16,
            TypeIR::U32 => Self::U32,
            TypeIR::U64 => Self::U64,
            TypeIR::I8 => Self::I8,
            TypeIR::I16 => Self::I16,
            TypeIR::I32 => Self::I32,
            TypeIR::I64 => Self::I64,
            TypeIR::Logica => Self::Logica,
            TypeIR::Verso => Self::Verso,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeIR {
    Bombom,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Logica,
    Verso,
    ListBombom,
    ListVerso,
    MapVersoBombom,
    MapVersoVerso,
    MapBombomBombom,
    MapBombomVerso,
    Map {
        key: MapKeyIR,
        value: MapValueIR,
    },
    FixedArray {
        element: ScalarTypeIR,
        size: u64,
    },
    Struct,
    /// Handle opaco nominal: uma palavra na máquina; a identidade concreta
    /// permanece em `ResolvedTypeTable`, como nas demais representações
    /// fisicamente ambíguas.
    OpaqueWordHandle,
    Pointer {
        is_volatile: bool,
    },
    // Fase 245: endereço cru de código, uma palavra, distinto de Pointer de
    // dados e do handle `Function` das closures/callables.
    FunctionPointer,
    // Fase 242: callable materializado — handle de 1 palavra para descritor
    // {code_ptr, env_ptr}. Mesma categoria de valor que Pointer/ListBombom.
    Function,
    // Fase 244: handle de uma palavra para um descritor
    // `{data_ptr, vtable_ptr}` de objeto de trato.
    //
    // A identidade nominal do trato permanece nos nós `MakeTraitObject` e
    // `TraitCall`, pois `TypeIR` continua pequeno e `Copy`.
    TraitObject,
    Union(UnionTypeId),
    Nulo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnionTypeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionTypeIR {
    pub id: UnionTypeId,
    pub canonical_key: String,
    pub members: Vec<UnionMemberIR>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionMemberIR {
    pub tag: u64,
    /// Chave canônica do membro, derivada do **tipo resolvido** pelo contrato
    /// compartilhado em [`crate::union_canon`]. É a única identidade terminal
    /// de um membro: nenhuma camada a reconstrói por `TypeIR::name()`, por
    /// nome de apelido, por posição de braço ou por texto de debug.
    pub canonical_member_key: String,
    pub ty: TypeIR,
    /// Identidade semântica completa do membro, internada no programa.
    ///
    /// Substitui a antiga `nominal_identity: Option<String>`: a seleção do
    /// membro na injeção compara este campo por igualdade exata, e nunca um
    /// nome textual nem a categoria operacional `ty`.
    pub resolved_type_id: ResolvedTypeId,
    /// Layout terminal do payload: tamanho, alinhamento e categoria de
    /// representação, decididos uma única vez por
    /// [`crate::union_payload::classify_union_payload`].
    ///
    /// Os três viajam juntos porque `size`, `align` e categoria separados
    /// tornavam representável um estado inconsistente — um agregado de 24 bytes
    /// classificado como handle de uma palavra, por exemplo.
    pub payload_layout: crate::union_payload::UnionPayloadLayout,
}

/// Valida a tabela internada de uniões em qualquer fronteira do pipeline.
///
/// IDs e tags seguem a ordem canônica armazenada; não há reconstrução por
/// texto de debug nem dependência da ordem de iteração de mapas.
pub fn validate_union_registry(unions: &[UnionTypeIR]) -> Result<(), String> {
    let mut keys = std::collections::BTreeSet::new();
    for (index, union) in unions.iter().enumerate() {
        let expected_id =
            u32::try_from(index).map_err(|_| "tabela de uniões excede u32".to_string())?;
        if union.id != UnionTypeId(expected_id) {
            return Err(format!(
                "ID de união não determinístico: esperado {expected_id}, recebido {}",
                union.id.0
            ));
        }
        if union.canonical_key.is_empty() {
            return Err(format!("união {} sem chave canônica", union.id.0));
        }
        if !keys.insert(union.canonical_key.as_bytes().to_vec()) {
            return Err(format!(
                "chave canônica de união duplicada: {}",
                union.canonical_key
            ));
        }
        if union.members.len() < 2 {
            return Err(format!(
                "união {} possui menos de dois membros distintos",
                union.id.0
            ));
        }
        let mut member_identities = std::collections::BTreeSet::new();
        let mut member_keys = std::collections::BTreeSet::new();
        let mut previous_key: Option<&str> = None;
        for (member_index, member) in union.members.iter().enumerate() {
            if member.tag != member_index as u64 {
                return Err(format!(
                    "tag inválida na união {}: esperado {member_index}, recebido {}",
                    union.id.0, member.tag
                ));
            }
            if member.canonical_member_key.is_empty() {
                return Err(format!(
                    "membro sem chave canônica na união {} tag {}",
                    union.id.0, member.tag
                ));
            }
            if !member_keys.insert(member.canonical_member_key.as_bytes().to_vec()) {
                return Err(format!(
                    "chave canônica de membro duplicada na união {}: {}",
                    union.id.0, member.canonical_member_key
                ));
            }
            // A ordem canônica do registry é a ordem crescente das chaves; a
            // tag é o índice nessa ordem e em nenhuma outra.
            if let Some(previous) = previous_key {
                if previous.as_bytes() >= member.canonical_member_key.as_bytes() {
                    return Err(format!(
                        "ordem canônica violada na união {}: '{}' antes de '{}'",
                        union.id.0, previous, member.canonical_member_key
                    ));
                }
            }
            previous_key = Some(&member.canonical_member_key);
            // HR3: um único predicado cobre tamanho, alinhamento, limites e a
            // coerência entre categoria e layout. Não há mais checagem parcial
            // de `size`/`align` isolados, e não há layout presumido.
            if !member.payload_layout.is_well_formed() {
                return Err(format!(
                    "layout de payload inválido na união {} tag {}: {}/{}/{}",
                    union.id.0,
                    member.tag,
                    member.payload_layout.size,
                    member.payload_layout.align,
                    member.payload_layout.representation.name()
                ));
            }
            if matches!(member.ty, TypeIR::Union(_) | TypeIR::Nulo) {
                return Err(format!(
                    "membro não achatado ou nulo na união {} tag {}",
                    union.id.0, member.tag
                ));
            }
            // A identidade de um membro é o `ResolvedTypeId` — nunca o par
            // (categoria operacional, nome textual). Dois membros com o mesmo
            // `TypeIR` e identidades diferentes são legítimos; dois membros com
            // a mesma identidade são registry inválido.
            if !member_identities.insert(member.resolved_type_id) {
                return Err(format!(
                    "membro duplicado na união {} tag {}: identidade resolvida {} repetida",
                    union.id.0, member.tag, member.resolved_type_id.0
                ));
            }
        }
    }
    Ok(())
}

/// Confirma que um `UnionTypeId` existe na tabela internada.
pub fn validate_union_reference(
    unions: &[UnionTypeIR],
    union_type_id: UnionTypeId,
) -> Result<&UnionTypeIR, String> {
    unions
        .get(union_type_id.0 as usize)
        .filter(|union| union.id == union_type_id)
        .ok_or_else(|| format!("união {} ausente do registro internado", union_type_id.0))
}

/// Confirma que uma operação interna tipada de união corresponde exatamente ao
/// membro internado.
///
/// Verifica, em qualquer fronteira do pipeline: a união existe; a tag pertence
/// ao registry; a chave canônica coincide com a tag; o tipo do payload
/// coincide; tamanho e alinhamento coincidem. Nenhuma dessas verificações
/// reconstrói a identidade do membro por `TypeIR::name()` ou por texto de
/// debug.
pub fn validate_union_member_reference(
    unions: &[UnionTypeIR],
    union_type_id: UnionTypeId,
    tag: u64,
    canonical_member_key: &str,
    payload_type: TypeIR,
    payload_layout: crate::union_payload::UnionPayloadLayout,
) -> Result<(), String> {
    let union = validate_union_reference(unions, union_type_id)?;
    let member = union
        .members
        .get(usize::try_from(tag).map_err(|_| "tag de união excede usize".to_string())?)
        .ok_or_else(|| {
            format!(
                "tag {tag} não pertence à união {}: {} membros",
                union_type_id.0,
                union.members.len()
            )
        })?;
    if member.tag != tag {
        return Err(format!(
            "tag divergente na união {}: esperado {}, recebido {tag}",
            union_type_id.0, member.tag
        ));
    }
    if member.canonical_member_key != canonical_member_key {
        return Err(format!(
            "chave canônica divergente na união {} tag {tag}: esperado '{}', recebido '{}'",
            union_type_id.0, member.canonical_member_key, canonical_member_key
        ));
    }
    if member.ty != payload_type {
        return Err(format!(
            "tipo de payload divergente na união {} tag {tag}: esperado '{}', recebido '{}'",
            union_type_id.0,
            member.ty.name(),
            payload_type.name()
        ));
    }
    if member.payload_layout != payload_layout {
        return Err(format!(
            "layout de payload divergente na união {} tag {tag}: esperado {}/{}/{}, recebido \
             {}/{}/{}",
            union_type_id.0,
            member.payload_layout.size,
            member.payload_layout.align,
            member.payload_layout.representation.name(),
            payload_layout.size,
            payload_layout.align,
            payload_layout.representation.name()
        ));
    }
    // A defesa é repetida em vez de confiada à origem: um layout bem formado no
    // registry não impede que uma camada intermediária tenha fabricado outro.
    if !payload_layout.is_well_formed() {
        return Err(format!(
            "layout de payload mal formado na união {} tag {tag}: {}/{}/{}",
            union_type_id.0,
            payload_layout.size,
            payload_layout.align,
            payload_layout.representation.name()
        ));
    }
    Ok(())
}

/// Confirma que a identidade semântica transportada por uma operação de união é
/// exatamente a identidade do membro daquela tag no registry.
///
/// É esta verificação que torna impossível uma camada posterior "corrigir" a
/// escolha do membro: se a tag e a identidade discordarem, o pipeline para.
pub fn validate_union_member_identity(
    unions: &[UnionTypeIR],
    union_type_id: UnionTypeId,
    tag: u64,
    resolved_member_type_id: ResolvedTypeId,
) -> Result<(), String> {
    let union = validate_union_reference(unions, union_type_id)?;
    let member = union
        .members
        .get(usize::try_from(tag).map_err(|_| "tag de união excede usize".to_string())?)
        .filter(|member| member.tag == tag)
        .ok_or_else(|| {
            format!(
                "tag {tag} não pertence à união {}: {} membros",
                union_type_id.0,
                union.members.len()
            )
        })?;
    if member.resolved_type_id != resolved_member_type_id {
        return Err(format!(
            "E-IR-UNION-MEMBER-IDENTITY-MISMATCH: união {} tag {tag} tem identidade resolvida {}, \
             recebida {}",
            union_type_id.0, member.resolved_type_id.0, resolved_member_type_id.0
        ));
    }
    Ok(())
}

/// Confirma que o conjunto de braços de um match cobre integralmente a união,
/// sem braço repetido e sem referência a união diferente.
pub fn validate_union_match_coverage(
    unions: &[UnionTypeIR],
    union_type_id: UnionTypeId,
    arm_keys: &[(u64, String)],
) -> Result<(), String> {
    let union = validate_union_reference(unions, union_type_id)?;
    let mut seen = std::collections::BTreeSet::new();
    for (tag, key) in arm_keys {
        if !seen.insert(key.as_bytes().to_vec()) {
            return Err(format!(
                "braço repetido na união {}: chave '{key}'",
                union_type_id.0
            ));
        }
        let Some(member) = union.members.iter().find(|member| member.tag == *tag) else {
            return Err(format!(
                "tag {tag} não pertence à união {}",
                union_type_id.0
            ));
        };
        if member.canonical_member_key != *key {
            return Err(format!(
                "braço da união {} associa tag {tag} à chave '{key}', mas o registry guarda '{}'",
                union_type_id.0, member.canonical_member_key
            ));
        }
    }
    if seen.len() != union.members.len() {
        return Err(format!(
            "cobertura incompleta da união {}: {} de {} membros",
            union_type_id.0,
            seen.len(),
            union.members.len()
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarTypeIR {
    Bombom,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Logica,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOpIR {
    Neg,
    Not,
    BitNot,
    Deref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOpIR {
    LogicalAnd,
    LogicalOr,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}
// @pinker-nav:end ir.modelo.representacao

// @pinker-nav:start ir.tipos.identidade-resolvida
// @pinker-nav:domain modelo
// @pinker-nav:layer ir
// @pinker-nav:summary Identidade semântica resolvida de tipos: `ResolvedTypeId` interna a identidade completa (`ResolvedTypeIR` = chave canônica de `union_canon` + representação operacional + identidade nominal + componentes internos `pointee`/`element`/`signature`/`union_members`; `element` também transporta o tipo de valor de mapa genérico), `TypeRefIR` acopla representação e identidade em um único contrato transportável, `ResolvedTypeTable` interna por chave canônica em `BTreeMap` e recusa qualquer divergência de representação, identidade nominal ou estrutura interna sob a mesma chave, `into_types` entrega a tabela sem renumeração tardia, e os validadores confirmam densidade, unicidade, ausência de chave envenenada, coerência de representação, coerência nominal e componentes estruturais. `TypeIR` continua sendo apenas a categoria operacional; as duas noções nunca se substituem.
/// Identidade semântica completa de um tipo, internada no programa.
///
/// **Não** é a categoria operacional: `ninho Alfa` e `ninho Beta` compartilham
/// `TypeIR::Struct` e possuem `ResolvedTypeId` diferentes; dois `leque`
/// distintos compartilham a representação escalar e permanecem distintos aqui.
/// Apelidos transparentes (`apelido X = Alfa`, `apelido Y = X`) resolvem ao
/// mesmo `ResolvedTypeId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResolvedTypeId(pub u32);

/// Categoria nominal declarada pelo usuário, espelhando [`union_canon::NominalTypeKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NominalTypeKindIR {
    Ninho,
    Leque,
    OpaqueBuiltin,
}

impl NominalTypeKindIR {
    pub fn as_str(&self) -> &'static str {
        match self {
            NominalTypeKindIR::Ninho => "ninho",
            NominalTypeKindIR::Leque => "leque",
            NominalTypeKindIR::OpaqueBuiltin => "handle opaco builtin",
        }
    }

    fn from_canon(kind: union_canon::NominalTypeKind) -> Self {
        match kind {
            union_canon::NominalTypeKind::Ninho => NominalTypeKindIR::Ninho,
            union_canon::NominalTypeKind::Leque => NominalTypeKindIR::Leque,
            union_canon::NominalTypeKind::OpaqueBuiltin => NominalTypeKindIR::OpaqueBuiltin,
        }
    }
}

/// Assinatura resolvida de um tipo função: identidades completas dos parâmetros
/// e do retorno.
///
/// `carinho(u8) -> u8` e `carinho(u64) -> u64` compartilham
/// `TypeIR::Function` e possuem assinaturas — e portanto identidades —
/// diferentes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSignatureIR {
    pub params: Vec<ResolvedTypeId>,
    pub ret: ResolvedTypeId,
}

/// Entrada da tabela de identidades resolvidas do programa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTypeIR {
    pub id: ResolvedTypeId,
    /// Chave canônica derivada por [`union_canon::canonical_type_key`].
    pub canonical_key: String,
    /// Categoria operacional correspondente. Nunca é a identidade.
    pub representation: TypeIR,
    pub nominal_kind: Option<NominalTypeKindIR>,
    pub nominal_name: Option<String>,
    /// Identidade do apontado, para `seta<T>` (inclusive `seta<carinho(...)>`).
    /// `seta<u8>` e `seta<u64>` diferem exatamente aqui.
    pub pointee: Option<ResolvedTypeId>,
    /// Identidade do elemento, para arrays fixos e `lista<Leque>`.
    pub element: Option<ResolvedTypeId>,
    /// Assinatura completa, para tipos função.
    pub signature: Option<ResolvedSignatureIR>,
    /// Identidades dos membros, para uniões.
    pub union_members: Option<Vec<ResolvedTypeId>>,
}

/// Contrato tipado transportável: representação operacional **e** identidade
/// semântica juntas, para que nenhuma camada possa carregar uma sem a outra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeRefIR {
    pub representation: TypeIR,
    pub resolved: ResolvedTypeId,
}

impl TypeRefIR {
    pub fn new(representation: TypeIR, resolved: ResolvedTypeId) -> Self {
        Self {
            representation,
            resolved,
        }
    }
}

/// Tabela de internação de identidades resolvidas.
///
/// A internação é por chave canônica; a mesma chave sempre devolve o mesmo ID e
/// uma chave já internada com representação ou identidade nominal divergente é
/// recusada como erro interno.
#[derive(Debug, Clone, Default)]
pub struct ResolvedTypeTable {
    types: Vec<ResolvedTypeIR>,
    index: std::collections::BTreeMap<String, u32>,
}

/// Componentes internos de uma identidade resolvida, já internados.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedTypeParts {
    pub nominal: Option<(NominalTypeKindIR, String)>,
    pub pointee: Option<ResolvedTypeId>,
    pub element: Option<ResolvedTypeId>,
    pub signature: Option<ResolvedSignatureIR>,
    pub union_members: Option<Vec<ResolvedTypeId>>,
}

impl ResolvedTypeTable {
    pub fn intern(
        &mut self,
        canonical_key: String,
        representation: TypeIR,
        parts: ResolvedTypeParts,
    ) -> Result<ResolvedTypeId, String> {
        if canonical_key.is_empty() {
            return Err("identidade resolvida sem chave canônica".to_string());
        }
        let (nominal_kind, nominal_name) = match parts.nominal {
            Some((kind, name)) => (Some(kind), Some(name)),
            None => (None, None),
        };
        if let Some(existing_id) = self.index.get(&canonical_key).copied() {
            let existing = &self.types[existing_id as usize];
            if existing.representation != representation {
                return Err(format!(
                    "identidade resolvida '{canonical_key}' já internada com representação '{}', recebida '{}'",
                    existing.representation.name(),
                    representation.name()
                ));
            }
            if existing.nominal_kind != nominal_kind || existing.nominal_name != nominal_name {
                return Err(format!(
                    "identidade resolvida '{canonical_key}' já internada com identidade nominal divergente"
                ));
            }
            if existing.pointee != parts.pointee
                || existing.element != parts.element
                || existing.signature != parts.signature
                || existing.union_members != parts.union_members
            {
                return Err(format!(
                    "identidade resolvida '{canonical_key}' já internada com estrutura interna divergente"
                ));
            }
            return Ok(ResolvedTypeId(existing_id));
        }
        let id = u32::try_from(self.types.len())
            .map_err(|_| "tabela de identidades resolvidas excede u32".to_string())?;
        self.index.insert(canonical_key.clone(), id);
        self.types.push(ResolvedTypeIR {
            id: ResolvedTypeId(id),
            canonical_key,
            representation,
            nominal_kind,
            nominal_name,
            pointee: parts.pointee,
            element: parts.element,
            signature: parts.signature,
            union_members: parts.union_members,
        });
        Ok(ResolvedTypeId(id))
    }

    pub fn get(&self, id: ResolvedTypeId) -> Option<&ResolvedTypeIR> {
        self.types.get(id.0 as usize).filter(|entry| entry.id == id)
    }

    pub fn key_of(&self, id: ResolvedTypeId) -> Option<&str> {
        self.get(id).map(|entry| entry.canonical_key.as_str())
    }

    pub fn nominal_name_of(&self, id: ResolvedTypeId) -> Option<&str> {
        self.get(id).and_then(|entry| entry.nominal_name.as_deref())
    }

    pub fn id_of_key(&self, key: &str) -> Option<ResolvedTypeId> {
        self.index.get(key).copied().map(ResolvedTypeId)
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Entrega a tabela final na ordem em que as identidades foram internadas.
    ///
    /// Nenhuma renumeração posterior é feita de propósito: os `ResolvedTypeId`
    /// já gravados em bindings, valores e membros de união são definitivos desde
    /// a internação, e um remapeamento tardio que esquecesse qualquer uma dessas
    /// posições produziria justamente a associação silenciosamente errada que
    /// HR4 descreve.
    pub fn into_types(self) -> Vec<ResolvedTypeIR> {
        self.types
    }
}

/// Representação operacional exigida por uma chave canônica de tipo escalar ou
/// nominal conhecida. `None` quando a chave é estrutural e a representação já é
/// validada pelo próprio construtor.
fn expected_representation_for_key(key: &str) -> Option<TypeIR> {
    if key.starts_with("opaque:") {
        return Some(TypeIR::OpaqueWordHandle);
    }
    match key {
        "bombom" => Some(TypeIR::Bombom),
        "u8" => Some(TypeIR::U8),
        "u16" => Some(TypeIR::U16),
        "u32" => Some(TypeIR::U32),
        "u64" => Some(TypeIR::U64),
        "i8" => Some(TypeIR::I8),
        "i16" => Some(TypeIR::I16),
        "i32" => Some(TypeIR::I32),
        "i64" => Some(TypeIR::I64),
        "logica" => Some(TypeIR::Logica),
        "verso" => Some(TypeIR::Verso),
        "lista<bombom>" => Some(TypeIR::ListBombom),
        "lista<verso>" => Some(TypeIR::ListVerso),
        "mapa<verso,bombom>" => Some(TypeIR::MapVersoBombom),
        "mapa<verso,verso>" => Some(TypeIR::MapVersoVerso),
        "mapa<bombom,bombom>" => Some(TypeIR::MapBombomBombom),
        "mapa<bombom,verso>" => Some(TypeIR::MapBombomVerso),
        "nulo" => Some(TypeIR::Nulo),
        _ => None,
    }
}

/// Chave canônica determinada por uma representação operacional autossuficiente.
///
/// É a inversa exata de [`expected_representation_for_key`] e existe apenas para
/// as representações cuja categoria operacional **já é** a identidade semântica
/// completa (escalares, `verso`, listas e mapas monomórficos, `nulo`). Para
/// `Struct`, `OpaqueWordHandle`, `Pointer`, `Function`, `FunctionPointer` e
/// `TraitObject` retorna
/// `None`: nesses casos a representação é ambígua por construção (HR4) e a
/// identidade tem de vir do tipo AST resolvido.
fn expected_key_for_representation(ty: TypeIR) -> Option<&'static str> {
    let key = match ty {
        TypeIR::Bombom => "bombom",
        TypeIR::U8 => "u8",
        TypeIR::U16 => "u16",
        TypeIR::U32 => "u32",
        TypeIR::U64 => "u64",
        TypeIR::I8 => "i8",
        TypeIR::I16 => "i16",
        TypeIR::I32 => "i32",
        TypeIR::I64 => "i64",
        TypeIR::Logica => "logica",
        TypeIR::Verso => "verso",
        TypeIR::ListBombom => "lista<bombom>",
        TypeIR::ListVerso => "lista<verso>",
        TypeIR::MapVersoBombom => "mapa<verso,bombom>",
        TypeIR::MapVersoVerso => "mapa<verso,verso>",
        TypeIR::MapBombomBombom => "mapa<bombom,bombom>",
        TypeIR::MapBombomVerso => "mapa<bombom,verso>",
        TypeIR::Nulo => "nulo",
        TypeIR::Struct
        | TypeIR::OpaqueWordHandle
        | TypeIR::Map { .. }
        | TypeIR::Pointer { .. }
        | TypeIR::FunctionPointer
        | TypeIR::Function
        | TypeIR::TraitObject
        | TypeIR::FixedArray { .. }
        | TypeIR::Union(_) => return None,
    };
    debug_assert_eq!(expected_representation_for_key(key), Some(ty));
    Some(key)
}

/// Interna a identidade de uma representação autossuficiente, sem contexto de
/// lowering.
///
/// Serve ao catálogo de intrínsecas embutidas, cujos retornos são escalares,
/// `verso`, listas/mapas monomórficos ou `nulo`. Representações ambíguas
/// (`Struct`, `Pointer`, `Function`, `FunctionPointer`, `TraitObject`, arrays e
/// uniões) são recusadas de propósito: elas exigem a identidade do tipo AST.
fn intern_representation_identity(
    table: &mut ResolvedTypeTable,
    ty: TypeIR,
) -> Result<ResolvedTypeId, String> {
    let key = expected_key_for_representation(ty).ok_or_else(|| {
        format!(
            "E-IR-TYPE-IDENTITY-LOST: a representação '{}' não determina a identidade semântica",
            ty.name()
        )
    })?;
    table.intern(key.to_string(), ty, ResolvedTypeParts::default())
}

/// Assinatura de intrínseca embutida com identidade de retorno já internada.
fn builtin_sig(
    table: &mut ResolvedTypeTable,
    ret_type: TypeIR,
) -> Result<FunctionSigIR, PinkerError> {
    let ret_resolved =
        intern_representation_identity(table, ret_type).map_err(|msg| PinkerError::Ir {
            msg,
            span: Span::new(Position::new(1, 1), Position::new(1, 1)),
        })?;
    Ok(FunctionSigIR {
        ret_type,
        ret_resolved,
    })
}

/// Assinatura builtin cuja representação não determina sozinha a identidade.
fn builtin_nominal_sig(
    table: &mut ResolvedTypeTable,
    ty: Type,
) -> Result<FunctionSigIR, PinkerError> {
    let ret_type = TypeIR::from_ast_with_context(&ty, &HashMap::new(), &HashSet::new())?;
    let ret_resolved = table
        .intern(
            union_canon::canonical_type_key(&ty),
            ret_type,
            ResolvedTypeParts {
                nominal: union_canon::nominal_identity_of(&ty)
                    .map(|(kind, name)| (NominalTypeKindIR::from_canon(kind), name)),
                ..ResolvedTypeParts::default()
            },
        )
        .map_err(|msg| PinkerError::Ir {
            msg,
            span: ty.span(),
        })?;
    Ok(FunctionSigIR {
        ret_type,
        ret_resolved,
    })
}

/// Valida a tabela de identidades resolvidas em qualquer fronteira do pipeline.
///
/// Confirma: IDs densos e na posição; chaves não vazias e únicas; nenhuma chave
/// envenenada por perda de resolução de apelido; coerência entre chave e
/// representação operacional; e coerência entre identidade nominal declarada,
/// prefixo da chave e representação.
///
/// A **ordem** da tabela é a ordem de internação do lowering, que é função
/// apenas da ordem sintática do programa (a tabela indexa por `BTreeMap`, nunca
/// por `HashMap`). A independência de ordem de iteração de mapas é verificada
/// por igualdade entre dois lowerings do mesmo programa, e não por uma ordenação
/// posterior — reordenar a tabela exigiria reescrever todo `ResolvedTypeId` já
/// gravado em bindings, valores e membros, o que reintroduziria exatamente a
/// classe de erro silencioso de HR4.
pub fn validate_resolved_type_table(resolved: &[ResolvedTypeIR]) -> Result<(), String> {
    let mut seen_keys = std::collections::BTreeSet::<&str>::new();
    for (index, entry) in resolved.iter().enumerate() {
        let expected_id =
            u32::try_from(index).map_err(|_| "tabela de identidades excede u32".to_string())?;
        if entry.id != ResolvedTypeId(expected_id) {
            return Err(format!(
                "ID de identidade resolvida fora da posição: esperado {expected_id}, recebido {}",
                entry.id.0
            ));
        }
        if entry.canonical_key.is_empty() {
            return Err(format!("identidade {expected_id} sem chave canônica"));
        }
        if union_canon::is_poisoned_key(&entry.canonical_key) {
            return Err(format!(
                "identidade {expected_id} carrega chave de identidade perdida: '{}'",
                entry.canonical_key
            ));
        }
        if !seen_keys.insert(entry.canonical_key.as_str()) {
            return Err(format!(
                "chave canônica repetida na tabela de identidades: '{}'",
                entry.canonical_key
            ));
        }
        if let Some(expected) = expected_representation_for_key(&entry.canonical_key) {
            if entry.representation != expected {
                return Err(format!(
                    "representação divergente para a identidade '{}': esperado '{}', recebido '{}'",
                    entry.canonical_key,
                    expected.name(),
                    entry.representation.name()
                ));
            }
        }
        match (entry.nominal_kind, entry.nominal_name.as_deref()) {
            (Some(NominalTypeKindIR::Ninho), Some(name)) => {
                if entry.representation != TypeIR::Struct {
                    return Err(format!(
                        "identidade nominal de ninho '{name}' com representação '{}'",
                        entry.representation.name()
                    ));
                }
                if entry.canonical_key != format!("struct:{}:{name}", name.len()) {
                    return Err(format!(
                        "chave canônica '{}' não corresponde ao ninho '{name}'",
                        entry.canonical_key
                    ));
                }
            }
            (Some(NominalTypeKindIR::Leque), Some(name)) => {
                if entry.representation != TypeIR::Bombom {
                    return Err(format!(
                        "identidade nominal de leque '{name}' com representação '{}'",
                        entry.representation.name()
                    ));
                }
                if entry.canonical_key != format!("enum:{}:{name}", name.len()) {
                    return Err(format!(
                        "chave canônica '{}' não corresponde ao leque '{name}'",
                        entry.canonical_key
                    ));
                }
            }
            (Some(NominalTypeKindIR::OpaqueBuiltin), Some(name)) => {
                if entry.representation != TypeIR::OpaqueWordHandle {
                    return Err(format!(
                        "identidade nominal de handle opaco builtin '{name}' com representação '{}'",
                        entry.representation.name()
                    ));
                }
                if entry.canonical_key != format!("opaque:{}:{name}", name.len()) {
                    return Err(format!(
                        "chave canônica '{}' não corresponde ao handle opaco builtin '{name}'",
                        entry.canonical_key
                    ));
                }
            }
            (None, None) => {
                if entry.canonical_key.starts_with("struct:")
                    || entry.canonical_key.starts_with("enum:")
                    || entry.canonical_key.starts_with("opaque:")
                {
                    return Err(format!(
                        "identidade nominal ausente para a chave nominal '{}'",
                        entry.canonical_key
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "identidade {expected_id} com categoria e nome nominais inconsistentes"
                ));
            }
        }
        validate_resolved_type_structure(resolved, entry)?;
    }
    Ok(())
}

/// Confirma que os componentes internos de uma identidade existem na tabela e
/// que a chave canônica é exatamente a composição das chaves dos componentes.
///
/// É esta checagem que impede que `seta<u8>` e `seta<u64>`, ou
/// `carinho(u8) -> u8` e `carinho(u64) -> u64`, compartilhem identidade.
fn validate_resolved_type_structure(
    resolved: &[ResolvedTypeIR],
    entry: &ResolvedTypeIR,
) -> Result<(), String> {
    let key_of = |id: ResolvedTypeId| -> Result<&str, String> {
        resolved
            .get(id.0 as usize)
            .filter(|component| component.id == id)
            .map(|component| component.canonical_key.as_str())
            .ok_or_else(|| {
                format!(
                    "identidade '{}' referencia componente {} ausente da tabela",
                    entry.canonical_key, id.0
                )
            })
    };
    let expect = |expected: String| -> Result<(), String> {
        if expected == entry.canonical_key {
            Ok(())
        } else {
            Err(format!(
                "chave canônica incoerente com os componentes: esperado '{expected}', armazenado '{}'",
                entry.canonical_key
            ))
        }
    };
    match entry.representation {
        TypeIR::Pointer { is_volatile } => {
            let Some(pointee) = entry.pointee else {
                return Err(format!(
                    "identidade de ponteiro '{}' sem identidade do apontado",
                    entry.canonical_key
                ));
            };
            expect(format!(
                "ptr:{}:{}",
                u8::from(is_volatile),
                key_of(pointee)?
            ))
        }
        TypeIR::FunctionPointer => {
            let Some(pointee) = entry.pointee else {
                return Err(format!(
                    "identidade de ponteiro cru de função '{}' sem assinatura do apontado",
                    entry.canonical_key
                ));
            };
            expect(format!("ptr:0:{}", key_of(pointee)?))
        }
        TypeIR::Function => {
            let Some(signature) = entry.signature.as_ref() else {
                return Err(format!(
                    "identidade de função '{}' sem assinatura resolvida",
                    entry.canonical_key
                ));
            };
            let mut params = Vec::with_capacity(signature.params.len());
            for param in &signature.params {
                let key = key_of(*param)?;
                params.push(format!("{}:{key}", key.len()));
            }
            let ret = key_of(signature.ret)?;
            expect(format!("fn({})->{}:{ret}", params.join(","), ret.len()))
        }
        TypeIR::FixedArray { size, .. } => {
            let Some(element) = entry.element else {
                return Err(format!(
                    "identidade de array '{}' sem identidade do elemento",
                    entry.canonical_key
                ));
            };
            let element = key_of(element)?;
            expect(format!("array:{size}:{}:{element}", element.len()))
        }
        // D1: `lista<Leque>` compartilha a representação de `lista<bombom>` e
        // carrega, além dela, a identidade concreta do elemento. É por este
        // componente que `lista<Cor>` e `lista<Token>` deixam de poder colapsar
        // numa identidade única.
        TypeIR::ListBombom | TypeIR::ListVerso => {
            if entry.pointee.is_some() || entry.signature.is_some() || entry.union_members.is_some()
            {
                return Err(format!(
                    "identidade de lista '{}' carrega componentes incompatíveis",
                    entry.canonical_key
                ));
            }
            let Some(element) = entry.element else {
                // Lista monomórfica: a representação já é a identidade completa.
                return expect(
                    match entry.representation {
                        TypeIR::ListVerso => "lista<verso>",
                        _ => "lista<bombom>",
                    }
                    .to_string(),
                );
            };
            if entry.representation != TypeIR::ListBombom {
                return Err(format!(
                    "identidade de lista '{}' com elemento nominal exige representação 'lista<bombom>'",
                    entry.canonical_key
                ));
            }
            let element_entry = resolved
                .get(element.0 as usize)
                .filter(|component| component.id == element)
                .ok_or_else(|| {
                    format!(
                        "identidade '{}' referencia componente {} ausente da tabela",
                        entry.canonical_key, element.0
                    )
                })?;
            let (Some(NominalTypeKindIR::Leque), Some(name)) = (
                element_entry.nominal_kind,
                element_entry.nominal_name.as_ref(),
            ) else {
                return Err(format!(
                    "identidade de lista '{}' referencia elemento sem identidade nominal de leque",
                    entry.canonical_key
                ));
            };
            expect(format!("lista<leque>:{}:{name}", name.len()))
        }
        TypeIR::Map { key, .. } => {
            if entry.pointee.is_some() || entry.signature.is_some() || entry.union_members.is_some()
            {
                return Err(format!(
                    "identidade de mapa '{}' carrega componentes incompatíveis",
                    entry.canonical_key
                ));
            }
            let Some(value) = entry.element else {
                return Err(format!(
                    "identidade de mapa '{}' sem identidade do valor",
                    entry.canonical_key
                ));
            };
            let key = key.type_ir().name();
            let value = key_of(value)?;
            expect(format!("mapa<{key},{value}>"))
        }
        TypeIR::Union(_) => {
            let Some(members) = entry.union_members.as_ref() else {
                return Err(format!(
                    "identidade de união '{}' sem identidades dos membros",
                    entry.canonical_key
                ));
            };
            let mut keys = Vec::with_capacity(members.len());
            for member in members {
                let key = key_of(*member)?;
                keys.push(format!("{}:{key}", key.len()));
            }
            expect(format!("union:[{}]", keys.join(",")))
        }
        _ => {
            if entry.pointee.is_some()
                || entry.signature.is_some()
                || entry.union_members.is_some()
                || entry.element.is_some()
            {
                return Err(format!(
                    "identidade '{}' carrega componentes incompatíveis com a representação '{}'",
                    entry.canonical_key,
                    entry.representation.name()
                ));
            }
            Ok(())
        }
    }
}

/// Confirma que um `ResolvedTypeId` existe na tabela e que a representação
/// declarada no nó coincide com a identidade internada.
pub fn validate_resolved_type_reference(
    resolved: &[ResolvedTypeIR],
    id: ResolvedTypeId,
    representation: TypeIR,
) -> Result<&ResolvedTypeIR, String> {
    let entry = resolved
        .get(id.0 as usize)
        .filter(|entry| entry.id == id)
        .ok_or_else(|| format!("identidade resolvida {} ausente da tabela internada", id.0))?;
    if entry.representation != representation {
        return Err(format!(
            "representação divergente para a identidade '{}': tabela guarda '{}', nó declara '{}'",
            entry.canonical_key,
            entry.representation.name(),
            representation.name()
        ));
    }
    Ok(entry)
}

/// Confronta a tabela de uniões com a tabela de identidades resolvidas.
///
/// Cada membro precisa apontar para uma identidade existente cuja chave
/// canônica seja exatamente a chave do membro e cuja representação coincida.
/// Duas identidades diferentes com o mesmo `TypeIR` continuam sendo membros
/// distintos; duas entradas com a mesma identidade são erro de registry.
pub fn validate_union_registry_identities(
    unions: &[UnionTypeIR],
    resolved: &[ResolvedTypeIR],
) -> Result<(), String> {
    for union in unions {
        let mut seen = std::collections::BTreeSet::new();
        for member in &union.members {
            let entry =
                validate_resolved_type_reference(resolved, member.resolved_type_id, member.ty)
                    .map_err(|error| format!("união {} tag {}: {error}", union.id.0, member.tag))?;
            if entry.canonical_key != member.canonical_member_key {
                return Err(format!(
                    "união {} tag {}: chave do membro '{}' não coincide com a identidade '{}'",
                    union.id.0, member.tag, member.canonical_member_key, entry.canonical_key
                ));
            }
            if !seen.insert(member.resolved_type_id) {
                return Err(format!(
                    "união {} possui dois membros com a identidade resolvida {}",
                    union.id.0, member.resolved_type_id.0
                ));
            }
        }
    }
    Ok(())
}
// @pinker-nav:end ir.tipos.identidade-resolvida

#[derive(Clone)]
struct FunctionSigIR {
    ret_type: TypeIR,
    /// Identidade semântica completa do retorno. Substitui o antigo
    /// `ret_struct_name: Option<String>`: o nome nominal, quando existir, é
    /// consultado na tabela de identidades e nunca é autoridade de seleção.
    ret_resolved: ResolvedTypeId,
}

// Fase 244: assinatura operacional de um método de trato objetificável.
// `param_types` não inclui o receiver contextual `si`.
#[derive(Clone)]
struct TraitMethodMetaIR {
    name: String,
    param_types: Vec<TypeIR>,
    ret_type: TypeIR,
    /// Tipo AST do retorno declarado, preservado para que a identidade
    /// semântica exata seja internada no ponto de uso — a resolução de apelidos
    /// e a internação de uniões exigem o contexto completo do lowering.
    ret_ast: Option<Type>,
    ret_trait_name: Option<String>,
}

#[derive(Clone)]
struct TraitMetaIR {
    methods: Vec<TraitMethodMetaIR>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CallableMetadata {
    ret_type: TypeIR,
    /// Tipo AST do retorno do callable, preservado para internar a identidade
    /// semântica exata no ponto de uso. Sem ele a chamada indireta devolveria
    /// apenas a categoria operacional, e um `leque` devolvido por um callable
    /// voltaria a colidir com qualquer outro escalar na injeção.
    ret_ast: Option<Type>,
    ret_trait_name: Option<String>,
    ret_pointer_pointee: Option<TypeIR>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RawFunctionMetadata {
    param_types: Vec<TypeIR>,
    ret_type: TypeIR,
    /// Tipo AST do retorno, pela mesma razão de [`CallableMetadata::ret_ast`]:
    /// a chamada por ponteiro cru precisa devolver a identidade semântica, não
    /// apenas a categoria operacional do retorno.
    ret_ast: Option<Type>,
    ret_pointer_pointee: Option<TypeIR>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CaptureMetadata {
    source_name: String,
    ty: TypeIR,
    /// Identidade semântica da variável capturada, preservada através da
    /// fronteira da closure. Mesma convenção de [`TypedValueIR::resolved`].
    resolved: Option<ResolvedTypeId>,
    trait_object_name: Option<String>,
    callable: Option<CallableMetadata>,
    raw_function: Option<RawFunctionMetadata>,
    pointer_pointee: Option<TypeIR>,
}

#[derive(Clone)]
struct BindingState {
    slot: String,
    ty: TypeIR,
    /// Mesma convenção de [`TypedValueIR::resolved`].
    resolved: Option<ResolvedTypeId>,
    ptr_array_bombom_size: Option<u64>,
}

// `LoweringContext` é construído em uma primeira passagem sobre o programa:
// coleta todas as assinaturas de funções e constantes antes de baixar qualquer corpo.
// Isso permite chamadas para-frente sem ordem de declaração obrigatória.
struct LoweringContext {
    module_name: String,
    function_sigs: HashMap<String, FunctionSigIR>,
    global_consts: HashMap<String, TypeIR>,
    type_aliases: HashMap<String, Type>,
    struct_decls: HashMap<String, StructDecl>,
    struct_names: HashSet<String>,
    struct_fields: HashMap<String, HashMap<String, TypeIR>>,
    struct_field_offsets: HashMap<String, HashMap<String, u64>>,
    enum_variants: HashMap<String, EnumInfoIR>,
    // Nomes **declarados** de `leque`. `enum_variants` também é indexado por
    // apelidos (para que `X.Variante` funcione), então não serve como autoridade
    // de identidade nominal: só o nome declarado é.
    enum_decl_names: HashSet<String>,
    // Fase 244: método e slot seguem a ordem declarada no `trato`.
    traits: HashMap<String, TraitMetaIR>,
    // Nome da função -> identidade nominal do objeto de trato retornado.
    function_ret_trait_names: HashMap<String, String>,
    // Fase 242: nome de função -> tipo de retorno DA FUNÇÃO REFERENCIADA
    // COMO VALOR CALLABLE, quando a própria função retorna um valor
    // callable (`carinho(...) -> carinho(...) -> T`). Usado só para
    // resolver o `ret_type` de uma chamada indireta cujo callee vem de um
    // `nova x = alguma_funcao(...)` sem anotação explícita; um nível de
    // encadeamento (callable retornando callable retornando callable não é
    // rastreado — limite honesto desta fase).
    callable_metadata: HashMap<String, CallableMetadata>,
    // Fase 245: função que retorna `seta<carinho(...) -> R>` -> assinatura
    // concreta do endereço cru retornado.
    raw_function_return_metadata: HashMap<String, RawFunctionMetadata>,
    // Fase 246: função que retorna `seta<T>` -> tipo concreto de `T`.
    // `TypeIR::Pointer` preserva a ABI de uma palavra, enquanto este catálogo
    // conserva a largura necessária para dereferências após chamadas.
    function_ret_pointer_pointees: HashMap<String, TypeIR>,
    // Fase 243: FunctionDecl de toda função do programa (inclusive
    // closures sintéticas `__anon_carinho_*`), para permitir a resolução
    // lazy de closures no ponto de criação (`FunctionLowerer::resolve_closure`)
    // abaixar o corpo da closure sob demanda, com o ambiente correto.
    all_functions: HashMap<String, FunctionDecl>,
    union_registry: std::cell::RefCell<UnionRegistryState>,
    // Tabela de internação de identidades semânticas resolvidas. É a única
    // autoridade de identidade do programa: todo binding, valor, assinatura e
    // membro de união referencia uma entrada desta tabela por `ResolvedTypeId`.
    // `RefCell` pelo mesmo motivo de `union_registry`: os lowerings emprestam
    // `context` imutavelmente e a internação é incremental.
    resolved_types: std::cell::RefCell<ResolvedTypeTable>,
    // Estado mutável compartilhado entre todos os `FunctionLowerer` da
    // mesma `lower_program`: capturas já resolvidas e corpos de closure já
    // abaixados. `RefCell` porque `FunctionLowerer` só empresta `context`
    // imutavelmente (mesmo padrão de `LoweringContext` imutável entre
    // lowerings independentes, só o registro de closures precisa mutar).
    closure_state: std::cell::RefCell<ClosureLoweringState>,
}

#[derive(Default)]
struct ClosureLoweringState {
    captures: HashMap<String, Vec<CaptureMetadata>>,
    // Vec (não HashMap) para preservar ordem determinística de resolução
    // (DFS na ordem de criação) na lista final de funções do programa.
    lowered: Vec<(String, FunctionIR)>,
    // Fase 243: nome do wrapper `__fnref_env_<nome>` -> tipo de retorno da
    // função original — permite que a inferência de `callable_ret_type`
    // (Fase 242, caso sem anotação explícita) continue funcionando quando
    // `ValueIR::FunctionRef` passa a apontar para o wrapper em vez do nome
    // original (`function_sigs` não conhece o wrapper).
    wrapper_metadata: HashMap<String, CallableMetadata>,
}

#[derive(Default)]
struct UnionRegistryState {
    types: Vec<UnionTypeIR>,
}

// Leques na IR: sem carga, o valor é o próprio discriminante imediato; com
// carga, o valor é um handle opaco (bombom) para o estado do runtime.
#[derive(Clone)]
struct EnumInfoIR {
    /// Nome **declarado** do leque. `enum_variants` também é indexado pelos
    /// apelidos que apontam para ele; este campo é a chave de deduplicação para
    /// publicar a metadata uma única vez por leque.
    declared_name: String,
    has_payload: bool,
    variants: HashMap<String, (u64, Vec<EnumPayloadTypeIR>)>,
}

/// Descrição de carga de variante durante o lowering.
///
/// Substitui o antigo `Vec<TypeIR>`: guardar apenas a categoria operacional
/// tornava `lista<bombom>`, `lista<Cor>` e `lista<Token>` indistinguíveis, e a
/// perda de identidade só apareceria na construção ou na extração — tarde
/// demais para produzir um diagnóstico fiel.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EnumPayloadTypeIR {
    /// Categoria operacional do valor da carga.
    operational_type: TypeIR,
    /// Classe de representação e tipo resolvido, decididos pela autoridade
    /// única em [`crate::enum_payload`].
    shape: crate::enum_payload::EnumPayloadShape,
}

impl EnumPayloadTypeIR {
    fn classify(
        declared: &Type,
        payload_aliases: &HashMap<String, Type>,
        enum_names: &HashSet<String>,
        struct_names: &HashSet<String>,
        type_aliases: &HashMap<String, Type>,
    ) -> Result<Self, PinkerError> {
        let shape = crate::enum_payload::classify_enum_payload(
            declared,
            payload_aliases,
            enum_names,
            struct_names,
        )
        .map_err(|rejection| PinkerError::Ir {
            msg: format!(
                "carga de variante sem classificação na IR: {}",
                rejection.message()
            ),
            span: declared.span(),
        })?;
        let operational_type =
            TypeIR::from_ast_with_context(&shape.resolved, type_aliases, struct_names)?;
        Ok(Self {
            operational_type,
            shape,
        })
    }
}

/// Metadata publicada de uma variante de leque.
///
/// Viaja no [`ProgramIR`] para que os validadores e os testes estruturais
/// possam conferir, sem reconstruir nada, que cada carga conserva ao mesmo
/// tempo a representação operacional e a identidade semântica resolvida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantMetaIR {
    pub enum_name: String,
    pub variant_name: String,
    pub discriminant: u64,
    pub payloads: Vec<EnumPayloadMetaIR>,
}

/// Carga de variante na metadata publicada: as duas dimensões, acopladas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumPayloadMetaIR {
    /// Categoria operacional. Nunca é a identidade.
    pub operational_type: TypeIR,
    /// Classe de representação que escolhe o helper de runtime.
    pub class: crate::enum_payload::EnumPayloadClass,
    /// Chave canônica da identidade semântica resolvida.
    pub canonical_key: String,
    /// Identidade semântica internada na tabela do programa.
    pub resolved_type_id: ResolvedTypeId,
    /// Identidade concreta do elemento, quando a carga é uma `lista<E>`.
    pub element_type_id: Option<ResolvedTypeId>,
}

fn is_generic_list_create_expr(expr: &Expr) -> bool {
    if let ExprKind::Call(callee, args) = &expr.kind {
        if let ExprKind::Ident(name) = &callee.kind {
            return name == "lista_criar" && args.is_empty();
        }
    }
    false
}

fn is_generic_map_create_expr(expr: &Expr) -> bool {
    if let ExprKind::Call(callee, args) = &expr.kind {
        if let ExprKind::Ident(name) = &callee.kind {
            return name == "mapa_criar" && args.is_empty();
        }
    }
    false
}

fn generic_map_monomorphic_callee(map_ty: TypeIR, name: &str) -> Option<&'static str> {
    match (map_ty, name) {
        (TypeIR::MapVersoBombom, "mapa_definir") => Some("mapa_verso_bombom_definir"),
        (TypeIR::MapVersoBombom, "mapa_obter") => Some("mapa_verso_bombom_obter"),
        (TypeIR::MapVersoBombom, "mapa_tem") => Some("mapa_verso_bombom_tem"),
        (TypeIR::MapVersoBombom, "mapa_tamanho") => Some("mapa_verso_bombom_tamanho"),
        (TypeIR::MapVersoBombom, "mapa_remover") => Some("mapa_verso_bombom_remover"),
        (TypeIR::MapVersoVerso, "mapa_definir") => Some("mapa_verso_verso_definir"),
        (TypeIR::MapVersoVerso, "mapa_obter") => Some("mapa_verso_verso_obter"),
        (TypeIR::MapVersoVerso, "mapa_tem") => Some("mapa_verso_verso_tem"),
        (TypeIR::MapVersoVerso, "mapa_tamanho") => Some("mapa_verso_verso_tamanho"),
        (TypeIR::MapVersoVerso, "mapa_remover") => Some("mapa_verso_verso_remover"),
        (TypeIR::MapBombomBombom, "mapa_definir") => Some("mapa_bombom_bombom_definir"),
        (TypeIR::MapBombomBombom, "mapa_obter") => Some("mapa_bombom_bombom_obter"),
        (TypeIR::MapBombomBombom, "mapa_tem") => Some("mapa_bombom_bombom_tem"),
        (TypeIR::MapBombomBombom, "mapa_tamanho") => Some("mapa_bombom_bombom_tamanho"),
        (TypeIR::MapBombomBombom, "mapa_remover") => Some("mapa_bombom_bombom_remover"),
        (TypeIR::MapBombomVerso, "mapa_definir") => Some("mapa_bombom_verso_definir"),
        (TypeIR::MapBombomVerso, "mapa_obter") => Some("mapa_bombom_verso_obter"),
        (TypeIR::MapBombomVerso, "mapa_tem") => Some("mapa_bombom_verso_tem"),
        (TypeIR::MapBombomVerso, "mapa_tamanho") => Some("mapa_bombom_verso_tamanho"),
        (TypeIR::MapBombomVerso, "mapa_remover") => Some("mapa_bombom_verso_remover"),
        _ => None,
    }
}

fn parse_impl_function_name(name: &str) -> Option<(String, String, String)> {
    let rest = name.strip_prefix("__impl_")?;
    let (trait_len, rest) = rest.split_once('_')?;
    let trait_len: usize = trait_len.parse().ok()?;
    if rest.len() < trait_len + 1 {
        return None;
    }
    let trait_name = rest[..trait_len].to_string();
    let rest = rest.get(trait_len + 1..)?;
    let (target_len, rest) = rest.split_once('_')?;
    let target_len: usize = target_len.parse().ok()?;
    if rest.len() < target_len + 1 {
        return None;
    }
    let target_type = rest[..target_len].to_string();
    let method_name = rest.get(target_len + 1..)?.to_string();
    if method_name.is_empty() {
        return None;
    }
    Some((trait_name, target_type, method_name))
}

fn trait_object_name_from_type(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    struct_names: &HashSet<String>,
) -> Result<Option<String>, PinkerError> {
    if TypeIR::from_ast_with_context(ty, aliases, struct_names)? != TypeIR::TraitObject {
        return Ok(None);
    }

    let mut resolved = ty;
    let mut resolving = HashSet::new();
    while let Type::Alias { name, span } = resolved {
        if !resolving.insert(name) {
            return Err(PinkerError::Ir {
                msg: format!("alias de tipo recursivo detectado em '{}'", name),
                span: *span,
            });
        }
        resolved = aliases.get(name).ok_or_else(|| PinkerError::Ir {
            msg: format!("tipo '{}' não existe", name),
            span: *span,
        })?;
    }

    match resolved {
        Type::Applied { name, args, .. } if name == "trato" => match args.as_slice() {
            [Type::Alias { name, .. }] => Ok(Some(name.clone())),
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

fn raw_function_metadata_from_type(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    struct_names: &HashSet<String>,
) -> Result<Option<RawFunctionMetadata>, PinkerError> {
    let mut resolved = ty;
    let mut resolving = HashSet::new();
    while let Type::Alias { name, span } = resolved {
        if !resolving.insert(name) {
            return Err(PinkerError::Ir {
                msg: format!("alias de tipo recursivo detectado em '{}'", name),
                span: *span,
            });
        }
        let Some(target) = aliases.get(name) else {
            return Ok(None);
        };
        resolved = target;
    }
    let Type::Pointer { base, .. } = resolved else {
        return Ok(None);
    };
    let Type::Function { params, ret, .. } = base.as_ref() else {
        return Ok(None);
    };
    Ok(Some(RawFunctionMetadata {
        param_types: params
            .iter()
            .map(|param| TypeIR::from_ast_with_context(param, aliases, struct_names))
            .collect::<Result<Vec<_>, _>>()?,
        ret_type: TypeIR::from_ast_with_context(ret, aliases, struct_names)?,
        ret_ast: Some(ret.as_ref().clone()),
        ret_pointer_pointee: pointer_pointee_from_type(ret, aliases, struct_names)?,
    }))
}

fn pointer_pointee_from_type(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    struct_names: &HashSet<String>,
) -> Result<Option<TypeIR>, PinkerError> {
    let mut resolved = ty;
    let mut resolving = HashSet::new();
    while let Type::Alias { name, span } = resolved {
        if !resolving.insert(name) {
            return Err(PinkerError::Ir {
                msg: format!("alias de tipo recursivo detectado em '{}'", name),
                span: *span,
            });
        }
        let Some(target) = aliases.get(name) else {
            return Ok(None);
        };
        resolved = target;
    }
    let Type::Pointer { base, .. } = resolved else {
        return Ok(None);
    };
    if matches!(base.as_ref(), Type::Function { .. }) {
        return Ok(None);
    }
    Ok(Some(TypeIR::from_ast_with_context(
        base,
        aliases,
        struct_names,
    )?))
}

fn raw_function_metadata_from_decl(
    function: &FunctionDecl,
    aliases: &HashMap<String, Type>,
    struct_names: &HashSet<String>,
) -> Result<RawFunctionMetadata, PinkerError> {
    Ok(RawFunctionMetadata {
        param_types: function
            .params
            .iter()
            .map(|param| TypeIR::from_ast_with_context(&param.ty, aliases, struct_names))
            .collect::<Result<Vec<_>, _>>()?,
        ret_type: TypeIR::from_ast_option_with_context(
            function.ret_type.as_ref(),
            aliases,
            struct_names,
        )?,
        ret_ast: function.ret_type.clone(),
        ret_pointer_pointee: function
            .ret_type
            .as_ref()
            .map(|ty| pointer_pointee_from_type(ty, aliases, struct_names))
            .transpose()?
            .flatten(),
    })
}

// `FunctionLowerer` mantém estado mutable por função durante o lowering:
// - `scopes`: pilha de escopos léxicos (topo = escopo atual).
// - `slot_counters`: contador por nome-fonte para gerar slots únicos (`%nome#N`).
// - `locals` acumula todas as variáveis locais declaradas (sem os params).
struct FunctionLowerer<'a> {
    context: &'a LoweringContext,
    scopes: Vec<HashMap<String, BindingState>>,
    params: Vec<BindingIR>,
    locals: Vec<LocalIR>,
    slot_counters: HashMap<String, usize>,
    block_counter: usize,
    loop_exit_stack: Vec<String>,
    loop_continue_stack: Vec<String>,
    // Fase 242: slot de binding callable -> tipo de retorno da chamada
    // indireta através dele. Só populado quando estaticamente derivável (ver
    // `LoweringContext.callable_metadata`); ausência = erro claro no
    // lowering da chamada, não pânico.
    callable_metadata: HashMap<String, CallableMetadata>,
    // Fase 245: slot de parâmetro/local `seta<carinho(...) -> R>` ->
    // assinatura concreta usada por `CallRaw`.
    raw_function_metadata: HashMap<String, RawFunctionMetadata>,
    // Fase 246: preserva o elemento de `seta<T>` para acessos de memória;
    // `TypeIR::Pointer` continua sendo a representação ABI de uma palavra.
    pointer_pointee_types: HashMap<String, TypeIR>,
    // Slot local/parâmetro -> nome nominal de `trato<Nome>`.
    trait_object_names: HashMap<String, String>,
}

struct TypedValueIR {
    value: ValueIR,
    ty: TypeIR,
    /// Identidade semântica do valor.
    ///
    /// `None` **não** significa "sem identidade": significa que a
    /// representação operacional já é a identidade completa e é internada sob
    /// demanda por [`TypedValueIR::identity`]. Isso só é verdade para as
    /// representações injetivas (escalares, `verso`, listas/mapas monomórficos,
    /// `nulo`, arrays desses, e uniões, cujo `UnionTypeId` já é nominal). Para
    /// `ninho`, `seta<T>`, `carinho(...)` e `trato<...>` — exatamente as
    /// representações que HR4 mostra serem ambíguas — `None` é perda de
    /// identidade e `identity` falha com `E-IR-TYPE-IDENTITY-LOST`, em vez de
    /// escolher um candidato aproximado.
    resolved: Option<ResolvedTypeId>,
    ptr_array_bombom_size: Option<u64>,
}

impl TypedValueIR {
    /// Identidade semântica exata do valor, ou erro interno se ela foi perdida.
    fn identity(
        &self,
        context: &LoweringContext,
        span: Span,
    ) -> Result<ResolvedTypeId, PinkerError> {
        match self.resolved {
            Some(resolved) => Ok(resolved),
            None => context.repr_identity(self.ty, span),
        }
    }
}

// Fase 2 escolhe IR estruturada: blocos e `if` seguem explícitos, sem SSA e sem saltos.
// Isso mantém o lowering pequeno e auditável sem quebrar o frontend estabilizado.
// @pinker-nav:start ir.lowering.programa-orquestracao
// @pinker-nav:domain lowering
// @pinker-nav:layer ir
// @pinker-nav:summary Ponto de entrada do lowering AST → IR: constrói o `LoweringContext` global, percorre os itens do programa, despacha constantes (`lower_const`) e funções (`FunctionLowerer`) e monta o `ProgramIR` (nome do módulo, modo freestanding). Aliases/structs/leques/tratos são ignorados aqui (já viraram fatos do contexto); não reexecuta análise semântica.
pub fn lower_program(program: &Program) -> Result<ProgramIR, PinkerError> {
    let context = LoweringContext::from_program(program)?;
    let mut consts = Vec::new();
    let mut functions = Vec::new();

    for item in &program.items {
        match item {
            Item::Const(const_decl) => consts.push(lower_const(const_decl, &context)?),
            // Fase 243: closures (`__anon_carinho_*`) são abaixadas lazily
            // no ponto de criação (`FunctionLowerer::resolve_closure`), com
            // o ambiente correto — não aqui, isoladas.
            Item::Function(function_decl) if function_decl.name.starts_with("__anon_carinho_") => {}
            Item::Function(function_decl) => {
                functions.push(FunctionLowerer::new(&context).lower_function(function_decl)?)
            }
            Item::TypeAlias(_) => {}
            Item::Struct(_) => {}
            Item::Enum(_) => {}
            Item::Trait(_) => {}
        }
    }

    // Fase 243: closure sintética nunca resolvida como valor (idioma de
    // chamada imediata `carinho(...) {...}(x)`, Fase 225) nunca passa por
    // `resolve_closure` — permanece função comum, sem `__env`, igual ao
    // comportamento anterior à Fase 243. Só closures genuinamente usadas
    // como valor recebem a convenção uniforme de ambiente.
    for item in &program.items {
        if let Item::Function(function_decl) = item {
            if function_decl.name.starts_with("__anon_carinho_") {
                let already = context
                    .closure_state
                    .borrow()
                    .captures
                    .contains_key(&function_decl.name);
                if !already {
                    let lowered = FunctionLowerer::new(&context).lower_function(function_decl)?;
                    let mut state = context.closure_state.borrow_mut();
                    state.lowered.push((function_decl.name.clone(), lowered));
                }
            }
        }
    }

    // A metadata das variantes é selada antes de a tabela de identidades ser
    // entregue: cada carga interna aqui a identidade do próprio tipo resolvido
    // e, quando é lista, a identidade concreta do elemento.
    let enum_variants_meta = context.seal_enum_variant_metadata()?;

    let ClosureLoweringState { lowered, .. } = context.closure_state.into_inner();
    functions.extend(lowered.into_iter().map(|(_, f)| f));
    let union_types = context.union_registry.into_inner().types;
    let resolved_types = context.resolved_types.into_inner().into_types();
    // A tabela de identidades e o registro de uniões são conferidos já aqui, no
    // ponto em que ambos ficam completos: qualquer incoerência é erro interno do
    // lowering e não deve chegar às camadas seguintes.
    let program_span = Span::new(Position::new(1, 1), Position::new(1, 1));
    validate_resolved_type_table(&resolved_types).map_err(|msg| PinkerError::Ir {
        msg: format!("E-IR-TYPE-IDENTITY-LOST: {msg}"),
        span: program_span,
    })?;
    validate_union_registry_identities(&union_types, &resolved_types).map_err(|msg| {
        PinkerError::Ir {
            msg,
            span: program_span,
        }
    })?;

    Ok(ProgramIR {
        module_name: context.module_name,
        is_freestanding: program.freestanding.is_some(),
        resolved_types,
        union_types,
        enum_variants: enum_variants_meta,
        consts,
        functions,
    })
}
// @pinker-nav:end ir.lowering.programa-orquestracao

pub fn render_program(program: &ProgramIR) -> String {
    let mut out = String::new();
    line(&mut out, 0, &format!("module {}", program.module_name));
    line(
        &mut out,
        0,
        &format!(
            "mode {}",
            if program.is_freestanding {
                "livre"
            } else {
                "hospedado"
            }
        ),
    );

    line(&mut out, 0, "consts:");
    if program.consts.is_empty() {
        line(&mut out, 1, "[]");
    } else {
        for const_ir in &program.consts {
            line(
                &mut out,
                1,
                &format!(
                    "const @{}: {} = {}",
                    const_ir.name,
                    const_ir.ty.render_name(),
                    render_value(&const_ir.value)
                ),
            );
        }
    }

    line(&mut out, 0, "functions:");
    for function in &program.functions {
        render_function(function, 1, &mut out);
    }

    out
}

impl LoweringContext {
    // @pinker-nav:start ir.lowering.contexto-declaracoes
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Primeira metade de `from_program`: coleta os fatos globais que todos os corpos consomem — nome do módulo, aliases de tipo (com leques registrados como alias para `bombom`), structs e seus campos/offsets de layout, variantes de leque com índices e cargas, e as assinaturas das funções e tipos das constantes declaradas no programa. Prepara o contexto; não reexecuta a checagem semântica.
    fn from_program(program: &Program) -> Result<Self, PinkerError> {
        let module_name = program
            .package
            .as_ref()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| "main".to_string());

        // Tabela de identidades semânticas do programa. Nasce aqui porque as
        // assinaturas das intrínsecas embutidas já precisam internar a
        // identidade do próprio retorno.
        let mut resolved_types = ResolvedTypeTable::default();
        let mut type_aliases = HashMap::new();
        let mut struct_decls = HashMap::new();
        let mut struct_names = HashSet::new();
        let mut enum_variants: HashMap<String, EnumInfoIR> = HashMap::new();
        let mut enum_decl_names: HashSet<String> = HashSet::new();
        // Tabelas exclusivas da classificação de cargas (D1). Não podem ser
        // `type_aliases`: ali um leque já foi reescrito para `bombom`, e usar
        // aquela tabela apagaria justamente a identidade nominal que a carga
        // precisa preservar.
        let mut payload_aliases: HashMap<String, Type> = HashMap::new();
        for item in &program.items {
            if let Item::TypeAlias(alias) = item {
                type_aliases.insert(alias.name.clone(), alias.target.clone());
                payload_aliases.insert(alias.name.clone(), alias.target.clone());
            } else if let Item::Struct(struct_decl) = item {
                struct_names.insert(struct_decl.name.clone());
                struct_decls.insert(struct_decl.name.clone(), struct_decl.clone());
            } else if let Item::Enum(enum_decl) = item {
                // O tipo leque abaixa para bombom na IR (discriminante imediato
                // ou handle); registrar como alias faz toda anotação de tipo
                // com o nome do leque resolver sozinha.
                type_aliases.insert(enum_decl.name.clone(), Type::Bombom(enum_decl.span));
                enum_decl_names.insert(enum_decl.name.clone());
            }
        }
        // As cargas são classificadas depois da coleta completa: um leque pode
        // referenciar outro declarado adiante, inclusive a si mesmo através de
        // `lista<si>`.
        for item in &program.items {
            let Item::Enum(enum_decl) = item else {
                continue;
            };
            let mut variants = HashMap::new();
            for (index, variant) in enum_decl.variants.iter().enumerate() {
                let mut payloads = Vec::with_capacity(variant.payloads.len());
                for payload in &variant.payloads {
                    payloads.push(EnumPayloadTypeIR::classify(
                        payload,
                        &payload_aliases,
                        &enum_decl_names,
                        &struct_names,
                        &type_aliases,
                    )?);
                }
                variants.insert(variant.name.clone(), (index as u64, payloads));
            }
            enum_variants.insert(
                enum_decl.name.clone(),
                EnumInfoIR {
                    declared_name: enum_decl.name.clone(),
                    has_payload: enum_decl
                        .variants
                        .iter()
                        .any(|variant| !variant.payloads.is_empty()),
                    variants,
                },
            );
        }
        // Apelidos de leque herdam a metadata do alvo. A propagação roda até o
        // ponto fixo porque a cadeia pode ter mais de um elo
        // (`apelido B = A; apelido A = Leque;`), e o `declared_name` de cada
        // entrada continua sendo o do leque real: o apelido não cria
        // identidade nominal nova.
        loop {
            let mut mudou = false;
            for (alias_name, target) in type_aliases.clone() {
                if enum_variants.contains_key(&alias_name) {
                    continue;
                }
                let (Type::Enum { name, .. } | Type::Alias { name, .. }) = target else {
                    continue;
                };
                if let Some(info) = enum_variants.get(&name).cloned() {
                    enum_variants.insert(alias_name, info);
                    mudou = true;
                }
            }
            if !mudou {
                break;
            }
        }
        let mut struct_fields = HashMap::new();
        let mut struct_field_offsets = HashMap::new();
        for item in &program.items {
            if let Item::Struct(struct_decl) = item {
                let mut fields = HashMap::new();
                for field in &struct_decl.fields {
                    let resolved =
                        TypeIR::from_ast_with_context(&field.ty, &type_aliases, &struct_names)?;
                    fields.insert(field.name.clone(), resolved);
                }
                struct_fields.insert(struct_decl.name.clone(), fields);
                let offsets =
                    layout::struct_field_offsets(&struct_decl.name, &type_aliases, &struct_decls)
                        .map_err(|msg| PinkerError::Ir {
                        msg: format!("layout de struct inválido na IR: {}", msg),
                        span: struct_decl.span,
                    })?;
                struct_field_offsets.insert(struct_decl.name.clone(), offsets);
            }
        }

        let mut traits = HashMap::new();

        for item in &program.items {
            let Item::Trait(trait_decl) = item else {
                continue;
            };

            let methods = trait_decl
                .methods
                .iter()
                .map(|method| {
                    let param_types = method
                        .params
                        .iter()
                        .skip(1)
                        .map(|param| {
                            TypeIR::from_ast_with_context(&param.ty, &type_aliases, &struct_names)
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    let ret_type = TypeIR::from_ast_option_with_context(
                        method.ret_type.as_ref(),
                        &type_aliases,
                        &struct_names,
                    )?;

                    let ret_trait_name = method
                        .ret_type
                        .as_ref()
                        .map(|ty| trait_object_name_from_type(ty, &type_aliases, &struct_names))
                        .transpose()?
                        .flatten();

                    Ok::<_, PinkerError>(TraitMethodMetaIR {
                        name: method.name.clone(),
                        param_types,
                        ret_type,
                        ret_ast: method.ret_type.clone(),
                        ret_trait_name,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            traits.insert(trait_decl.name.clone(), TraitMetaIR { methods });
        }

        let mut function_sigs = HashMap::new();
        // (nome, tipo AST do retorno, representação) das funções declaradas.
        let mut pending_declared_sigs: Vec<(String, Option<Type>, TypeIR)> = Vec::new();
        let mut global_consts = HashMap::new();
        let mut callable_metadata = HashMap::new();
        let mut raw_function_return_metadata = HashMap::new();
        let mut function_ret_pointer_pointees = HashMap::new();
        let mut function_ret_trait_names = HashMap::new();
        let mut all_functions = HashMap::new();

        for item in &program.items {
            match item {
                Item::Function(function) => {
                    all_functions.insert(function.name.clone(), function.clone());

                    if let Some(ret_type) = function.ret_type.as_ref() {
                        if let Some(trait_name) =
                            trait_object_name_from_type(ret_type, &type_aliases, &struct_names)?
                        {
                            function_ret_trait_names.insert(function.name.clone(), trait_name);
                        }
                    }

                    // A identidade semântica do retorno de uma função declarada
                    // pode exigir resolução integral de apelidos e internação de
                    // uniões, o que só é possível com o contexto já montado. A
                    // assinatura é registrada aqui apenas com a representação e
                    // é selada em `seal_declared_signature_identities`, antes de
                    // qualquer corpo ser abaixado.
                    pending_declared_sigs.push((
                        function.name.clone(),
                        function.ret_type.clone(),
                        TypeIR::from_ast_option_with_context(
                            function.ret_type.as_ref(),
                            &type_aliases,
                            &struct_names,
                        )?,
                    ));
                    // Fase 242: quando a função retorna um valor callable,
                    // registra o ret_type DESSE callable (um nível), para
                    // permitir chamada indireta imediata sobre o resultado.
                    if let Some(Type::Function { ret, .. }) = function.ret_type.as_ref() {
                        callable_metadata.insert(
                            function.name.clone(),
                            CallableMetadata {
                                ret_type: TypeIR::from_ast_with_context(
                                    ret,
                                    &type_aliases,
                                    &struct_names,
                                )?,
                                ret_ast: Some(ret.as_ref().clone()),
                                ret_trait_name: trait_object_name_from_type(
                                    ret,
                                    &type_aliases,
                                    &struct_names,
                                )?,
                                ret_pointer_pointee: pointer_pointee_from_type(
                                    ret,
                                    &type_aliases,
                                    &struct_names,
                                )?,
                            },
                        );
                    }
                    if let Some(ret_type) = function.ret_type.as_ref() {
                        if let Some(metadata) =
                            raw_function_metadata_from_type(ret_type, &type_aliases, &struct_names)?
                        {
                            raw_function_return_metadata.insert(function.name.clone(), metadata);
                        }
                        if let Some(pointee) =
                            pointer_pointee_from_type(ret_type, &type_aliases, &struct_names)?
                        {
                            function_ret_pointer_pointees.insert(function.name.clone(), pointee);
                        }
                    }
                }
                Item::Const(const_decl) => {
                    global_consts.insert(
                        const_decl.name.clone(),
                        TypeIR::from_ast_with_context(
                            &const_decl.ty,
                            &type_aliases,
                            &struct_names,
                        )?,
                    );
                }
                Item::TypeAlias(_) | Item::Struct(_) | Item::Enum(_) | Item::Trait(_) => {}
            }
        }
        // @pinker-nav:end ir.lowering.contexto-declaracoes

        // @pinker-nav:start ir.lowering.assinaturas-intrinsecos
        // @pinker-nav:domain lowering
        // @pinker-nav:layer ir
        // @pinker-nav:summary Segunda metade de `from_program`: catálogo centralizado de assinaturas das intrínsecas embutidas e internas (E/S, texto/verso, listas, mapas, CSV/JSON, tempo, ambiente, acaso, arquivo, caminho, processo) — cada `function_sigs.insert` registra o tipo de retorno usado depois para tipar chamadas no lowering de expressões. Encerra montando o `LoweringContext`. Não valida os corpos das intrínsecas; apenas declara contratos de retorno.
        function_sigs.insert(
            "ouvir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "ouvir_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "ouvir_verso_ou".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "aleatorio_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "aleatorio_proximo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "lista_bombom_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::ListBombom)?,
        );
        function_sigs.insert(
            "lista_bombom_anexar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "lista_bombom_obter".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "lista_bombom_tamanho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "lista_bombom_definir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "lista_bombom_tirar_ultimo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "lista_verso_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::ListVerso)?,
        );
        function_sigs.insert(
            "lista_verso_anexar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "lista_verso_obter".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "lista_verso_tamanho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "lista_verso_definir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "lista_verso_tirar_ultimo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "mapa_verso_bombom_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::MapVersoBombom)?,
        );
        function_sigs.insert(
            "mapa_verso_bombom_definir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "mapa_verso_bombom_obter".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_verso_bombom_tem".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "mapa_verso_bombom_tamanho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_verso_verso_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::MapVersoVerso)?,
        );
        function_sigs.insert(
            "mapa_verso_verso_definir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "mapa_verso_verso_obter".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "mapa_verso_verso_tem".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "mapa_verso_verso_tamanho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_verso_verso_remover".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "mapa_bombom_bombom_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::MapBombomBombom)?,
        );
        function_sigs.insert(
            "mapa_bombom_bombom_definir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "mapa_bombom_bombom_obter".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_bombom_bombom_tem".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "mapa_bombom_bombom_tamanho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_bombom_bombom_remover".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_bombom_verso_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::MapBombomVerso)?,
        );
        function_sigs.insert(
            "mapa_bombom_verso_definir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "mapa_bombom_verso_obter".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "mapa_bombom_verso_tem".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "mapa_bombom_verso_tamanho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_bombom_verso_remover".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_leque_criar_0".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_leque_anexar_b".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_leque_anexar_v".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_leque_tag".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_leque_carga_b".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "__pinker_internal_leque_carga_v".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // D1: cargas de lista reutilizam integralmente o caminho de uma palavra.
        // Os quatro nomes internos existem porque a assinatura de retorno é por
        // símbolo; todos colapsam no mesmo par de símbolos do runtime nativo.
        function_sigs.insert(
            crate::enum_payload::ANEXAR_LISTA_BOMBOM.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            crate::enum_payload::ANEXAR_LISTA_VERSO.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            crate::enum_payload::CARGA_LISTA_BOMBOM.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::ListBombom)?,
        );
        function_sigs.insert(
            crate::enum_payload::CARGA_LISTA_VERSO.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::ListVerso)?,
        );
        function_sigs.insert(
            crate::enum_payload::ANEXAR_SAIDA_PROCESSO.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            crate::enum_payload::CARGA_SAIDA_PROCESSO.to_string(),
            builtin_nominal_sig(
                &mut resolved_types,
                crate::falha_operacional::CargaResultado::SaidaProcesso
                    .tipo(crate::falha_operacional::span_sintetico()),
            )?,
        );
        // As uniões não registram intrínsecas chamáveis: tag e extração são
        // `ValueIR::UnionTag`/`ValueIR::UnionExtract`, nós tipados criados pelo
        // lowering de `Stmt::UnionMatch`.
        function_sigs.insert(
            "argumento".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "argumento_ou".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "tem_chave".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "tem_argumento_nomeado".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "pedir_argumento".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "argumento_nomeado_ou".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "tem_flag".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "ambiente_ou".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "buscar_contexto".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "argumento_nomeado_ou_ambiente_ou".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "caminho_existe".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "e_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "e_diretorio".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "juntar_caminho".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "tamanho_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "e_vazio".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "criar_diretorio".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "remover_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "remover_diretorio".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "diretorio_atual".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "quantos_argumentos".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "tem_argumento".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "sair".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "abrir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "ler_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "ler_verso_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "ler_arquivo_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // Parte B: leque com carga é handle de uma palavra na IR.
        for nome in crate::falha_operacional::nomes() {
            function_sigs.insert(
                nome.to_string(),
                builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
            );
        }
        // Parte E1: assinaturas derivadas da autoridade única de `valor_json`.
        //
        // Quem devolve `ValorJson` precisa de assinatura NOMINAL: a
        // representação `handle opaco` não determina identidade semântica
        // sozinha, e derivá-la da representação apagaria a diferença entre esta
        // família e qualquer outra que também seja uma palavra.
        for nome in crate::valor_json::ACESSORES {
            let (retorno, _) = crate::valor_json::assinatura_ir(nome)
                .expect("acessor JSON sem assinatura na autoridade");
            let sig = if matches!(retorno, TypeIR::OpaqueWordHandle) {
                builtin_nominal_sig(
                    &mut resolved_types,
                    Type::OpaqueHandle {
                        name: crate::valor_json::TIPO_VALOR_JSON.to_string(),
                        span: Span::new(Position::new(1, 1), Position::new(1, 1)),
                    },
                )?
            } else {
                builtin_sig(&mut resolved_types, retorno)?
            };
            function_sigs.insert(nome.to_string(), sig);
        }
        function_sigs.insert(
            crate::saida_processo::ACESSOR_CODIGO.to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        for nome in [
            crate::saida_processo::ACESSOR_SAIDA,
            crate::saida_processo::ACESSOR_ERRO,
        ] {
            function_sigs.insert(
                nome.to_string(),
                builtin_sig(&mut resolved_types, TypeIR::Verso)?,
            );
        }
        function_sigs.insert(
            "arquivo_ou".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "fechar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "criar_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "abrir_anexo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "escrever".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "escrever_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "truncar_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "anexar_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "juntar_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "tamanho_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "indice_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "fatiar_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "contem_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "comeca_com".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "termina_com".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "igual_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "vazio_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        function_sigs.insert(
            "aparar_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "minusculo_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "maiusculo_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "indice_verso_em".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        // Fase 140
        function_sigs.insert(
            "buscar_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "nao_vazio_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Logica)?,
        );
        // Fase 137
        function_sigs.insert(
            "dividir_verso_em".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "dividir_verso_contar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        // Fase 138
        function_sigs.insert(
            "substituir_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // Fase 139
        function_sigs.insert(
            "juntar_verso_com".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "formatar_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // Fase 158
        function_sigs.insert(
            "ler_linha_csv_bombom".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::ListBombom)?,
        );
        function_sigs.insert(
            "emitir_linha_csv_bombom".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "ler_json_plano_bombom".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::MapVersoBombom)?,
        );
        function_sigs.insert(
            "emitir_json_plano_bombom".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // Fase 160
        function_sigs.insert(
            "tempo_unix".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "formatar_tempo_unix".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // Fase 161
        function_sigs.insert(
            "executar_processo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        // Fase 165
        function_sigs.insert(
            "executar_com_entrada".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        // Fase 166
        function_sigs.insert(
            "pipeline_minimo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        // Fase 163
        function_sigs.insert(
            "capturar_stdout".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        // Fase 164
        function_sigs.insert(
            "capturar_stderr".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "afirmar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "dormir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "copiar_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "renomear_arquivo".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "verso_para_bombom".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "bombom_para_verso".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Verso)?,
        );
        function_sigs.insert(
            "aleatorio_entre".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Bombom)?,
        );
        function_sigs.insert(
            "mapa_verso_bombom_remover".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "lista_bombom_inserir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert(
            "lista_verso_inserir".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );
        function_sigs.insert("alocar".to_string(), {
            // `alocar` devolve `seta<u8>`: a identidade do apontado é
            // explícita, porque `TypeIR::Pointer` não a determina.
            let pointee =
                intern_representation_identity(&mut resolved_types, TypeIR::U8).map_err(|msg| {
                    PinkerError::Ir {
                        msg,
                        span: Span::new(Position::new(1, 1), Position::new(1, 1)),
                    }
                })?;
            let ret_type = TypeIR::Pointer { is_volatile: false };
            let ret_resolved = resolved_types
                .intern(
                    "ptr:0:u8".to_string(),
                    ret_type,
                    ResolvedTypeParts {
                        pointee: Some(pointee),
                        ..ResolvedTypeParts::default()
                    },
                )
                .map_err(|msg| PinkerError::Ir {
                    msg,
                    span: Span::new(Position::new(1, 1), Position::new(1, 1)),
                })?;
            FunctionSigIR {
                ret_type,
                ret_resolved,
            }
        });
        function_sigs.insert(
            "liberar".to_string(),
            builtin_sig(&mut resolved_types, TypeIR::Nulo)?,
        );

        let mut context = Self {
            module_name,
            function_sigs,
            global_consts,
            type_aliases,
            struct_decls,
            struct_names,
            struct_fields,
            struct_field_offsets,
            enum_variants,
            enum_decl_names,
            traits,
            function_ret_trait_names,
            callable_metadata,
            raw_function_return_metadata,
            function_ret_pointer_pointees,
            all_functions,
            union_registry: std::cell::RefCell::new(UnionRegistryState::default()),
            resolved_types: std::cell::RefCell::new(resolved_types),
            closure_state: std::cell::RefCell::new(ClosureLoweringState::default()),
        };
        context.seal_declared_signature_identities(pending_declared_sigs)?;
        Ok(context)
    }
    // @pinker-nav:end ir.lowering.assinaturas-intrinsecos

    fn resolve_type(&self, ty: &Type) -> Result<TypeIR, PinkerError> {
        let resolved = self.resolve_union_ast_type(ty, &mut Vec::new())?;
        if let Type::Union { members, span } = resolved {
            return self.intern_union(&members, span);
        }
        TypeIR::from_ast_with_context(ty, &self.type_aliases, &self.struct_names)
    }

    // @pinker-nav:start ir.lowering.identidade-resolvida
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Internação da identidade semântica no lowering: `resolved_identity` resolve apelidos em profundidade e interna a identidade completa do tipo AST; `intern_resolved_ast` interna primeiro componentes de containers, ponteiros, arrays, assinaturas e uniões; `repr_identity` cobre somente categorias cuja identidade é derivável da representação e recusa nominais com `E-IR-TYPE-IDENTITY-LOST`; `internal_identity` reserva identidades sintéticas. Nenhuma função deriva identidade nominal de `TypeIR::name()`, span ou ordem de mapa.
    /// Interna a identidade semântica completa de um tipo escrito na fonte.
    ///
    /// Apelidos são resolvidos integralmente antes da chave: `apelido X = Alfa`
    /// e `Alfa` produzem o mesmo `ResolvedTypeId`.
    fn resolved_identity(&self, ty: &Type) -> Result<ResolvedTypeId, PinkerError> {
        let resolved = self.resolve_union_ast_type(ty, &mut Vec::new())?;
        self.intern_resolved_ast(&resolved, ty.span())
    }

    /// Interna a identidade de um tipo **já resolvido**, recursivamente.
    fn intern_resolved_ast(
        &self,
        resolved: &Type,
        span: Span,
    ) -> Result<ResolvedTypeId, PinkerError> {
        let key = union_canon::canonical_type_key(resolved);
        if union_canon::is_poisoned_key(&key) {
            return Err(PinkerError::Ir {
                msg: format!(
                    "E-IR-TYPE-IDENTITY-LOST: identidade semântica perdida antes da internação ('{key}')"
                ),
                span,
            });
        }
        if let Some(existing) = self.resolved_types.borrow().id_of_key(&key) {
            return Ok(existing);
        }

        // Componentes primeiro: a internação do agregado nunca acontece com o
        // `RefCell` da tabela emprestado, para que a recursão seja segura.
        let mut parts = ResolvedTypeParts {
            nominal: union_canon::nominal_identity_of(resolved)
                .map(|(kind, name)| (NominalTypeKindIR::from_canon(kind), name)),
            ..ResolvedTypeParts::default()
        };
        match resolved {
            Type::Pointer { base, .. } => {
                parts.pointee = Some(self.intern_resolved_ast(base, span)?);
            }
            Type::Function { params, ret, .. } => {
                let mut param_ids = Vec::with_capacity(params.len());
                for param in params {
                    param_ids.push(self.intern_resolved_ast(param, span)?);
                }
                parts.signature = Some(ResolvedSignatureIR {
                    params: param_ids,
                    ret: self.intern_resolved_ast(ret, span)?,
                });
            }
            Type::FixedArray { element, .. } => {
                parts.element = Some(self.intern_resolved_ast(element, span)?);
            }
            // `lista<Leque>` carrega a identidade do elemento: sem isto, duas
            // listas de leques diferentes ficariam distintas apenas pela chave
            // canônica e idênticas em estrutura interna.
            Type::ListEnum { element, .. } => {
                parts.element = Some(self.intern_resolved_ast(
                    &Type::Enum {
                        name: element.clone(),
                        span,
                    },
                    span,
                )?);
            }
            // Mapas genéricos podem transportar leques sob a representação
            // operacional `bombom`; a identidade do valor precisa sobreviver
            // para usos diretos como scrutinee de `encaixe`.
            Type::Map { value, .. } => {
                parts.element = Some(self.intern_resolved_ast(value, span)?);
            }
            Type::Union { members, .. } => {
                let mut member_ids = Vec::with_capacity(members.len());
                for member in members {
                    member_ids.push(self.intern_resolved_ast(member, span)?);
                }
                parts.union_members = Some(member_ids);
            }
            _ => {}
        }

        let representation = match resolved {
            Type::Union { members, span } => self.intern_union(members, *span)?,
            other => TypeIR::from_ast_with_context(other, &self.type_aliases, &self.struct_names)?,
        };

        self.resolved_types
            .borrow_mut()
            .intern(key, representation, parts)
            .map_err(|msg| PinkerError::Ir { msg, span })
    }

    /// Sela a metadata publicada das variantes de `leque`.
    ///
    /// Roda depois de todo o lowering, quando a tabela de identidades já contém
    /// tudo o que os corpos internaram, e antes de a tabela ser entregue. Cada
    /// carga interna aqui a identidade do próprio tipo resolvido e, quando é
    /// `lista<E>`, a identidade concreta do elemento — as duas dimensões que a
    /// representação operacional sozinha não determina.
    ///
    /// A iteração é feita sobre os nomes **declarados**: `enum_variants` também
    /// é indexado pelos apelidos que apontam para cada leque, e um apelido não
    /// cria uma identidade nominal nova.
    fn seal_enum_variant_metadata(&self) -> Result<Vec<EnumVariantMetaIR>, PinkerError> {
        let mut declared: Vec<&EnumInfoIR> = Vec::new();
        for (name, info) in &self.enum_variants {
            if *name == info.declared_name {
                declared.push(info);
            }
        }
        declared.sort_by(|a, b| a.declared_name.cmp(&b.declared_name));

        let mut meta = Vec::new();
        for info in declared {
            let mut variants: Vec<(&String, &(u64, Vec<EnumPayloadTypeIR>))> =
                info.variants.iter().collect();
            variants.sort_by_key(|(_, (discriminant, _))| *discriminant);
            for (variant_name, (discriminant, payloads)) in variants {
                let mut payload_meta = Vec::with_capacity(payloads.len());
                for payload in payloads {
                    let span = payload.shape.resolved.span();
                    let resolved_type_id =
                        self.intern_resolved_ast(&payload.shape.resolved, span)?;
                    // Listas monomórficas (`bombom`/`verso`) já têm a
                    // identidade completa na própria chave/representação. Só
                    // `lista<Leque>` precisa publicar separadamente a
                    // identidade nominal do elemento, exatamente como a
                    // entrada internada acima.
                    let element_type_id = match &payload.shape.resolved {
                        Type::ListEnum { element, .. } => Some(self.intern_resolved_ast(
                            &Type::Enum {
                                name: element.clone(),
                                span,
                            },
                            span,
                        )?),
                        _ => None,
                    };
                    payload_meta.push(EnumPayloadMetaIR {
                        operational_type: payload.operational_type,
                        class: payload.shape.class,
                        canonical_key: payload.shape.canonical_key(),
                        resolved_type_id,
                        element_type_id,
                    });
                }
                meta.push(EnumVariantMetaIR {
                    enum_name: info.declared_name.clone(),
                    variant_name: variant_name.clone(),
                    discriminant: *discriminant,
                    payloads: payload_meta,
                });
            }
        }
        Ok(meta)
    }

    /// Identidade de um valor cuja categoria operacional **é** a identidade
    /// completa.
    ///
    /// Vale para escalares, `verso`, listas e mapas monomórficos, arrays de
    /// escalar, `nulo` e uniões já internadas. As categorias nominais ou
    /// paramétricas (`ninho`, `leque` — que também abaixa para escalar —,
    /// `seta<T>`, `carinho(...)`, `trato<...>`) **não** podem ser derivadas da
    /// representação e produzem `E-IR-TYPE-IDENTITY-LOST`.
    fn repr_identity(&self, ty: TypeIR, span: Span) -> Result<ResolvedTypeId, PinkerError> {
        let lost = || {
            PinkerError::Ir {
            msg: format!(
                "E-IR-TYPE-IDENTITY-LOST: a representação '{}' não determina a identidade semântica",
                ty.name()
            ),
            span,
        }
        };
        let (key, parts) = match ty {
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
            | TypeIR::Verso
            | TypeIR::ListBombom
            | TypeIR::ListVerso
            | TypeIR::MapVersoBombom
            | TypeIR::MapVersoVerso
            | TypeIR::MapBombomBombom
            | TypeIR::MapBombomVerso
            | TypeIR::Nulo => (
                expected_key_for_representation(ty)
                    .ok_or_else(lost)?
                    .to_string(),
                ResolvedTypeParts::default(),
            ),
            TypeIR::FixedArray { element, size } => {
                let element_id = self.repr_identity(element.to_type_ir(), span)?;
                let element_key = {
                    let table = self.resolved_types.borrow();
                    table.key_of(element_id).ok_or_else(lost)?.to_string()
                };
                (
                    format!("array:{size}:{}:{element_key}", element_key.len()),
                    ResolvedTypeParts {
                        element: Some(element_id),
                        ..ResolvedTypeParts::default()
                    },
                )
            }
            TypeIR::Union(union_type_id) => {
                let member_keys = {
                    let registry = self.union_registry.borrow();
                    let union = registry
                        .types
                        .get(union_type_id.0 as usize)
                        .filter(|union| union.id == union_type_id)
                        .ok_or_else(lost)?;
                    union
                        .members
                        .iter()
                        .map(|member| {
                            (member.canonical_member_key.clone(), member.resolved_type_id)
                        })
                        .collect::<Vec<_>>()
                };
                let key = format!(
                    "union:[{}]",
                    member_keys
                        .iter()
                        .map(|(key, _)| format!("{}:{key}", key.len()))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                (
                    key,
                    ResolvedTypeParts {
                        union_members: Some(
                            member_keys.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
                        ),
                        ..ResolvedTypeParts::default()
                    },
                )
            }
            TypeIR::Struct
            | TypeIR::OpaqueWordHandle
            | TypeIR::Map { .. }
            | TypeIR::Pointer { .. }
            | TypeIR::FunctionPointer
            | TypeIR::Function
            | TypeIR::TraitObject => return Err(lost()),
        };
        self.resolved_types
            .borrow_mut()
            .intern(key, ty, parts)
            .map_err(|msg| PinkerError::Ir { msg, span })
    }

    /// Interna uma identidade sintética do próprio lowering (por exemplo o
    /// ambiente oculto `__env` das closures), com chave reservada que nunca
    /// coincide com um tipo escrito pelo usuário.
    fn internal_identity(
        &self,
        tag: &str,
        ty: TypeIR,
        span: Span,
    ) -> Result<ResolvedTypeId, PinkerError> {
        // A chave reservada não é envenenada: é uma identidade legítima e
        // distinta, apenas inalcançável pela sintaxe de tipos do usuário.
        let mut parts = ResolvedTypeParts::default();
        let key = match ty {
            TypeIR::Pointer { is_volatile } => {
                let opaque = self
                    .resolved_types
                    .borrow_mut()
                    .intern(
                        format!("interno<{tag}>"),
                        TypeIR::Bombom,
                        ResolvedTypeParts::default(),
                    )
                    .map_err(|msg| PinkerError::Ir { msg, span })?;
                parts.pointee = Some(opaque);
                format!("ptr:{}:interno<{tag}>", u8::from(is_volatile))
            }
            _ => format!("interno<{tag}>"),
        };
        self.resolved_types
            .borrow_mut()
            .intern(key, ty, parts)
            .map_err(|msg| PinkerError::Ir { msg, span })
    }

    /// Sela a identidade semântica do retorno de cada função declarada.
    ///
    /// Roda depois de o contexto estar montado e antes de qualquer corpo ser
    /// abaixado, porque a identidade de um retorno pode exigir resolução
    /// integral de apelidos e internação de uniões — ambas dependentes do
    /// contexto completo. Nomes já ocupados pelo catálogo de intrínsecas
    /// embutidas continuam pertencendo às embutidas, exatamente como antes.
    fn seal_declared_signature_identities(
        &mut self,
        pending: Vec<(String, Option<Type>, TypeIR)>,
    ) -> Result<(), PinkerError> {
        let mut sealed = Vec::with_capacity(pending.len());
        for (name, ast_ret, ret_type) in pending {
            let span = ast_ret
                .as_ref()
                .map(|ty| ty.span())
                .unwrap_or_else(|| Span::new(Position::new(1, 1), Position::new(1, 1)));
            let ret_resolved = match ast_ret.as_ref() {
                Some(ty) => self.resolved_identity(ty)?,
                None => self.repr_identity(TypeIR::Nulo, span)?,
            };
            sealed.push((
                name,
                FunctionSigIR {
                    ret_type,
                    ret_resolved,
                },
            ));
        }
        for (name, sig) in sealed {
            self.function_sigs.entry(name).or_insert(sig);
        }
        Ok(())
    }
    // @pinker-nav:end ir.lowering.identidade-resolvida

    fn resolve_union_ast_type(
        &self,
        ty: &Type,
        resolving: &mut Vec<String>,
    ) -> Result<Type, PinkerError> {
        match ty {
            Type::Alias { name, span } => {
                if self.struct_names.contains(name) {
                    return Ok(Type::Struct {
                        name: name.clone(),
                        span: *span,
                    });
                }
                // Somente o nome **declarado** do leque é identidade nominal.
                // Um apelido é transparente: `apelido X = Cor` precisa resolver
                // para `Cor`, e não produzir a identidade `enum:1:X` — usar o
                // texto do apelido como identidade é o erro que HR4 proíbe.
                if self.enum_decl_names.contains(name) {
                    return Ok(Type::Enum {
                        name: name.clone(),
                        span: *span,
                    });
                }
                if resolving.contains(name) {
                    return Err(PinkerError::Ir {
                        msg: format!("alias de tipo recursivo detectado em '{name}'"),
                        span: *span,
                    });
                }
                let Some(target) = self.type_aliases.get(name) else {
                    return Ok(ty.clone());
                };
                resolving.push(name.clone());
                let resolved = self.resolve_union_ast_type(target, resolving)?;
                resolving.pop();
                Ok(resolved.with_span(*span))
            }
            Type::Union { members, span } => {
                // Achatamento, deduplicação e ordem vêm do contrato
                // compartilhado — os mesmos consumidos pela semântica.
                let mut resolved_members = Vec::with_capacity(members.len());
                for member in members {
                    resolved_members.push(self.resolve_union_ast_type(member, resolving)?);
                }
                let canonical = union_canon::canonicalize_resolved_members(resolved_members);
                if canonical.len() < 2 {
                    return Err(PinkerError::Ir {
                        msg: "união exige dois membros canônicos distintos".to_string(),
                        span: *span,
                    });
                }
                Ok(Type::Union {
                    members: canonical,
                    span: *span,
                })
            }
            // Apelidos são transparentes **em profundidade**: `seta<Apelido>`,
            // `carinho(Apelido) -> Apelido` e `[Apelido; N]` têm de resolver os
            // componentes, senão a chave canônica ficaria envenenada e a
            // identidade seria perdida em tipos compostos perfeitamente legais.
            Type::Pointer {
                base,
                is_volatile,
                span,
            } => Ok(Type::Pointer {
                base: Box::new(self.resolve_union_ast_type(base, resolving)?),
                is_volatile: *is_volatile,
                span: *span,
            }),
            Type::Function { params, ret, span } => {
                let mut resolved_params = Vec::with_capacity(params.len());
                for param in params {
                    resolved_params.push(self.resolve_union_ast_type(param, resolving)?);
                }
                Ok(Type::Function {
                    params: resolved_params,
                    ret: Box::new(self.resolve_union_ast_type(ret, resolving)?),
                    span: *span,
                })
            }
            Type::FixedArray {
                element,
                size,
                span,
            } => Ok(Type::FixedArray {
                element: Box::new(self.resolve_union_ast_type(element, resolving)?),
                size: *size,
                span: *span,
            }),
            Type::Map { key, value, span } => Ok(Type::Map {
                key: Box::new(self.resolve_union_ast_type(key, resolving)?),
                value: Box::new(self.resolve_union_ast_type(value, resolving)?),
                span: *span,
            }),
            Type::ListEnum { element, span } => {
                let resolved_element = self.resolve_union_ast_type(
                    &Type::Alias {
                        name: element.clone(),
                        span: *span,
                    },
                    resolving,
                )?;
                let Type::Enum { name, .. } = resolved_element else {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "elemento '{}' de lista de leque não resolveu para leque",
                            element
                        ),
                        span: *span,
                    });
                };
                Ok(Type::ListEnum {
                    element: name,
                    span: *span,
                })
            }
            _ => Ok(ty.clone()),
        }
    }

    fn intern_union(&self, members: &[Type], span: Span) -> Result<TypeIR, PinkerError> {
        let canonical_key = union_canon::union_key(members);
        if let Some(existing) = self
            .union_registry
            .borrow()
            .types
            .iter()
            .find(|union| union.canonical_key == canonical_key)
            .map(|union| union.id)
        {
            return Ok(TypeIR::Union(existing));
        }
        // A identidade resolvida de cada membro é internada **antes** de
        // emprestar o registro de uniões mutavelmente: `intern_resolved_ast`
        // pode internar componentes e, para membros que são eles mesmos uniões,
        // reentrar em `intern_union`.
        let mut member_irs = Vec::with_capacity(members.len());
        for (tag, member) in members.iter().enumerate() {
            let ty = TypeIR::from_ast_with_context(member, &self.type_aliases, &self.struct_names)?;
            let resolved_type_id = self.intern_resolved_ast(member, span)?;
            // HR3: sem fallback. Um membro cuja representação de payload não
            // seja conhecida para a plataforma suportada é erro aqui, e a
            // semântica já o terá recusado antes com o código estável
            // correspondente.
            let payload_layout = crate::union_payload::classify_union_payload(
                member,
                &self.type_aliases,
                &self.struct_decls,
            )
            .map_err(|rejection| PinkerError::Ir {
                msg: rejection.message(),
                span,
            })?;
            member_irs.push(UnionMemberIR {
                tag: tag as u64,
                canonical_member_key: union_canon::member_key_text(member),
                ty,
                resolved_type_id,
                payload_layout,
            });
        }
        let mut registry = self.union_registry.borrow_mut();
        // Reconferido depois da internação das identidades: um membro que seja
        // união pode ter registrado a mesma união pai por reentrância.
        if let Some(existing) = registry
            .types
            .iter()
            .find(|union| union.canonical_key == canonical_key)
        {
            return Ok(TypeIR::Union(existing.id));
        }
        let id = UnionTypeId(
            u32::try_from(registry.types.len()).map_err(|_| PinkerError::Ir {
                msg: "registro de uniões excedeu u32".to_string(),
                span,
            })?,
        );
        registry.types.push(UnionTypeIR {
            id,
            canonical_key,
            members: member_irs,
        });
        Ok(TypeIR::Union(id))
    }
}

impl<'a> FunctionLowerer<'a> {
    // @pinker-nav:start ir.lowering.funcoes-blocos
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Configuração do `FunctionLowerer` e lowering de funções/blocos estruturados: aloca parâmetros e preserva metadados nominais/estruturais de callables, ponteiros crus e pointees de ponteiros de dados em aliases, retornos, ternários, chamadas por expressão e capturas de closure. Inclui resolvedores de método de `impl` direto e qualificado por trato; preserva a estrutura aninhada, sem ainda dividir o fluxo em CFG.
    fn new(context: &'a LoweringContext) -> Self {
        Self {
            context,
            scopes: Vec::new(),
            params: Vec::new(),
            locals: Vec::new(),
            slot_counters: HashMap::new(),
            block_counter: 0,
            loop_exit_stack: Vec::new(),
            loop_continue_stack: Vec::new(),
            callable_metadata: HashMap::new(),
            raw_function_metadata: HashMap::new(),
            pointer_pointee_types: HashMap::new(),
            trait_object_names: HashMap::new(),
        }
    }

    /// Nome nominal do receptor de um `impl`, consultado na tabela de
    /// identidades. O nome nunca é autoridade de identidade — é apenas a chave
    /// textual do catálogo de métodos, derivada da identidade resolvida.
    fn impl_receiver_key(&self, typed: &TypedValueIR) -> Option<String> {
        if typed.ty == TypeIR::Struct {
            return self.nominal_name_of_value(typed);
        }
        Some(typed.ty.name().to_string())
    }

    /// Identidade semântica de um nome de função usado como valor.
    ///
    /// Deriva da assinatura declarada (`carinho(P...) -> R`), não do nome do
    /// símbolo nem do wrapper sintético de `__env`.
    fn function_value_identity(
        &self,
        name: &str,
        span: Span,
    ) -> Result<Option<ResolvedTypeId>, PinkerError> {
        let Some(declaration) = self.context.all_functions.get(name) else {
            return Ok(None);
        };
        let params = declaration
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect::<Vec<_>>();
        let ret = declaration
            .ret_type
            .clone()
            .unwrap_or_else(|| Type::Nulo(span));
        let identity = self.context.resolved_identity(&Type::Function {
            params,
            ret: Box::new(ret),
            span,
        })?;
        Ok(Some(identity))
    }

    /// Identidade semântica do valor devolvido por uma chamada indireta.
    ///
    /// `trato<Nome>` é reconstruído a partir do nome nominal que a metadata do
    /// callable já preserva; nas demais representações a identidade é a própria
    /// representação e fica `None` (resolvida sob demanda).
    fn callable_ret_identity(
        &self,
        metadata: &CallableMetadata,
        span: Span,
    ) -> Result<Option<ResolvedTypeId>, PinkerError> {
        // O caminho normal é o tipo AST do retorno declarado: é ele que resolve
        // apelidos e distingue dois `leque` de mesma representação.
        if let Some(ret_ast) = metadata.ret_ast.as_ref() {
            return Ok(Some(self.context.resolved_identity(ret_ast)?));
        }
        if metadata.ret_type != TypeIR::TraitObject {
            return Ok(None);
        }
        let Some(trait_name) = metadata.ret_trait_name.as_ref() else {
            return Err(PinkerError::Ir {
                msg: "E-IR-TYPE-IDENTITY-LOST: retorno 'trato' de callable sem nome nominal"
                    .to_string(),
                span,
            });
        };
        let identity = self.context.resolved_identity(&Type::Applied {
            name: "trato".to_string(),
            args: vec![Type::Alias {
                name: trait_name.clone(),
                span,
            }],
            span,
        })?;
        Ok(Some(identity))
    }

    /// Identidade semântica do valor devolvido por uma chamada por ponteiro cru.
    fn raw_ret_identity(
        &self,
        metadata: &RawFunctionMetadata,
        span: Span,
    ) -> Result<Option<ResolvedTypeId>, PinkerError> {
        let _ = span;
        match metadata.ret_ast.as_ref() {
            Some(ret_ast) => Ok(Some(self.context.resolved_identity(ret_ast)?)),
            None => Ok(None),
        }
    }

    /// Identidade do apontado de um valor ponteiro, com sua representação.
    fn pointee_identity_of(&self, typed: &TypedValueIR) -> Option<(ResolvedTypeId, TypeIR)> {
        let resolved = typed.resolved?;
        let table = self.context.resolved_types.borrow();
        let pointee = table.get(resolved)?.pointee?;
        let representation = table.get(pointee)?.representation;
        Some((pointee, representation))
    }

    fn pointer_element_layout(
        &self,
        pointer: &TypedValueIR,
        span: Span,
    ) -> Result<layout::TypeLayout, PinkerError> {
        let (pointee, representation) =
            self.pointee_identity_of(pointer)
                .ok_or_else(|| PinkerError::Ir {
                    msg: "E-IR-POINTER-LAYOUT: identidade do elemento de seta<T> foi perdida"
                        .to_string(),
                    span,
                })?;
        let nominal_name = self
            .context
            .resolved_types
            .borrow()
            .get(pointee)
            .and_then(|entry| entry.nominal_name.clone());
        let element = match representation {
            TypeIR::Bombom => Type::Bombom(span),
            TypeIR::U8 => Type::U8(span),
            TypeIR::U16 => Type::U16(span),
            TypeIR::U32 => Type::U32(span),
            TypeIR::U64 => Type::U64(span),
            TypeIR::I8 => Type::I8(span),
            TypeIR::I16 => Type::I16(span),
            TypeIR::I32 => Type::I32(span),
            TypeIR::I64 => Type::I64(span),
            TypeIR::Logica => Type::Logica(span),
            TypeIR::FixedArray {
                element: ScalarTypeIR::Bombom,
                size,
            } => Type::FixedArray {
                element: Box::new(Type::Bombom(span)),
                size,
                span,
            },
            TypeIR::Struct => Type::Struct {
                name: nominal_name.ok_or_else(|| PinkerError::Ir {
                    msg: "E-IR-POINTER-LAYOUT: identidade nominal do ninho apontado foi perdida"
                        .to_string(),
                    span,
                })?,
                span,
            },
            _ => {
                return Err(PinkerError::Ir {
                    msg: format!(
                        "E-IR-POINTER-LAYOUT: tipo '{}' não participa da aritmética D5",
                        representation.name()
                    ),
                    span,
                })
            }
        };
        layout::layout_of_type(
            &element,
            &self.context.type_aliases,
            &self.context.struct_decls,
        )
        .map_err(|msg| PinkerError::Ir {
            msg: format!("E-IR-POINTER-LAYOUT: {}", msg),
            span,
        })
    }

    /// Nome nominal (`ninho`/`leque`) da identidade de um valor, se houver.
    fn nominal_name_of_value(&self, typed: &TypedValueIR) -> Option<String> {
        let resolved = typed.resolved?;
        self.context
            .resolved_types
            .borrow()
            .nominal_name_of(resolved)
            .map(str::to_string)
    }

    fn callable_metadata_from_return_type(
        &self,
        ret: &Type,
    ) -> Result<CallableMetadata, PinkerError> {
        Ok(CallableMetadata {
            ret_type: self.context.resolve_type(ret)?,
            ret_ast: Some(ret.clone()),
            ret_trait_name: trait_object_name_from_type(
                ret,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            ret_pointer_pointee: pointer_pointee_from_type(
                ret,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
        })
    }

    fn callable_metadata_for_value(&self, value: &ValueIR) -> Option<CallableMetadata> {
        match value {
            ValueIR::FunctionRef(name) => self
                .context
                .closure_state
                .borrow()
                .wrapper_metadata
                .get(name)
                .cloned(),
            ValueIR::MakeClosure { function_name, .. } => self
                .context
                .function_sigs
                .get(function_name)
                .map(|sig| CallableMetadata {
                    ret_type: sig.ret_type,
                    ret_ast: self
                        .context
                        .all_functions
                        .get(function_name)
                        .and_then(|declaration| declaration.ret_type.clone()),
                    ret_trait_name: self
                        .context
                        .function_ret_trait_names
                        .get(function_name)
                        .cloned(),
                    ret_pointer_pointee: self
                        .context
                        .function_ret_pointer_pointees
                        .get(function_name)
                        .copied(),
                }),
            ValueIR::Local(slot) => self.callable_metadata.get(slot).cloned(),
            ValueIR::Call { callee, .. } => self.context.callable_metadata.get(callee).cloned(),
            _ => None,
        }
    }

    fn raw_function_metadata_for_value(
        &self,
        value: &ValueIR,
    ) -> Result<Option<RawFunctionMetadata>, PinkerError> {
        match value {
            ValueIR::RawFunctionRef(name) => self
                .context
                .all_functions
                .get(name)
                .map(|function| {
                    raw_function_metadata_from_decl(
                        function,
                        &self.context.type_aliases,
                        &self.context.struct_names,
                    )
                })
                .transpose(),
            ValueIR::Local(slot) => Ok(self.raw_function_metadata.get(slot).cloned()),
            ValueIR::Call { callee, args, .. } if callee == "__ternario" => {
                let [_, true_value, false_value] = args.as_slice() else {
                    return Ok(None);
                };
                let true_metadata = self.raw_function_metadata_for_value(true_value)?;
                let false_metadata = self.raw_function_metadata_for_value(false_value)?;
                Ok(match (true_metadata, false_metadata) {
                    (Some(true_metadata), Some(false_metadata))
                        if true_metadata == false_metadata =>
                    {
                        Some(true_metadata)
                    }
                    _ => None,
                })
            }
            ValueIR::Call { callee, .. } => Ok(self
                .context
                .raw_function_return_metadata
                .get(callee)
                .cloned()),
            _ => Ok(None),
        }
    }

    fn pointer_pointee_for_expr(&self, expr: &Expr) -> Result<Option<TypeIR>, PinkerError> {
        match &expr.kind {
            ExprKind::Ident(name) => Ok(self
                .resolve_existing_binding(name)
                .and_then(|binding| self.pointer_pointee_types.get(&binding.slot).copied())),
            ExprKind::Cast { target, .. } => pointer_pointee_from_type(
                target,
                &self.context.type_aliases,
                &self.context.struct_names,
            ),
            ExprKind::Call(callee, _) => {
                if let ExprKind::Ident(name) = &callee.kind {
                    if name == "alocar" {
                        return Ok(Some(TypeIR::U8));
                    }
                    if let Some(binding) = self.resolve_existing_binding(name) {
                        if let Some(pointee) = self
                            .raw_function_metadata
                            .get(&binding.slot)
                            .and_then(|metadata| metadata.ret_pointer_pointee)
                        {
                            return Ok(Some(pointee));
                        }
                        if let Some(pointee) = self
                            .callable_metadata
                            .get(&binding.slot)
                            .and_then(|metadata| metadata.ret_pointer_pointee)
                        {
                            return Ok(Some(pointee));
                        }
                    }
                    return Ok(self
                        .context
                        .function_ret_pointer_pointees
                        .get(name)
                        .copied());
                }
                Ok(self
                    .raw_function_metadata_for_expr(callee)?
                    .and_then(|metadata| metadata.ret_pointer_pointee))
            }
            ExprKind::Binary(lhs, BinaryOp::Add | BinaryOp::Sub, rhs) => {
                let left = self.pointer_pointee_for_expr(lhs)?;
                if left.is_some() {
                    Ok(left)
                } else {
                    self.pointer_pointee_for_expr(rhs)
                }
            }
            _ => Ok(None),
        }
    }

    fn raw_function_metadata_for_expr(
        &self,
        expr: &Expr,
    ) -> Result<Option<RawFunctionMetadata>, PinkerError> {
        match &expr.kind {
            ExprKind::Ident(name) => Ok(self
                .resolve_existing_binding(name)
                .and_then(|binding| self.raw_function_metadata.get(&binding.slot).cloned())),
            ExprKind::AddressOf(operand) => {
                let ExprKind::Ident(name) = &operand.kind else {
                    return Ok(None);
                };
                self.context
                    .all_functions
                    .get(name)
                    .map(|function| {
                        raw_function_metadata_from_decl(
                            function,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )
                    })
                    .transpose()
            }
            ExprKind::Call(callee, args) if matches!(&callee.kind, ExprKind::Ident(name) if name == "__ternario") =>
            {
                let [_, true_value, false_value] = args.as_slice() else {
                    return Ok(None);
                };
                let true_metadata = self.raw_function_metadata_for_expr(true_value)?;
                let false_metadata = self.raw_function_metadata_for_expr(false_value)?;
                Ok(match (true_metadata, false_metadata) {
                    (Some(true_metadata), Some(false_metadata))
                        if true_metadata == false_metadata =>
                    {
                        Some(true_metadata)
                    }
                    _ => None,
                })
            }
            _ => Ok(None),
        }
    }

    fn callable_metadata_for_expr(
        &self,
        expr: &Expr,
    ) -> Result<Option<CallableMetadata>, PinkerError> {
        match &expr.kind {
            ExprKind::Ident(name) => {
                if let Some(binding) = self.resolve_existing_binding(name) {
                    return Ok((binding.ty == TypeIR::Function)
                        .then(|| self.callable_metadata.get(&binding.slot).cloned())
                        .flatten());
                }

                Ok(self
                    .context
                    .function_sigs
                    .get(name)
                    .map(|sig| CallableMetadata {
                        ret_type: sig.ret_type,
                        ret_ast: self
                            .context
                            .all_functions
                            .get(name)
                            .and_then(|declaration| declaration.ret_type.clone()),
                        ret_trait_name: self.context.function_ret_trait_names.get(name).cloned(),
                        ret_pointer_pointee: self
                            .context
                            .function_ret_pointer_pointees
                            .get(name)
                            .copied(),
                    }))
            }
            ExprKind::Call(callee, args) => {
                let ExprKind::Ident(name) = &callee.kind else {
                    return Ok(None);
                };
                if name != "__ternario" {
                    return Ok(self.context.callable_metadata.get(name).cloned());
                }

                let [_, true_value, false_value] = args.as_slice() else {
                    return Err(PinkerError::Ir {
                        msg: "lowering encontrou ternário callable sem três argumentos".to_string(),
                        span: expr.span,
                    });
                };
                let true_metadata = self.callable_metadata_for_expr(true_value)?;
                let false_metadata = self.callable_metadata_for_expr(false_value)?;
                let (Some(true_metadata), Some(false_metadata)) = (true_metadata, false_metadata)
                else {
                    return Err(PinkerError::Ir {
                        msg: "ternário callable exige metadados nos dois braços".to_string(),
                        span: expr.span,
                    });
                };
                if true_metadata.ret_type != false_metadata.ret_type {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "ternário callable possui retornos estruturais incompatíveis: {} e {}",
                            true_metadata.ret_type.name(),
                            false_metadata.ret_type.name()
                        ),
                        span: expr.span,
                    });
                }
                if true_metadata.ret_trait_name != false_metadata.ret_trait_name {
                    return Err(PinkerError::Ir {
                        msg: "ternário callable possui retornos nominais incompatíveis".to_string(),
                        span: expr.span,
                    });
                }

                Ok(Some(true_metadata))
            }
            _ => Ok(None),
        }
    }

    fn resolve_impl_method(&self, receiver: &TypedValueIR, method_name: &str) -> Option<String> {
        let receiver_key = self.impl_receiver_key(receiver)?;
        let candidates: Vec<String> = self
            .context
            .function_sigs
            .keys()
            .filter_map(|name| {
                let (_, target_type, method) = parse_impl_function_name(name)?;
                (target_type == receiver_key && method == method_name).then(|| name.clone())
            })
            .collect();
        match candidates.as_slice() {
            [function_name] => Some(function_name.clone()),
            _ => None,
        }
    }

    fn resolve_qualified_impl_method(
        &self,
        receiver: &TypedValueIR,
        trait_name: &str,
        method_name: &str,
    ) -> Option<String> {
        let receiver_key = self.impl_receiver_key(receiver)?;
        self.context.function_sigs.keys().find_map(|name| {
            let (candidate_trait, target_type, method) = parse_impl_function_name(name)?;
            (candidate_trait == trait_name && target_type == receiver_key && method == method_name)
                .then(|| name.clone())
        })
    }

    fn resolve_trait_impl_symbol(
        &self,
        trait_name: &str,
        target_type: &str,
        method_name: &str,
    ) -> Option<String> {
        self.context.function_sigs.keys().find_map(|name| {
            let (candidate_trait, candidate_target, candidate_method) =
                parse_impl_function_name(name)?;

            (candidate_trait == trait_name
                && candidate_target == target_type
                && candidate_method == method_name)
                .then(|| name.clone())
        })
    }

    fn trait_object_name_for_expr(&self, expr: &Expr) -> Result<Option<String>, PinkerError> {
        let trait_name = match &expr.kind {
            ExprKind::Ident(name) => {
                let Some(binding) = self.resolve_existing_binding(name) else {
                    return Ok(None);
                };

                self.trait_object_names.get(&binding.slot).cloned()
            }
            ExprKind::Cast { target, .. } => trait_object_name_from_type(
                target,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            ExprKind::Call(callee, args) => match &callee.kind {
                ExprKind::Ident(function_name) if function_name == "__ternario" => {
                    let [_, true_value, false_value] = args.as_slice() else {
                        return Err(PinkerError::Ir {
                            msg: "lowering encontrou ternário sem três argumentos".to_string(),
                            span: expr.span,
                        });
                    };
                    let true_trait = self.trait_object_name_for_expr(true_value)?;
                    let false_trait = self.trait_object_name_for_expr(false_value)?;
                    match (true_trait, false_trait) {
                        (Some(true_trait), Some(false_trait)) if true_trait == false_trait => {
                            Some(true_trait)
                        }
                        (None, None) => None,
                        (Some(true_trait), Some(false_trait)) => {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "ternário perdeu compatibilidade nominal entre trato<{}> e trato<{}>",
                                    true_trait, false_trait
                                ),
                                span: expr.span,
                            });
                        }
                        _ => {
                            return Err(PinkerError::Ir {
                                msg: "ternário de objeto de trato exige identidade nominal nos dois braços"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                    }
                }
                ExprKind::Ident(function_name) => {
                    if let Some(binding) = self.resolve_existing_binding(function_name) {
                        if binding.ty == TypeIR::Function {
                            let metadata =
                                self.callable_metadata.get(&binding.slot).ok_or_else(|| {
                                    PinkerError::Ir {
                                        msg: format!(
                                            "lowering perdeu os metadados do callable '{}'",
                                            function_name
                                        ),
                                        span: expr.span,
                                    }
                                })?;
                            if metadata.ret_type == TypeIR::TraitObject
                                && metadata.ret_trait_name.is_none()
                            {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "lowering perdeu a identidade nominal do trato retornado pelo callable '{}'",
                                        function_name
                                    ),
                                    span: expr.span,
                                });
                            }
                            metadata.ret_trait_name.clone()
                        } else {
                            None
                        }
                    } else {
                        self.context
                            .function_ret_trait_names
                            .get(function_name)
                            .cloned()
                    }
                }
                ExprKind::FieldAccess { base, field } => {
                    let trait_name = match &base.kind {
                        ExprKind::Ident(name) if self.context.traits.contains_key(name) => {
                            name.clone()
                        }
                        _ => {
                            let Some(name) = self.trait_object_name_for_expr(base)? else {
                                return Ok(None);
                            };
                            name
                        }
                    };
                    self.context
                        .traits
                        .get(&trait_name)
                        .and_then(|meta| meta.methods.iter().find(|method| method.name == *field))
                        .and_then(|method| method.ret_trait_name.clone())
                }
                _ => None,
            },
            _ => None,
        };
        Ok(trait_name)
    }

    fn concrete_snapshot_size(&self, value: &TypedValueIR, span: Span) -> Result<u64, PinkerError> {
        match value.ty {
            TypeIR::Bombom | TypeIR::U64 | TypeIR::I64 => Ok(8),
            TypeIR::U32 | TypeIR::I32 => Ok(4),
            TypeIR::U16 | TypeIR::I16 => Ok(2),
            TypeIR::U8 | TypeIR::I8 | TypeIR::Logica => Ok(1),

            // Categorias representadas por handle ou ponteiro de uma palavra.
            TypeIR::Verso
            | TypeIR::ListBombom
            | TypeIR::ListVerso
            | TypeIR::MapVersoBombom
            | TypeIR::MapVersoVerso
            | TypeIR::MapBombomBombom
            | TypeIR::MapBombomVerso
            | TypeIR::Map { .. }
            | TypeIR::OpaqueWordHandle
            | TypeIR::Pointer { .. }
            | TypeIR::Function
            | TypeIR::Union(_)
            | TypeIR::FunctionPointer => Ok(layout::POINTER_SIZE),

            TypeIR::FixedArray { element, size } => {
                let element_size: u64 = match element {
                    ScalarTypeIR::Bombom | ScalarTypeIR::U64 | ScalarTypeIR::I64 => 8,
                    ScalarTypeIR::U32 | ScalarTypeIR::I32 => 4,
                    ScalarTypeIR::U16 | ScalarTypeIR::I16 => 2,
                    ScalarTypeIR::U8 | ScalarTypeIR::I8 | ScalarTypeIR::Logica => 1,
                };

                element_size
                    .checked_mul(size)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: "overflow ao calcular snapshot de array".to_string(),
                        span,
                    })
            }

            TypeIR::Struct => {
                let struct_name =
                    self.nominal_name_of_value(value)
                        .ok_or_else(|| PinkerError::Ir {
                            msg: "E-IR-TYPE-IDENTITY-LOST: snapshot de ninho sem identidade \
                                  resolvida"
                                .to_string(),
                            span,
                        })?;

                let ast_type = Type::Struct {
                    name: struct_name,
                    span,
                };

                layout::layout_of_type(
                    &ast_type,
                    &self.context.type_aliases,
                    &self.context.struct_decls,
                )
                .map(|layout| layout.size)
                .map_err(|msg| PinkerError::Ir {
                    msg: format!("layout inválido para snapshot do objeto de trato: {}", msg),
                    span,
                })
            }

            TypeIR::TraitObject | TypeIR::Nulo => Err(PinkerError::Ir {
                msg: "tipo concreto inválido para materialização de objeto de trato".to_string(),
                span,
            }),
        }
    }

    fn trait_vtable(
        &self,
        trait_name: &str,
        target_type: &str,
        span: Span,
    ) -> Result<Vec<String>, PinkerError> {
        let trait_meta = self
            .context
            .traits
            .get(trait_name)
            .ok_or_else(|| PinkerError::Ir {
                msg: format!("lowering não encontrou metadados do trato '{}'", trait_name),
                span,
            })?;

        trait_meta
            .methods
            .iter()
            .map(|method| {
                self.resolve_trait_impl_symbol(trait_name, target_type, &method.name)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!(
                            "lowering não encontrou impl de '{}.{}' para '{}'",
                            trait_name, method.name, target_type
                        ),
                        span,
                    })
            })
            .collect()
    }

    fn lower_trait_call(
        &mut self,
        object: TypedValueIR,
        trait_name: &str,
        method_name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<TypedValueIR, PinkerError> {
        let (method_slot, method_count, method) = {
            let trait_meta =
                self.context
                    .traits
                    .get(trait_name)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("lowering não encontrou metadados do trato '{}'", trait_name),
                        span,
                    })?;

            let (slot, method) = trait_meta
                .methods
                .iter()
                .enumerate()
                .find(|(_, method)| method.name == method_name)
                .ok_or_else(|| PinkerError::Ir {
                    msg: format!(
                        "lowering não encontrou método '{}.{}'",
                        trait_name, method_name
                    ),
                    span,
                })?;

            (slot as u64, trait_meta.methods.len() as u64, method.clone())
        };

        if args.len() != method.param_types.len() {
            return Err(PinkerError::Ir {
                msg: format!(
                    "lowering recebeu aridade inconsistente em '{}.{}'",
                    trait_name, method_name
                ),
                span,
            });
        }

        let lowered_args = args
            .iter()
            .map(|arg| self.lower_value(arg).map(|typed| typed.value))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(TypedValueIR {
            value: ValueIR::TraitCall {
                object: Box::new(object.value),
                trait_name: trait_name.to_string(),
                method_name: method_name.to_string(),
                method_slot,
                method_count,
                args: lowered_args,
                param_types: method.param_types,
                ret_type: method.ret_type,
            },
            ty: method.ret_type,
            resolved: method
                .ret_ast
                .as_ref()
                .map(|ty| self.context.resolved_identity(ty))
                .transpose()?,
            ptr_array_bombom_size: None,
        })
    }

    fn lower_function(mut self, function: &FunctionDecl) -> Result<FunctionIR, PinkerError> {
        self.push_scope();

        for param in &function.params {
            let binding = self.allocate_binding(
                &param.name,
                self.context.resolve_type(&param.ty)?,
                Some(self.context.resolved_identity(&param.ty)?),
                pointer_to_bombom_array_size(&param.ty, &self.context.type_aliases),
                None,
            )?;

            if let Some(trait_name) = trait_object_name_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.trait_object_names
                    .insert(binding.slot.clone(), trait_name);
            }

            if let Type::Function { ret, .. } = &param.ty {
                let ret_ty = self.context.resolve_type(ret)?;
                self.callable_metadata.insert(
                    binding.slot.clone(),
                    CallableMetadata {
                        ret_type: ret_ty,
                        ret_ast: Some(ret.as_ref().clone()),
                        ret_trait_name: trait_object_name_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                        ret_pointer_pointee: pointer_pointee_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                    },
                );
            }
            if let Some(metadata) = raw_function_metadata_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.raw_function_metadata
                    .insert(binding.slot.clone(), metadata);
            }
            if let Some(pointee) = pointer_pointee_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.pointer_pointee_types
                    .insert(binding.slot.clone(), pointee);
            }
            self.params.push(binding);
        }

        let entry = self.lower_block(&function.body, "entry".to_string(), false)?;

        self.pop_scope();

        Ok(FunctionIR {
            name: function.name.clone(),
            params: self.params,
            locals: self.locals,
            ret_type: TypeIR::from_ast_option_with_context(
                function.ret_type.as_ref(),
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            entry,
            span: function.span,
        })
    }

    // Fase 243: resolve um literal `carinho` no ponto de criação. Espelha
    // `semantic.rs::resolve_closure_value` — mesma varredura sintática
    // (`ast::free_identifiers_in_function`), mesma regra de captura (nome
    // livre que resolve como binding LOCAL nesta função, via
    // `resolve_existing_binding`; os demais não são captura, resolvidos
    // normalmente dentro do próprio corpo da closure). Cada literal tem
    // exatamente um ponto de criação (desaçucaramento da Fase 225 o
    // substitui por um único `Ident`), então esta função nunca é chamada
    // duas vezes para o mesmo nome em um programa válido.
    // Fase 243: gera (memoizado) um wrapper sintético `__fnref_env_<nome>`
    // para uma função top-level usada como valor callable. O wrapper tem a
    // MESMA assinatura pública de `nome`, mais um parâmetro oculto final
    // `__env` (ignorado), e o corpo é só `mimo nome(params...)`. Isso torna
    // a convenção de chamada indireta uniforme (todo callable — closure ou
    // referência a função top-level — aceita `__env` por último) sem tocar
    // em nenhuma chamada direta existente a `nome` (que continua chamando
    // `nome` de verdade, não o wrapper).
    fn ensure_fnref_wrapper(&mut self, name: &str, span: Span) -> Result<String, PinkerError> {
        let wrapper_name = format!("__fnref_env_{}", name);
        if self
            .context
            .closure_state
            .borrow()
            .wrapper_metadata
            .contains_key(&wrapper_name)
        {
            return Ok(wrapper_name);
        }

        let function = self
            .context
            .all_functions
            .get(name)
            .cloned()
            .ok_or_else(|| PinkerError::Ir {
                msg: format!(
                    "lowering falhou ao materializar wrapper de '{}' (função sem corpo AST — provável intrínseca; referenciar intrínsecas como valor não é suportado)",
                    name
                ),
                span,
            })?;

        let mut wrapper = FunctionLowerer::new(self.context);
        wrapper.push_scope();
        let mut call_args = Vec::new();
        let mut wrapper_params = Vec::new();
        for param in &function.params {
            let binding = wrapper.allocate_binding(
                &param.name,
                wrapper.context.resolve_type(&param.ty)?,
                Some(wrapper.context.resolved_identity(&param.ty)?),
                pointer_to_bombom_array_size(&param.ty, &wrapper.context.type_aliases),
                None,
            )?;
            call_args.push(ValueIR::Local(binding.slot.clone()));
            wrapper_params.push(binding);
        }
        let ret_type = TypeIR::from_ast_option_with_context(
            function.ret_type.as_ref(),
            &wrapper.context.type_aliases,
            &wrapper.context.struct_names,
        )?;
        let env_pointer = TypeIR::Pointer { is_volatile: false };
        let env_identity = wrapper
            .context
            .internal_identity("env", env_pointer, function.span)?;
        let env_binding =
            wrapper.allocate_binding("__env", env_pointer, Some(env_identity), None, None)?;
        wrapper_params.push(env_binding);
        wrapper.pop_scope();

        let wrapper_fn = FunctionIR {
            name: wrapper_name.clone(),
            params: wrapper_params,
            locals: Vec::new(),
            ret_type,
            entry: BlockIR {
                label: "entry".to_string(),
                instructions: vec![InstructionIR::Return {
                    value: Some(ValueIR::Call {
                        callee: name.to_string(),
                        args: call_args,
                        ret_type,
                    }),
                    span,
                }],
                span,
            },
            span,
        };

        let mut state = self.context.closure_state.borrow_mut();
        state.wrapper_metadata.insert(
            wrapper_name.clone(),
            CallableMetadata {
                ret_type,
                ret_ast: function.ret_type.clone(),
                ret_trait_name: function
                    .ret_type
                    .as_ref()
                    .map(|ty| {
                        trait_object_name_from_type(
                            ty,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )
                    })
                    .transpose()?
                    .flatten(),
                ret_pointer_pointee: function
                    .ret_type
                    .as_ref()
                    .map(|ty| {
                        pointer_pointee_from_type(
                            ty,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )
                    })
                    .transpose()?
                    .flatten(),
            },
        );
        state.lowered.push((wrapper_name.clone(), wrapper_fn));
        Ok(wrapper_name)
    }

    fn resolve_closure(&mut self, name: &str, span: Span) -> Result<TypedValueIR, PinkerError> {
        if self
            .context
            .closure_state
            .borrow()
            .captures
            .contains_key(name)
        {
            return Err(PinkerError::Ir {
                msg: format!(
                    "closure '{}' referenciada mais de uma vez (não suportado nesta fase)",
                    name
                ),
                span,
            });
        }
        let function = self
            .context
            .all_functions
            .get(name)
            .cloned()
            .ok_or_else(|| PinkerError::Ir {
                msg: format!("lowering falhou ao resolver closure '{}'", name),
                span,
            })?;
        let param_names: HashSet<String> = function.params.iter().map(|p| p.name.clone()).collect();
        let free = transitive_free_identifiers_in_function(&function, |name| {
            self.context.all_functions.get(name).cloned()
        });
        let mut captures: Vec<CaptureMetadata> = Vec::new();
        let mut capture_values: Vec<ValueIR> = Vec::new();
        for candidate in &free {
            if param_names.contains(candidate) {
                continue;
            }
            let Some(binding) = self.resolve_existing_binding(candidate) else {
                continue;
            };
            captures.push(CaptureMetadata {
                source_name: candidate.clone(),
                ty: binding.ty,
                // A captura preserva a identidade exata da variável capturada:
                // um `ninho` capturado continua sendo aquele `ninho` dentro do
                // corpo da closure.
                resolved: binding.resolved,
                trait_object_name: self.trait_object_names.get(&binding.slot).cloned(),
                callable: self.callable_metadata.get(&binding.slot).cloned(),
                raw_function: self.raw_function_metadata.get(&binding.slot).cloned(),
                pointer_pointee: self.pointer_pointee_types.get(&binding.slot).copied(),
            });
            capture_values.push(ValueIR::Local(binding.slot));
        }
        self.context
            .closure_state
            .borrow_mut()
            .captures
            .insert(name.to_string(), captures.clone());
        let lowered =
            FunctionLowerer::new(self.context).lower_closure_function(&function, &captures)?;
        self.context
            .closure_state
            .borrow_mut()
            .lowered
            .push((name.to_string(), lowered));
        // A closure é um valor de função: sua identidade é a assinatura
        // declarada, do mesmo modo que um nome de função usado como valor.
        let closure_identity = self.function_value_identity(name, span)?;
        Ok(TypedValueIR {
            value: ValueIR::MakeClosure {
                function_name: name.to_string(),
                captures: capture_values,
            },
            ty: TypeIR::Function,
            resolved: closure_identity,
            ptr_array_bombom_size: None,
        })
    }

    // Fase 243: abaixa o corpo de uma closure com o ambiente já resolvido.
    // Quando há capturas, injeta um parâmetro oculto final `__env` (ponteiro)
    // e, antes do corpo real, uma sequência de `Let` sintéticos que
    // dereferenciam `__env + i*8` para cada captura (ordem determinística
    // de primeira referência) — mesma disciplina de 1 palavra por valor da
    // Fase 242. Sem chamada de usuário alcança este parâmetro: só a própria
    // chamada indireta o preenche (ver `cfg_ir`/`backend_s`).
    fn lower_closure_function(
        mut self,
        function: &FunctionDecl,
        captures: &[CaptureMetadata],
    ) -> Result<FunctionIR, PinkerError> {
        self.push_scope();

        // Fase 243: `__env` é SEMPRE o parâmetro real final (trailing) —
        // uniforme para toda função indiretamente chamável, capturante ou
        // não (closures sem captura E wrappers de função top-level, ver
        // `ensure_fnref_wrapper`). Isso é o que permite ao call site de
        // `call_indirect` emitir sempre N+1 argumentos sem ramificação:
        // quem não usa `__env` simplesmente o ignora. O slot é alocado
        // primeiro (para as expressões de desempacotamento abaixo), mas só
        // entra em `self.params` (posição final) depois dos parâmetros
        // reais.
        let env_pointer = TypeIR::Pointer { is_volatile: false };
        let env_identity = self
            .context
            .internal_identity("env", env_pointer, function.span)?;
        let env_binding =
            self.allocate_binding("__env", env_pointer, Some(env_identity), None, None)?;

        let mut prelude = Vec::new();
        for (index, capture) in captures.iter().enumerate() {
            // Capturas entram no escopo ANTES dos parâmetros para que um
            // parâmetro homônimo possa sombreá-las (§14.3) — a inserção
            // posterior do parâmetro no mesmo mapa de escopo sobrescreve.
            let capture_binding = self.allocate_binding(
                &capture.source_name,
                capture.ty,
                capture.resolved,
                None,
                Some(false),
            )?;
            if let Some(trait_name) = &capture.trait_object_name {
                self.trait_object_names
                    .insert(capture_binding.slot.clone(), trait_name.clone());
            }
            if let Some(callable) = &capture.callable {
                self.callable_metadata
                    .insert(capture_binding.slot.clone(), callable.clone());
            }
            if let Some(raw_function) = &capture.raw_function {
                self.raw_function_metadata
                    .insert(capture_binding.slot.clone(), raw_function.clone());
            }
            if let Some(pointer_pointee) = capture.pointer_pointee {
                self.pointer_pointee_types
                    .insert(capture_binding.slot.clone(), pointer_pointee);
            }
            let ptr_expr = ValueIR::Binary {
                op: BinaryOpIR::Add,
                lhs: Box::new(ValueIR::Local(env_binding.slot.clone())),
                rhs: Box::new(ValueIR::Int((index as u64) * 8)),
                ty: TypeIR::Pointer { is_volatile: false },
            };
            prelude.push(InstructionIR::Let {
                slot: capture_binding.slot,
                value: ValueIR::Deref {
                    ptr: Box::new(ptr_expr),
                    result_type: capture.ty,
                    is_volatile: false,
                },
                span: function.span,
            });
        }

        for param in &function.params {
            let binding = self.allocate_binding(
                &param.name,
                self.context.resolve_type(&param.ty)?,
                Some(self.context.resolved_identity(&param.ty)?),
                pointer_to_bombom_array_size(&param.ty, &self.context.type_aliases),
                None,
            )?;

            if let Some(trait_name) = trait_object_name_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.trait_object_names
                    .insert(binding.slot.clone(), trait_name);
            }

            if let Type::Function { ret, .. } = &param.ty {
                let ret_ty = self.context.resolve_type(ret)?;
                self.callable_metadata.insert(
                    binding.slot.clone(),
                    CallableMetadata {
                        ret_type: ret_ty,
                        ret_ast: Some(ret.as_ref().clone()),
                        ret_trait_name: trait_object_name_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                        ret_pointer_pointee: pointer_pointee_from_type(
                            ret,
                            &self.context.type_aliases,
                            &self.context.struct_names,
                        )?,
                    },
                );
            }
            if let Some(metadata) = raw_function_metadata_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.raw_function_metadata
                    .insert(binding.slot.clone(), metadata);
            }
            if let Some(pointee) = pointer_pointee_from_type(
                &param.ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )? {
                self.pointer_pointee_types
                    .insert(binding.slot.clone(), pointee);
            }
            self.params.push(binding);
        }
        self.params.push(env_binding);

        let mut entry = self.lower_block(&function.body, "entry".to_string(), false)?;
        entry.instructions.splice(0..0, prelude);

        self.pop_scope();

        Ok(FunctionIR {
            name: function.name.clone(),
            params: self.params,
            locals: self.locals,
            ret_type: TypeIR::from_ast_option_with_context(
                function.ret_type.as_ref(),
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            entry,
            span: function.span,
        })
    }

    fn lower_block(
        &mut self,
        block: &Block,
        label: String,
        create_scope: bool,
    ) -> Result<BlockIR, PinkerError> {
        if create_scope {
            self.push_scope();
        }

        let mut instructions = Vec::new();
        for stmt in &block.stmts {
            instructions.push(self.lower_stmt(stmt)?);
        }

        if create_scope {
            self.pop_scope();
        }

        Ok(BlockIR {
            label,
            instructions,
            span: block.span,
        })
    }
    // @pinker-nav:end ir.lowering.funcoes-blocos

    // @pinker-nav:start ir.lowering.comandos-controle
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Abaixa comandos AST de um bloco para `InstructionIR`: despacho de `Stmt`, declaração local (`nova`/`muda`, incluindo o desvio de `lista_criar`/`mapa_criar` para o criar monomórfico anotado), atribuição a slot/deref/campo/índice, retorno (`mimo`), `falar`, asm inline, e o controle estruturado `talvez`/`senão` e `sempre que` com `quebrar`/`continuar` carregando destinos simbólicos de laço. Preserva spans; `if`/`while` continuam com blocos filhos — a divisão em blocos básicos ocorre depois em `cfg_ir`.
    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<InstructionIR, PinkerError> {
        match stmt {
            Stmt::Let(let_stmt) => self.lower_let(let_stmt),
            Stmt::Assign(assign_stmt) => {
                let value = self.lower_value(&assign_stmt.expr)?;
                match &assign_stmt.target {
                    AssignTarget::Ident(name) => {
                        let binding = self.resolve_binding(name, assign_stmt.span)?;
                        if binding.ty == TypeIR::Function {
                            let metadata = self
                                .callable_metadata_for_expr(&assign_stmt.expr)?
                                .or_else(|| self.callable_metadata_for_value(&value.value))
                                .ok_or_else(|| {
                                    PinkerError::Ir {
                                        msg: format!(
                                            "lowering perdeu os metadados na reatribuição do callable '{}'",
                                            name
                                        ),
                                        span: assign_stmt.span,
                                    }
                                })?;
                            self.callable_metadata
                                .insert(binding.slot.clone(), metadata);
                        }
                        Ok(InstructionIR::Assign {
                            slot: binding.slot,
                            value: value.value,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::Deref(ptr_expr) => {
                        let pointee_type = self.pointer_pointee_for_expr(ptr_expr)?;
                        let ptr = self.lower_value(ptr_expr)?;
                        let is_volatile = match ptr.ty {
                            TypeIR::Pointer { is_volatile } => is_volatile,
                            _ => {
                                return Err(PinkerError::Ir {
                                    msg: "escrita indireta exige ponteiro no lowering IR"
                                        .to_string(),
                                    span: assign_stmt.span,
                                });
                            }
                        };
                        Ok(InstructionIR::StoreIndirect {
                            ptr: ptr.value,
                            value: value.value,
                            value_type: pointee_type.unwrap_or(value.ty),
                            is_volatile,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::Index { base, index } => {
                        let base_lowered = self.lower_value(base)?;
                        let element_type = match base_lowered.ty {
                            TypeIR::FixedArray { element, .. } => match element {
                                ScalarTypeIR::Bombom => TypeIR::Bombom,
                                _ => return Err(PinkerError::Ir {
                                    msg:
                                        "escrita por índice nesta fase aceita apenas '[bombom; N]'"
                                            .to_string(),
                                    span: assign_stmt.span,
                                }),
                            },
                            _ => {
                                return Err(PinkerError::Ir {
                                    msg:
                                        "escrita por índice exige base de array fixo no lowering IR"
                                            .to_string(),
                                    span: assign_stmt.span,
                                })
                            }
                        };
                        let index_lowered = self.lower_value(index)?;
                        Ok(InstructionIR::StoreIndexed {
                            base: base_lowered.value,
                            index: index_lowered.value,
                            value: value.value,
                            element_type,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::FieldDeref { base, field } => {
                        let base_lowered = self.lower_value(base)?;
                        let Some(base_struct_name) = self.nominal_name_of_value(&base_lowered)
                        else {
                            return Err(PinkerError::Ir {
                                msg: "escrita a campo exige base do tipo 'ninho' no lowering IR"
                                    .to_string(),
                                span: assign_stmt.span,
                            });
                        };
                        let base_struct_name = &base_struct_name;
                        let field_type = self
                            .context
                            .struct_fields
                            .get(base_struct_name)
                            .and_then(|fields| fields.get(field.as_str()))
                            .copied()
                            .ok_or_else(|| PinkerError::Ir {
                                msg: format!(
                                    "campo '{}' não encontrado em '{}' para escrita",
                                    field, base_struct_name
                                ),
                                span: assign_stmt.span,
                            })?;
                        let field_offset = self
                            .context
                            .struct_field_offsets
                            .get(base_struct_name)
                            .and_then(|fields| fields.get(field.as_str()))
                            .copied()
                            .ok_or_else(|| PinkerError::Ir {
                                msg: format!(
                                    "offset de campo '{}' não encontrado no layout de '{}' para escrita",
                                    field, base_struct_name
                                ),
                                span: assign_stmt.span,
                            })?;
                        let is_volatile = match &base_lowered.value {
                            ValueIR::Deref { is_volatile, .. } => *is_volatile,
                            _ => false,
                        };
                        Ok(InstructionIR::StoreFieldIndirect {
                            base: base_lowered.value,
                            field: field.clone(),
                            field_offset,
                            value: value.value,
                            value_type: field_type,
                            is_volatile,
                            span: assign_stmt.span,
                        })
                    }
                }
            }
            Stmt::Return(return_stmt) => self.lower_return(return_stmt),
            Stmt::Expr(expr) => Ok(InstructionIR::Expr {
                value: self.lower_value(expr)?.value,
                span: expr.span,
            }),
            Stmt::If(if_stmt) => self.lower_if(if_stmt),
            Stmt::While(while_stmt) => self.lower_while(while_stmt),
            Stmt::Break(break_stmt) => self.lower_break(break_stmt),
            Stmt::Continue(continue_stmt) => self.lower_continue(continue_stmt),
            Stmt::Falar(falar_stmt) => self.lower_falar(falar_stmt),
            Stmt::InlineAsm(inline_asm_stmt) => self.lower_inline_asm(inline_asm_stmt),
            Stmt::EnumMatch(enum_match) => self.lower_enum_match(enum_match),
            Stmt::UnionMatch(union_match) => self.lower_union_match(union_match),
        }
    }

    fn lower_enum_match(
        &mut self,
        enum_match: &EnumMatchStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let scrutinee = self.lower_value(&enum_match.scrutinee)?;
        let scrutinee_identity = scrutinee.identity(self.context, enum_match.scrutinee.span)?;

        self.push_scope();
        let scrutinee_binding = self.allocate_binding(
            "encaixe_leque_alvo",
            scrutinee.ty,
            Some(scrutinee_identity),
            None,
            Some(false),
        )?;
        self.pop_scope();

        let mut arms = Vec::with_capacity(enum_match.arms.len());
        for arm in &enum_match.arms {
            self.push_scope();
            let pattern = self.lower_enum_pattern(&arm.pattern, scrutinee_identity)?;
            let body_label = self.next_block_label("encaixe_leque_braco");
            let body = self.lower_block(&arm.body, body_label, false);
            self.pop_scope();
            arms.push(EnumMatchArmIR {
                pattern,
                body: body?,
                span: arm.span,
            });
        }
        let otherwise = enum_match
            .otherwise
            .as_ref()
            .map(|block| {
                let label = self.next_block_label("encaixe_leque_senao");
                self.lower_block(block, label, true)
            })
            .transpose()?;

        Ok(InstructionIR::EnumMatch(EnumMatchIR {
            scrutinee: scrutinee.value,
            scrutinee_binding,
            arms,
            otherwise,
            span: enum_match.span,
        }))
    }

    fn lower_enum_pattern(
        &mut self,
        pattern: &EnumPattern,
        expected_type_id: ResolvedTypeId,
    ) -> Result<EnumPatternIR, PinkerError> {
        match pattern {
            EnumPattern::Binding { name, span } => Err(PinkerError::Ir {
                msg: format!(
                    "binding raiz '{}' não é permitido em 'encaixe' de leque",
                    name
                ),
                span: *span,
            }),
            EnumPattern::Variant {
                enum_name,
                variant,
                payloads,
                span,
            } => {
                let enum_info = self
                    .context
                    .enum_variants
                    .get(enum_name)
                    .cloned()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("leque '{}' do padrão ausente na IR", enum_name),
                        span: *span,
                    })?;
                let enum_identity = self.context.resolved_identity(&Type::Enum {
                    name: enum_info.declared_name.clone(),
                    span: *span,
                })?;
                if enum_identity != expected_type_id {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "INVALID_NESTED_PATTERN_TYPE: identidade esperada {} difere do leque '{}' ({})",
                            expected_type_id.0, enum_info.declared_name, enum_identity.0
                        ),
                        span: *span,
                    });
                }
                let (discriminant, declared_payloads) = enum_info
                    .variants
                    .get(variant)
                    .cloned()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!(
                            "variante '{}.{}' do padrão ausente na IR",
                            enum_info.declared_name, variant
                        ),
                        span: *span,
                    })?;
                if declared_payloads.len() != payloads.len() {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "INVALID_PATTERN_PAYLOAD_ARITY: '{}.{}' exige {}, padrão possui {}",
                            enum_info.declared_name,
                            variant,
                            declared_payloads.len(),
                            payloads.len()
                        ),
                        span: *span,
                    });
                }

                let mut lowered_payloads = Vec::with_capacity(payloads.len());
                for (index, (payload, declared)) in
                    payloads.iter().zip(declared_payloads).enumerate()
                {
                    let resolved_type_id = self
                        .context
                        .intern_resolved_ast(&declared.shape.resolved, payload.span())?;
                    let lowered_pattern = match payload {
                        EnumPattern::Binding { name, span } => EnumPatternIR::Binding {
                            binding: self.allocate_binding(
                                name,
                                declared.operational_type,
                                Some(resolved_type_id),
                                None,
                                Some(false),
                            )?,
                            span: *span,
                        },
                        EnumPattern::Variant { .. } => {
                            self.lower_enum_pattern(payload, resolved_type_id)?
                        }
                    };
                    lowered_payloads.push(EnumPatternPayloadIR {
                        index: index as u64,
                        operational_type: declared.operational_type,
                        class: declared.shape.class,
                        canonical_key: declared.shape.canonical_key(),
                        resolved_type_id,
                        extract_intrinsic: declared.shape.carga_intrinsic().to_string(),
                        extracted_binding: self.allocate_binding(
                            "encaixe_leque_carga",
                            declared.operational_type,
                            Some(resolved_type_id),
                            None,
                            Some(false),
                        )?,
                        pattern: Box::new(lowered_pattern),
                    });
                }

                Ok(EnumPatternIR::Variant {
                    enum_name: enum_info.declared_name,
                    expected_type_id: enum_identity,
                    variant_name: variant.clone(),
                    discriminant,
                    has_payload: enum_info.has_payload,
                    payloads: lowered_payloads,
                    span: *span,
                })
            }
        }
    }

    // Abaixa `Stmt::UnionMatch` para `InstructionIR::UnionMatch`: avalia o scrutinee uma única vez, obtém o `UnionTypeId` do valor, resolve cada tipo de braço pelo contrato compartilhado de `union_canon`, localiza exatamente um membro do `UnionTypeIR` internado pela chave canônica, **copia** tag, tipo, tamanho e alinhamento do membro, cria o binding próprio do braço e abaixa o corpo no escopo desse binding. Preserva a ordem de fonte dos braços e revalida a cobertura como defesa de fronteira; a tag nunca é derivada de posição, ordem textual, nome de apelido ou `TypeIR` isolado.
    fn lower_union_match(
        &mut self,
        union_match: &UnionMatchStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let scrutinee = self.lower_value(&union_match.scrutinee)?;
        let TypeIR::Union(union_type_id) = scrutinee.ty else {
            return Err(PinkerError::Ir {
                msg: format!(
                    "'encaixe' de união exige scrutinee de união; encontrado '{}'",
                    scrutinee.ty.name()
                ),
                span: union_match.scrutinee.span,
            });
        };

        let union_ir = {
            let registry = self.context.union_registry.borrow();
            registry
                .types
                .get(union_type_id.0 as usize)
                .cloned()
                .ok_or_else(|| PinkerError::Ir {
                    msg: format!(
                        "união {} ausente do registro internado no 'encaixe'",
                        union_type_id.0
                    ),
                    span: union_match.span,
                })?
        };

        // Slots de lowering para o scrutinee e a tag. São slots normalizados
        // desta camada (como qualquer `%nome#N`), não identidade de membro:
        // a identidade continua sendo a chave canônica do registry.
        self.push_scope();
        let scrutinee_binding = self.allocate_binding(
            "encaixe_uniao_alvo",
            TypeIR::Union(union_type_id),
            None,
            None,
            Some(false),
        )?;
        let tag_binding =
            self.allocate_binding("encaixe_uniao_tag", TypeIR::Bombom, None, None, Some(false))?;
        self.pop_scope();

        let mut arms = Vec::with_capacity(union_match.arms.len());
        let mut covered = HashSet::<String>::new();
        for arm in &union_match.arms {
            let resolved_member = self
                .context
                .resolve_union_ast_type(&arm.member_type, &mut Vec::new())?;
            let key = union_canon::member_key(&resolved_member);
            let mut matching = union_ir
                .members
                .iter()
                .filter(|member| member.canonical_member_key == key.canonical_type_key);
            let member = matching.next().ok_or_else(|| PinkerError::Ir {
                msg: format!(
                    "braço '{}' de 'encaixe' não pertence à união {}",
                    arm.member_type.name(),
                    union_type_id.0
                ),
                span: arm.span,
            })?;
            if matching.next().is_some() {
                return Err(PinkerError::Ir {
                    msg: format!(
                        "chave canônica ambígua na união {}: '{}'",
                        union_type_id.0, key.canonical_type_key
                    ),
                    span: arm.span,
                });
            }
            if !covered.insert(member.canonical_member_key.clone()) {
                return Err(PinkerError::Ir {
                    msg: format!(
                        "membro '{}' repetido no 'encaixe' da união {}",
                        member.canonical_member_key, union_type_id.0
                    ),
                    span: arm.span,
                });
            }

            self.push_scope();
            // O `encaixe` liga o braço à identidade **exata** do membro: o valor
            // desempacotado é aquele membro, não "algum membro com a mesma
            // representação". É isso que permite reinjetar o valor na mesma
            // união sem reescolher a tag.
            let binding = self.allocate_binding(
                &arm.binding,
                member.ty,
                Some(member.resolved_type_id),
                None,
                Some(false),
            )?;
            let body_label = self.next_block_label("encaixe_uniao_braco");
            let body = self.lower_block(&arm.body, body_label, false);
            self.pop_scope();
            let body = body?;

            arms.push(UnionMatchArmIR {
                tag: member.tag,
                canonical_member_key: member.canonical_member_key.clone(),
                resolved_member_type_id: member.resolved_type_id,
                binding,
                payload_type: member.ty,
                payload_layout: member.payload_layout,
                body,
                span: arm.span,
            });
        }

        // Defesa de fronteira: a semântica já exigiu cobertura exata, e o
        // lowering recusa qualquer divergência restante.
        if covered.len() != union_ir.members.len() {
            return Err(PinkerError::Ir {
                msg: format!(
                    "cobertura incompleta no 'encaixe' da união {}: {} de {} membros",
                    union_type_id.0,
                    covered.len(),
                    union_ir.members.len()
                ),
                span: union_match.span,
            });
        }

        Ok(InstructionIR::UnionMatch(UnionMatchIR {
            scrutinee: scrutinee.value,
            scrutinee_binding,
            tag_binding,
            union_type_id,
            arms,
            span: union_match.span,
        }))
    }

    fn lower_falar(&mut self, falar_stmt: &FalarStmt) -> Result<InstructionIR, PinkerError> {
        let mut args = Vec::with_capacity(falar_stmt.args.len());
        for arg in &falar_stmt.args {
            let typed = self.lower_value(arg)?;
            args.push(FalarArgIR {
                value: typed.value,
                ty: typed.ty,
            });
        }
        Ok(InstructionIR::Falar {
            args,
            span: falar_stmt.span,
        })
    }

    fn lower_inline_asm(
        &mut self,
        inline_asm_stmt: &InlineAsmStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let mut operands = Vec::new();
        for operand in &inline_asm_stmt.operands {
            let constraint =
                crate::inline_asm::parse_constraint(&operand.constraint).map_err(|error| {
                    PinkerError::Ir {
                        msg: error.to_string(),
                        span: operand.span,
                    }
                })?;
            match &operand.direction {
                crate::ast::InlineAsmDirection::Input => {
                    let value = self.lower_value(&operand.value)?;
                    operands.push(InlineAsmOperandIR::Input {
                        name: operand.name.clone(),
                        constraint,
                        value: value.value,
                        ty: value.ty,
                    });
                }
                crate::ast::InlineAsmDirection::Output => {
                    let ExprKind::Ident(target) = &operand.value.kind else {
                        return Err(PinkerError::Ir {
                            msg: "saida de sussurro perdeu alvo simples validado".to_string(),
                            span: operand.value.span,
                        });
                    };
                    let binding = self.resolve_binding(target, operand.value.span)?;
                    operands.push(InlineAsmOperandIR::Output {
                        name: operand.name.clone(),
                        constraint,
                        slot: binding.slot,
                        ty: binding.ty,
                    });
                }
                crate::ast::InlineAsmDirection::Unknown(direction) => {
                    return Err(PinkerError::Ir {
                        msg: format!("direção de operando de sussurro não validada: '{direction}'"),
                        span: operand.span,
                    });
                }
            }
        }
        let clobbers = inline_asm_stmt
            .clobbers
            .iter()
            .map(|clobber| {
                crate::inline_asm::parse_clobber(&clobber.name).map_err(|error| PinkerError::Ir {
                    msg: error.to_string(),
                    span: clobber.span,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(InstructionIR::InlineAsm {
            chunks: inline_asm_stmt.chunks.clone(),
            operands,
            clobbers,
            span: inline_asm_stmt.span,
        })
    }

    fn lower_let(&mut self, let_stmt: &LetStmt) -> Result<InstructionIR, PinkerError> {
        // `nova l: lista<...> = lista_criar();` — a criação genérica abaixa
        // para o criar monomorphizado do tipo anotado (semântica já validou).
        if let Some(annotated_ty) = let_stmt.ty.as_ref() {
            if is_generic_list_create_expr(&let_stmt.init) {
                let slot_ty = self.context.resolve_type(annotated_ty)?;
                let callee = match slot_ty {
                    TypeIR::ListVerso => "lista_verso_criar",
                    _ => "lista_bombom_criar",
                };
                let binding = self.allocate_binding(
                    &let_stmt.name,
                    slot_ty,
                    Some(self.context.resolved_identity(annotated_ty)?),
                    None,
                    Some(let_stmt.is_mut),
                )?;
                return Ok(InstructionIR::Let {
                    slot: binding.slot,
                    value: ValueIR::Call {
                        callee: callee.to_string(),
                        args: Vec::new(),
                        ret_type: slot_ty,
                    },
                    span: let_stmt.span,
                });
            }
            if is_generic_map_create_expr(&let_stmt.init) {
                let slot_ty = self.context.resolve_type(annotated_ty)?;
                let callee = match slot_ty {
                    TypeIR::MapVersoBombom => "mapa_verso_bombom_criar",
                    TypeIR::MapVersoVerso => "mapa_verso_verso_criar",
                    TypeIR::MapBombomBombom => "mapa_bombom_bombom_criar",
                    TypeIR::MapBombomVerso => "mapa_bombom_verso_criar",
                    TypeIR::Map {
                        key: MapKeyIR::Bombom,
                        ..
                    } => "__pinker_internal_mapa_criar_chave_bombom",
                    TypeIR::Map {
                        key: MapKeyIR::Verso,
                        ..
                    } => "__pinker_internal_mapa_criar_chave_verso",
                    _ => {
                        return Err(PinkerError::Ir {
                            msg: format!(
                                "mapa_criar() exige anotação de mapa; encontrado '{}'",
                                slot_ty.name()
                            ),
                            span: let_stmt.span,
                        });
                    }
                };
                let binding = self.allocate_binding(
                    &let_stmt.name,
                    slot_ty,
                    Some(self.context.resolved_identity(annotated_ty)?),
                    None,
                    Some(let_stmt.is_mut),
                )?;
                return Ok(InstructionIR::Let {
                    slot: binding.slot,
                    value: ValueIR::Call {
                        callee: callee.to_string(),
                        args: Vec::new(),
                        ret_type: slot_ty,
                    },
                    span: let_stmt.span,
                });
            }
        }
        let trait_object_name = match let_stmt.ty.as_ref() {
            Some(ty) => trait_object_name_from_type(
                ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?,
            None => None,
        }
        .or(self.trait_object_name_for_expr(&let_stmt.init)?);

        let value = self.lower_value(&let_stmt.init)?;
        let ty = if let Some(annotated_ty) = let_stmt.ty.as_ref() {
            self.context.resolve_type(annotated_ty)?
        } else {
            value.ty
        };
        // A identidade do slot vem da anotação quando ela existe (é ela que o
        // usuário escreveu) e, na ausência dela, da identidade exata do valor.
        // Nenhuma das duas é derivada de nome textual.
        let resolved = match let_stmt.ty.as_ref() {
            Some(annotated_ty) => Some(self.context.resolved_identity(annotated_ty)?),
            None => value.resolved,
        };
        let ptr_array_bombom_size = let_stmt
            .ty
            .as_ref()
            .and_then(|annotated_ty| {
                pointer_to_bombom_array_size(annotated_ty, &self.context.type_aliases)
            })
            .or(value.ptr_array_bombom_size);
        // Fase 242: quando a variável é callable, registra o ret_type da
        // chamada indireta através dela — via anotação explícita, ou
        // derivado da origem do valor (referência de função, cópia de outra
        // variável callable, ou retorno de uma função que devolve callable).
        let callable_metadata = if ty == TypeIR::Function {
            if let Some(Type::Function { ret, .. }) = let_stmt.ty.as_ref() {
                Some(self.callable_metadata_from_return_type(ret)?)
            } else {
                self.callable_metadata_for_value(&value.value)
            }
        } else {
            None
        };
        let raw_function_metadata = if ty == TypeIR::FunctionPointer {
            if let Some(annotated_ty) = let_stmt.ty.as_ref() {
                raw_function_metadata_from_type(
                    annotated_ty,
                    &self.context.type_aliases,
                    &self.context.struct_names,
                )?
            } else {
                self.raw_function_metadata_for_value(&value.value)?
            }
        } else {
            None
        };
        let pointer_pointee_type = if let Some(annotated_ty) = let_stmt.ty.as_ref() {
            pointer_pointee_from_type(
                annotated_ty,
                &self.context.type_aliases,
                &self.context.struct_names,
            )?
        } else {
            self.pointer_pointee_for_expr(&let_stmt.init)?
        };
        let binding = self.allocate_binding(
            &let_stmt.name,
            ty,
            resolved,
            ptr_array_bombom_size,
            Some(let_stmt.is_mut),
        )?;
        if let Some(metadata) = callable_metadata {
            self.callable_metadata
                .insert(binding.slot.clone(), metadata);
        }
        if let Some(metadata) = raw_function_metadata {
            self.raw_function_metadata
                .insert(binding.slot.clone(), metadata);
        }
        if let Some(pointee) = pointer_pointee_type {
            self.pointer_pointee_types
                .insert(binding.slot.clone(), pointee);
        }

        if ty == TypeIR::TraitObject {
            let trait_name = trait_object_name.ok_or_else(|| PinkerError::Ir {
                msg: format!(
                    "lowering perdeu a identidade nominal do objeto de trato '{}'",
                    let_stmt.name
                ),
                span: let_stmt.span,
            })?;

            self.trait_object_names
                .insert(binding.slot.clone(), trait_name);
        }

        Ok(InstructionIR::Let {
            slot: binding.slot,
            value: value.value,
            span: let_stmt.span,
        })
    }

    fn lower_return(&mut self, return_stmt: &ReturnStmt) -> Result<InstructionIR, PinkerError> {
        let value = return_stmt
            .expr
            .as_ref()
            .map(|expr| self.lower_value(expr).map(|typed| typed.value))
            .transpose()?;
        Ok(InstructionIR::Return {
            value,
            span: return_stmt.span,
        })
    }

    fn lower_if(&mut self, if_stmt: &IfStmt) -> Result<InstructionIR, PinkerError> {
        let condition = self.lower_value(&if_stmt.condition)?.value;
        let then_label = self.next_block_label("then");
        let then_block = self.lower_block(&if_stmt.then_branch, then_label, true)?;
        let else_block = match &if_stmt.else_branch {
            Some(ElseBlock::Block(block)) => {
                let else_label = self.next_block_label("else");
                Some(self.lower_block(block, else_label, true)?)
            }
            Some(ElseBlock::If(nested_if)) => {
                let else_label = self.next_block_label("else");
                self.push_scope();
                let nested_instruction = self.lower_if(nested_if)?;
                self.pop_scope();
                Some(BlockIR {
                    label: else_label,
                    instructions: vec![nested_instruction],
                    span: nested_if.span,
                })
            }
            None => None,
        };

        Ok(InstructionIR::If {
            condition,
            then_block,
            else_block,
            span: if_stmt.span,
        })
    }

    fn lower_while(&mut self, while_stmt: &WhileStmt) -> Result<InstructionIR, PinkerError> {
        let condition = self.lower_value(&while_stmt.condition)?.value;
        let body_label = self.next_block_label("loop");
        let loop_exit_label = self.next_block_label("loop_break_join");
        let loop_continue_label = self.next_block_label("loop_continue");
        self.loop_exit_stack.push(loop_exit_label);
        self.loop_continue_stack.push(loop_continue_label);
        let body_block = self.lower_block(&while_stmt.body, body_label, true)?;
        self.loop_continue_stack.pop();
        self.loop_exit_stack.pop();
        Ok(InstructionIR::While {
            condition,
            body_block,
            span: while_stmt.span,
        })
    }

    fn lower_continue(
        &mut self,
        continue_stmt: &ContinueStmt,
    ) -> Result<InstructionIR, PinkerError> {
        let Some(loop_continue_label) = self.loop_continue_stack.last() else {
            return Err(PinkerError::Ir {
                msg: "lowering encontrou 'continuar' fora de loop".to_string(),
                span: continue_stmt.span,
            });
        };

        Ok(InstructionIR::Continue {
            loop_continue_label: loop_continue_label.clone(),
            span: continue_stmt.span,
        })
    }

    fn lower_break(&mut self, break_stmt: &BreakStmt) -> Result<InstructionIR, PinkerError> {
        let Some(loop_exit_label) = self.loop_exit_stack.last() else {
            return Err(PinkerError::Ir {
                msg: "lowering encontrou 'quebrar' fora de loop".to_string(),
                span: break_stmt.span,
            });
        };

        Ok(InstructionIR::Break {
            loop_exit_label: loop_exit_label.clone(),
            span: break_stmt.span,
        })
    }
    // @pinker-nav:end ir.lowering.comandos-controle

    // @pinker-nav:start ir.lowering.expressoes-valores
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Grande despachante que abaixa expressões AST para `TypedValueIR` (valor, representação e identidade resolvida): literais, bindings/globais, operadores, dereferência, chamadas, métodos, intrínsecas genéricas, construção/leitura de leque, campos, índices, cast, `peso` e `alinhamento`. Operações de lista/mapa que devolvem elemento preservam a identidade exata do container, inclusive leques representados como `bombom`; não executa nem seleciona instruções de máquina.
    fn lower_value(&mut self, expr: &Expr) -> Result<TypedValueIR, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(value) => Ok(TypedValueIR {
                value: ValueIR::Int(*value),
                ty: TypeIR::Bombom,
                resolved: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::BoolLit(value) => Ok(TypedValueIR {
                value: ValueIR::Bool(*value),
                ty: TypeIR::Logica,
                resolved: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::StringLit(value) => Ok(TypedValueIR {
                value: ValueIR::String(value.clone()),
                ty: TypeIR::Verso,
                resolved: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::InternalMapIterCreate(map) => {
                let map = self.lower_value(map)?;
                Ok(TypedValueIR {
                    value: ValueIR::Call {
                        callee: "__pinker_internal_mapa_verso_bombom_iterador_criar".to_string(),
                        args: vec![map.value],
                        ret_type: TypeIR::Bombom,
                    },
                    ty: TypeIR::Bombom,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::InternalMapIterNextKey(iterator) => {
                let iterator = self.lower_value(iterator)?;
                Ok(TypedValueIR {
                    value: ValueIR::Call {
                        callee: "__pinker_internal_mapa_verso_bombom_iterador_proxima_chave"
                            .to_string(),
                        args: vec![iterator.value],
                        ret_type: TypeIR::Verso,
                    },
                    ty: TypeIR::Verso,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Ident(name) => {
                // Fase 243: nome sintético de literal `carinho` — resolve
                // como criação de closure (com ou sem capturas), no ponto
                // exato onde `self.scopes` reflete o escopo léxico vigente.
                if name.starts_with("__anon_carinho_") {
                    return self.resolve_closure(name, expr.span);
                }
                if let Some(binding) = self.resolve_existing_binding(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::Local(binding.slot),
                        ty: binding.ty,
                        resolved: binding.resolved,
                        ptr_array_bombom_size: binding.ptr_array_bombom_size,
                    });
                }

                if let Some(ty) = self.context.global_consts.get(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::GlobalConst(name.clone()),
                        ty: *ty,
                        resolved: None,
                        ptr_array_bombom_size: None,
                    });
                }

                // Fase 242/243: nome solto de função top-level materializa
                // um valor callable. Desde a Fase 243, `FunctionRef` aponta
                // para um wrapper sintético (`__fnref_env_<nome>`) que
                // aceita e ignora o parâmetro oculto `__env` — a mesma
                // convenção uniforme das closures — sem alterar em nada a
                // função real nem suas chamadas diretas existentes.
                if self.context.function_sigs.contains_key(name) {
                    let wrapper_name = self.ensure_fnref_wrapper(name, expr.span)?;
                    // A identidade do valor callable é a assinatura completa:
                    // `carinho(u8) -> u8` e `carinho(u64) -> u64` compartilham
                    // `TypeIR::Function` e precisam de identidades distintas.
                    let resolved = self.function_value_identity(name, expr.span)?;
                    return Ok(TypedValueIR {
                        value: ValueIR::FunctionRef(wrapper_name),
                        ty: TypeIR::Function,
                        resolved,
                        ptr_array_bombom_size: None,
                    });
                }

                Err(PinkerError::Ir {
                    msg: format!("lowering falhou ao resolver identificador '{}'", name),
                    span: expr.span,
                })
            }
            ExprKind::AddressOf(operand) => {
                let ExprKind::Ident(name) = &operand.kind else {
                    return Err(PinkerError::Ir {
                        msg: "lowering de endereço cru exige função top-level resolvida"
                            .to_string(),
                        span: operand.span,
                    });
                };
                if !self.context.function_sigs.contains_key(name) {
                    return Err(PinkerError::Ir {
                        msg: format!(
                            "lowering não encontrou símbolo '{}' para endereço cru",
                            name
                        ),
                        span: operand.span,
                    });
                }
                // O endereço cru de uma função é `seta<carinho(...)>`: a
                // identidade é o ponteiro para a assinatura declarada, e não a
                // categoria `seta<carinho>`, que é a mesma para toda função.
                let signature = self.function_value_identity(name, expr.span)?;
                let resolved = match signature {
                    Some(signature) => {
                        let key = {
                            let table = self.context.resolved_types.borrow();
                            table.key_of(signature).map(str::to_string)
                        };
                        match key {
                            Some(key) => Some(
                                self.context
                                    .resolved_types
                                    .borrow_mut()
                                    .intern(
                                        format!("ptr:0:{key}"),
                                        TypeIR::FunctionPointer,
                                        ResolvedTypeParts {
                                            pointee: Some(signature),
                                            ..ResolvedTypeParts::default()
                                        },
                                    )
                                    .map_err(|msg| PinkerError::Ir {
                                        msg,
                                        span: expr.span,
                                    })?,
                            ),
                            None => None,
                        }
                    }
                    None => None,
                };
                Ok(TypedValueIR {
                    value: ValueIR::RawFunctionRef(name.clone()),
                    ty: TypeIR::FunctionPointer,
                    resolved,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Unary(op, operand) => {
                let pointee_type = if *op == UnaryOp::Deref {
                    self.pointer_pointee_for_expr(operand)?
                } else {
                    None
                };
                let operand = self.lower_value(operand)?;
                if *op == UnaryOp::Deref {
                    let TypeIR::Pointer { is_volatile } = operand.ty else {
                        return Err(PinkerError::Ir {
                            msg: "dereferência exige operando do tipo seta no lowering IR"
                                .to_string(),
                            span: expr.span,
                        });
                    };
                    // A identidade do valor dereferenciado é a identidade do
                    // apontado registrada na própria identidade do ponteiro:
                    // `seta<u8>` e `seta<u64>` compartilham `TypeIR::Pointer` e
                    // se distinguem exatamente aqui.
                    let pointee_identity = self.pointee_identity_of(&operand);
                    let (result_type, result_resolved) = match pointee_identity {
                        Some((pointee_id, TypeIR::Struct)) => (TypeIR::Struct, Some(pointee_id)),
                        _ => {
                            if let Some(size) = operand.ptr_array_bombom_size {
                                (
                                    TypeIR::FixedArray {
                                        element: ScalarTypeIR::Bombom,
                                        size,
                                    },
                                    None,
                                )
                            } else if let Some(pointee_type) = pointee_type {
                                let resolved = pointee_identity
                                    .filter(|(_, repr)| *repr == pointee_type)
                                    .map(|(id, _)| id);
                                (pointee_type, resolved)
                            } else {
                                (TypeIR::Bombom, None)
                            }
                        }
                    };
                    return Ok(TypedValueIR {
                        value: ValueIR::Deref {
                            ptr: Box::new(operand.value),
                            result_type,
                            is_volatile,
                        },
                        ty: result_type,
                        resolved: result_resolved,
                        ptr_array_bombom_size: None,
                    });
                }
                Ok(TypedValueIR {
                    value: ValueIR::Unary {
                        op: UnaryOpIR::from_ast(*op),
                        operand: Box::new(operand.value),
                        ty: match op {
                            UnaryOp::Neg | UnaryOp::BitNot => operand.ty,
                            UnaryOp::Not => TypeIR::Logica,
                            UnaryOp::Deref => unreachable!("deref tratada acima"),
                        },
                    },
                    ty: match op {
                        UnaryOp::Neg => operand.ty,
                        UnaryOp::Not => TypeIR::Logica,
                        UnaryOp::BitNot => operand.ty,
                        UnaryOp::Deref => unreachable!("deref tratada acima"),
                    },
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_is_int_lit = matches!(lhs.kind, ExprKind::IntLit(_));
                let lhs = self.lower_value(lhs)?;
                let rhs = self.lower_value(rhs)?;
                if *op == BinaryOp::Add && matches!(lhs.ty, TypeIR::Pointer { .. }) {
                    let element_layout = self.pointer_element_layout(&lhs, expr.span)?;
                    let result_type = lhs.ty;
                    let result_resolved = lhs.resolved;
                    let result_array_size = lhs.ptr_array_bombom_size;
                    return Ok(TypedValueIR {
                        value: ValueIR::PointerOffset {
                            pointer: Box::new(lhs.value),
                            offset: Box::new(rhs.value),
                            pointer_type: result_type,
                            element_size: element_layout.size,
                            element_align: element_layout.align,
                        },
                        ty: result_type,
                        resolved: result_resolved,
                        ptr_array_bombom_size: result_array_size,
                    });
                }
                let operation_type = if lhs_is_int_lit && rhs.ty.is_integer() {
                    rhs.ty
                } else {
                    lhs.ty
                };
                let result_type = match op {
                    BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr
                    | BinaryOp::Eq
                    | BinaryOp::Neq
                    | BinaryOp::Lt
                    | BinaryOp::Lte
                    | BinaryOp::Gt
                    | BinaryOp::Gte => TypeIR::Logica,
                    _ => operation_type,
                };
                Ok(TypedValueIR {
                    value: ValueIR::Binary {
                        op: BinaryOpIR::from_ast(*op),
                        lhs: Box::new(lhs.value),
                        rhs: Box::new(rhs.value),
                        ty: operation_type,
                    },
                    ty: result_type,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Call(callee, args) => {
                // Construção `Leque.Variante(cargas...)` abaixa para uma
                // cadeia composável: criar_0(tag) seguido de um anexar por
                // carga, cada anexar devolvendo o mesmo handle.
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    if let ExprKind::Ident(base_name) = &base.kind {
                        if let Some(info) = self.context.enum_variants.get(base_name) {
                            let Some((discriminant, payload_types)) =
                                info.variants.get(field).cloned()
                            else {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "construção inválida de '{}.{}' na IR",
                                        base_name, field
                                    ),
                                    span: expr.span,
                                });
                            };
                            if payload_types.is_empty() || args.len() != payload_types.len() {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "construção de '{}.{}' com aridade inconsistente na IR",
                                        base_name, field
                                    ),
                                    span: expr.span,
                                });
                            }
                            let mut chain = ValueIR::Call {
                                callee: "__pinker_internal_leque_criar_0".to_string(),
                                args: vec![ValueIR::Int(discriminant)],
                                ret_type: TypeIR::Bombom,
                            };
                            for (arg, payload_ty) in args.iter().zip(payload_types) {
                                let payload = self.lower_value(arg)?;
                                // A identidade da carga é conferida aqui, e não
                                // pela representação: `lista<Cor>` e
                                // `lista<Token>` são a mesma palavra e tipos
                                // diferentes.
                                let expected_identity = self
                                    .context
                                    .intern_resolved_ast(&payload_ty.shape.resolved, arg.span)?;
                                if let Some(actual) = payload.resolved {
                                    if actual != expected_identity {
                                        return Err(PinkerError::Ir {
                                            msg: format!(
                                                "E-IR-ENUM-PAYLOAD-IDENTITY: carga de '{}.{}' exige identidade '{}' e recebeu '{}'",
                                                base_name,
                                                field,
                                                payload_ty.shape.canonical_key(),
                                                self.context
                                                    .resolved_types
                                                    .borrow()
                                                    .key_of(actual)
                                                    .unwrap_or("?")
                                            ),
                                            span: arg.span,
                                        });
                                    }
                                }
                                // O helper deriva da classe de representação
                                // decidida pela autoridade única, nunca de um
                                // `match` parcial sobre o tipo-fonte.
                                let anexar = payload_ty.shape.anexar_intrinsic();
                                chain = ValueIR::Call {
                                    callee: anexar.to_string(),
                                    args: vec![chain, payload.value],
                                    ret_type: TypeIR::Bombom,
                                };
                            }
                            return Ok(TypedValueIR {
                                value: chain,
                                ty: TypeIR::Bombom,
                                resolved: None,
                                ptr_array_bombom_size: None,
                            });
                        }
                    }
                }
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    if let ExprKind::Ident(trait_name) = &base.kind {
                        if self.context.traits.contains_key(trait_name) && !args.is_empty() {
                            let receiver = self.lower_value(&args[0])?;

                            if receiver.ty == TypeIR::TraitObject {
                                return self.lower_trait_call(
                                    receiver,
                                    trait_name,
                                    field,
                                    &args[1..],
                                    expr.span,
                                );
                            }

                            if let Some(function_name) =
                                self.resolve_qualified_impl_method(&receiver, trait_name, field)
                            {
                                let mut ir_args = Vec::with_capacity(args.len());
                                ir_args.push(receiver.value);
                                for arg in args.iter().skip(1) {
                                    ir_args.push(self.lower_value(arg)?.value);
                                }
                                let ret_type = self
                                    .context
                                    .function_sigs
                                    .get(&function_name)
                                    .map(|sig| sig.ret_type)
                                    .ok_or_else(|| PinkerError::Ir {
                                        msg: format!(
                                            "lowering falhou ao resolver método interno '{}'",
                                            function_name
                                        ),
                                        span: expr.span,
                                    })?;
                                return Ok(TypedValueIR {
                                    value: ValueIR::Call {
                                        callee: function_name.clone(),
                                        args: ir_args,
                                        ret_type,
                                    },
                                    ty: ret_type,
                                    resolved: self
                                        .context
                                        .function_sigs
                                        .get(&function_name)
                                        .map(|sig| sig.ret_resolved),
                                    ptr_array_bombom_size: None,
                                });
                            }
                        }
                    }
                    let receiver = self.lower_value(base)?;

                    if receiver.ty == TypeIR::TraitObject {
                        let trait_name =
                            self.trait_object_name_for_expr(base)?.ok_or_else(|| {
                                PinkerError::Ir {
                                    msg: format!(
                                        "lowering perdeu a identidade nominal do receiver de '{}'",
                                        field
                                    ),
                                    span: expr.span,
                                }
                            })?;

                        return self.lower_trait_call(
                            receiver,
                            &trait_name,
                            field,
                            args,
                            expr.span,
                        );
                    }

                    let function_name =
                        if let Some(function_name) = self.resolve_impl_method(&receiver, field) {
                            function_name
                        } else if self.context.function_sigs.contains_key(field) {
                            field.clone()
                        } else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering falhou ao resolver método '{}' para receiver '{}'",
                                    field,
                                    self.impl_receiver_key(&receiver)
                                        .unwrap_or_else(|| receiver.ty.name().to_string())
                                ),
                                span: expr.span,
                            });
                        };
                    let mut ir_args = Vec::with_capacity(args.len() + 1);
                    ir_args.push(receiver.value);
                    for arg in args {
                        ir_args.push(self.lower_value(arg)?.value);
                    }
                    let ret_type = self
                        .context
                        .function_sigs
                        .get(&function_name)
                        .map(|sig| sig.ret_type)
                        .ok_or_else(|| PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver método interno '{}'",
                                function_name
                            ),
                            span: expr.span,
                        })?;
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            callee: function_name.clone(),
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        resolved: self
                            .context
                            .function_sigs
                            .get(&function_name)
                            .map(|sig| sig.ret_resolved),
                        ptr_array_bombom_size: None,
                    });
                }

                let ExprKind::Ident(name) = &callee.kind else {
                    let lowered_callee = self.lower_value(callee)?;
                    if lowered_callee.ty != TypeIR::FunctionPointer {
                        return Err(PinkerError::Ir {
                            msg: "lowering de chamada por expressão exige ponteiro cru de função"
                                .to_string(),
                            span: callee.span,
                        });
                    }
                    let Some(metadata) =
                        self.raw_function_metadata_for_value(&lowered_callee.value)?
                    else {
                        return Err(PinkerError::Ir {
                            msg: "lowering perdeu a assinatura da expressão de ponteiro cru"
                                .to_string(),
                            span: callee.span,
                        });
                    };
                    let ir_args = args
                        .iter()
                        .map(|arg| self.lower_value(arg).map(|typed| typed.value))
                        .collect::<Result<Vec<_>, _>>()?;
                    let raw_resolved = self.raw_ret_identity(&metadata, expr.span)?;
                    return Ok(TypedValueIR {
                        value: ValueIR::CallRaw {
                            callee: Box::new(lowered_callee.value),
                            args: ir_args,
                            param_types: metadata.param_types,
                            ret_type: metadata.ret_type,
                        },
                        ty: metadata.ret_type,
                        resolved: raw_resolved,
                        ptr_array_bombom_size: None,
                    });
                };

                // Fase 242: variável local (parâmetro/`nova`) de tipo função
                // tem precedência sobre função top-level homônima — chamada
                // indireta real, callee é um valor (slot), não um símbolo.
                if let Some(binding) = self.resolve_existing_binding(name) {
                    if binding.ty == TypeIR::FunctionPointer {
                        let Some(metadata) = self.raw_function_metadata.get(&binding.slot).cloned()
                        else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering perdeu a assinatura do ponteiro cru de função '{}'",
                                    name
                                ),
                                span: expr.span,
                            });
                        };
                        let ir_args = args
                            .iter()
                            .map(|arg| self.lower_value(arg).map(|typed| typed.value))
                            .collect::<Result<Vec<_>, _>>()?;
                        let raw_resolved = self.raw_ret_identity(&metadata, expr.span)?;
                        return Ok(TypedValueIR {
                            value: ValueIR::CallRaw {
                                callee: Box::new(ValueIR::Local(binding.slot)),
                                args: ir_args,
                                param_types: metadata.param_types,
                                ret_type: metadata.ret_type,
                            },
                            ty: metadata.ret_type,
                            resolved: raw_resolved,
                            ptr_array_bombom_size: None,
                        });
                    }
                    if binding.ty == TypeIR::Function {
                        let Some(metadata) = self.callable_metadata.get(&binding.slot).cloned()
                        else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering falhou ao inferir retorno da chamada indireta de '{}' (encadeamento de callable retornando callable além de um nível não é suportado nesta fase)",
                                    name
                                ),
                                span: expr.span,
                            });
                        };
                        if metadata.ret_type == TypeIR::TraitObject
                            && metadata.ret_trait_name.is_none()
                        {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering perdeu a identidade nominal do trato retornado pela chamada indireta de '{}'",
                                    name
                                ),
                                span: expr.span,
                            });
                        }
                        let typed_args: Vec<TypedValueIR> = args
                            .iter()
                            .map(|arg| self.lower_value(arg))
                            .collect::<Result<Vec<_>, _>>()?;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|typed| typed.value).collect();
                        let resolved = self.callable_ret_identity(&metadata, expr.span)?;
                        return Ok(TypedValueIR {
                            value: ValueIR::CallIndirect {
                                callee: Box::new(ValueIR::Local(binding.slot)),
                                args: ir_args,
                                ret_type: metadata.ret_type,
                            },
                            ty: metadata.ret_type,
                            resolved,
                            ptr_array_bombom_size: None,
                        });
                    }
                }

                // Intrínsecas genéricas de lista (Fase 211): abaixam para a
                // forma monomorphizada conforme o tipo da lista no argumento 1.
                if matches!(
                    name.as_str(),
                    "lista_tamanho"
                        | "lista_obter"
                        | "lista_anexar"
                        | "lista_definir"
                        | "lista_tirar_ultimo"
                        | "lista_inserir"
                ) {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let prefix = match typed_args.first().map(|arg| arg.ty) {
                        Some(TypeIR::ListVerso) => "lista_verso",
                        _ => "lista_bombom",
                    };
                    let suffix = name.strip_prefix("lista").unwrap_or_default();
                    let mono_name = format!("{}{}", prefix, suffix);
                    let element_identity =
                        if matches!(name.as_str(), "lista_obter" | "lista_tirar_ultimo") {
                            typed_args.first().and_then(|list| {
                                list.resolved.and_then(|identity| {
                                    self.context
                                        .resolved_types
                                        .borrow()
                                        .get(identity)
                                        .and_then(|entry| entry.element)
                                })
                            })
                        } else {
                            None
                        };
                    let ret_type = self
                        .context
                        .function_sigs
                        .get(&mono_name)
                        .map(|sig| sig.ret_type)
                        .ok_or_else(|| PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver intrínseca genérica '{}' ('{}')",
                                name, mono_name
                            ),
                            span: expr.span,
                        })?;
                    let ir_args: Vec<ValueIR> =
                        typed_args.into_iter().map(|typed| typed.value).collect();
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            callee: mono_name,
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        resolved: element_identity,
                        ptr_array_bombom_size: None,
                    });
                }

                if matches!(
                    name.as_str(),
                    "mapa_definir" | "mapa_obter" | "mapa_tem" | "mapa_tamanho" | "mapa_remover"
                ) {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let Some(first_arg) = typed_args.first() else {
                        return Err(PinkerError::Ir {
                            msg: format!(
                                "lowering falhou ao resolver intrínseca genérica '{}' sem mapa",
                                name
                            ),
                            span: expr.span,
                        });
                    };
                    if let Some(mono_name) = generic_map_monomorphic_callee(first_arg.ty, name) {
                        let ret_type = self
                            .context
                            .function_sigs
                            .get(mono_name)
                            .map(|sig| sig.ret_type)
                            .ok_or_else(|| PinkerError::Ir {
                                msg: format!(
                                    "lowering falhou ao resolver intrínseca genérica '{}' ('{}')",
                                    name, mono_name
                                ),
                                span: expr.span,
                            })?;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|typed| typed.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::Call {
                                callee: mono_name.to_string(),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            resolved: None,
                            ptr_array_bombom_size: None,
                        });
                    }
                    if let TypeIR::Map { value, .. } = first_arg.ty {
                        let value_identity = if name == "mapa_obter" {
                            let map_identity = first_arg.identity(self.context, expr.span)?;
                            let table = self.context.resolved_types.borrow();
                            let map_entry =
                                table.get(map_identity).ok_or_else(|| PinkerError::Ir {
                                    msg: format!(
                                        "identidade {} do mapa genérico ausente no lowering",
                                        map_identity.0
                                    ),
                                    span: expr.span,
                                })?;
                            Some(map_entry.element.ok_or_else(|| PinkerError::Ir {
                                msg: "identidade do valor de mapa genérico perdida antes de mapa_obter"
                                    .to_string(),
                                span: expr.span,
                            })?)
                        } else {
                            None
                        };
                        let ret_type = match name.as_str() {
                            "mapa_obter" => value.type_ir(),
                            "mapa_tem" => TypeIR::Logica,
                            "mapa_tamanho" => TypeIR::Bombom,
                            "mapa_definir" | "mapa_remover" => TypeIR::Nulo,
                            _ => unreachable!(),
                        };
                        let ir_args = typed_args.into_iter().map(|typed| typed.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::Call {
                                callee: format!("__pinker_internal_{name}"),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            resolved: value_identity,
                            ptr_array_bombom_size: None,
                        });
                    }
                }

                if matches!(
                    name.as_str(),
                    "__pinker_internal_mapa_iterador_criar"
                        | "__pinker_internal_mapa_iterador_proxima_chave_bombom"
                        | "__pinker_internal_mapa_iterador_proxima_chave_verso"
                ) {
                    let typed_args = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let ret_type = if name.ends_with("_verso") {
                        TypeIR::Verso
                    } else {
                        TypeIR::Bombom
                    };
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            callee: name.clone(),
                            args: typed_args.into_iter().map(|typed| typed.value).collect(),
                            ret_type,
                        },
                        ty: ret_type,
                        resolved: None,
                        ptr_array_bombom_size: None,
                    });
                }

                // `formatar_verso` (Fase 219/B8): argumentos `bombom` são
                // convertidos para verso já na IR (mesmo texto que o
                // interpretador produziria), permitindo que o runtime nativo
                // trate todos os argumentos uniformemente como versos.
                if name == "formatar_verso" {
                    let mut ir_args = Vec::with_capacity(args.len());
                    for (idx, arg) in args.iter().enumerate() {
                        let typed = self.lower_value(arg)?;
                        if idx > 0 && typed.ty != TypeIR::Verso {
                            ir_args.push(ValueIR::Call {
                                callee: "bombom_para_verso".to_string(),
                                args: vec![typed.value],
                                ret_type: TypeIR::Verso,
                            });
                        } else {
                            ir_args.push(typed.value);
                        }
                    }
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            callee: name.clone(),
                            args: ir_args,
                            ret_type: TypeIR::Verso,
                        },
                        ty: TypeIR::Verso,
                        resolved: None,
                        ptr_array_bombom_size: None,
                    });
                }

                if name == "__ternario" {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let ret_type = typed_args[1].ty;
                    // Os dois ramos precisam concordar na identidade semântica,
                    // não apenas na representação: um ternário entre dois
                    // `ninho` diferentes (ou dois `leque` diferentes) não pode
                    // produzir um valor de identidade indeterminada.
                    // Ternário de callables tem contrato próprio (Fase 244): a
                    // concordância dos dois braços é exigida pela metadata de
                    // callable, com diagnósticos específicos. Aqui a identidade
                    // apenas acompanha o primeiro braço para não antecipar (e
                    // mascarar) aqueles diagnósticos.
                    if ret_type == TypeIR::Function {
                        let resolved = typed_args[1].resolved;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|t| t.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::Call {
                                callee: name.clone(),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            resolved,
                            ptr_array_bombom_size: None,
                        });
                    }
                    let resolved = match (typed_args[1].resolved, typed_args[2].resolved) {
                        (Some(left), Some(right)) => {
                            if left != right {
                                return Err(PinkerError::Ir {
                                    msg: format!(
                                        "E-IR-TYPE-IDENTITY-LOST: ramos do ternário têm \
                                         identidades resolvidas distintas ({} e {})",
                                        left.0, right.0
                                    ),
                                    span: expr.span,
                                });
                            }
                            Some(left)
                        }
                        (None, None) => None,
                        (Some(known), None) | (None, Some(known)) => {
                            // O ramo sem identidade explícita só é aceito quando
                            // sua representação já é a identidade completa e
                            // coincide com a do outro ramo.
                            let derived = self.context.repr_identity(ret_type, expr.span)?;
                            if derived != known {
                                return Err(PinkerError::Ir {
                                    msg: "E-IR-TYPE-IDENTITY-LOST: ramos do ternário não \
                                          concordam na identidade resolvida"
                                        .to_string(),
                                    span: expr.span,
                                });
                            }
                            Some(known)
                        }
                    };
                    let ir_args: Vec<ValueIR> = typed_args.into_iter().map(|t| t.value).collect();
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            callee: name.clone(),
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        resolved,
                        ptr_array_bombom_size: None,
                    });
                }

                let args = args
                    .iter()
                    .map(|arg| self.lower_value(arg).map(|typed| typed.value))
                    .collect::<Result<Vec<_>, _>>()?;

                let ret_type = self
                    .context
                    .function_sigs
                    .get(name)
                    .map(|sig| sig.ret_type)
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("lowering falhou ao resolver chamada '{}'", name),
                        span: expr.span,
                    })?;

                Ok(TypedValueIR {
                    value: ValueIR::Call {
                        callee: name.clone(),
                        args,
                        ret_type,
                    },
                    ty: ret_type,
                    resolved: self
                        .context
                        .function_sigs
                        .get(name)
                        .map(|sig| sig.ret_resolved),
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::FieldAccess { base, field } => {
                // `Leque.Variante`: em leque sem carga vira o discriminante
                // imediato; em leque com carga vira um handle recém-criado.
                if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(info) = self.context.enum_variants.get(base_name) {
                        let Some((discriminant, _)) = info.variants.get(field) else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "variante '{}' não existe no leque '{}'",
                                    field, base_name
                                ),
                                span: expr.span,
                            });
                        };
                        // O valor de uma variante **é** do leque de origem: a
                        // representação escalar não apaga a identidade nominal.
                        // Sem isto, dois `leque` distintos ficariam
                        // indistinguíveis na injeção em união (HR4).
                        let leque_identity = self.context.resolved_identity(&Type::Alias {
                            name: base_name.clone(),
                            span: base.span,
                        })?;
                        if info.has_payload {
                            return Ok(TypedValueIR {
                                value: ValueIR::Call {
                                    callee: "__pinker_internal_leque_criar_0".to_string(),
                                    args: vec![ValueIR::Int(*discriminant)],
                                    ret_type: TypeIR::Bombom,
                                },
                                ty: TypeIR::Bombom,
                                resolved: Some(leque_identity),
                                ptr_array_bombom_size: None,
                            });
                        }
                        return Ok(TypedValueIR {
                            value: ValueIR::Int(*discriminant),
                            ty: TypeIR::Bombom,
                            resolved: Some(leque_identity),
                            ptr_array_bombom_size: None,
                        });
                    }
                }
                let base = self.lower_value(base)?;
                let Some(base_struct_name) = self.nominal_name_of_value(&base) else {
                    return Err(PinkerError::Ir {
                        msg: "acesso a campo com base não-struct na IR".to_string(),
                        span: expr.span,
                    });
                };
                let base_struct_name = &base_struct_name;
                let result_type = self
                    .context
                    .struct_fields
                    .get(base_struct_name)
                    .and_then(|fields| fields.get(field))
                    .copied()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!("campo '{}' não encontrado em '{}'", field, base_struct_name),
                        span: expr.span,
                    })?;
                let field_offset = self
                    .context
                    .struct_field_offsets
                    .get(base_struct_name)
                    .and_then(|fields| fields.get(field))
                    .copied()
                    .ok_or_else(|| PinkerError::Ir {
                        msg: format!(
                            "offset de campo '{}' não encontrado no layout de '{}'",
                            field, base_struct_name
                        ),
                        span: expr.span,
                    })?;
                Ok(TypedValueIR {
                    value: ValueIR::FieldAccess {
                        base: Box::new(base.value),
                        field: field.clone(),
                        field_offset,
                        result_type,
                    },
                    ty: result_type,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Index { base, index } => {
                let base = self.lower_value(base)?;
                let index = self.lower_value(index)?;
                let TypeIR::FixedArray { element, .. } = base.ty else {
                    return Err(PinkerError::Ir {
                        msg: "indexação com base não-array na IR".to_string(),
                        span: expr.span,
                    });
                };
                let element_type = match element {
                    ScalarTypeIR::Bombom => TypeIR::Bombom,
                    ScalarTypeIR::U8 => TypeIR::U8,
                    ScalarTypeIR::U16 => TypeIR::U16,
                    ScalarTypeIR::U32 => TypeIR::U32,
                    ScalarTypeIR::U64 => TypeIR::U64,
                    ScalarTypeIR::I8 => TypeIR::I8,
                    ScalarTypeIR::I16 => TypeIR::I16,
                    ScalarTypeIR::I32 => TypeIR::I32,
                    ScalarTypeIR::I64 => TypeIR::I64,
                    ScalarTypeIR::Logica => TypeIR::Logica,
                };
                Ok(TypedValueIR {
                    value: ValueIR::Index {
                        base: Box::new(base.value),
                        index: Box::new(index.value),
                        element_type,
                    },
                    ty: element_type,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Cast {
                expr: source,
                target,
            } => {
                let lowered_source = self.lower_value(source)?;
                let target_type = self.context.resolve_type(target)?;

                if target_type == TypeIR::TraitObject {
                    let trait_name = trait_object_name_from_type(
                        target,
                        &self.context.type_aliases,
                        &self.context.struct_names,
                    )?
                    .ok_or_else(|| PinkerError::Ir {
                        msg: "materialização sem nome nominal de trato".to_string(),
                        span: expr.span,
                    })?;

                    let concrete_type_name =
                        self.impl_receiver_key(&lowered_source)
                            .ok_or_else(|| PinkerError::Ir {
                                msg: "materialização sem identidade do tipo concreto".to_string(),
                                span: expr.span,
                            })?;

                    let concrete_size = self.concrete_snapshot_size(&lowered_source, expr.span)?;

                    let vtable_methods =
                        self.trait_vtable(&trait_name, &concrete_type_name, expr.span)?;

                    // A identidade do objeto de trato é `trato<Nome>`: dois
                    // tratos diferentes compartilham `TypeIR::TraitObject` e não
                    // podem colapsar na mesma identidade.
                    let trait_object_identity = self.context.resolved_identity(&Type::Applied {
                        name: "trato".to_string(),
                        args: vec![Type::Alias {
                            name: trait_name.clone(),
                            span: expr.span,
                        }],
                        span: expr.span,
                    })?;

                    return Ok(TypedValueIR {
                        value: ValueIR::MakeTraitObject {
                            value: Box::new(lowered_source.value),
                            trait_name,
                            concrete_type: lowered_source.ty,
                            concrete_type_name,
                            concrete_size,
                            vtable_methods,
                        },
                        ty: TypeIR::TraitObject,
                        resolved: Some(trait_object_identity),
                        ptr_array_bombom_size: None,
                    });
                }

                if let TypeIR::Union(union_type_id) = target_type {
                    // A identidade semântica exata do valor de origem é
                    // obrigatória: sem ela não há injeção possível, e escolher
                    // um membro por representação ou por primeira ocorrência é
                    // exatamente o defeito HR4.
                    let source_identity = lowered_source.identity(self.context, expr.span)?;
                    let member = {
                        let registry = self.context.union_registry.borrow();
                        let union = registry
                            .types
                            .iter()
                            .find(|union| union.id == union_type_id)
                            .ok_or_else(|| PinkerError::Ir {
                                msg: "injeção perdeu o registro da união".to_string(),
                                span: expr.span,
                            })?;
                        let mut exact = union
                            .members
                            .iter()
                            .filter(|member| member.resolved_type_id == source_identity);
                        let member = exact.next().cloned().ok_or_else(|| {
                            // Nenhum membro tem esta identidade. Se existe um
                            // membro com a mesma representação, o diagnóstico
                            // aponta a confusão entre categoria e identidade em
                            // vez de aceitar o candidato aproximado.
                            let operational = union
                                .members
                                .iter()
                                .any(|member| member.ty == lowered_source.ty);
                            let key = self
                                .context
                                .resolved_types
                                .borrow()
                                .key_of(source_identity)
                                .unwrap_or("<desconhecida>")
                                .to_string();
                            if operational {
                                PinkerError::Ir {
                                    msg: format!(
                                        "E-IR-UNION-MEMBER-IDENTITY-MISMATCH: a união {} possui \
                                         membro com a representação '{}', mas nenhum com a \
                                         identidade '{key}'",
                                        union_type_id.0,
                                        lowered_source.ty.name()
                                    ),
                                    span: expr.span,
                                }
                            } else {
                                PinkerError::Ir {
                                    msg: format!(
                                        "tipo fonte de identidade '{key}' não pertence à união {} \
                                         durante o lowering",
                                        union_type_id.0
                                    ),
                                    span: expr.span,
                                }
                            }
                        })?;
                        if let Some(duplicate) = exact.next() {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "E-IR-UNION-IDENTITY-DUPLICATE: a união {} tem a identidade \
                                     resolvida {} nas tags {} e {}",
                                    union_type_id.0, source_identity.0, member.tag, duplicate.tag
                                ),
                                span: expr.span,
                            });
                        }
                        member
                    };
                    return Ok(TypedValueIR {
                        value: ValueIR::UnionInject {
                            value: Box::new(lowered_source.value),
                            union_type_id,
                            // A tag é **copiada** do membro exato; nenhuma camada
                            // posterior torna a escolher membro.
                            tag: member.tag,
                            resolved_member_type_id: member.resolved_type_id,
                            canonical_member_key: member.canonical_member_key.clone(),
                            payload_type: member.ty,
                            payload_layout: member.payload_layout,
                        },
                        ty: target_type,
                        resolved: Some(self.context.repr_identity(target_type, expr.span)?),
                        ptr_array_bombom_size: None,
                    });
                }

                Ok(TypedValueIR {
                    value: ValueIR::Cast {
                        value: Box::new(lowered_source.value),
                        target_type,
                    },
                    ty: target_type,
                    // O cast continua sem fabricar proveniência; esta identidade
                    // descreve somente o tipo-alvo e permite que uma operação
                    // tipada posterior recupere o layout de `seta<T>`.
                    resolved: Some(self.context.resolved_identity(target)?),
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::SizeOfType { target } => {
                let layout = layout::layout_of_type(
                    target,
                    &self.context.type_aliases,
                    &self.context.struct_decls,
                )
                .map_err(|msg| PinkerError::Ir {
                    msg: format!("consulta de peso inválida na IR: {}", msg),
                    span: expr.span,
                })?;
                Ok(TypedValueIR {
                    value: ValueIR::Int(layout.size),
                    ty: TypeIR::Bombom,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::AlignOfType { target } => {
                let layout = layout::layout_of_type(
                    target,
                    &self.context.type_aliases,
                    &self.context.struct_decls,
                )
                .map_err(|msg| PinkerError::Ir {
                    msg: format!("consulta de alinhamento inválida na IR: {}", msg),
                    span: expr.span,
                })?;
                Ok(TypedValueIR {
                    value: ValueIR::Int(layout.align),
                    ty: TypeIR::Bombom,
                    resolved: None,
                    ptr_array_bombom_size: None,
                })
            }
        }
    }

    // @pinker-nav:end ir.lowering.expressoes-valores

    // @pinker-nav:start ir.lowering.bindings-escopos
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Normalização de nomes-fonte em slots e gestão de escopos léxicos: `allocate_binding` gera `%nome#N` (contador por nome-fonte), registra o binding no escopo atual e coleta `LocalIR`; a resolução sobe a pilha de escopos; e os rótulos de bloco/laço são gerados aqui. Slots são nomes normalizados desta camada — não são SSA nem registradores físicos de máquina.
    fn allocate_binding(
        &mut self,
        source_name: &str,
        ty: TypeIR,
        resolved: Option<ResolvedTypeId>,
        ptr_array_bombom_size: Option<u64>,
        is_mut: Option<bool>,
    ) -> Result<BindingIR, PinkerError> {
        // O slot transporta a identidade que o chamador determinou. `None`
        // segue a convenção de [`TypedValueIR::resolved`]: a identidade é a da
        // própria representação e é internada sob demanda. A exigência de
        // identidade exata é cobrada nos pontos que a **consomem** — injeção em
        // união, concordância de ramos e acesso nominal —, e não na alocação do
        // slot, para que nenhuma dessas checagens possa ser satisfeita por uma
        // identidade fabricada só para preencher o campo.
        let slot_identity = resolved;

        let next = self
            .slot_counters
            .entry(source_name.to_string())
            .or_insert(0);
        let slot = format!("%{}#{}", source_name, *next);
        *next += 1;

        let binding = BindingIR {
            source_name: source_name.to_string(),
            slot: slot.clone(),
            ty,
            resolved: slot_identity,
        };

        self.scopes.last_mut().unwrap().insert(
            source_name.to_string(),
            BindingState {
                slot: slot.clone(),
                ty,
                resolved: slot_identity,
                ptr_array_bombom_size,
            },
        );

        if let Some(is_mut) = is_mut {
            self.locals.push(LocalIR {
                source_name: source_name.to_string(),
                slot,
                ty,
                resolved: slot_identity,
                is_mut,
            });
        }

        Ok(binding)
    }

    fn resolve_binding(&self, source_name: &str, span: Span) -> Result<BindingState, PinkerError> {
        self.resolve_existing_binding(source_name)
            .ok_or_else(|| PinkerError::Ir {
                msg: format!("lowering falhou ao resolver variável '{}'", source_name),
                span,
            })
    }

    fn resolve_existing_binding(&self, source_name: &str) -> Option<BindingState> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(source_name).cloned())
    }

    fn next_block_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.block_counter);
        self.block_counter += 1;
        label
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    // @pinker-nav:end ir.lowering.bindings-escopos
}

// @pinker-nav:start ir.lowering.constantes
// @pinker-nav:domain lowering
// @pinker-nav:layer ir
// @pinker-nav:summary Abaixa uma constante global: cria um `FunctionLowerer` mínimo para o inicializador, abaixa o valor e o tipo declarado e monta `ConstIR`. Consome o contexto já preparado; não valida o inicializador (a semântica já o fez).
fn lower_const(const_decl: &ConstDecl, context: &LoweringContext) -> Result<ConstIR, PinkerError> {
    let mut lowerer = FunctionLowerer::new(context);
    let value = lowerer.lower_value(&const_decl.init)?;
    Ok(ConstIR {
        name: const_decl.name.clone(),
        ty: context.resolve_type(&const_decl.ty)?,
        value: value.value,
        span: const_decl.span,
    })
}
// @pinker-nav:end ir.lowering.constantes

// `resolve_struct_name_from_type` foi removida: derivar identidade de um nome
// textual de `ninho` (e, no caso de `seta<Ninho>`, do nome do apontado) era
// justamente a autoridade paralela que HR4 descreve. A identidade agora vem de
// `LoweringContext::resolved_identity` e o nome nominal, quando necessário, é
// consultado na tabela de identidades resolvidas.

fn pointer_to_bombom_array_size(ty: &Type, aliases: &HashMap<String, Type>) -> Option<u64> {
    match ty {
        Type::Pointer { base, .. } => match base.as_ref() {
            Type::FixedArray { element, size, .. }
                if matches!(element.as_ref(), Type::Bombom(_)) =>
            {
                Some(*size)
            }
            Type::Alias { name, .. } => aliases
                .get(name)
                .and_then(|target| pointer_to_bombom_array_size(target, aliases)),
            _ => None,
        },
        Type::Alias { name, .. } => aliases
            .get(name)
            .and_then(|target| pointer_to_bombom_array_size(target, aliases)),
        _ => None,
    }
}

// @pinker-nav:start ir.renderizacao.textual
// @pinker-nav:domain renderizacao
// @pinker-nav:layer ir
// @pinker-nav:summary Renderização textual auditável da IR já construída: `render_function`/`render_block`/`render_instruction`/`render_value` (com o helper `line`) percorrem `FunctionIR`/`BlockIR`/`InstructionIR`/`ValueIR` e produzem a forma legível consumida por depuração e testes. Recebe uma `ProgramIR` pronta (a entrada pública `render_program` fica junto à orquestração e delega a estas funções); não modifica a IR, não valida invariantes, não executa e não gera assembly.
fn render_function(function: &FunctionIR, indent: usize, out: &mut String) {
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

fn render_value(value: &ValueIR) -> String {
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

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
// @pinker-nav:end ir.renderizacao.textual

impl TypeIR {
    /// Quantidade de palavras da representação já transportável pela ABI
    /// nativa atual. Arrays fixos permanecem valores inline multi-palavra;
    /// `nulo` não é valor. Todas as demais categorias são escalares ou
    /// handles/ponteiros opacos de uma palavra.
    pub fn native_abi_words(&self) -> Option<usize> {
        match self {
            TypeIR::FixedArray { .. } => None,
            TypeIR::Nulo => Some(0),
            _ => Some(1),
        }
    }

    pub fn is_native_abi_word(&self) -> bool {
        self.native_abi_words() == Some(1)
    }

    /// Valores que podem ser copiados diretamente para uma palavra do
    /// ambiente de closure. `Struct` continua sendo valor agregado por
    /// valor, ainda que alguns limites da ABI o transportem por endereço.
    pub fn is_closure_environment_word(&self) -> bool {
        self.is_native_abi_word() && !matches!(self, TypeIR::Struct)
    }

    pub fn is_unsigned(&self) -> bool {
        matches!(
            self,
            TypeIR::Bombom | TypeIR::U8 | TypeIR::U16 | TypeIR::U32 | TypeIR::U64
        )
    }

    pub fn is_signed(&self) -> bool {
        matches!(self, TypeIR::I8 | TypeIR::I16 | TypeIR::I32 | TypeIR::I64)
    }

    pub fn is_integer(&self) -> bool {
        self.is_unsigned() || self.is_signed()
    }

    pub fn is_compatible_with(&self, other: TypeIR) -> bool {
        *self == other
            || ((*self == TypeIR::Bombom && other == TypeIR::U64)
                || (*self == TypeIR::U64 && other == TypeIR::Bombom))
    }

    // @pinker-nav:start ir.tipos.conversao-ast
    // @pinker-nav:domain tipos
    // @pinker-nav:layer ir
    // @pinker-nav:summary Converte tipos AST semanticamente válidos em `TypeIR`: resolve aliases (com detecção de recursão), reduz leques a `bombom` (discriminante/handle), reduz listas de leque a `lista<bombom>`, converte primitivos, listas/mapas, arrays fixos (via `ScalarTypeIR`), ponteiros (com volatilidade) e structs, e recusa tipo função materializável ou genérico não monomorfizado. Conversão mecânica que respeita os limites de materialização da IR; não reexecuta a checagem semântica de tipos.
    fn from_ast_inner(
        ty: &Type,
        aliases: &HashMap<String, Type>,
        struct_names: &HashSet<String>,
        resolving: &mut Vec<String>,
    ) -> Result<Self, PinkerError> {
        match ty {
            Type::Bombom(_) => Ok(TypeIR::Bombom),
            Type::U8(_) => Ok(TypeIR::U8),
            Type::U16(_) => Ok(TypeIR::U16),
            Type::U32(_) => Ok(TypeIR::U32),
            Type::U64(_) => Ok(TypeIR::U64),
            Type::I8(_) => Ok(TypeIR::I8),
            Type::I16(_) => Ok(TypeIR::I16),
            Type::I32(_) => Ok(TypeIR::I32),
            Type::I64(_) => Ok(TypeIR::I64),
            Type::Logica(_) => Ok(TypeIR::Logica),
            Type::Verso(_) => Ok(TypeIR::Verso),
            Type::ListBombom(_) => Ok(TypeIR::ListBombom),
            Type::ListVerso(_) => Ok(TypeIR::ListVerso),
            // Elementos de leque são bombom na IR (discriminante ou handle);
            // a lista genérica reaproveita o runtime de lista<bombom>.
            Type::ListEnum { .. } => Ok(TypeIR::ListBombom),
            Type::MapVersoBombom(_) => Ok(TypeIR::MapVersoBombom),
            Type::MapVersoVerso(_) => Ok(TypeIR::MapVersoVerso),
            Type::MapBombomBombom(_) => Ok(TypeIR::MapBombomBombom),
            Type::MapBombomVerso(_) => Ok(TypeIR::MapBombomVerso),
            Type::Map {
                key, value, span, ..
            } => {
                let key = match Self::from_ast_inner(key, aliases, struct_names, resolving)? {
                    TypeIR::Bombom => MapKeyIR::Bombom,
                    TypeIR::Verso => MapKeyIR::Verso,
                    _ => {
                        return Err(PinkerError::Ir {
                            msg: "tipo de chave de mapa genérico escapou da validação semântica"
                                .to_string(),
                            span: *span,
                        })
                    }
                };
                let value_ty = Self::from_ast_inner(value, aliases, struct_names, resolving)?;
                let value = MapValueIR::from_type_ir(value_ty).ok_or_else(|| PinkerError::Ir {
                    msg: "representação de valor de mapa genérico escapou da validação semântica"
                        .to_string(),
                    span: *span,
                })?;
                Ok(TypeIR::Map { key, value })
            }
            // Tipos leque são nominais apenas na semântica; na IR o valor é o
            // discriminante inteiro.
            Type::Enum { .. } => Ok(TypeIR::Bombom),
            Type::Union { .. } => Ok(TypeIR::Union(UnionTypeId(0))),
            Type::FixedArray {
                element,
                size,
                span,
            } => {
                let resolved_element =
                    Self::from_ast_inner(element, aliases, struct_names, resolving)?;
                let element = ScalarTypeIR::from_type_ir(resolved_element).ok_or_else(|| {
                    PinkerError::Ir {
                        msg: "array fixo aninhado ainda não é suportado nesta fase".to_string(),
                        span: *span,
                    }
                })?;
                Ok(TypeIR::FixedArray {
                    element,
                    size: *size,
                })
            }
            Type::Pointer {
                base,
                is_volatile,
                span,
            } => {
                let resolved_base = Self::from_ast_inner(base, aliases, struct_names, resolving)?;
                if resolved_base == TypeIR::Nulo {
                    return Err(PinkerError::Ir {
                        msg: "tipo base de 'seta' não pode ser 'nulo'".to_string(),
                        span: *span,
                    });
                }
                if matches!(resolved_base, TypeIR::Pointer { .. }) {
                    return Err(PinkerError::Ir {
                        msg: "seta de seta ainda não é suportada nesta fase".to_string(),
                        span: *span,
                    });
                }
                if resolved_base == TypeIR::Function {
                    return Ok(TypeIR::FunctionPointer);
                }
                Ok(TypeIR::Pointer {
                    is_volatile: *is_volatile,
                })
            }
            // Fase 242: tipo função materializado como handle callable de 1
            // palavra (mesma categoria de Pointer/handle).
            Type::Function { .. } => Ok(TypeIR::Function),
            Type::Applied { name, args, span } if name == "trato" => match args.as_slice() {
                [Type::Alias { .. }] => Ok(TypeIR::TraitObject),
                _ => Err(PinkerError::Ir {
                    msg: "tipo de objeto de trato inválido antes da IR".to_string(),
                    span: *span,
                }),
            },
            Type::Applied { span, .. } => Err(PinkerError::Ir {
                msg: "tipo genérico aplicado não monomorfizado antes da IR".to_string(),
                span: *span,
            }),
            Type::Nulo(_) => Ok(TypeIR::Nulo),
            Type::Struct { .. } => Ok(TypeIR::Struct),
            Type::OpaqueHandle { .. } => Ok(TypeIR::OpaqueWordHandle),
            Type::Alias { name, span } => {
                if struct_names.contains(name) {
                    return Ok(TypeIR::Struct);
                }
                if resolving.iter().any(|current| current == name) {
                    return Err(PinkerError::Ir {
                        msg: format!("alias de tipo recursivo detectado em '{}'", name),
                        span: *span,
                    });
                }
                let Some(target) = aliases.get(name) else {
                    return Err(PinkerError::Ir {
                        msg: format!("tipo '{}' não existe", name),
                        span: *span,
                    });
                };
                resolving.push(name.clone());
                let resolved = Self::from_ast_inner(target, aliases, struct_names, resolving);
                resolving.pop();
                resolved
            }
        }
    }

    pub fn from_ast_with_context(
        ty: &Type,
        aliases: &HashMap<String, Type>,
        struct_names: &HashSet<String>,
    ) -> Result<Self, PinkerError> {
        Self::from_ast_inner(ty, aliases, struct_names, &mut Vec::new())
    }

    pub fn from_ast_option_with_context(
        ty: Option<&Type>,
        aliases: &HashMap<String, Type>,
        struct_names: &HashSet<String>,
    ) -> Result<Self, PinkerError> {
        ty.map(|ty| Self::from_ast_with_context(ty, aliases, struct_names))
            .transpose()
            .map(|resolved| resolved.unwrap_or(TypeIR::Nulo))
    }
    // @pinker-nav:end ir.tipos.conversao-ast

    pub fn name(&self) -> &'static str {
        match self {
            TypeIR::Bombom => "bombom",
            TypeIR::U8 => "u8",
            TypeIR::U16 => "u16",
            TypeIR::U32 => "u32",
            TypeIR::U64 => "u64",
            TypeIR::I8 => "i8",
            TypeIR::I16 => "i16",
            TypeIR::I32 => "i32",
            TypeIR::I64 => "i64",
            TypeIR::Logica => "logica",
            TypeIR::Verso => "verso",
            TypeIR::ListBombom => "lista<bombom>",
            TypeIR::ListVerso => "lista<verso>",
            TypeIR::MapVersoBombom => "mapa<verso,bombom>",
            TypeIR::MapVersoVerso => "mapa<verso,verso>",
            TypeIR::MapBombomBombom => "mapa<bombom,bombom>",
            TypeIR::MapBombomVerso => "mapa<bombom,verso>",
            TypeIR::Map { .. } => "mapa",
            TypeIR::FixedArray { .. } => "array",
            TypeIR::Struct => "struct",
            TypeIR::OpaqueWordHandle => "handle opaco",
            TypeIR::Pointer { .. } => "seta",
            TypeIR::Function => "carinho",
            TypeIR::FunctionPointer => "seta<carinho>",
            TypeIR::TraitObject => "trato",
            TypeIR::Union(_) => "uniao",
            TypeIR::Nulo => "nulo",
        }
    }

    pub fn render_name(&self) -> String {
        match self {
            TypeIR::FixedArray { element, size } => {
                format!("[{}; {}]", element.name(), size)
            }
            TypeIR::Pointer { is_volatile } => {
                if *is_volatile {
                    "fragil seta<?>".to_string()
                } else {
                    "seta<?>".to_string()
                }
            }
            TypeIR::Struct => "struct".to_string(),
            TypeIR::OpaqueWordHandle => "handle opaco".to_string(),
            TypeIR::TraitObject => "trato<?>".to_string(),
            TypeIR::Union(id) => format!("uniao#{}", id.0),
            TypeIR::ListBombom => "lista<bombom>".to_string(),
            TypeIR::ListVerso => "lista<verso>".to_string(),
            TypeIR::MapVersoBombom => "mapa<verso,bombom>".to_string(),
            TypeIR::MapVersoVerso => "mapa<verso,verso>".to_string(),
            TypeIR::MapBombomBombom => "mapa<bombom,bombom>".to_string(),
            TypeIR::MapBombomVerso => "mapa<bombom,verso>".to_string(),
            TypeIR::Map { key, value } => format!(
                "mapa<{},{}>",
                match key {
                    MapKeyIR::Bombom => "bombom",
                    MapKeyIR::Verso => "verso",
                },
                value.type_ir().name()
            ),
            _ => self.name().to_string(),
        }
    }
}

impl ScalarTypeIR {
    fn from_type_ir(ty: TypeIR) -> Option<Self> {
        match ty {
            TypeIR::Bombom => Some(ScalarTypeIR::Bombom),
            TypeIR::U8 => Some(ScalarTypeIR::U8),
            TypeIR::U16 => Some(ScalarTypeIR::U16),
            TypeIR::U32 => Some(ScalarTypeIR::U32),
            TypeIR::U64 => Some(ScalarTypeIR::U64),
            TypeIR::I8 => Some(ScalarTypeIR::I8),
            TypeIR::I16 => Some(ScalarTypeIR::I16),
            TypeIR::I32 => Some(ScalarTypeIR::I32),
            TypeIR::I64 => Some(ScalarTypeIR::I64),
            TypeIR::Logica => Some(ScalarTypeIR::Logica),
            TypeIR::Verso
            | TypeIR::ListBombom
            | TypeIR::ListVerso
            | TypeIR::MapVersoBombom
            | TypeIR::MapVersoVerso
            | TypeIR::MapBombomBombom
            | TypeIR::MapBombomVerso
            | TypeIR::Map { .. }
            | TypeIR::FixedArray { .. }
            | TypeIR::Union(_)
            | TypeIR::Struct
            | TypeIR::OpaqueWordHandle
            | TypeIR::Pointer { .. }
            | TypeIR::Function
            | TypeIR::FunctionPointer
            | TypeIR::TraitObject
            | TypeIR::Nulo => None,
        }
    }

    /// Representação operacional equivalente, para reentrar na internação de
    /// identidade dos elementos de `array`.
    fn to_type_ir(self) -> TypeIR {
        match self {
            ScalarTypeIR::Bombom => TypeIR::Bombom,
            ScalarTypeIR::U8 => TypeIR::U8,
            ScalarTypeIR::U16 => TypeIR::U16,
            ScalarTypeIR::U32 => TypeIR::U32,
            ScalarTypeIR::U64 => TypeIR::U64,
            ScalarTypeIR::I8 => TypeIR::I8,
            ScalarTypeIR::I16 => TypeIR::I16,
            ScalarTypeIR::I32 => TypeIR::I32,
            ScalarTypeIR::I64 => TypeIR::I64,
            ScalarTypeIR::Logica => TypeIR::Logica,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ScalarTypeIR::Bombom => "bombom",
            ScalarTypeIR::U8 => "u8",
            ScalarTypeIR::U16 => "u16",
            ScalarTypeIR::U32 => "u32",
            ScalarTypeIR::U64 => "u64",
            ScalarTypeIR::I8 => "i8",
            ScalarTypeIR::I16 => "i16",
            ScalarTypeIR::I32 => "i32",
            ScalarTypeIR::I64 => "i64",
            ScalarTypeIR::Logica => "logica",
        }
    }
}

impl UnaryOpIR {
    fn from_ast(op: UnaryOp) -> Self {
        match op {
            UnaryOp::Neg => UnaryOpIR::Neg,
            UnaryOp::Not => UnaryOpIR::Not,
            UnaryOp::BitNot => UnaryOpIR::BitNot,
            UnaryOp::Deref => UnaryOpIR::Deref,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            UnaryOpIR::Neg => "neg",
            UnaryOpIR::Not => "not",
            UnaryOpIR::BitNot => "bitnot",
            UnaryOpIR::Deref => "deref",
        }
    }
}

impl BinaryOpIR {
    fn from_ast(op: BinaryOp) -> Self {
        match op {
            BinaryOp::LogicalAnd => BinaryOpIR::LogicalAnd,
            BinaryOp::LogicalOr => BinaryOpIR::LogicalOr,
            BinaryOp::BitAnd => BinaryOpIR::BitAnd,
            BinaryOp::BitOr => BinaryOpIR::BitOr,
            BinaryOp::BitXor => BinaryOpIR::BitXor,
            BinaryOp::Shl => BinaryOpIR::Shl,
            BinaryOp::Shr => BinaryOpIR::Shr,
            BinaryOp::Add => BinaryOpIR::Add,
            BinaryOp::Sub => BinaryOpIR::Sub,
            BinaryOp::Mul => BinaryOpIR::Mul,
            BinaryOp::Div => BinaryOpIR::Div,
            BinaryOp::Mod => BinaryOpIR::Mod,
            BinaryOp::Eq => BinaryOpIR::Eq,
            BinaryOp::Neq => BinaryOpIR::Neq,
            BinaryOp::Lt => BinaryOpIR::Lt,
            BinaryOp::Lte => BinaryOpIR::Lte,
            BinaryOp::Gt => BinaryOpIR::Gt,
            BinaryOp::Gte => BinaryOpIR::Gte,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            BinaryOpIR::LogicalAnd => "and",
            BinaryOpIR::LogicalOr => "or",
            BinaryOpIR::BitAnd => "bitand",
            BinaryOpIR::BitOr => "bitor",
            BinaryOpIR::BitXor => "bitxor",
            BinaryOpIR::Shl => "shl",
            BinaryOpIR::Shr => "shr",
            BinaryOpIR::Add => "add",
            BinaryOpIR::Sub => "sub",
            BinaryOpIR::Mul => "mul",
            BinaryOpIR::Div => "div",
            BinaryOpIR::Mod => "mod",
            BinaryOpIR::Eq => "eq",
            BinaryOpIR::Neq => "neq",
            BinaryOpIR::Lt => "lt",
            BinaryOpIR::Lte => "lte",
            BinaryOpIR::Gt => "gt",
            BinaryOpIR::Gte => "gte",
        }
    }
}

#[cfg(test)]
mod trait_object_alias_tests {
    use super::*;
    use crate::token::Position;

    fn span() -> Span {
        Span::new(Position::new(1, 1), Position::new(1, 1))
    }

    fn alias(name: &str) -> Type {
        Type::Alias {
            name: name.to_string(),
            span: span(),
        }
    }

    fn trait_object(name: &str) -> Type {
        Type::Applied {
            name: "trato".to_string(),
            args: vec![alias(name)],
            span: span(),
        }
    }

    #[test]
    fn trait_object_name_resolve_aliases_externos_sem_resolver_nome_interno() {
        let aliases = HashMap::from([
            ("ObjetoBase".to_string(), trait_object("Medivel")),
            ("ObjetoPublico".to_string(), alias("ObjetoBase")),
            ("Numero".to_string(), Type::Bombom(span())),
        ]);
        let structs = HashSet::new();

        assert_eq!(
            trait_object_name_from_type(&trait_object("Medivel"), &aliases, &structs).unwrap(),
            Some("Medivel".to_string())
        );
        assert_eq!(
            trait_object_name_from_type(&alias("ObjetoBase"), &aliases, &structs).unwrap(),
            Some("Medivel".to_string())
        );
        assert_eq!(
            trait_object_name_from_type(&alias("ObjetoPublico"), &aliases, &structs).unwrap(),
            Some("Medivel".to_string())
        );
        assert_eq!(
            trait_object_name_from_type(&alias("Numero"), &aliases, &structs).unwrap(),
            None
        );
    }

    #[test]
    fn trait_object_name_rejeita_alias_ciclico_e_inexistente() {
        let aliases = HashMap::from([("A".to_string(), alias("B")), ("B".to_string(), alias("A"))]);
        let structs = HashSet::new();

        let ciclo = trait_object_name_from_type(&alias("A"), &aliases, &structs)
            .expect_err("ciclo não pode virar ausência silenciosa")
            .to_string();
        assert!(ciclo.contains("alias de tipo recursivo"));

        let ausente = trait_object_name_from_type(&alias("Ausente"), &aliases, &structs)
            .expect_err("alias ausente não pode virar ausência silenciosa")
            .to_string();
        assert!(ausente.contains("tipo 'Ausente' não existe"));
    }
}
