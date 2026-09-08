//! Guardião estrutural da decomposição física do módulo `ir`
//! (#621, #624, #626 e #632 — unidades IR-1, IR-2, IR-4 e IR-3 do inventário
//! da #601).
//!
//! `src/ir.rs` é a autoridade do lowering. A #621 desceu o `impl FunctionLowerer`
//! inteiro — as cinco regiões `ir.lowering.funcoes-blocos`,
//! `ir.lowering.comandos-controle`, `ir.lowering.expressoes-valores`,
//! `ir.lowering.bindings-escopos` e `ir.lowering.constantes` — para
//! `src/ir/lowering.rs`. A #624 desceu a montagem do contexto global e a
//! orquestração do programa — as cinco regiões
//! `ir.lowering.programa-orquestracao`, `ir.lowering.contexto-declaracoes`,
//! `ir.lowering.assinaturas-intrinsecos`, `ir.lowering.metodos-identidade` e
//! `ir.lowering.identidade-resolvida` — para `src/ir/context.rs`. A #626 desceu
//! a renderização textual — a região `ir.renderizacao.textual` — para
//! `src/ir/render.rs`. A #632 desceu o modelo de dados da IR e a identidade
//! semântica resolvida de tipos — as regiões `ir.modelo.representacao` e
//! `ir.tipos.identidade-resolvida` — para `src/ir/model.rs`. Nenhuma das quatro
//! divide a autoridade: as `struct FunctionLowerer`/`LoweringContext` e todo o
//! estado, a resolução de tipo e de união (`resolve_type`,
//! `resolve_union_ast_type`, `intern_union`), a entrada pública `render_program`,
//! os `impl TypeIR`/`ScalarTypeIR`/`UnaryOpIR`/`BinaryOpIR` e a conversão
//! AST→`TypeIR` continuam no pai, e o pai continua sendo um arquivo — não virou
//! `mod.rs`.
//!
//! Este arquivo prova o estado CUMULATIVO das quatro unidades, não só o do
//! último corte. As formas de cegueira silenciosa que ele fecha:
//!
//! 1. um oráculo textual que continuasse lendo só `src/ir.rs` seguiria verde e
//!    pararia de observar os irmãos — a OG-1 da #601. As duas consultas do
//!    lowering a `method_dispatch` desceram, uma para cada irmão, então a
//!    cegueira cairia justamente sobre C2; o consumo do registry declarativo de
//!    intrínsecas desceu junto, e com ele C1. Com a IR-3 desce o modelo inteiro
//!    da IR, e com ele a maior parte da superfície pública do módulo. Os censos
//!    de C2, de C5, da D6 e da Parte G leem `fonte_de_modulo::ir()`, e o teste
//!    abaixo prova que a lista lida por eles é exatamente o que existe no disco;
//! 2. a implementação podia ficar duplicada, ficar para trás no pai, ou o corte
//!    podia arrastar código que não é dele, inclusive código que uma unidade
//!    anterior já tinha movido;
//! 3. o corte podia promover visibilidade ou mudar a superfície pública do
//!    módulo. A IR-1 expôs `new`, `lower_function` e `lower_const` como
//!    `pub(super)`; a IR-2 expôs `resolved_identity`, `intern_resolved_ast`,
//!    `repr_identity` e `internal_identity`; a IR-4 expôs `render_function`,
//!    `render_value` e `line`; a IR-3 expôs `from_type_ir`, `from_canon`,
//!    `expected_key_for_representation`, `intern_representation_identity`,
//!    `builtin_sig` e `builtin_nominal_sig` — todos pela mesma razão: são os
//!    símbolos que o pai ou outro irmão chamam. `lower_program` e
//!    `lower_program_composto` já eram `pub` antes do move e continuam `pub`; os
//!    quarenta itens de topo da IR-3 também. O pai reexporta os dois primeiros e
//!    os quarenta para preservar `pinker_v0::ir::X` item a item, e reexporta
//!    `is_generic_map_intrinsic` para preservar o mesmo caminho de crate. Essas
//!    são as reexportações devidas, e nenhuma outra pode existir. A forma
//!    pública de cada arquivo — item de topo, campo de struct e método — é
//!    comparada linha a linha: é `IR_PUBLIC_TYPE_SHAPE` na forma que um teste
//!    consegue observar;
//! 4. o corte podia arrastar a validação da IR ou a fronteira de CFG para um
//!    irmão. Nenhuma das duas desceu: `src/ir_validate.rs` e `src/cfg_ir.rs`
//!    continuam donos do que sempre foram;
//! 5. o corte podia arrastar região ou definição de unidade vizinha. Depois da
//!    IR-3 a única região que fica no pai é `ir.tipos.conversao-ast`, que não é
//!    de nenhuma unidade, e o conjunto do que ficou é comparado por igualdade,
//!    não por presença.
//!
//! Ele NÃO congela LOC, não congela a árvore como snapshot ornamental e não
//! afirma nada sobre a regra de despacho, que é de `src/method_dispatch.rs`.
#[path = "common/fonte_de_modulo.rs"]
mod fonte_de_modulo;
#[path = "common/rust_source.rs"]
mod rust_source;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use fonte_de_modulo::{ir, IR_ARQUIVOS};
use rust_source::codigo_executavel;

/// As regiões que cada unidade moveu, e o irmão onde passam a morar.
const REGIOES_MOVIDAS: &[(&str, &str)] = &[
    // IR-1 (#621): o `impl FunctionLowerer` inteiro.
    ("ir.lowering.funcoes-blocos", "lowering.rs"),
    ("ir.lowering.comandos-controle", "lowering.rs"),
    ("ir.lowering.expressoes-valores", "lowering.rs"),
    ("ir.lowering.bindings-escopos", "lowering.rs"),
    ("ir.lowering.constantes", "lowering.rs"),
    // IR-2 (#624): a orquestração do programa e a montagem do contexto.
    ("ir.lowering.programa-orquestracao", "context.rs"),
    ("ir.lowering.contexto-declaracoes", "context.rs"),
    ("ir.lowering.assinaturas-intrinsecos", "context.rs"),
    ("ir.lowering.metodos-identidade", "context.rs"),
    ("ir.lowering.identidade-resolvida", "context.rs"),
    // IR-4 (#626): a renderização textual da IR já construída.
    ("ir.renderizacao.textual", "render.rs"),
    // IR-3 (#632): o modelo de dados da IR e a identidade resolvida de tipos.
    ("ir.modelo.representacao", "model.rs"),
    ("ir.tipos.identidade-resolvida", "model.rs"),
];

/// As regiões que os quatro cortes deixaram onde estavam.
///
/// `ir.tipos.conversao-ast` é a vizinha imediata do span da IR-3 — a que vem
/// depois — e não pertence a nenhuma unidade do inventário: ela mora dentro do
/// `impl LoweringContext` do pai. É ela que ficaria vermelha se o corte tivesse
/// escorregado uma região para a frente, e a igualdade de conjunto abaixo é o
/// que recusa uma região arrastada em silêncio.
const REGIOES_RETIDAS: &[&str] = &["ir.tipos.conversao-ast"];

