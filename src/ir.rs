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
    BinaryOp, Block, BreakStmt, ConstDecl, ContinueStmt, ElseBlock, Expr, ExprKind, FunctionDecl,
    IfStmt, Item, LetStmt, Program, ReturnStmt, Stmt, Type, UnaryOp, WhileStmt,
};
use crate::error::PinkerError;
use crate::token::Span;
use std::collections::HashMap;

/// Programa completo na IR estruturada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramIR {
    pub module_name: String,
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

/// Variável local declarada por `nova`. `is_mut` reflete a palavra-chave `mut`.
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
}

/// Expressão na IR. `Call` carrega `ret_type` explicitamente para que camadas posteriores
/// não precisem consultar a tabela de funções — o tipo está embutido no nó.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueIR {
    Local(String),
    GlobalConst(String),
    Int(u64),
    Bool(bool),
    Unary {
        op: UnaryOpIR,
        operand: Box<ValueIR>,
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
}

/// Tipos do sistema de tipos da v0. `Nulo` representa ausência de retorno (funções sem `-> tipo`);
/// não é exposto como tipo de usuário — apenas interno ao pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeIR {
    Bombom,
    Logica,
    Nulo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOpIR {
    Neg,
    Not,
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
}

#[derive(Clone)]
struct BindingState {
    slot: String,
    ty: TypeIR,
}

// `LoweringContext` é construído em uma primeira passagem sobre o programa:
// coleta todas as assinaturas de funções e constantes antes de baixar qualquer corpo.
// Isso permite chamadas para-frente sem ordem de declaração obrigatória.
struct LoweringContext {
    module_name: String,
    function_sigs: HashMap<String, FunctionSigIR>,
    global_consts: HashMap<String, TypeIR>,
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
}

// Fase 2 escolhe IR estruturada: blocos e `if` seguem explícitos, sem SSA e sem saltos.
// Isso mantém o lowering pequeno e auditável sem quebrar o frontend estabilizado.
pub fn lower_program(program: &Program) -> Result<ProgramIR, PinkerError> {
    let context = LoweringContext::from_program(program);
    let mut consts = Vec::new();
    let mut functions = Vec::new();

    for item in &program.items {
        match item {
            Item::Const(const_decl) => consts.push(lower_const(const_decl, &context)?),
            Item::Function(function_decl) => {
                functions.push(FunctionLowerer::new(&context).lower_function(function_decl)?)
            }
        }
    }

    Ok(ProgramIR {
        module_name: context.module_name,
        consts,
        functions,
    })
}

pub fn render_program(program: &ProgramIR) -> String {
    let mut out = String::new();
    line(&mut out, 0, &format!("module {}", program.module_name));

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
                    const_ir.ty.name(),
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
    fn from_program(program: &Program) -> Self {
        let module_name = program
            .package
            .as_ref()
            .map(|package| package.name.clone())
            .unwrap_or_else(|| "main".to_string());

        let mut function_sigs = HashMap::new();
        let mut global_consts = HashMap::new();

        for item in &program.items {
            match item {
                Item::Function(function) => {
                    function_sigs.insert(
                        function.name.clone(),
                        FunctionSigIR {
                            ret_type: TypeIR::from_ast_option(function.ret_type.as_ref()),
                        },
                    );
                }
                Item::Const(const_decl) => {
                    global_consts.insert(const_decl.name.clone(), TypeIR::from_ast(&const_decl.ty));
                }
            }
        }

        Self {
            module_name,
            function_sigs,
            global_consts,
        }
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
            let binding = self.allocate_binding(&param.name, TypeIR::from_ast(&param.ty), None);
            self.params.push(binding);
        }

        let entry = self.lower_block(&function.body, "entry".to_string(), false)?;

        self.pop_scope();

        Ok(FunctionIR {
            name: function.name.clone(),
            params: self.params,
            locals: self.locals,
            ret_type: TypeIR::from_ast_option(function.ret_type.as_ref()),
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
                let binding = self.resolve_binding(&assign_stmt.name, assign_stmt.span)?;
                let value = self.lower_value(&assign_stmt.expr)?.value;
                Ok(InstructionIR::Assign {
                    slot: binding.slot,
                    value,
                    span: assign_stmt.span,
                })
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
        }
    }

    fn lower_let(&mut self, let_stmt: &LetStmt) -> Result<InstructionIR, PinkerError> {
        let value = self.lower_value(&let_stmt.init)?;
        let ty = let_stmt
            .ty
            .as_ref()
            .map(TypeIR::from_ast)
            .unwrap_or(value.ty);
        let binding = self.allocate_binding(&let_stmt.name, ty, Some(let_stmt.is_mut));
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
            }),
            ExprKind::BoolLit(value) => Ok(TypedValueIR {
                value: ValueIR::Bool(*value),
                ty: TypeIR::Logica,
            }),
            ExprKind::Ident(name) => {
                if let Some(binding) = self.resolve_existing_binding(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::Local(binding.slot),
                        ty: binding.ty,
                    });
                }

                if let Some(ty) = self.context.global_consts.get(name) {
                    return Ok(TypedValueIR {
                        value: ValueIR::GlobalConst(name.clone()),
                        ty: *ty,
                    });
                }

                Err(PinkerError::Ir {
                    msg: format!("lowering falhou ao resolver identificador '{}'", name),
                    span: expr.span,
                })
            }
            ExprKind::Unary(op, operand) => {
                let operand = self.lower_value(operand)?;
                Ok(TypedValueIR {
                    value: ValueIR::Unary {
                        op: UnaryOpIR::from_ast(*op),
                        operand: Box::new(operand.value),
                    },
                    ty: match op {
                        UnaryOp::Neg => TypeIR::Bombom,
                        UnaryOp::Not => TypeIR::Logica,
                    },
                })
            }
            ExprKind::Binary(lhs, op, rhs) => {
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
                        | BinaryOp::BitAnd
                        | BinaryOp::BitOr
                        | BinaryOp::BitXor
                        | BinaryOp::Shl
                        | BinaryOp::Shr => TypeIR::Bombom,
                        BinaryOp::Eq
                        | BinaryOp::Neq
                        | BinaryOp::Lt
                        | BinaryOp::Lte
                        | BinaryOp::Gt
                        | BinaryOp::Gte => TypeIR::Logica,
                    },
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
                })
            }
        }
    }

    fn allocate_binding(
        &mut self,
        source_name: &str,
        ty: TypeIR,
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
        ty: TypeIR::from_ast(&const_decl.ty),
        value: value.value,
        span: const_decl.span,
    })
}

