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
    ContinueStmt, ElseBlock, Expr, ExprKind, FalarStmt, FunctionDecl, IfStmt, InlineAsmStmt, Item,
    LetStmt, Program, ReturnStmt, Stmt, StructDecl, Type, UnaryOp, WhileStmt,
};
use crate::error::PinkerError;
use crate::layout;
use crate::token::Span;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingIR {
    pub source_name: String,
    pub slot: String,
    pub ty: TypeIR,
}

/// Variável local declarada por `nova`. `is_mut` reflete a palavra-chave `muda`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalIR {
    pub source_name: String,
    pub slot: String,
    pub ty: TypeIR,
    pub is_mut: bool,
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
        span: Span,
    },
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
    },
    Call {
        callee: String,
        args: Vec<ValueIR>,
        ret_type: TypeIR,
    },
    // Fase 242: referência a função top-level como valor (materializa o
    // descritor callable {code_ptr, env_ptr}; env_ptr nulo/estático aqui).
    FunctionRef(String),
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
}

/// Tipos do sistema de tipos da v0. `Nulo` representa ausência de retorno (funções sem `-> tipo`);
/// não é exposto como tipo de usuário — apenas interno ao pipeline.
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
    FixedArray { element: ScalarTypeIR, size: u64 },
    Struct,
    Pointer { is_volatile: bool },
    // Fase 242: callable materializado — handle de 1 palavra para descritor
    // {code_ptr, env_ptr}. Mesma categoria de valor que Pointer/ListBombom.
    Function,
    // Fase 244: handle de uma palavra para um descritor
    // `{data_ptr, vtable_ptr}` de objeto de trato.
    //
    // A identidade nominal do trato permanece nos nós `MakeTraitObject` e
    // `TraitCall`, pois `TypeIR` continua pequeno e `Copy`.
    TraitObject,
    Nulo,
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

#[derive(Clone)]
struct FunctionSigIR {
    ret_type: TypeIR,
    ret_struct_name: Option<String>,
}

// Fase 244: assinatura operacional de um método de trato objetificável.
// `param_types` não inclui o receiver contextual `si`.
#[derive(Clone)]
struct TraitMethodMetaIR {
    name: String,
    param_types: Vec<TypeIR>,
    ret_type: TypeIR,
    ret_struct_name: Option<String>,
    ret_trait_name: Option<String>,
}

#[derive(Clone)]
struct TraitMetaIR {
    methods: Vec<TraitMethodMetaIR>,
}

#[derive(Clone)]
struct BindingState {
    slot: String,
    ty: TypeIR,
    struct_name: Option<String>,
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
    callable_ret_types: HashMap<String, TypeIR>,
    // Fase 243: FunctionDecl de toda função do programa (inclusive
    // closures sintéticas `__anon_carinho_*`), para permitir a resolução
    // lazy de closures no ponto de criação (`FunctionLowerer::resolve_closure`)
    // abaixar o corpo da closure sob demanda, com o ambiente correto.
    all_functions: HashMap<String, FunctionDecl>,
    // Estado mutável compartilhado entre todos os `FunctionLowerer` da
    // mesma `lower_program`: capturas já resolvidas e corpos de closure já
    // abaixados. `RefCell` porque `FunctionLowerer` só empresta `context`
    // imutavelmente (mesmo padrão de `LoweringContext` imutável entre
    // lowerings independentes, só o registro de closures precisa mutar).
    closure_state: std::cell::RefCell<ClosureLoweringState>,
}

#[derive(Default)]
struct ClosureLoweringState {
    captures: HashMap<String, Vec<(String, TypeIR)>>,
    // Vec (não HashMap) para preservar ordem determinística de resolução
    // (DFS na ordem de criação) na lista final de funções do programa.
    lowered: Vec<(String, FunctionIR)>,
    // Fase 243: nome do wrapper `__fnref_env_<nome>` -> tipo de retorno da
    // função original — permite que a inferência de `callable_ret_type`
    // (Fase 242, caso sem anotação explícita) continue funcionando quando
    // `ValueIR::FunctionRef` passa a apontar para o wrapper em vez do nome
    // original (`function_sigs` não conhece o wrapper).
    wrapper_ret_types: HashMap<String, TypeIR>,
}