/// As definições que cada unidade moveu inteiras, e o irmão onde passam a morar.
/// Uma definição, no irmão certo, e nenhuma deixada para trás no pai.
///
/// `new` aparece com a assinatura inteira porque o pai tem um `new` próprio, de
/// `TypeRefIR`: um oráculo que procurasse só `fn new(` não distinguiria os dois
/// e ficaria verde com a implementação duplicada.
const DEFINICOES_MOVIDAS: &[(&str, &str)] = &[
    ("fn allocate_binding(", "lowering.rs"),
    ("fn callable_metadata_for_expr(", "lowering.rs"),
    ("fn callable_metadata_for_value(", "lowering.rs"),
    ("fn callable_metadata_from_return_type(", "lowering.rs"),
    ("fn callable_ret_identity(", "lowering.rs"),
    ("fn concrete_snapshot_size(", "lowering.rs"),
    ("fn ensure_fnref_wrapper(", "lowering.rs"),
    ("fn function_value_identity(", "lowering.rs"),
    ("fn impl_receiver_key(", "lowering.rs"),
    ("fn lower_block(", "lowering.rs"),
    ("fn lower_break(", "lowering.rs"),
    ("fn lower_closure_function(", "lowering.rs"),
    ("fn lower_const(", "lowering.rs"),
    ("fn lower_continue(", "lowering.rs"),
    ("fn lower_enum_match(", "lowering.rs"),
    ("fn lower_enum_pattern(", "lowering.rs"),
    ("fn lower_falar(", "lowering.rs"),
    ("fn lower_function(", "lowering.rs"),
    ("fn lower_if(", "lowering.rs"),
    ("fn lower_inline_asm(", "lowering.rs"),
    ("fn lower_let(", "lowering.rs"),
    ("fn lower_return(", "lowering.rs"),
    ("fn lower_stmt(", "lowering.rs"),
    ("fn lower_trait_call(", "lowering.rs"),
    ("fn lower_union_match(", "lowering.rs"),
    ("fn lower_value(", "lowering.rs"),
    ("fn lower_while(", "lowering.rs"),
    ("fn new(context: &'a LoweringContext)", "lowering.rs"),
    ("fn next_block_label(", "lowering.rs"),
    ("fn nominal_name_of_value(", "lowering.rs"),
    ("fn pointee_identity_of(", "lowering.rs"),
    ("fn pointer_element_layout(", "lowering.rs"),
    ("fn pointer_pointee_for_expr(", "lowering.rs"),
    ("fn pop_scope(", "lowering.rs"),
    ("fn push_scope(", "lowering.rs"),
    ("fn raw_function_metadata_for_expr(", "lowering.rs"),
    ("fn raw_function_metadata_for_value(", "lowering.rs"),
    ("fn raw_ret_identity(", "lowering.rs"),
    ("fn resolve_binding(", "lowering.rs"),
    ("fn resolve_closure(", "lowering.rs"),
    ("fn resolve_existing_binding(", "lowering.rs"),
    ("fn resolve_impl_method(", "lowering.rs"),
    ("fn resolve_qualified_impl_method(", "lowering.rs"),
    ("fn resolve_trait_impl_symbol(", "lowering.rs"),
    ("fn trait_object_name_for_expr(", "lowering.rs"),
    ("fn trait_vtable(", "lowering.rs"),
    ("fn from_program_composto(", "context.rs"),
    ("fn intern_resolved_ast(", "context.rs"),
    ("fn internal_identity(", "context.rs"),
    ("fn lower_program(", "context.rs"),
    ("fn lower_program_composto(", "context.rs"),
    ("fn register_impl_methods(", "context.rs"),
    ("fn repr_identity(", "context.rs"),
    ("fn resolved_identity(", "context.rs"),
    ("fn seal_declared_signature_identities(", "context.rs"),
    ("fn seal_enum_variant_metadata(", "context.rs"),
    ("fn line(", "render.rs"),
    ("fn render_block(", "render.rs"),
    ("fn render_enum_pattern(", "render.rs"),
    ("fn render_function(", "render.rs"),
    ("fn render_instruction(", "render.rs"),
    ("fn render_value(", "render.rs"),
    ("fn as_str(", "model.rs"),
    ("fn builtin_nominal_sig(", "model.rs"),
    ("fn builtin_sig(", "model.rs"),
    ("fn expected_key_for_representation(", "model.rs"),
    ("fn expected_representation_for_key(", "model.rs"),
    ("fn from_canon(", "model.rs"),
    ("fn get(&self, id: ResolvedTypeId)", "model.rs"),
    ("fn id_of_key(", "model.rs"),
    ("fn intern(", "model.rs"),
    ("fn intern_representation_identity(", "model.rs"),
    ("fn into_types(", "model.rs"),
    ("fn is_generic_map_intrinsic(", "model.rs"),
    ("fn key_of(", "model.rs"),
    (
        "fn new(representation: TypeIR, resolved: ResolvedTypeId)",
        "model.rs",
    ),
    ("fn nominal_name_of(", "model.rs"),
    ("fn validate_resolved_type_reference(", "model.rs"),
    ("fn validate_resolved_type_structure(", "model.rs"),
    ("fn validate_resolved_type_table(", "model.rs"),
    ("fn validate_union_match_coverage(", "model.rs"),
    ("fn validate_union_member_identity(", "model.rs"),
    ("fn validate_union_member_reference(", "model.rs"),
    ("fn validate_union_reference(", "model.rs"),
    ("fn validate_union_registry(", "model.rs"),
    ("fn validate_union_registry_identities(", "model.rs"),
    // `impl ScalarTypeIR` tem um `from_type_ir` de mesma assinatura e ficou no
    // pai: sem a visibilidade no texto o oráculo não distinguiria os dois.
    ("pub(super) fn from_type_ir(", "model.rs"),
];

/// As definições que os cortes NÃO moveram e que continuam no pai.
///
/// `resolve_type`, `resolve_union_ast_type` e `intern_union` são métodos do
/// mesmo `impl LoweringContext` cuja maior parte desceu com a IR-2, e não estão
/// em nenhuma das cinco regiões da unidade: arrastá-las junto seria mover código
/// que não é do corte. `render_program` é a entrada pública da renderização: ela
/// não está na região `ir.renderizacao.textual` e fica no pai, delegando ao
/// irmão — é ela que ancora a fronteira de cima da IR-4. `is_compatible_with`,
/// `to_type_ir` e `from_ast_with_context` são corpos dos `impl TypeIR` e
/// `impl ScalarTypeIR`, que ficam depois do span da IR-3 e não pertencem a
/// nenhuma das duas regiões dela: são eles que ancoram a fronteira de baixo do
/// corte, a que separar o modelo dos seus `impl` no pai poderia arrastar.
const DEFINICOES_RETIDAS: &[&str] = &[
    "fn from_ast_with_context(",
    "fn intern_union(",
    "fn is_compatible_with(",
    "fn render_program(",
    "fn resolve_type(",
    "fn resolve_union_ast_type(",
    "fn to_type_ir(",
];

/// Irmãos que carregam produção, não teste.
const IRMAOS_DE_PRODUCAO: &[&str] = &["context.rs", "lowering.rs", "model.rs", "render.rs"];

