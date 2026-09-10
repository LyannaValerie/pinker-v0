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
use crate::method_identity::{self, MethodIdentity, QualifiedMethodResolution};
use crate::source_map::SourceId;
use crate::token::{Position, Span};
use crate::union_canon;
use std::collections::{BTreeMap, HashMap, HashSet};

mod context;
mod lowering;
mod model;
mod render;

use model::{
    builtin_nominal_sig, builtin_sig, expected_key_for_representation,
    intern_representation_identity,
};
use render::{line, render_function, render_value};

pub use context::{lower_program, lower_program_composto};
pub(crate) use model::is_generic_map_intrinsic;
pub use model::{
    validate_resolved_type_reference, validate_resolved_type_table, validate_union_match_coverage,
    validate_union_member_identity, validate_union_member_reference, validate_union_reference,
    validate_union_registry, validate_union_registry_identities, BinaryOpIR, BindingIR, BlockIR,
    ConstIR, EnumMatchArmIR, EnumMatchIR, EnumPatternIR, EnumPatternPayloadIR, FalarArgIR,
    FunctionIR, InlineAsmOperandIR, InstructionIR, LocalIR, MapKeyIR, MapValueIR,
    NominalTypeKindIR, ProgramIR, ResolvedSignatureIR, ResolvedTypeIR, ResolvedTypeId,
    ResolvedTypeParts, ResolvedTypeTable, ScalarTypeIR, TypeIR, TypeRefIR, UnaryOpIR,
    UnionMatchArmIR, UnionMatchIR, UnionMemberIR, UnionTypeIR, UnionTypeId, ValueIR,
};

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