// Leques na IR: sem carga, o valor é o próprio discriminante imediato; com
// carga, o valor é um handle opaco (bombom) para o estado do runtime.
#[derive(Clone)]
struct EnumInfoIR {
    has_payload: bool,
    variants: HashMap<String, (u64, Vec<TypeIR>)>,
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
    // `LoweringContext.callable_ret_types`); ausência = erro claro no
    // lowering da chamada, não pânico.
    callable_ret_types: HashMap<String, TypeIR>,
    // Slot local/parâmetro -> nome nominal de `trato<Nome>`.
    trait_object_names: HashMap<String, String>,
}

struct TypedValueIR {
    value: ValueIR,
    ty: TypeIR,
    struct_name: Option<String>,
    ptr_array_bombom_size: Option<u64>,
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

    let ClosureLoweringState { lowered, .. } = context.closure_state.into_inner();
    functions.extend(lowered.into_iter().map(|(_, f)| f));

    Ok(ProgramIR {
        module_name: context.module_name,
        is_freestanding: program.freestanding.is_some(),
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

        let mut type_aliases = HashMap::new();
        let mut struct_decls = HashMap::new();
        let mut struct_names = HashSet::new();
        let mut enum_variants: HashMap<String, EnumInfoIR> = HashMap::new();
        for item in &program.items {
            if let Item::TypeAlias(alias) = item {
                type_aliases.insert(alias.name.clone(), alias.target.clone());
            } else if let Item::Struct(struct_decl) = item {
                struct_names.insert(struct_decl.name.clone());
                struct_decls.insert(struct_decl.name.clone(), struct_decl.clone());
            } else if let Item::Enum(enum_decl) = item {
                // O tipo leque abaixa para bombom na IR (discriminante imediato
                // ou handle); registrar como alias faz toda anotação de tipo
                // com o nome do leque resolver sozinha.
                type_aliases.insert(enum_decl.name.clone(), Type::Bombom(enum_decl.span));
                let variants = enum_decl
                    .variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        let payloads = variant
                            .payloads
                            .iter()
                            .map(|ty| match ty {
                                Type::Verso(_) => TypeIR::Verso,
                                _ => TypeIR::Bombom,
                            })
                            .collect::<Vec<_>>();
                        (variant.name.clone(), (index as u64, payloads))
                    })
                    .collect();
                enum_variants.insert(
                    enum_decl.name.clone(),
                    EnumInfoIR {
                        has_payload: enum_decl
                            .variants
                            .iter()
                            .any(|variant| !variant.payloads.is_empty()),
                        variants,
                    },
                );
            }
        }
        for (alias_name, target) in type_aliases.clone() {
            if let Type::Enum { name, .. } = target {
                if let Some(info) = enum_variants.get(&name).cloned() {
                    enum_variants.insert(alias_name, info);
                }
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

                    let ret_struct_name = method.ret_type.as_ref().and_then(|ty| {
                        resolve_struct_name_from_type(ty, &type_aliases, &struct_names)
                    });
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
                        ret_struct_name,
                        ret_trait_name,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            traits.insert(trait_decl.name.clone(), TraitMetaIR { methods });
        }

        let mut function_sigs = HashMap::new();
        let mut global_consts = HashMap::new();
        let mut callable_ret_types = HashMap::new();
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

                    function_sigs.insert(
                        function.name.clone(),
                        FunctionSigIR {
                            ret_type: TypeIR::from_ast_option_with_context(
                                function.ret_type.as_ref(),
                                &type_aliases,
                                &struct_names,
                            )?,
                            ret_struct_name: function.ret_type.as_ref().and_then(|ty| {
                                resolve_struct_name_from_type(ty, &type_aliases, &struct_names)
                            }),
                        },
                    );
                    // Fase 242: quando a função retorna um valor callable,
                    // registra o ret_type DESSE callable (um nível), para
                    // permitir chamada indireta imediata sobre o resultado.
                    if let Some(Type::Function { ret, .. }) = function.ret_type.as_ref() {
                        let inner_ret =
                            TypeIR::from_ast_with_context(ret, &type_aliases, &struct_names)?;
                        callable_ret_types.insert(function.name.clone(), inner_ret);
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
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ouvir_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ouvir_verso_ou".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "aleatorio_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "aleatorio_proximo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::ListBombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_anexar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_obter".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_tamanho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_definir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_tirar_ultimo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::ListVerso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_anexar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_obter".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_tamanho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_definir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_tirar_ultimo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_bombom_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::MapVersoBombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_bombom_definir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_bombom_obter".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_bombom_tem".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_bombom_tamanho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_verso_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::MapVersoVerso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_verso_definir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_verso_obter".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_verso_tem".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_verso_tamanho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_verso_remover".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_bombom_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::MapBombomBombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_bombom_definir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_bombom_obter".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_bombom_tem".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_bombom_tamanho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_bombom_remover".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_verso_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::MapBombomVerso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_verso_definir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_verso_obter".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_verso_tem".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_verso_tamanho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_bombom_verso_remover".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_leque_criar_0".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_leque_anexar_b".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_leque_anexar_v".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_leque_tag".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_leque_carga_b".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "__pinker_internal_leque_carga_v".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "argumento".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "argumento_ou".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "tem_chave".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "tem_argumento_nomeado".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "pedir_argumento".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "argumento_nomeado_ou".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "tem_flag".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ambiente_ou".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "buscar_contexto".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "argumento_nomeado_ou_ambiente_ou".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "caminho_existe".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "e_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "e_diretorio".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "juntar_caminho".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "tamanho_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "e_vazio".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "criar_diretorio".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "remover_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "remover_diretorio".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "diretorio_atual".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "quantos_argumentos".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "tem_argumento".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "sair".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "abrir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ler_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ler_verso_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ler_arquivo_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "arquivo_ou".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "fechar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "criar_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "abrir_anexo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "escrever".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "escrever_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "truncar_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "anexar_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "juntar_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "tamanho_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "indice_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "contem_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "comeca_com".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "termina_com".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "igual_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "vazio_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "aparar_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "minusculo_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "maiusculo_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "indice_verso_em".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        // Fase 140
        function_sigs.insert(
            "buscar_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "nao_vazio_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Logica,
                ret_struct_name: None,
            },
        );
        // Fase 137
        function_sigs.insert(
            "dividir_verso_em".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "dividir_verso_contar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        // Fase 138
        function_sigs.insert(
            "substituir_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        // Fase 139
        function_sigs.insert(
            "juntar_verso_com".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "formatar_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        // Fase 158
        function_sigs.insert(
            "ler_linha_csv_bombom".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::ListBombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "emitir_linha_csv_bombom".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "ler_json_plano_bombom".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::MapVersoBombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "emitir_json_plano_bombom".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        // Fase 160
        function_sigs.insert(
            "tempo_unix".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "formatar_tempo_unix".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        // Fase 161
        function_sigs.insert(
            "executar_processo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        // Fase 165
        function_sigs.insert(
            "executar_com_entrada".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        // Fase 166
        function_sigs.insert(
            "pipeline_minimo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        // Fase 163
        function_sigs.insert(
            "capturar_stdout".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        // Fase 164
        function_sigs.insert(
            "capturar_stderr".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "afirmar".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "dormir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "copiar_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "renomear_arquivo".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "verso_para_bombom".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "bombom_para_verso".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Verso,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "aleatorio_entre".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Bombom,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "mapa_verso_bombom_remover".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_bombom_inserir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );
        function_sigs.insert(
            "lista_verso_inserir".to_string(),
            FunctionSigIR {
                ret_type: TypeIR::Nulo,
                ret_struct_name: None,
            },
        );

        Ok(Self {
            module_name,
            function_sigs,
            global_consts,
            type_aliases,
            struct_decls,
            struct_names,
            struct_fields,
            struct_field_offsets,
            enum_variants,
            traits,
            function_ret_trait_names,
            callable_ret_types,
            all_functions,
            closure_state: std::cell::RefCell::new(ClosureLoweringState::default()),
        })
    }
    // @pinker-nav:end ir.lowering.assinaturas-intrinsecos

    fn resolve_type(&self, ty: &Type) -> Result<TypeIR, PinkerError> {
        TypeIR::from_ast_with_context(ty, &self.type_aliases, &self.struct_names)
    }
}

impl<'a> FunctionLowerer<'a> {
    // @pinker-nav:start ir.lowering.funcoes-blocos
    // @pinker-nav:domain lowering
    // @pinker-nav:layer ir
    // @pinker-nav:summary Configuração do `FunctionLowerer` e lowering de funções e blocos estruturados: constrói o lowerer, aloca os parâmetros como bindings, abaixa o bloco de entrada, coleta locais e tipo de retorno em `FunctionIR`, e percorre `BlockIR` abrindo/fechando escopo opcional. Inclui os resolvedores de método de `impl` (direto e qualificado por trato) consultados pelo lowering de expressões. Preserva a estrutura aninhada; não divide o fluxo em blocos básicos de CFG.
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
            callable_ret_types: HashMap::new(),
            trait_object_names: HashMap::new(),
        }
    }

    fn impl_receiver_key(typed: &TypedValueIR) -> Option<String> {
        if typed.ty == TypeIR::Struct {
            return typed.struct_name.clone();
        }
        Some(typed.ty.name().to_string())
    }

    fn resolve_impl_method(&self, receiver: &TypedValueIR, method_name: &str) -> Option<String> {
        let receiver_key = Self::impl_receiver_key(receiver)?;
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
        let receiver_key = Self::impl_receiver_key(receiver)?;
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
            ExprKind::Call(callee, _) => match &callee.kind {
                ExprKind::Ident(function_name) => self
                    .context
                    .function_ret_trait_names
                    .get(function_name)
                    .cloned(),
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
            | TypeIR::Pointer { .. }
            | TypeIR::Function => Ok(layout::POINTER_SIZE),

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
                let struct_name = value.struct_name.as_ref().ok_or_else(|| PinkerError::Ir {
                    msg: "snapshot de ninho sem identidade nominal".to_string(),
                    span,
                })?;

                let ast_type = Type::Struct {
                    name: struct_name.clone(),
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
            struct_name: method.ret_struct_name,
            ptr_array_bombom_size: None,
        })
    }

    fn lower_function(mut self, function: &FunctionDecl) -> Result<FunctionIR, PinkerError> {
        self.push_scope();

        for param in &function.params {
            let binding = self.allocate_binding(
                &param.name,
                self.context.resolve_type(&param.ty)?,
                resolve_struct_name_from_type(
                    &param.ty,
                    &self.context.type_aliases,
                    &self.context.struct_names,
                ),
                pointer_to_bombom_array_size(&param.ty, &self.context.type_aliases),
                None,
            );

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
                self.callable_ret_types.insert(binding.slot.clone(), ret_ty);
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
            .wrapper_ret_types
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
                resolve_struct_name_from_type(
                    &param.ty,
                    &wrapper.context.type_aliases,
                    &wrapper.context.struct_names,
                ),
                pointer_to_bombom_array_size(&param.ty, &wrapper.context.type_aliases),
                None,
            );
            call_args.push(ValueIR::Local(binding.slot.clone()));
            wrapper_params.push(binding);
        }
        let ret_type = TypeIR::from_ast_option_with_context(
            function.ret_type.as_ref(),
            &wrapper.context.type_aliases,
            &wrapper.context.struct_names,
        )?;
        let env_binding = wrapper.allocate_binding(
            "__env",
            TypeIR::Pointer { is_volatile: false },
            None,
            None,
            None,
        );
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
        state
            .wrapper_ret_types
            .insert(wrapper_name.clone(), ret_type);
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
        let mut captures: Vec<(String, TypeIR)> = Vec::new();
        let mut capture_values: Vec<ValueIR> = Vec::new();
        for candidate in &free {
            if param_names.contains(candidate) {
                continue;
            }
            let Some(binding) = self.resolve_existing_binding(candidate) else {
                continue;
            };
            captures.push((candidate.clone(), binding.ty));
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
        Ok(TypedValueIR {
            value: ValueIR::MakeClosure {
                function_name: name.to_string(),
                captures: capture_values,
            },
            ty: TypeIR::Function,
            struct_name: None,
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
        captures: &[(String, TypeIR)],
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
        let env_binding = self.allocate_binding(
            "__env",
            TypeIR::Pointer { is_volatile: false },
            None,
            None,
            None,
        );

        let mut prelude = Vec::new();
        for (index, (capture_name, capture_ty)) in captures.iter().enumerate() {
            // Capturas entram no escopo ANTES dos parâmetros para que um
            // parâmetro homônimo possa sombreá-las (§14.3) — a inserção
            // posterior do parâmetro no mesmo mapa de escopo sobrescreve.
            let capture_binding =
                self.allocate_binding(capture_name, *capture_ty, None, None, Some(false));
            let ptr_expr = ValueIR::Binary {
                op: BinaryOpIR::Add,
                lhs: Box::new(ValueIR::Local(env_binding.slot.clone())),
                rhs: Box::new(ValueIR::Int((index as u64) * 8)),
            };
            prelude.push(InstructionIR::Let {
                slot: capture_binding.slot,
                value: ValueIR::Deref {
                    ptr: Box::new(ptr_expr),
                    result_type: *capture_ty,
                    is_volatile: false,
                },
                span: function.span,
            });
        }

        for param in &function.params {
            let binding = self.allocate_binding(
                &param.name,
                self.context.resolve_type(&param.ty)?,
                resolve_struct_name_from_type(
                    &param.ty,
                    &self.context.type_aliases,
                    &self.context.struct_names,
                ),
                pointer_to_bombom_array_size(&param.ty, &self.context.type_aliases),
                None,
            );

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
                self.callable_ret_types.insert(binding.slot.clone(), ret_ty);
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
                        Ok(InstructionIR::Assign {
                            slot: binding.slot,
                            value: value.value,
                            span: assign_stmt.span,
                        })
                    }
                    AssignTarget::Deref(ptr_expr) => {
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
                            value_type: value.ty,
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
                        let Some(base_struct_name) = base_lowered.struct_name.as_ref() else {
                            return Err(PinkerError::Ir {
                                msg: "escrita a campo exige base do tipo 'ninho' no lowering IR"
                                    .to_string(),
                                span: assign_stmt.span,
                            });
                        };
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
        }
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
        Ok(InstructionIR::InlineAsm {
            chunks: inline_asm_stmt.chunks.clone(),
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
                    None,
                    None,
                    Some(let_stmt.is_mut),
                );
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
                    None,
                    None,
                    Some(let_stmt.is_mut),
                );
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
        let struct_name = let_stmt
            .ty
            .as_ref()
            .and_then(|annotated_ty| {
                resolve_struct_name_from_type(
                    annotated_ty,
                    &self.context.type_aliases,
                    &self.context.struct_names,
                )
            })
            .or(value.struct_name.clone());
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
        let callable_ret_ty = if ty == TypeIR::Function {
            if let Some(Type::Function { ret, .. }) = let_stmt.ty.as_ref() {
                Some(self.context.resolve_type(ret)?)
            } else {
                match &value.value {
                    ValueIR::FunctionRef(name) => self
                        .context
                        .function_sigs
                        .get(name)
                        .map(|sig| sig.ret_type)
                        .or_else(|| {
                            self.context
                                .closure_state
                                .borrow()
                                .wrapper_ret_types
                                .get(name)
                                .copied()
                        }),
                    ValueIR::MakeClosure { function_name, .. } => self
                        .context
                        .function_sigs
                        .get(function_name)
                        .map(|sig| sig.ret_type),
                    ValueIR::Local(slot) => self.callable_ret_types.get(slot).copied(),
                    ValueIR::Call { callee, .. } => {
                        self.context.callable_ret_types.get(callee).copied()
                    }
                    _ => None,
                }
            }
        } else {
            None
        };
        let binding = self.allocate_binding(
            &let_stmt.name,
            ty,
            struct_name,
            ptr_array_bombom_size,
            Some(let_stmt.is_mut),
        );
        if let Some(ret_ty) = callable_ret_ty {
            self.callable_ret_types.insert(binding.slot.clone(), ret_ty);
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
    // @pinker-nav:summary Grande despachante que abaixa expressões AST para `TypedValueIR` (valor + `TypeIR` + nome de struct + metadados de ponteiro-para-array): literais, identificadores locais e constantes globais, operadores unários/binários, dereferência, chamadas diretas (com tipo de retorno vindo do catálogo de assinaturas), métodos de `impl` e qualificados, intrínsecas genéricas de lista/mapa direcionadas ao nome monomórfico, construção/leitura de leque (discriminante/handle), acesso a campo e offset de struct, indexação, cast, `peso` e `alinhamento`. Consome informação já validada; não executa a expressão nem seleciona instruções de máquina.
    fn lower_value(&mut self, expr: &Expr) -> Result<TypedValueIR, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(value) => Ok(TypedValueIR {
                value: ValueIR::Int(*value),
                ty: TypeIR::Bombom,
                struct_name: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::BoolLit(value) => Ok(TypedValueIR {
                value: ValueIR::Bool(*value),
                ty: TypeIR::Logica,
                struct_name: None,
                ptr_array_bombom_size: None,
            }),
            ExprKind::StringLit(value) => Ok(TypedValueIR {
                value: ValueIR::String(value.clone()),
                ty: TypeIR::Verso,
                struct_name: None,
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
                    struct_name: None,
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
                    struct_name: None,
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
                        struct_name: binding.struct_name,
                        ptr_array_bombom_size: binding.ptr_array_bombom_size,
                    });
                }

                if let Some(ty) = self.context.global_consts.get(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::GlobalConst(name.clone()),
                        ty: *ty,
                        struct_name: None,
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
                    return Ok(TypedValueIR {
                        value: ValueIR::FunctionRef(wrapper_name),
                        ty: TypeIR::Function,
                        struct_name: None,
                        ptr_array_bombom_size: None,
                    });
                }

                Err(PinkerError::Ir {
                    msg: format!("lowering falhou ao resolver identificador '{}'", name),
                    span: expr.span,
                })
            }
            ExprKind::Unary(op, operand) => {
                let operand = self.lower_value(operand)?;
                if *op == UnaryOp::Deref {
                    let TypeIR::Pointer { is_volatile } = operand.ty else {
                        return Err(PinkerError::Ir {
                            msg: "dereferência exige operando do tipo seta no lowering IR"
                                .to_string(),
                            span: expr.span,
                        });
                    };
                    let (result_type, result_struct_name) =
                        if let Some(struct_name) = operand.struct_name {
                            (TypeIR::Struct, Some(struct_name))
                        } else if let Some(size) = operand.ptr_array_bombom_size {
                            (
                                TypeIR::FixedArray {
                                    element: ScalarTypeIR::Bombom,
                                    size,
                                },
                                None,
                            )
                        } else {
                            (TypeIR::Bombom, None)
                        };
                    return Ok(TypedValueIR {
                        value: ValueIR::Deref {
                            ptr: Box::new(operand.value),
                            result_type,
                            is_volatile,
                        },
                        ty: result_type,
                        struct_name: result_struct_name,
                        ptr_array_bombom_size: None,
                    });
                }
                Ok(TypedValueIR {
                    value: ValueIR::Unary {
                        op: UnaryOpIR::from_ast(*op),
                        operand: Box::new(operand.value),
                    },
                    ty: match op {
                        UnaryOp::Neg => operand.ty,
                        UnaryOp::Not => TypeIR::Logica,
                        UnaryOp::BitNot => operand.ty,
                        UnaryOp::Deref => unreachable!("deref tratada acima"),
                    },
                    struct_name: None,
                    ptr_array_bombom_size: None,
                })
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_is_int_lit = matches!(lhs.kind, ExprKind::IntLit(_));
                let lhs = self.lower_value(lhs)?;
                let rhs = self.lower_value(rhs)?;
                Ok(TypedValueIR {
                    value: ValueIR::Binary {
                        op: BinaryOpIR::from_ast(*op),
                        lhs: Box::new(lhs.value),
                        rhs: Box::new(rhs.value),
                    },
                    ty: match op {
                        BinaryOp::LogicalAnd | BinaryOp::LogicalOr => TypeIR::Logica,
                        BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Mod
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor
                        | BinaryOp::Shl
                        | BinaryOp::Shr => {
                            if lhs_is_int_lit && rhs.ty.is_integer() {
                                rhs.ty
                            } else {
                                lhs.ty
                            }
                        }
                        BinaryOp::Eq
                        | BinaryOp::Neq
                        | BinaryOp::Lt
                        | BinaryOp::Lte
                        | BinaryOp::Gt
                        | BinaryOp::Gte => TypeIR::Logica,
                    },
                    struct_name: None,
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
                                let anexar = match payload_ty {
                                    TypeIR::Verso => "__pinker_internal_leque_anexar_v",
                                    _ => "__pinker_internal_leque_anexar_b",
                                };
                                chain = ValueIR::Call {
                                    callee: anexar.to_string(),
                                    args: vec![chain, payload.value],
                                    ret_type: TypeIR::Bombom,
                                };
                            }
                            return Ok(TypedValueIR {
                                value: chain,
                                ty: TypeIR::Bombom,
                                struct_name: None,
                                ptr_array_bombom_size: None,
                            });
                        }
                    }
                }
                if let ExprKind::FieldAccess { base, field } = &callee.kind {
                    if let ExprKind::Ident(trait_name) = &base.kind {
                        if !args.is_empty() {
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
                                    struct_name: self
                                        .context
                                        .function_sigs
                                        .get(&function_name)
                                        .and_then(|sig| sig.ret_struct_name.clone()),
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
                                    Self::impl_receiver_key(&receiver)
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
                        struct_name: self
                            .context
                            .function_sigs
                            .get(&function_name)
                            .and_then(|sig| sig.ret_struct_name.clone()),
                        ptr_array_bombom_size: None,
                    });
                }

                let ExprKind::Ident(name) = &callee.kind else {
                    return Err(PinkerError::Ir {
                        msg: "IR da v0 suporta apenas chamadas diretas por nome".to_string(),
                        span: expr.span,
                    });
                };

                // Fase 242: variável local (parâmetro/`nova`) de tipo função
                // tem precedência sobre função top-level homônima — chamada
                // indireta real, callee é um valor (slot), não um símbolo.
                if let Some(binding) = self.resolve_existing_binding(name) {
                    if binding.ty == TypeIR::Function {
                        let Some(ret_type) = self.callable_ret_types.get(&binding.slot).copied()
                        else {
                            return Err(PinkerError::Ir {
                                msg: format!(
                                    "lowering falhou ao inferir retorno da chamada indireta de '{}' (encadeamento de callable retornando callable além de um nível não é suportado nesta fase)",
                                    name
                                ),
                                span: expr.span,
                            });
                        };
                        let typed_args: Vec<TypedValueIR> = args
                            .iter()
                            .map(|arg| self.lower_value(arg))
                            .collect::<Result<Vec<_>, _>>()?;
                        let ir_args: Vec<ValueIR> =
                            typed_args.into_iter().map(|typed| typed.value).collect();
                        return Ok(TypedValueIR {
                            value: ValueIR::CallIndirect {
                                callee: Box::new(ValueIR::Local(binding.slot)),
                                args: ir_args,
                                ret_type,
                            },
                            ty: ret_type,
                            struct_name: None,
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
                        struct_name: None,
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
                            struct_name: None,
                            ptr_array_bombom_size: None,
                        });
                    }
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
                        struct_name: None,
                        ptr_array_bombom_size: None,
                    });
                }

                if name == "__ternario" {
                    let typed_args: Vec<TypedValueIR> = args
                        .iter()
                        .map(|arg| self.lower_value(arg))
                        .collect::<Result<Vec<_>, _>>()?;
                    let ret_type = typed_args[1].ty;
                    let ir_args: Vec<ValueIR> = typed_args.into_iter().map(|t| t.value).collect();
                    return Ok(TypedValueIR {
                        value: ValueIR::Call {
                            callee: name.clone(),
                            args: ir_args,
                            ret_type,
                        },
                        ty: ret_type,
                        struct_name: None,
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
                    struct_name: self
                        .context
                        .function_sigs
                        .get(name)
                        .and_then(|sig| sig.ret_struct_name.clone()),
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
                        if info.has_payload {
                            return Ok(TypedValueIR {
                                value: ValueIR::Call {
                                    callee: "__pinker_internal_leque_criar_0".to_string(),
                                    args: vec![ValueIR::Int(*discriminant)],
                                    ret_type: TypeIR::Bombom,
                                },
                                ty: TypeIR::Bombom,
                                struct_name: None,
                                ptr_array_bombom_size: None,
                            });
                        }
                        return Ok(TypedValueIR {
                            value: ValueIR::Int(*discriminant),
                            ty: TypeIR::Bombom,
                            struct_name: None,
                            ptr_array_bombom_size: None,
                        });
                    }
                }
                let base = self.lower_value(base)?;
                let Some(base_struct_name) = base.struct_name.as_ref() else {
                    return Err(PinkerError::Ir {
                        msg: "acesso a campo com base não-struct na IR".to_string(),
                        span: expr.span,
                    });
                };
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
                    struct_name: None,
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
                    struct_name: None,
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
                        Self::impl_receiver_key(&lowered_source).ok_or_else(|| {
                            PinkerError::Ir {
                                msg: "materialização sem identidade do tipo concreto".to_string(),
                                span: expr.span,
                            }
                        })?;

                    let concrete_size = self.concrete_snapshot_size(&lowered_source, expr.span)?;

                    let vtable_methods =
                        self.trait_vtable(&trait_name, &concrete_type_name, expr.span)?;

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
                        struct_name: None,
                        ptr_array_bombom_size: None,
                    });
                }

                Ok(TypedValueIR {
                    value: ValueIR::Cast {
                        value: Box::new(lowered_source.value),
                        target_type,
                    },
                    ty: target_type,
                    struct_name: None,
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
                    struct_name: None,
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
                    struct_name: None,
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
        struct_name: Option<String>,
        ptr_array_bombom_size: Option<u64>,
        is_mut: Option<bool>,
    ) -> BindingIR {
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
        };

        self.scopes.last_mut().unwrap().insert(
            source_name.to_string(),
            BindingState {
                slot: slot.clone(),
                ty,
                struct_name: struct_name.clone(),
                ptr_array_bombom_size,
            },
        );

        if let Some(is_mut) = is_mut {
            self.locals.push(LocalIR {
                source_name: source_name.to_string(),
                slot,
                ty,
                is_mut,
            });
        }

        binding
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

fn resolve_struct_name_from_type(
    ty: &Type,
    aliases: &HashMap<String, Type>,
    struct_names: &HashSet<String>,
) -> Option<String> {
    match ty {
        Type::Struct { name, .. } => Some(name.clone()),
        Type::Alias { name, .. } => {
            if struct_names.contains(name) {
                Some(name.clone())
            } else {
                aliases
                    .get(name)
                    .and_then(|target| resolve_struct_name_from_type(target, aliases, struct_names))
            }
        }
        Type::Pointer { base, .. } => resolve_struct_name_from_type(base, aliases, struct_names),
        _ => None,
    }
}

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
        InstructionIR::InlineAsm { chunks, .. } => {
            line(out, indent, &format!("inline_asm [{}]", chunks.join(" | ")));
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
        ValueIR::Unary { op, operand } => format!("{}({})", op.name(), render_value(operand)),
        ValueIR::Deref {
            ptr, is_volatile, ..
        } => {
            if *is_volatile {
                format!("deref_fragil({})", render_value(ptr))
            } else {
                format!("deref({})", render_value(ptr))
            }
        }
        ValueIR::Binary { op, lhs, rhs } => {
            format!(
                "{}({}, {})",
                op.name(),
                render_value(lhs),
                render_value(rhs)
            )
        }
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
            // Tipos leque são nominais apenas na semântica; na IR o valor é o
            // discriminante inteiro.
            Type::Enum { .. } => Ok(TypeIR::Bombom),
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
            TypeIR::FixedArray { .. } => "array",
            TypeIR::Struct => "struct",
            TypeIR::Pointer { .. } => "seta",
            TypeIR::Function => "carinho",
            TypeIR::TraitObject => "trato",
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
            TypeIR::TraitObject => "trato<?>".to_string(),
            TypeIR::ListBombom => "lista<bombom>".to_string(),
            TypeIR::ListVerso => "lista<verso>".to_string(),
            TypeIR::MapVersoBombom => "mapa<verso,bombom>".to_string(),
            TypeIR::MapVersoVerso => "mapa<verso,verso>".to_string(),
            TypeIR::MapBombomBombom => "mapa<bombom,bombom>".to_string(),
            TypeIR::MapBombomVerso => "mapa<bombom,verso>".to_string(),
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
            | TypeIR::FixedArray { .. }
            | TypeIR::Struct
            | TypeIR::Pointer { .. }
            | TypeIR::Function
            | TypeIR::TraitObject
            | TypeIR::Nulo => None,
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