/// A forma pública de cada arquivo do módulo, exaustiva e comparada linha a
/// linha: todo `pub`, `pub(super)` e `pub(crate)` do código executável — item de
/// topo, campo de struct, variante e método.
///
/// É `IR_PUBLIC_TYPE_SHAPE` na forma que um teste consegue observar. O censo de
/// itens de topo mais abaixo prova que os caminhos públicos do módulo não
/// mudaram; este prova o que aquele não vê — um campo promovido, um método
/// alargado, um `pub` novo dentro de um tipo que já era público. A IR-3 desce o
/// modelo de dados inteiro, então é exatamente aqui que uma promoção silenciosa
/// apareceria.
const FORMA_PUBLICA: &[(&str, &[&str])] = &[
    (
        "ir.rs",
        &[
            "pub canonical_key: String,",
            "pub class: crate::enum_payload::EnumPayloadClass,",
            "pub discriminant: u64,",
            "pub element_type_id: Option<ResolvedTypeId>,",
            "pub enum_name: String,",
            "pub fn from_ast_option_with_context(",
            "pub fn from_ast_with_context(",
            "pub fn is_closure_environment_word(&self) -> bool {",
            "pub fn is_compatible_with(&self, other: TypeIR) -> bool {",
            "pub fn is_integer(&self) -> bool {",
            "pub fn is_native_abi_word(&self) -> bool {",
            "pub fn is_signed(&self) -> bool {",
            "pub fn is_unsigned(&self) -> bool {",
            "pub fn name(&self) -> &'static str {",
            "pub fn name(&self) -> &'static str {",
            "pub fn native_abi_words(&self) -> Option<usize> {",
            "pub fn render_name(&self) -> String {",
            "pub fn render_program(program: &ProgramIR) -> String {",
            "pub operational_type: TypeIR,",
            "pub payloads: Vec<EnumPayloadMetaIR>,",
            "pub resolved_type_id: ResolvedTypeId,",
            "pub struct EnumPayloadMetaIR {",
            "pub struct EnumVariantMetaIR {",
            "pub use context::{lower_program, lower_program_composto};",
            "pub use model::{",
            "pub variant_name: String,",
            "pub(crate) use model::is_generic_map_intrinsic;",
        ],
    ),
    (
        "context.rs",
        &[
            "pub fn lower_program(program: &Program) -> Result<ProgramIR, PinkerError> {",
            "pub fn lower_program_composto(",
            "pub(super) fn intern_resolved_ast(",
            "pub(super) fn internal_identity(",
            "pub(super) fn repr_identity(",
            "pub(super) fn resolved_identity(&self, ty: &Type) -> Result<ResolvedTypeId, PinkerError> {",
        ],
    ),
    (
        "lowering.rs",
        &[
            "pub(super) fn lower_const(",
            "pub(super) fn lower_function(",
            "pub(super) fn new(context: &'a LoweringContext) -> Self {",
        ],
    ),
    (
        "model.rs",
        &[
            "pub arms: Vec<EnumMatchArmIR>,",
            "pub arms: Vec<UnionMatchArmIR>,",
            "pub binding: BindingIR,",
            "pub body: BlockIR,",
            "pub body: BlockIR,",
            "pub canonical_key: String,",
            "pub canonical_key: String,",
            "pub canonical_key: String,",
            "pub canonical_member_key: String,",
            "pub canonical_member_key: String,",
            "pub class: crate::enum_payload::EnumPayloadClass,",
            "pub consts: Vec<ConstIR>,",
            "pub element: Option<ResolvedTypeId>,",
            "pub element: Option<ResolvedTypeId>,",
            "pub entry: BlockIR,",
            "pub enum BinaryOpIR {",
            "pub enum EnumPatternIR {",
            "pub enum InlineAsmOperandIR {",
            "pub enum InstructionIR {",
            "pub enum MapKeyIR {",
            "pub enum MapValueIR {",
            "pub enum NominalTypeKindIR {",
            "pub enum ScalarTypeIR {",
            "pub enum TypeIR {",
            "pub enum UnaryOpIR {",
            "pub enum ValueIR {",
            "pub enum_variants: Vec<EnumVariantMetaIR>,",
            "pub extract_intrinsic: String,",
            "pub extracted_binding: BindingIR,",
            "pub fn as_str(&self) -> &'static str {",
            "pub fn get(&self, id: ResolvedTypeId) -> Option<&ResolvedTypeIR> {",
            "pub fn id_of_key(&self, key: &str) -> Option<ResolvedTypeId> {",
            "pub fn intern(",
            "pub fn into_types(self) -> Vec<ResolvedTypeIR> {",
            "pub fn is_empty(&self) -> bool {",
            "pub fn key_of(&self, id: ResolvedTypeId) -> Option<&str> {",
            "pub fn len(&self) -> usize {",
            "pub fn new(representation: TypeIR, resolved: ResolvedTypeId) -> Self {",
            "pub fn nominal_name_of(&self, id: ResolvedTypeId) -> Option<&str> {",
            "pub fn type_ref(&self) -> Option<TypeRefIR> {",
            "pub fn type_ref(&self) -> Option<TypeRefIR> {",
            "pub fn validate_resolved_type_reference(",
            "pub fn validate_resolved_type_table(resolved: &[ResolvedTypeIR]) -> Result<(), String> {",
            "pub fn validate_union_match_coverage(",
            "pub fn validate_union_member_identity(",
            "pub fn validate_union_member_reference(",
            "pub fn validate_union_reference(",
            "pub fn validate_union_registry(unions: &[UnionTypeIR]) -> Result<(), String> {",
            "pub fn validate_union_registry_identities(",
            "pub functions: Vec<FunctionIR>,",
            "pub id: ResolvedTypeId,",
            "pub id: UnionTypeId,",
            "pub index: u64,",
            "pub instructions: Vec<InstructionIR>,",
            "pub is_freestanding: bool,",
            "pub is_mut: bool,",
            "pub label: String,",
            "pub locals: Vec<LocalIR>,",
            "pub members: Vec<UnionMemberIR>,",
            "pub module_name: String,",
            "pub name: String,",
            "pub name: String,",
            "pub nominal: Option<(NominalTypeKindIR, String)>,",
            "pub nominal_kind: Option<NominalTypeKindIR>,",
            "pub nominal_name: Option<String>,",
            "pub operational_type: TypeIR,",
            "pub otherwise: Option<BlockIR>,",
            "pub params: Vec<BindingIR>,",
            "pub params: Vec<ResolvedTypeId>,",
            "pub pattern: Box<EnumPatternIR>,",
            "pub pattern: EnumPatternIR,",
            "pub payload_layout: crate::union_payload::UnionPayloadLayout,",
            "pub payload_layout: crate::union_payload::UnionPayloadLayout,",
            "pub payload_type: TypeIR,",
            "pub pointee: Option<ResolvedTypeId>,",
            "pub pointee: Option<ResolvedTypeId>,",
            "pub representation: TypeIR,",
            "pub representation: TypeIR,",
            "pub resolved: Option<ResolvedTypeId>,",
            "pub resolved: Option<ResolvedTypeId>,",
            "pub resolved: ResolvedTypeId,",
            "pub resolved_member_type_id: ResolvedTypeId,",
            "pub resolved_type_id: ResolvedTypeId,",
            "pub resolved_type_id: ResolvedTypeId,",
            "pub resolved_types: Vec<ResolvedTypeIR>,",
            "pub ret: ResolvedTypeId,",
            "pub ret_type: TypeIR,",
            "pub scrutinee: ValueIR,",
            "pub scrutinee: ValueIR,",
            "pub scrutinee_binding: BindingIR,",
            "pub scrutinee_binding: BindingIR,",
            "pub signature: Option<ResolvedSignatureIR>,",
            "pub signature: Option<ResolvedSignatureIR>,",
            "pub slot: String,",
            "pub slot: String,",
            "pub source_name: String,",
            "pub source_name: String,",
            "pub span: Span,",
            "pub span: Span,",
            "pub span: Span,",
            "pub span: Span,",
            "pub span: Span,",
            "pub span: Span,",
            "pub span: Span,",
            "pub struct BindingIR {",
            "pub struct BlockIR {",
            "pub struct ConstIR {",
            "pub struct EnumMatchArmIR {",
            "pub struct EnumMatchIR {",
            "pub struct EnumPatternPayloadIR {",
            "pub struct FalarArgIR {",
            "pub struct FunctionIR {",
            "pub struct LocalIR {",
            "pub struct ProgramIR {",
            "pub struct ResolvedSignatureIR {",
            "pub struct ResolvedTypeIR {",
            "pub struct ResolvedTypeId(pub u32);",
            "pub struct ResolvedTypeParts {",
            "pub struct ResolvedTypeTable {",
            "pub struct TypeRefIR {",
            "pub struct UnionMatchArmIR {",
            "pub struct UnionMatchIR {",
            "pub struct UnionMemberIR {",
            "pub struct UnionTypeIR {",
            "pub struct UnionTypeId(pub u32);",
            "pub tag: u64,",
            "pub tag: u64,",
            "pub tag_binding: BindingIR,",
            "pub ty: TypeIR,",
            "pub ty: TypeIR,",
            "pub ty: TypeIR,",
            "pub ty: TypeIR,",
            "pub ty: TypeIR,",
            "pub union_members: Option<Vec<ResolvedTypeId>>,",
            "pub union_members: Option<Vec<ResolvedTypeId>>,",
            "pub union_type_id: UnionTypeId,",
            "pub union_types: Vec<UnionTypeIR>,",
            "pub value: ValueIR,",
            "pub value: ValueIR,",
            "pub(crate) fn is_generic_map_intrinsic(name: &str) -> bool {",
            "pub(crate) fn type_ir(self) -> TypeIR {",
            "pub(crate) fn type_ir(self) -> TypeIR {",
            "pub(super) fn builtin_nominal_sig(",
            "pub(super) fn builtin_sig(",
            "pub(super) fn expected_key_for_representation(ty: TypeIR) -> Option<&'static str> {",
            "pub(super) fn from_canon(kind: union_canon::NominalTypeKind) -> Self {",
            "pub(super) fn from_type_ir(ty: TypeIR) -> Option<Self> {",
            "pub(super) fn intern_representation_identity(",
        ],
    ),
    (
        "render.rs",
        &[
            "pub(super) fn line(out: &mut String, indent: usize, text: &str) {",
            "pub(super) fn render_function(function: &FunctionIR, indent: usize, out: &mut String) {",
            "pub(super) fn render_value(value: &ValueIR) -> String {",
        ],
    ),
];

