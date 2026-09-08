//! Modelo de dados da IR estruturada e identidade semântica resolvida de
//! tipos, movidos de `src/ir.rs` pela unidade IR-3 do inventário da #601
//! (Task #632).
//!
//! Só o arquivo mudou: as duas regiões cartografadas
//! `ir.modelo.representacao` e `ir.tipos.identidade-resolvida` — a
//! representação (`ProgramIR`, `ConstIR`, `FunctionIR`, `BlockIR`,
//! `InstructionIR`, `ValueIR`, `TypeIR`, `ScalarTypeIR`, os operadores e os
//! validadores de registro de união) e a identidade resolvida (`ResolvedTypeId`,
//! `ResolvedTypeIR`, `TypeRefIR`, `ResolvedTypeTable` e os validadores de
//! identidade) — chegam aqui na mesma ordem, com os mesmos corpos, os mesmos
//! `derive`, os mesmos campos e os mesmos diagnósticos. `super` mudou de
//! significado ao descer um nível, e o `use` abaixo devolve ao irmão o
//! vocabulário do pai sem promover nada: um filho enxerga os itens privados do
//! pai por privacidade de módulo, e este `use` é privado.
//!
//! A autoridade não se moveu com o corte. O `impl TypeIR`, o `impl
//! ScalarTypeIR`, o `impl UnaryOpIR` e o `impl BinaryOpIR` não estão em nenhuma
//! das duas regiões e continuam no pai, junto de `LoweringContext`, da conversão
//! AST→`TypeIR` (`ir.tipos.conversao-ast`) e da entrada pública
//! `render_program`. A validação da IR continua em `src/ir_validate.rs` e a
//! fronteira de CFG continua em `src/cfg_ir.rs`; nada disso desceu. A seleção de
//! método (`crate::method_dispatch`, C2) e o registry declarativo de intrínsecas
//! (`crate::intrinsics::registry`, C1) não são consultados por nenhuma das duas
//! regiões — nem antes, nem agora.
//!
//! Os quarenta itens de topo que já eram `pub` continuam `pub` aqui, e o pai
//! reexporta os quarenta para preservar `pinker_v0::ir::X` item a item: o
//! módulo filho é detalhe físico privado, então reexportar menos apagaria
//! caminho público. `is_generic_map_intrinsic` já era `pub(crate)` e o pai
//! reexporta o mesmo caminho de crate, consumido por `src/ir_validate.rs` e
//! `src/cfg_ir_validate.rs`. `expected_key_for_representation`,
//! `intern_representation_identity`, `builtin_sig`, `builtin_nominal_sig`,
//! `NominalTypeKindIR::from_canon` e `MapValueIR::from_type_ir` são os seis
//! símbolos chamados de fora do corte e, por isso, os únicos que passaram de
//! privados a `pub(super)` — exatamente os seis `exports` que o
//! `unit_costs.json` da #601 nomeia para a IR-3.

use super::*;

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

    pub(super) fn from_type_ir(ty: TypeIR) -> Option<Self> {
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

    pub(super) fn from_canon(kind: union_canon::NominalTypeKind) -> Self {
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
pub(super) fn expected_key_for_representation(ty: TypeIR) -> Option<&'static str> {
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
pub(super) fn intern_representation_identity(
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
pub(super) fn builtin_sig(
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
pub(super) fn builtin_nominal_sig(
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
