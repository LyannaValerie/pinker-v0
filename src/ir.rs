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
    AssignTarget, BinaryOp, Block, BreakStmt, ConstDecl, ContinueStmt, ElseBlock, Expr, ExprKind,
    FalarStmt, FunctionDecl, IfStmt, InlineAsmStmt, Item, LetStmt, Program, ReturnStmt, Stmt,
    StructDecl, Type, UnaryOp, WhileStmt,
};
use crate::error::PinkerError;
use crate::layout;
use crate::token::Span;
use std::collections::{HashMap, HashSet};

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
    FixedArray { element: ScalarTypeIR, size: u64 },
    Struct,
    Pointer { is_volatile: bool },
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

#[derive(Clone)]
struct FunctionSigIR {
    ret_type: TypeIR,
    ret_struct_name: Option<String>,
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
}

struct TypedValueIR {
    value: ValueIR,
    ty: TypeIR,
    struct_name: Option<String>,
    ptr_array_bombom_size: Option<u64>,
}

// Fase 2 escolhe IR estruturada: blocos e `if` seguem explícitos, sem SSA e sem saltos.
// Isso mantém o lowering pequeno e auditável sem quebrar o frontend estabilizado.
pub fn lower_program(program: &Program) -> Result<ProgramIR, PinkerError> {
    let context = LoweringContext::from_program(program)?;
    let mut consts = Vec::new();
    let mut functions = Vec::new();

    for item in &program.items {
        match item {
            Item::Const(const_decl) => consts.push(lower_const(const_decl, &context)?),
            Item::Function(function_decl) => {
                functions.push(FunctionLowerer::new(&context).lower_function(function_decl)?)
            }
            Item::TypeAlias(_) => {}
            Item::Struct(_) => {}
        }
    }

    Ok(ProgramIR {
        module_name: context.module_name,
        is_freestanding: program.freestanding.is_some(),
        consts,
        functions,
    })
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
    fn from_program(program: &Program) -> Result<Self, PinkerError> {
        let module_name = program
            .package
            .as_ref()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| "main".to_string());

        let mut type_aliases = HashMap::new();
        let mut struct_decls = HashMap::new();
        let mut struct_names = HashSet::new();
        for item in &program.items {
            if let Item::TypeAlias(alias) = item {
                type_aliases.insert(alias.name.clone(), alias.target.clone());
            } else if let Item::Struct(struct_decl) = item {
                struct_names.insert(struct_decl.name.clone());
                struct_decls.insert(struct_decl.name.clone(), struct_decl.clone());
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

        let mut function_sigs = HashMap::new();
        let mut global_consts = HashMap::new();

        for item in &program.items {
            match item {
                Item::Function(function) => {
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
                Item::TypeAlias(_) | Item::Struct(_) => {}
            }
        }
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
            "ambiente_ou".to_string(),
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

        Ok(Self {
            module_name,
            function_sigs,
            global_consts,
            type_aliases,
            struct_decls,
            struct_names,
            struct_fields,
            struct_field_offsets,
        })
    }

    fn resolve_type(&self, ty: &Type) -> Result<TypeIR, PinkerError> {
        TypeIR::from_ast_with_context(ty, &self.type_aliases, &self.struct_names)
    }
}

impl<'a> FunctionLowerer<'a> {
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
        }
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
        let binding = self.allocate_binding(
            &let_stmt.name,
            ty,
            struct_name,
            ptr_array_bombom_size,
            Some(let_stmt.is_mut),
        );
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
            ExprKind::Ident(name) => {
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
                let ExprKind::Ident(name) = &callee.kind else {
                    return Err(PinkerError::Ir {
                        msg: "IR da v0 suporta apenas chamadas diretas por nome".to_string(),
                        span: expr.span,
                    });
                };

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
}

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
            TypeIR::FixedArray { .. } => "array",
            TypeIR::Struct => "struct",
            TypeIR::Pointer { .. } => "seta",
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
            | TypeIR::FixedArray { .. }
            | TypeIR::Struct
            | TypeIR::Pointer { .. }
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