/// A visibilidade restrita que cada irmão pode ter, exaustiva.
///
/// É o custo Rust inteiro das quatro unidades. Para a IR-1 o inventário da #601
/// previu `4 pub(super)` e o baseline desmentiu um: as únicas ocorrências de
/// `resolve_closure` fora do corte eram comentários. Para a IR-2 previu
/// `4 pub(super)` e os quatro se confirmaram, exatamente os quatro `exports` que
/// o `unit_costs.json` nomeia. Para a IR-4 previu `3 pub(super)` e os três se
/// confirmaram: `render_function`, `render_value` e `line` são o que
/// `render_program` chama do pai. Para a IR-3 previu `6 pub(super)` e os seis se
/// confirmaram, com os mesmos nomes: `from_type_ir` é o que o pai chama, e
/// `from_canon`, `expected_key_for_representation`,
/// `intern_representation_identity`, `builtin_sig` e `builtin_nominal_sig` são o
/// que `context.rs` chama. Nenhum `pub(crate)` novo em nenhuma das quatro.
const PUB_RESTRITO_AUTORIZADO: &[(&str, &[&str])] = &[
    (
        "context.rs",
        &[
            "pub(super) fn resolved_identity(",
            "pub(super) fn intern_resolved_ast(",
            "pub(super) fn repr_identity(",
            "pub(super) fn internal_identity(",
        ],
    ),
    (
        "lowering.rs",
        &[
            "pub(super) fn new(",
            "pub(super) fn lower_function(",
            "pub(super) fn lower_const(",
        ],
    ),
    (
        "model.rs",
        &[
            "pub(super) fn from_type_ir(",
            "pub(super) fn from_canon(",
            "pub(super) fn expected_key_for_representation(",
            "pub(super) fn intern_representation_identity(",
            "pub(super) fn builtin_sig(",
            "pub(super) fn builtin_nominal_sig(",
        ],
    ),
    (
        "render.rs",
        &[
            "pub(super) fn render_function(",
            "pub(super) fn render_value(",
            "pub(super) fn line(",
        ],
    ),
];

/// As reexportações que o pai deve — e as únicas que podem existir no módulo.
///
/// A da IR-2 devolve a entrada pública do lowering. A da IR-3 devolve os
/// quarenta itens de topo do modelo, item a item: o irmão é `mod` privado, então
/// reexportar trinta e nove apagaria um caminho de `pinker_v0::ir::*`. A
/// terceira devolve o mesmo caminho de crate a `is_generic_map_intrinsic`, que
/// `src/ir_validate.rs` e `src/cfg_ir_validate.rs` consomem por
/// `crate::ir::is_generic_map_intrinsic`.
const REEXPORTACOES_DEVIDAS: &[&str] = &[
    "pub use context::{lower_program, lower_program_composto};",
    "pub use model::{",
    "pub(crate) use model::is_generic_map_intrinsic;",
];

/// A superfície pública do módulo, congelada item a item.
///
/// É o contrato `PUBLIC_PATHS_BEFORE == AFTER` da #621 e da #624 na forma que um
/// teste consegue observar. A #621 não movia nenhum item público; a #624 move
/// dois, e por isso a lista continua idêntica somente porque a reexportação
/// devolve os dois caminhos ao pai.
const API_PUBLICA_CONGELADA: &[&str] = &[
    "BinaryOpIR",
    "BindingIR",
    "BlockIR",
    "ConstIR",
    "EnumMatchArmIR",
    "EnumMatchIR",
    "EnumPatternIR",
    "EnumPatternPayloadIR",
    "EnumPayloadMetaIR",
    "EnumVariantMetaIR",
    "FalarArgIR",
    "FunctionIR",
    "InlineAsmOperandIR",
    "InstructionIR",
    "LocalIR",
    "MapKeyIR",
    "MapValueIR",
    "NominalTypeKindIR",
    "ProgramIR",
    "ResolvedSignatureIR",
    "ResolvedTypeIR",
    "ResolvedTypeId",
    "ResolvedTypeParts",
    "ResolvedTypeTable",
    "ScalarTypeIR",
    "TypeIR",
    "TypeRefIR",
    "UnaryOpIR",
    "UnionMatchArmIR",
    "UnionMatchIR",
    "UnionMemberIR",
    "UnionTypeIR",
    "UnionTypeId",
    "ValueIR",
    "lower_program",
    "lower_program_composto",
    "render_program",
    "validate_resolved_type_reference",
    "validate_resolved_type_table",
    "validate_union_match_coverage",
    "validate_union_member_identity",
    "validate_union_member_reference",
    "validate_union_reference",
    "validate_union_registry",
    "validate_union_registry_identities",
];

fn diretorio_dos_irmaos() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/ir")
}

fn fonte(nome: &str) -> &'static str {
    IR_ARQUIVOS
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, fonte)| *fonte)
        .unwrap_or_else(|| panic!("{nome} não faz parte do módulo lido pelos oráculos"))
}

fn pai() -> &'static str {
    fonte("ir.rs")
}