fn render_function(function: &FunctionIR, indent: usize, out: &mut String) {
    line(
        out,
        indent,
        &format!("func {} -> {}", function.name, function.ret_type.name()),
    );

    if function.params.is_empty() {
        line(out, indent + 1, "params: []");
    } else {
        line(out, indent + 1, "params:");
        for param in &function.params {
            line(
                out,
                indent + 2,
                &format!("{}: {}", param.slot, param.ty.name()),
            );
        }
    }

    if function.locals.is_empty() {
        line(out, indent + 1, "locals: []");
    } else {
        line(out, indent + 1, "locals:");
        for local in &function.locals {
            let mutability = if local.is_mut { " mut" } else { "" };
            line(
                out,
                indent + 2,
                &format!("{}: {}{}", local.slot, local.ty.name(), mutability),
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
    }
}

fn render_value(value: &ValueIR) -> String {
    match value {
        ValueIR::Local(slot) => slot.clone(),
        ValueIR::GlobalConst(name) => format!("@{}", name),
        ValueIR::Int(value) => format!("{}:bombom", value),
        ValueIR::Bool(value) => format!("{}:logica", if *value { "verdade" } else { "falso" }),
        ValueIR::Unary { op, operand } => format!("{}({})", op.name(), render_value(operand)),
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
            ret_type.name()
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

impl TypeIR {
    pub fn from_ast(ty: &Type) -> Self {
        match ty {
            Type::Bombom(_) => TypeIR::Bombom,
            Type::Logica(_) => TypeIR::Logica,
            Type::Nulo(_) => TypeIR::Nulo,
        }
    }

    pub fn from_ast_option(ty: Option<&Type>) -> Self {
        ty.map(Self::from_ast).unwrap_or(TypeIR::Nulo)
    }

    pub fn name(&self) -> &'static str {
        match self {
            TypeIR::Bombom => "bombom",
            TypeIR::Logica => "logica",
            TypeIR::Nulo => "nulo",
        }
    }
}

impl UnaryOpIR {
    fn from_ast(op: UnaryOp) -> Self {
        match op {
            UnaryOp::Neg => UnaryOpIR::Neg,
            UnaryOp::Not => UnaryOpIR::Not,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            UnaryOpIR::Neg => "neg",
            UnaryOpIR::Not => "not",
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
            BinaryOpIR::Eq => "eq",
            BinaryOpIR::Neq => "neq",
            BinaryOpIR::Lt => "lt",
            BinaryOpIR::Lte => "lte",
            BinaryOpIR::Gt => "gt",
            BinaryOpIR::Gte => "gte",
        }
    }
}
