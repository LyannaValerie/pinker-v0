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
use crate::method_dispatch::{
    self, DispatchCandidate, DispatchRelation, MethodSelection, RepresentativeSelection,
};
use crate::method_identity::{self, MethodIdentity};
use crate::source_map::SourceId;
use crate::token::{Position, Span};
use crate::union_canon;
use std::collections::{BTreeMap, HashMap, HashSet};

mod context;
mod lowering;
mod render;

use render::{line, render_function, render_value};

pub use context::{lower_program, lower_program_composto};

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
        /// #532 — quem é o callee, decidido na resolução e não aqui.
        ///
        /// `callee` continua sendo a grafia: é ela que escolhe QUAL builtin, e
        /// é ela que o backend nativo usa como símbolo de função Pinker. O que
        /// ela deixou de decidir é SE a chamada é builtin — pergunta que as
        /// camadas a jusante faziam por conta própria, comparando a grafia com
        /// as próprias tabelas, e que respondia "sim" para uma função do
        /// usuário homônima.
        identidade: crate::intrinsics::identity::CalleeIdentity,
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
    /// #532 — assinaturas declaradas pelo PROGRAMA, sem as intrínsecas.
    ///
    /// `function_sigs` mistura os dois namespaces numa chave só e a entrada da
    /// intrínseca vencia (`or_insert`). Com a grafia canônica liberada, uma
    /// função do usuário homônima passava a ser tipada pela assinatura da
    /// intrínseca. Quem decide qual das duas responder é a identidade do
    /// callee, e esta é a metade que responde ao usuário.
    declared_sigs: HashMap<String, FunctionSigIR>,
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
    // Visão derivada da decisão semântica: a mesma identidade estruturada usa
    // aqui o `ResolvedTypeId` já internado, nunca o spelling do `__impl_*`.
    impl_methods: BTreeMap<MethodIdentity<ResolvedTypeId>, String>,
    /// #577 — unidade que DECLAROU cada relação `(trato canônico, alvo
    /// resolvido)`, pelo `SourceId` do próprio bloco `impl`. Mesma pergunta que
    /// a autoridade semântica responde, na identidade de alvo desta camada.
    fontes_das_relacoes: BTreeMap<(String, ResolvedTypeId), SourceId>,
    /// Tratos que cada unidade-fonte pode enxergar, por `SourceId`.
    ///
    /// Vazio quando não houve composição modular, e nesse caso nada é
    /// filtrado. O despacho não qualificado é por `(tipo do receiver, método)`,
    /// e essa visão é global sobre a agregação: sem restringi-la ao ambiente de
    /// quem escreveu a chamada, o lowering aceitaria um método que a autoridade
    /// semântica já tinha recusado — e, com duas candidatas, falharia depois de
    /// `--check` ter dito que o programa era válido.
    traits_visiveis_por_fonte: HashMap<SourceId, crate::module_resolve::TratosNoDespacho>,
    // Defaults redundantes produzidos provisoriamente para spellings
    // alias-equivalentes não são funções aceitas e não chegam à IR emitida.
    ignored_impl_functions: HashSet<String>,
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

/// #532 — a criação genérica é reconhecida pela IDENTIDADE do callee.
///
/// `lista.criar` e `mapa.criar` chegam aqui como identidade resolvida; uma
/// função do usuário com a mesma grafia é `Ident` e nunca satisfaz esta
/// pergunta.
fn chamada_intrinseca_sem_argumentos(expr: &Expr, canonica: &str) -> bool {
    let ExprKind::Call(callee, args) = &expr.kind else {
        return false;
    };
    let ExprKind::Intrinsic(identity) = &callee.kind else {
        return false;
    };
    identity.canonical_public_spelling() == canonica && args.is_empty()
}

fn is_generic_list_create_expr(expr: &Expr) -> bool {
    chamada_intrinseca_sem_argumentos(expr, "lista_criar")
}

fn is_generic_map_create_expr(expr: &Expr) -> bool {
    chamada_intrinseca_sem_argumentos(expr, "mapa_criar")
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
    fn resolve_type(&self, ty: &Type) -> Result<TypeIR, PinkerError> {
        let resolved = self.resolve_union_ast_type(ty, &mut Vec::new())?;
        if let Type::Union { members, span } = resolved {
            return self.intern_union(&members, span);
        }
        TypeIR::from_ast_with_context(ty, &self.type_aliases, &self.struct_names)
    }

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