/// Um irmão novo no disco que ninguém registrou seria invisível para todo
/// oráculo que lê o módulo por `fonte_de_modulo` — e o de C2 é um deles.
#[test]
fn o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem() {
    let no_disco: BTreeSet<String> = fs::read_dir(diretorio_dos_irmaos())
        .expect("src/ir/ legível")
        .map(|entrada| entrada.expect("entrada de diretório").path())
        .filter(|caminho| caminho.extension().is_some_and(|ext| ext == "rs"))
        .map(|caminho| {
            caminho
                .file_name()
                .expect("nome de arquivo")
                .to_str()
                .expect("utf-8")
                .to_string()
        })
        .collect();
    let declarados: BTreeSet<String> = IR_ARQUIVOS
        .iter()
        .map(|(nome, _)| (*nome).to_string())
        .filter(|nome| nome != "ir.rs")
        .collect();
    assert_eq!(
        no_disco, declarados,
        "src/ir/ divergiu da lista lida pelos oráculos estruturais"
    );
}

/// Sem o `mod`, o irmão não entra no crate e o que desceu deixa de existir sem
/// que nada fique vermelho. É a sensitivity M1 da #621.
#[test]
fn o_pai_inclui_o_irmao() {
    let codigo = codigo_executavel(pai());
    for (nome, _) in IR_ARQUIVOS {
        if *nome == "ir.rs" {
            continue;
        }
        let modulo = nome.trim_end_matches(".rs");
        let declaracao = format!("mod {modulo};");
        assert_eq!(
            codigo.matches(&declaracao).count(),
            1,
            "src/ir.rs deveria declarar `{declaracao}` exatamente uma vez"
        );
        // O filho é detalhe físico, não caminho público. `pub mod lowering;`
        // criaria `pinker_v0::ir::lowering::*` sem apagar nenhum dos itens
        // congelados — ampliação de superfície que o censo de itens não veria
        // sozinho.
        assert_eq!(
            codigo.matches(&format!("pub {declaracao}")).count(),
            0,
            "src/ir.rs tornou o irmão `{modulo}` um caminho público"
        );
    }
    // O pai continua sendo um arquivo: a #621 mantém a forma que a #608 decidiu
    // e a #610, a #612, a #615, a #617 e a #619 mantiveram. Convertê-lo em
    // `mod.rs` moveria o campo `file` das nove regiões que ficaram e faria as
    // projeções FROZEN pararem com E-SNAP-PATH-ALTERADO.
    assert!(
        !diretorio_dos_irmaos().join("mod.rs").exists(),
        "o pai virou mod.rs, contrariando a forma decidida pela #608"
    );
}

/// Presença única: nem região perdida, nem região duplicada, nem implementação
/// deixada para trás no arquivo antigo. São as sensitivities M2 e M3 da #621.
#[test]
fn cada_regiao_e_cada_definicao_movida_aparece_uma_vez_no_arquivo_certo() {
    let modulo = ir();
    for (chave, arquivo) in REGIOES_MOVIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            fonte(arquivo).contains(&marcador),
            "a região {chave} deveria morar em src/ir/{arquivo}"
        );
        assert!(
            !pai().contains(&marcador),
            "a região {chave} ficou para trás em src/ir.rs"
        );
    }
    for chave in REGIOES_RETIDAS {
        conferir_regiao_unica(&modulo, chave);
        let marcador = format!("// @pinker-nav:start {chave}");
        assert!(
            pai().contains(&marcador),
            "a região {chave} não é de nenhuma das três unidades e deveria continuar em src/ir.rs"
        );
    }

    // Presença não basta: a lista precisa ser o conjunto EXATO do que ficou. Só
    // com igualdade uma região arrastada em silêncio — ou uma sobra de uma IR-3
    // executada pela metade — fica vermelha aqui, e não apenas na
    // cartografia. É a mesma disciplina de
    // `o_conjunto_de_arquivos_do_modulo_e_exatamente_o_que_os_oraculos_leem`.
    let no_pai: BTreeSet<&str> = regioes_declaradas(pai()).collect();
    let retidas: BTreeSet<&str> = REGIOES_RETIDAS.iter().copied().collect();
    assert_eq!(
        no_pai, retidas,
        "o conjunto de regiões que ficaram em src/ir.rs divergiu do declarado"
    );
    for (nome, _) in IR_ARQUIVOS {
        if *nome == "ir.rs" {
            continue;
        }
        let no_irmao: BTreeSet<&str> = regioes_declaradas(fonte(nome)).collect();
        let esperadas: BTreeSet<&str> = REGIOES_MOVIDAS
            .iter()
            .filter(|(_, arquivo)| arquivo == nome)
            .map(|(chave, _)| *chave)
            .collect();
        assert_eq!(
            no_irmao, esperadas,
            "o conjunto de regiões de src/ir/{nome} divergiu do declarado"
        );
    }

    let codigo = codigo_executavel(&modulo);
    let codigo_do_pai = codigo_executavel(pai());
    for (definicao, arquivo) in DEFINICOES_MOVIDAS {
        assert_eq!(
            codigo.matches(definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo ir"
        );
        assert_eq!(
            codigo_do_pai.matches(definicao).count(),
            0,
            "a implementação de `{definicao}` ficou para trás em src/ir.rs"
        );
        assert_eq!(
            codigo_executavel(fonte(arquivo)).matches(definicao).count(),
            1,
            "`{definicao}` deveria morar em src/ir/{arquivo}"
        );
    }

    // O outro lado do mesmo contrato: o corte não pode arrastar código que não
    // é dele. Cada definição retida tem uma implementação só, e ela está no pai.
    for definicao in DEFINICOES_RETIDAS {
        assert_eq!(
            codigo.matches(definicao).count(),
            1,
            "`{definicao}` deveria ter exatamente uma definição no módulo ir"
        );
        assert_eq!(
            codigo_do_pai.matches(definicao).count(),
            1,
            "`{definicao}` não é de nenhuma das três unidades e deveria continuar em src/ir.rs"
        );
    }
}

/// As chaves de região declaradas por uma fonte, na ordem em que aparecem.
fn regioes_declaradas(fonte: &'static str) -> impl Iterator<Item = &'static str> {
    fonte.lines().filter_map(|linha| {
        linha
            .trim_start()
            .strip_prefix("// @pinker-nav:start ")
            .map(str::trim)
    })
}

fn conferir_regiao_unica(modulo: &str, chave: &str) {
    for marcador in [
        format!("// @pinker-nav:start {chave}"),
        format!("// @pinker-nav:end {chave}"),
    ] {
        assert_eq!(
            modulo.matches(&marcador).count(),
            1,
            "`{marcador}` deveria aparecer exatamente uma vez no módulo ir"
        );
    }
}

/// A decomposição é física: não promove nada. A forma pública de cada arquivo —
/// pai inclusive — é declarada linha a linha e comparada por igualdade, e o
/// custo de visibilidade de cada irmão é nomeado símbolo a símbolo. É a
/// sensitivity M4 da #621 e a M6 da #632.
#[test]
fn a_decomposicao_nao_promoveu_visibilidade() {
    for (nome, fonte) in IR_ARQUIVOS {
        let codigo = codigo_executavel(fonte);
        let mut observada: Vec<&str> = codigo
            .lines()
            .map(str::trim)
            .filter(|linha| linha.contains("pub ") || linha.contains("pub("))
            .collect();
        observada.sort_unstable();
        let declarada = itens_autorizados(FORMA_PUBLICA, nome, "forma pública");
        assert_eq!(
            observada, declarada,
            "a forma pública de {nome} divergiu da declarada; a decomposição é física e não promove nada"
        );
    }

    // O custo do corte, nomeado: cada irmão expõe ao pai exatamente os símbolos
    // que o pai (ou outro irmão) chama, e nenhum a mais.
    for nome in IRMAOS_DE_PRODUCAO {
        let codigo = codigo_executavel(fonte(nome));
        let restritos = itens_autorizados(PUB_RESTRITO_AUTORIZADO, nome, "visibilidade restrita");
        assert_eq!(
            codigo.matches("pub(super)").count(),
            restritos.len(),
            "src/ir/{nome} tem mais `pub(super)` do que o corte previa"
        );
        for item in restritos {
            assert_eq!(
                codigo.matches(item).count(),
                1,
                "src/ir/{nome} deveria conter `{item}` exatamente uma vez"
            );
        }
    }
}

/// A visibilidade de crate do módulo é a mesma de antes dos quatro cortes: três
/// declarações, nem uma a mais.
///
/// Os três nasceram dentro das duas regiões da IR-3 — `MapKeyIR::type_ir` e
/// `MapValueIR::type_ir` em `ir.modelo.representacao`, e
/// `is_generic_map_intrinsic` entre as duas regiões — e desceram com elas. Os
/// dois primeiros são métodos e viajam com o tipo, que o pai reexporta; o
/// terceiro é uma função livre, e por isso o pai reexporta o caminho
/// `crate::ir::is_generic_map_intrinsic` que `src/ir_validate.rs` e
/// `src/cfg_ir_validate.rs` consomem. Reexportar caminho existente não é
/// promover: nenhum item mudou de visibilidade.
#[test]
fn a_visibilidade_de_crate_do_modulo_e_a_mesma_de_sempre() {
    let modelo = codigo_executavel(fonte("model.rs"));
    assert_eq!(
        modelo.matches("pub(crate)").count(),
        3,
        "src/ir/model.rs mudou de quantidade de `pub(crate)`; nenhuma das quatro unidades promove visibilidade"
    );
    assert_eq!(
        modelo.matches("pub(crate) fn type_ir(").count(),
        2,
        "os dois `type_ir` de chave e valor de mapa deixaram de ser `pub(crate)`"
    );
    assert_eq!(
        modelo
            .matches("pub(crate) fn is_generic_map_intrinsic(")
            .count(),
        1,
        "`is_generic_map_intrinsic` deixou de ser o terceiro `pub(crate)` do módulo"
    );

    // O pai não declara nenhum: o único `pub(crate)` dele é a reexportação
    // mecânica do caminho de crate que a IR-3 teria apagado.
    let pai = codigo_executavel(pai());
    assert_eq!(
        pai.matches("pub(crate)").count(),
        1,
        "src/ir.rs mudou de quantidade de `pub(crate)`; só a reexportação do caminho existente é devida"
    );
    assert_eq!(
        pai.matches("pub(crate) use model::is_generic_map_intrinsic;")
            .count(),
        1,
        "src/ir.rs deveria devolver `crate::ir::is_generic_map_intrinsic` exatamente uma vez"
    );

    // E nenhum outro irmão ganhou visibilidade de crate.
    for (nome, codigo) in irmaos_de_producao() {
        if nome == "model.rs" {
            continue;
        }
        assert_eq!(
            codigo.matches("pub(crate)").count(),
            0,
            "src/ir/{nome} criou visibilidade de crate; o corte previa zero"
        );
    }
}

/// A lista exaustiva de um irmão, ou o panic que recusa um irmão não
/// declarado: um arquivo novo não entra no módulo sem dizer o que expõe.
fn itens_autorizados(
    lista: &'static [(&'static str, &'static [&'static str])],
    nome: &str,
    classe: &str,
) -> &'static [&'static str] {
    lista
        .iter()
        .find(|(arquivo, _)| *arquivo == nome)
        .map(|(_, itens)| *itens)
        .unwrap_or_else(|| {
            panic!("src/ir/{nome} não declarou que itens de {classe} o corte previa")
        })
}

/// A superfície pública do módulo não mudou de tamanho nem de conteúdo.
#[test]
fn a_superficie_publica_do_modulo_e_exatamente_a_congelada() {
    let modulo = ir();
    let observados: BTreeSet<&str> = itens_publicos_de_topo(&modulo).into_iter().collect();
    let congelados: BTreeSet<&str> = API_PUBLICA_CONGELADA.iter().copied().collect();
    assert_eq!(
        observados, congelados,
        "a superfície pública de ir mudou; as quatro unidades são decomposição física e não mudam API pública"
    );

    // Itens não são a única forma de caminho público: um `pub mod` ou um
    // `pub use` acrescentaria caminhos sem mudar a contagem acima.
    let codigo = codigo_executavel(&modulo);
    assert_eq!(
        codigo.matches("pub mod ").count(),
        0,
        "o módulo passou a expor um submódulo público; o corte é físico e os irmãos são privados"
    );
    // Duas reexportações públicas — a da entrada do lowering e a dos quarenta
    // itens do modelo — e nenhuma outra: uma terceira abriria caminho novo sem
    // mudar a contagem acima.
    let codigo_do_pai = codigo_executavel(pai());
    assert_eq!(
        codigo.matches("pub use ").count(),
        2,
        "o módulo mudou de quantidade de reexportações públicas; só as duas devidas podem existir"
    );
    for devida in REEXPORTACOES_DEVIDAS {
        assert_eq!(
            codigo_do_pai.matches(devida).count(),
            1,
            "src/ir.rs deveria conter `{devida}` exatamente uma vez"
        );
    }
    for item in ["pub fn lower_program(", "pub fn lower_program_composto("] {
        assert_eq!(
            codigo_executavel(fonte("context.rs")).matches(item).count(),
            1,
            "`{item}` deveria continuar público em src/ir/context.rs"
        );
    }

    // A reexportação da IR-3 é devida item a item: o conjunto reexportado pelo
    // pai tem de ser exatamente o conjunto de itens públicos de topo do irmão.
    // Um a menos apaga um caminho de `pinker_v0::ir::*`; um a mais o inventa.
    let reexportados = nomes_reexportados(&codigo_do_pai);
    let codigo_do_modelo = codigo_executavel(fonte("model.rs"));
    let publicos_do_modelo: BTreeSet<&str> = itens_publicos_de_topo(&codigo_do_modelo)
        .into_iter()
        .collect();
    assert_eq!(
        reexportados.len(),
        40,
        "a reexportação do modelo mudou de tamanho; o inventário da #601 previu quarenta itens"
    );
    assert_eq!(
        reexportados,
        publicos_do_modelo
            .iter()
            .map(|nome| (*nome).to_string())
            .collect::<BTreeSet<String>>(),
        "o conjunto reexportado de src/ir/model.rs divergiu dos itens públicos que ele declara"
    );
}

/// Os nomes reexportados pelo bloco `pub use model::{...};` do pai.
fn nomes_reexportados(codigo_do_pai: &str) -> BTreeSet<String> {
    let inicio = codigo_do_pai
        .find("pub use model::{")
        .expect("o pai reexporta o modelo");
    let resto = &codigo_do_pai[inicio + "pub use model::{".len()..];
    let fim = resto.find("};").expect("a reexportação fecha");
    resto[..fim]
        .split(',')
        .map(str::trim)
        .filter(|nome| !nome.is_empty())
        .map(str::to_string)
        .collect()
}

/// Os nomes dos itens públicos declarados no nível de topo de uma fonte.
fn itens_publicos_de_topo(codigo: &str) -> Vec<&str> {
    codigo
        .lines()
        .filter_map(|linha| linha.strip_prefix("pub "))
        .filter_map(|resto| {
            for palavra in ["fn ", "const ", "struct ", "enum ", "trait ", "type "] {
                if let Some(nome) = resto.strip_prefix(palavra) {
                    return Some(
                        nome.split(|c: char| !c.is_alphanumeric() && c != '_')
                            .next()
                            .unwrap_or(""),
                    );
                }
            }
            None
        })
        .collect()
}

/// Um irmão de produção que ganhasse um `#[cfg(test)]` esconderia tudo o que
/// viesse depois de qualquer censo que corte ali — e os censos de C1, C2 e C5
/// cortam ali.
#[test]
fn irmao_de_producao_nao_esconde_producao_atras_de_cfg_test() {
    for nome in IRMAOS_DE_PRODUCAO {
        assert_eq!(
            codigo_executavel(fonte(nome))
                .matches("#[cfg(test)]")
                .count(),
            0,
            "src/ir/{nome} é produção e não pode cortar o censo com um `#[cfg(test)]`"
        );
    }
}

/// C2 continua com uma dona só, e nenhum dos irmãos virou a segunda.
///
/// Os três cortes movem o consumo, nunca a regra: `src/method_dispatch.rs`
/// continua decidindo alcance, precedência, desempate e representante. A #621
/// desceu `select_impl_method` para `lowering.rs` e a #624 desceu
/// `select_representative` para `context.rs`, cada uma dentro da região que a
/// contém; a #626 não desceu consulta nenhuma, porque renderizar texto não
/// pergunta nada a C2 — e `render.rs` entra aqui justamente para provar que
/// continua não perguntando. As duas consultas da fase continuam sendo uma
/// cada, e nenhuma sobrou no pai.
#[test]
fn o_modulo_consome_c2_e_nao_cria_uma_segunda_autoridade() {
    let irmaos = irmaos_de_producao();
    let pai = codigo_executavel(pai());
    let modulo = codigo_executavel(&ir());

    // O vocabulário da precedência e a pergunta de alcance continuam fora da
    // fase — nos irmãos inclusive, que é onde a tentação nasce.
    for termo in [
        "NivelDeDespacho",
        "nivel_de_despacho",
        "PorUnidadeImportada",
    ] {
        for (nome, codigo) in &irmaos {
            assert_eq!(
                codigo.matches(termo).count(),
                0,
                "src/ir/{nome} voltou a aplicar `{termo}` por conta própria"
            );
        }
        assert_eq!(
            pai.matches(termo).count(),
            0,
            "src/ir.rs voltou a aplicar `{termo}` por conta própria"
        );
    }

    // Uma consulta por decisão, cada uma no irmão que a região levou. Todo
    // outro irmão — e o pai — fica em zero: é assim que uma cópia nova, em
    // qualquer arquivo do módulo, fica vermelha.
    for (decisao, dono) in [
        ("select_impl_method(", "lowering.rs"),
        ("select_representative(", "context.rs"),
    ] {
        for (nome, codigo) in &irmaos {
            let esperado = usize::from(nome == &dono);
            assert_eq!(
                codigo.matches(decisao).count(),
                esperado,
                "src/ir/{nome} deveria consultar `{decisao}` {esperado} vez(es)"
            );
        }
        assert_eq!(
            pai.matches(decisao).count(),
            0,
            "src/ir.rs voltou a consultar `{decisao}` por conta própria"
        );
        // O módulo inteiro continua com exatamente uma consulta por decisão: os
        // cortes não puderam nem duplicar nem apagar nenhuma delas.
        assert_eq!(
            modulo.matches(decisao).count(),
            1,
            "o módulo ir deveria consultar `{decisao}` exatamente uma vez"
        );
    }
}

/// Cada irmão de produção com o seu código executável, para os censos de
/// autoridade. A lista vem de [`IRMAOS_DE_PRODUCAO`]: um irmão novo entra nos
/// censos sem que nenhum deles precise ser reescrito.
fn irmaos_de_producao() -> Vec<(&'static str, String)> {
    IRMAOS_DE_PRODUCAO
        .iter()
        .map(|nome| (*nome, codigo_executavel(fonte(nome))))
        .collect()
}

/// C5 continua estruturada e C1 continua com dona única, também nos irmãos.
///
/// `lowering.rs` é o maior pedaço de lowering que existe e `context.rs` é onde
/// as assinaturas de intrínseca são declaradas: são os dois lugares em que
/// reconstruir a origem de um corpo default pela grafia do nome sintético, ou
/// repetir o censo de assinaturas de intrínseca, custaria menos linhas do que
/// consultar a autoridade. `render.rs` é o terceiro: renderizar é justamente
/// onde um nome sintético passa como texto, e imprimir por grafia o que a
/// autoridade materializou seria reconstrução — C5 é textual e nasceria aqui.
#[test]
fn os_irmaos_nao_reconstroem_c5_nem_duplicam_c1() {
    let irmaos = irmaos_de_producao();
    for (nome, codigo) in &irmaos {
        for termo in [
            "__impl_",
            "__trait_default_check_",
            "trait_default_body",
            "TraitDefaultBody",
        ] {
            assert_eq!(
                codigo.matches(termo).count(),
                0,
                "src/ir/{nome} voltou a decidir origem de corpo default por `{termo}`"
            );
        }
    }

    // C1: a região `ir.lowering.assinaturas-intrinsecos` desceu com a IR-2, e
    // com ela a única leitura do registry declarativo. Ela continua sendo uma
    // leitura só, e continua sendo leitura: os helpers de assinatura
    // (`builtin_sig`, `builtin_nominal_sig`) desceram com a IR-3 para o irmão
    // que declara o modelo, continuam sendo uma definição só, e uma cópia deles
    // em qualquer outro arquivo seria censo local — é o que o bloco logo abaixo
    // prova.
    for (nome, codigo) in &irmaos {
        let esperado = usize::from(nome == &"context.rs");
        assert_eq!(
            codigo.matches("intrinsics::registry").count(),
            esperado,
            "src/ir/{nome} deveria consultar o registry declarativo {esperado} vez(es)"
        );
        if *nome == "context.rs" || *nome == "model.rs" {
            continue;
        }
        for termo in ["builtin_sig", "builtin_nominal_sig"] {
            assert_eq!(
                codigo.matches(termo).count(),
                0,
                "src/ir/{nome} passou a manter censo próprio de intrínseca por `{termo}`"
            );
        }
    }

    // Os dois helpers de assinatura desceram com a IR-3 e continuam sendo uma
    // definição só, agora em `model.rs`. `context.rs` — que declara as
    // assinaturas de intrínseca — continua sendo o único consumidor, e não tem
    // cópia própria: uma segunda definição em qualquer arquivo seria censo
    // local, que é o que C1 proíbe.
    let modelo = codigo_executavel(fonte("model.rs"));
    let contexto = codigo_executavel(fonte("context.rs"));
    for termo in ["fn builtin_sig(", "fn builtin_nominal_sig("] {
        assert_eq!(
            modelo.matches(termo).count(),
            1,
            "`{termo}` deveria ter uma definição só, em src/ir/model.rs"
        );
        assert_eq!(
            contexto.matches(termo).count(),
            0,
            "src/ir/context.rs passou a manter cópia própria de `{termo}`"
        );
    }
    assert_eq!(
        codigo_executavel(pai())
            .matches("intrinsics::registry")
            .count(),
        0,
        "src/ir.rs voltou a consultar o registry por conta própria"
    );
}

/// A validação da IR e a fronteira de CFG não desceram com nenhum dos cortes.
///
/// `src/ir_validate.rs` e `src/cfg_ir.rs` continuam donos do que sempre foram;
/// os irmãos constroem `InstructionIR` estruturada e param aí.
#[test]
fn os_irmaos_nao_absorveram_validacao_de_ir_nem_fronteira_de_cfg() {
    for nome in IRMAOS_DE_PRODUCAO {
        let irmao = codigo_executavel(fonte(nome));
        for termo in [
            "ir_validate",
            "cfg_ir",
            "validate_program",
            "BasicBlock",
            "CfgProgram",
        ] {
            assert_eq!(
                irmao.matches(termo).count(),
                0,
                "src/ir/{nome} passou a executar `{termo}`, que é de outra autoridade"
            );
        }
    }
}

/// A ordem de fase não mudou: quem monta o contexto, quem constrói o lowerer e
/// quem despacha constantes continuam sendo o mesmo código, na mesma ordem.
///
/// Antes da IR-2 a orquestração morava no pai; agora mora em `context.rs`, e é
/// dali que ela chama os `pub(super)` do outro irmão. Este teste é o que a
/// sensitivity M10 da #624 perturba: mudar a fase de lugar, ou inverter a
/// montagem do contexto e o despacho de constantes, fica vermelho aqui.
#[test]
fn o_modulo_continua_orquestrando_o_lowering_na_mesma_ordem() {
    let contexto = codigo_executavel(fonte("context.rs"));
    let pai = codigo_executavel(pai());
    assert_eq!(
        contexto
            .matches("FunctionLowerer::new(&context).lower_function(")
            .count(),
        2,
        "src/ir/context.rs deveria construir o lowerer e abaixar a função nas duas entradas"
    );
    assert_eq!(
        contexto
            .matches("lower_const(const_decl, &context)")
            .count(),
        1,
        "src/ir/context.rs deveria despachar constantes exatamente uma vez"
    );
    for chamada in [
        "FunctionLowerer::new(&context).lower_function(",
        "lower_const(const_decl, &context)",
        "LoweringContext::from_program_composto(",
    ] {
        assert_eq!(
            pai.matches(chamada).count(),
            0,
            "`{chamada}` ficou para trás em src/ir.rs"
        );
    }
    // As duas chamadas moram na orquestração do programa, depois da montagem do
    // contexto: um deslocamento de fase mudaria este vizinho.
    let montagem = contexto
        .find("LoweringContext::from_program_composto(")
        .expect("a montagem do contexto continua no módulo");
    let despacho = contexto
        .find("lower_const(const_decl, &context)")
        .expect("o despacho de constantes continua no módulo");
    assert!(
        montagem < despacho,
        "o despacho de constantes passou a acontecer antes da montagem do contexto"
    );
}

/// A renderização continua sendo a mesma cadeia, na mesma direção.
///
/// É o observável próprio da IR-4, o que a sensitivity M10 perturba. A entrada
/// pública `render_program` não é da unidade: ela fica no pai, imprime módulo,
/// modo e constantes e delega ao irmão — nessa ordem. O irmão desce dali para
/// blocos, instruções, padrões e valores. Trocar a ordem das fases da entrada,
/// quebrar um elo da cadeia ou inverter a direção da dependência (mover
/// `render_program` para o irmão, ou devolver um dos corpos ao pai só para
/// deixar o import mais bonito) fica vermelho aqui.
#[test]
fn a_renderizacao_continua_delegando_do_pai_para_o_irmao() {
    let pai = codigo_executavel(pai());
    let render = codigo_executavel(fonte("render.rs"));

    // A entrada pública ficou no pai e é dele que sai a delegação.
    assert_eq!(
        pai.matches("fn render_program(").count(),
        1,
        "a entrada pública da renderização deveria continuar em src/ir.rs"
    );
    assert_eq!(
        render.matches("fn render_program(").count(),
        0,
        "src/ir/render.rs absorveu a entrada pública, que não é da unidade"
    );
    assert_eq!(
        pai.matches("use render::{line, render_function, render_value};")
            .count(),
        1,
        "src/ir.rs deveria importar do irmão exatamente os três símbolos que chama"
    );

    // As quatro fases da entrada, na ordem em que sempre imprimiram. A busca
    // começa no corpo de `render_program` porque `consts:` e `functions:`
    // também são nomes de campo de `ProgramIR`, declarados antes dela.
    let corpo = {
        let inicio = fonte("ir.rs")
            .find("pub fn render_program(")
            .expect("render_program continua no pai");
        &fonte("ir.rs")[inicio..]
    };
    let mut anterior = 0;
    for fase in ["module {}", "mode {}", "consts:", "functions:"] {
        let posicao = corpo
            .find(fase)
            .unwrap_or_else(|| panic!("`{fase}` deixou de ser impressa por render_program"));
        assert!(
            posicao > anterior,
            "a fase `{fase}` da renderização mudou de posição na entrada pública"
        );
        anterior = posicao;
    }

    // A cadeia do irmão: cada elo chamado de dentro do próprio irmão, e o pai
    // sem nenhuma cópia de nenhum deles.
    for elo in [
        "render_block(",
        "render_instruction(",
        "render_enum_pattern(",
    ] {
        assert!(
            render.matches(elo).count() >= 2,
            "`{elo}` deveria ser definido e chamado dentro de src/ir/render.rs"
        );
        assert_eq!(
            pai.matches(elo).count(),
            0,
            "`{elo}` não é chamado pelo pai e não deveria aparecer em src/ir.rs"
        );
    }
}

/// O observável próprio da IR-3: a representação e a identidade resolvida
/// continuam com uma dona só, e o corte parou onde a região parava.
///
/// A unidade é o modelo de dados, não tudo o que fala de tipo. Os quatro `impl`
/// que operam sobre os tipos movidos — `TypeIR`, `ScalarTypeIR`, `UnaryOpIR` e
/// `BinaryOpIR` — não estão em nenhuma das duas regiões e ficam no pai, junto do
/// `impl LoweringContext` e da conversão AST→`TypeIR`. Arrastá-los junto seria
/// mover código que não é do corte só para deixar o arquivo mais bonito; deixar
/// o modelo para trás seria não executar a unidade. Este teste é o que a
/// sensitivity M12 da #632 perturba.
#[test]
fn a_representacao_e_a_identidade_continuam_com_uma_dona_so() {
    let pai = codigo_executavel(pai());
    let modelo = codigo_executavel(fonte("model.rs"));

    // A tabela de identidade interna num lugar só, e é o irmão.
    for dono in ["impl ResolvedTypeTable {", "impl TypeRefIR {", "fn intern("] {
        assert_eq!(
            modelo.matches(dono).count(),
            1,
            "`{dono}` deveria morar em src/ir/model.rs exatamente uma vez"
        );
        assert_eq!(
            pai.matches(dono).count(),
            0,
            "`{dono}` ficou para trás em src/ir.rs"
        );
    }

    // Os `impl` que não são da unidade continuam no pai, e o irmão não tem cópia.
    for retido in [
        "impl TypeIR {",
        "impl ScalarTypeIR {",
        "impl UnaryOpIR {",
        "impl BinaryOpIR {",
        "impl LoweringContext {",
    ] {
        assert_eq!(
            pai.matches(retido).count(),
            1,
            "`{retido}` não é da unidade e deveria continuar em src/ir.rs"
        );
        assert_eq!(
            modelo.matches(retido).count(),
            0,
            "src/ir/model.rs absorveu `{retido}`, que não é do corte"
        );
    }

    // A direção da dependência é a mesma de sempre: o pai consome o modelo do
    // irmão pelos quatro símbolos privados que chama, e o irmão não conhece o
    // estado do pai.
    for estado_do_pai in ["LoweringContext", "FunctionLowerer", "TypedValueIR"] {
        assert_eq!(
            modelo.matches(estado_do_pai).count(),
            0,
            "src/ir/model.rs passou a depender de `{estado_do_pai}`, que é estado do pai"
        );
    }
}
