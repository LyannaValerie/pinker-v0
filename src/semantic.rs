//! Checagem semântica — validação antes do lowering para IR.
//!
//! `SemanticChecker` opera em duas passagens sobre o programa:
//! 1. **Declaração**: coleta todas as funções e constantes em tabelas globais (`funcs`, `consts`).
//!    Detecta duplicações e conflitos de nomes entre funções e constantes.
//! 2. **Verificação**: valida cada corpo de função e constante (tipos, escopos, retornos, aridade).
//!
//! Invariantes mantidas:
//! - Sombreamento de variável no mesmo escopo é proibido; escopos aninhados permitem sombra.
//! - `principal` é obrigatória, sem parâmetros, retorno `bombom`.
//! - Retorno de função com tipo declarado deve ser alcançável em todos os caminhos simples
//!   (análise superficial: sequência + talvez/senão — sem análise de fluxo completa).
//! - `Nulo` nunca aparece como tipo de usuário; representa ausência de retorno internamente.

use crate::ast::*;
use crate::error::PinkerError;
use crate::token::{Position, Span};
use std::collections::HashMap;

#[derive(Clone)]
struct VarMeta {
    ty: Type,
    is_mut: bool,
}

struct Scope {
    vars: HashMap<String, VarMeta>,
}

pub struct SemanticChecker {
    funcs: HashMap<String, FunctionDecl>,
    consts: HashMap<String, ConstDecl>,
    scopes: Vec<Scope>,
    current_func_name: Option<String>,
    current_func_ret: Option<Type>,
    loop_depth: usize,
}

impl SemanticChecker {
    pub fn new() -> Self {
        Self {
            funcs: HashMap::new(),
            consts: HashMap::new(),
            scopes: Vec::new(),
            current_func_name: None,
            current_func_ret: None,
            loop_depth: 0,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope {
            vars: HashMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn root_span(program: &Program) -> Span {
        program
            .package
            .as_ref()
            .map(|package| package.span)
            .or_else(|| program.items.first().map(Item::span))
            .unwrap_or_else(|| Span::single(Position::new(1, 1)))
    }

    fn check_type_match(expected: &Type, actual: &Type) -> bool {
        matches!(
            (expected, actual),
            (Type::Bombom(_), Type::Bombom(_)) | (Type::Logica(_), Type::Logica(_))
        )
    }

    fn declare_var(
        &mut self,
        name: &str,
        ty: Type,
        is_mut: bool,
        span: Span,
    ) -> Result<(), PinkerError> {
        let scope = self.scopes.last_mut().expect("escopo ativo ausente");
        if scope.vars.contains_key(name) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "variável '{}' já declarada no escopo atual; sombreamento no mesmo escopo é proibido",
                    name
                ),
                span,
            });
        }
        scope.vars.insert(name.to_string(), VarMeta { ty, is_mut });
        Ok(())
    }

    fn resolve_var(&self, name: &str) -> Option<VarMeta> {
        for scope in self.scopes.iter().rev() {
            if let Some(meta) = scope.vars.get(name) {
                return Some(meta.clone());
            }
        }

        self.consts.get(name).map(|constant| VarMeta {
            ty: constant.ty.clone(),
            is_mut: false,
        })
    }

    // --- Passagem 1: declaração global ---
    // Registra funções e constantes antes de verificar qualquer corpo.
    // Erros aqui interrompem antes da passagem 2.
    pub fn check_program(&mut self, program: &Program) -> Result<(), PinkerError> {
        for item in &program.items {
            match item {
                Item::Function(function) => {
                    if self.funcs.contains_key(&function.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("função '{}' já declarada", function.name),
                            span: function.span,
                        });
                    }
                    if self.consts.contains_key(&function.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por uma constante global",
                                function.name
                            ),
                            span: function.span,
                        });
                    }
                    self.funcs.insert(function.name.clone(), function.clone());
                }
                Item::Const(constant) => {
                    if self.consts.contains_key(&constant.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("constante '{}' já declarada", constant.name),
                            span: constant.span,
                        });
                    }
                    if self.funcs.contains_key(&constant.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("nome '{}' já utilizado por uma função", constant.name),
                            span: constant.span,
                        });
                    }
                    self.consts.insert(constant.name.clone(), constant.clone());
                }
            }
        }

        // --- Passagem 2: verificação de corpos ---
        self.check_principal(program)?;

        for item in &program.items {
            match item {
                Item::Function(function) => self.check_function(function)?,
                Item::Const(constant) => self.check_const_body(constant)?,
            }
        }

        Ok(())
    }

    // `principal` é a política fixa de entrada da v0: sem parâmetros e retorno bombom.
    fn check_principal(&self, program: &Program) -> Result<(), PinkerError> {
        let Some(main_fn) = self.funcs.get("principal") else {
            return Err(PinkerError::Semantic {
                msg: "função 'principal' (entry point) não encontrada".to_string(),
                span: Self::root_span(program),
            });
        };

        if !main_fn.params.is_empty() {
            return Err(PinkerError::Semantic {
                msg: "a função 'principal' não deve ter parâmetros".to_string(),
                span: main_fn.span,
            });
        }

        match &main_fn.ret_type {
            Some(Type::Bombom(_)) => Ok(()),
            _ => Err(PinkerError::Semantic {
                msg: "a função 'principal' deve declarar retorno 'bombom'".to_string(),
                span: main_fn.span,
            }),
        }
    }

    fn check_const_body(&mut self, constant: &ConstDecl) -> Result<(), PinkerError> {
        self.push_scope();
        let init_ty = self.check_value_expr(
            &constant.init,
            "resultado de função sem retorno não pode inicializar constante",
        )?;
        self.pop_scope();

        if !Self::check_type_match(&constant.ty, &init_ty) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "tipo incompatível na constante '{}': esperado '{}', encontrado '{}'",
                    constant.name,
                    constant.ty.name(),
                    init_ty.name()
                ),
                span: constant.init.span,
            });
        }

        Ok(())
    }

    fn check_function(&mut self, function: &FunctionDecl) -> Result<(), PinkerError> {
        self.current_func_name = Some(function.name.clone());
        self.current_func_ret = function.ret_type.clone();
        self.loop_depth = 0;
        self.push_scope();

        // Parâmetros entram no escopo da função antes do corpo (não são mutáveis).
        for param in &function.params {
            self.declare_var(&param.name, param.ty.clone(), false, param.span)?;
        }

        self.check_block(&function.body, true)?;

        // A v0 só resolve fluxo simples: sequência, blocos e cadeias de talvez/senao.
        if self.current_func_ret.is_some() && !self.block_returns(&function.body) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "função '{}' com retorno declarado não retorna em todos os caminhos simples",
                    function.name
                ),
                span: function.body.span,
            });
        }

        self.pop_scope();
        self.current_func_name = None;
        self.current_func_ret = None;
        self.loop_depth = 0;
        Ok(())
    }

    fn check_block(&mut self, block: &Block, function_level: bool) -> Result<(), PinkerError> {
        if !function_level {
            self.push_scope();
        }

        for stmt in &block.stmts {
            match stmt {
                Stmt::Let(let_stmt) => {
                    let init_ty = self.check_value_expr(
                        &let_stmt.init,
                        "resultado de função sem retorno não pode ser usado em inicialização de variável",
                    )?;

                    let ty = match &let_stmt.ty {
                        Some(declared_ty) => {
                            if !Self::check_type_match(declared_ty, &init_ty) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo de inicialização incompatível para '{}': esperado '{}', encontrado '{}'",
                                        let_stmt.name,
                                        declared_ty.name(),
                                        init_ty.name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            declared_ty.clone()
                        }
                        None => init_ty,
                    };

                    self.declare_var(&let_stmt.name, ty, let_stmt.is_mut, let_stmt.span)?;
                }
                Stmt::Return(return_stmt) => self.check_return_stmt(return_stmt)?,
                Stmt::Assign(assign_stmt) => {
                    let value_ty = self.check_value_expr(
                        &assign_stmt.expr,
                        "resultado de função sem retorno não pode ser usado em atribuição",
                    )?;

                    let Some(var_meta) = self.resolve_var(&assign_stmt.name) else {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "variável '{}' não declarada para atribuição",
                                assign_stmt.name
                            ),
                            span: assign_stmt.span,
                        });
                    };

                    if !var_meta.is_mut {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "reatribuição inválida: '{}' não é mutável",
                                assign_stmt.name
                            ),
                            span: assign_stmt.span,
                        });
                    }

                    if !Self::check_type_match(&var_meta.ty, &value_ty) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo incompatível na atribuição para '{}': esperado '{}', encontrado '{}'",
                                assign_stmt.name,
                                var_meta.ty.name(),
                                value_ty.name()
                            ),
                            span: assign_stmt.expr.span,
                        });
                    }
                }
                Stmt::If(if_stmt) => {
                    let cond_ty = self.check_value_expr(
                        &if_stmt.condition,
                        "condição não pode usar resultado de função sem retorno",
                    )?;
                    if !matches!(cond_ty, Type::Logica(_)) {
                        return Err(PinkerError::Semantic {
                            msg: "condição de 'talvez' deve ser 'logica'".to_string(),
                            span: if_stmt.condition.span,
                        });
                    }

                    self.check_block(&if_stmt.then_branch, false)?;

                    if let Some(else_branch) = &if_stmt.else_branch {
                        match else_branch {
                            ElseBlock::Block(block) => self.check_block(block, false)?,
                            ElseBlock::If(if_stmt) => self.check_if_as_nested_branch(if_stmt)?,
                        }
                    }
                }
                Stmt::While(while_stmt) => {
                    let cond_ty = self.check_value_expr(
                        &while_stmt.condition,
                        "condição não pode usar resultado de função sem retorno",
                    )?;
                    if !matches!(cond_ty, Type::Logica(_)) {
                        return Err(PinkerError::Semantic {
                            msg: "condição de 'sempre que' deve ser 'logica'".to_string(),
                            span: while_stmt.condition.span,
                        });
                    }

                    self.loop_depth += 1;
                    let body_result = self.check_block(&while_stmt.body, false);
                    self.loop_depth -= 1;
                    body_result?;
                }
                Stmt::Break(break_stmt) => {
                    if self.loop_depth == 0 {
                        return Err(PinkerError::Semantic {
                            msg: "'quebrar' só pode ser usado dentro de 'sempre que'".to_string(),
                            span: break_stmt.span,
                        });
                    }
                }
                Stmt::Continue(continue_stmt) => {
                    if self.loop_depth == 0 {
                        return Err(PinkerError::Semantic {
                            msg: "'continuar' só pode ser usado dentro de 'sempre que'".to_string(),
                            span: continue_stmt.span,
                        });
                    }
                }
                Stmt::Expr(expr) => {
                    self.check_expr(expr)?;
                }
            }
        }

        if !function_level {
            self.pop_scope();
        }

        Ok(())
    }

    fn check_if_as_nested_branch(&mut self, if_stmt: &IfStmt) -> Result<(), PinkerError> {
        self.push_scope();
        let cond_ty = self.check_value_expr(
            &if_stmt.condition,
            "condição não pode usar resultado de função sem retorno",
        )?;
        if !matches!(cond_ty, Type::Logica(_)) {
            self.pop_scope();
            return Err(PinkerError::Semantic {
                msg: "condição de 'talvez' deve ser 'logica'".to_string(),
                span: if_stmt.condition.span,
            });
        }

        self.check_block(&if_stmt.then_branch, false)?;
        if let Some(else_branch) = &if_stmt.else_branch {
            match else_branch {
                ElseBlock::Block(block) => self.check_block(block, false)?,
                ElseBlock::If(inner) => self.check_if_as_nested_branch(inner)?,
            }
        }
        self.pop_scope();
        Ok(())
    }

    fn check_return_stmt(&mut self, return_stmt: &ReturnStmt) -> Result<(), PinkerError> {
        let current_ret = self.current_func_ret.clone();
        match (current_ret, &return_stmt.expr) {
            (None, None) => Ok(()),
            (None, Some(_)) => Err(PinkerError::Semantic {
                msg: "mimo com valor não é permitido em função sem retorno declarado".to_string(),
                span: return_stmt.span,
            }),
            (Some(_), None) => Err(PinkerError::Semantic {
                msg: "mimo sem valor não é permitido em função com retorno declarado".to_string(),
                span: return_stmt.span,
            }),
            (Some(expected), Some(expr)) => {
                let value_ty = self.check_value_expr(
                    expr,
                    "resultado de função sem retorno não pode ser retornado como valor",
                )?;
                if !Self::check_type_match(&expected, &value_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno incompatível em '{}': esperado '{}', encontrado '{}'",
                            self.current_func_name
                                .as_deref()
                                .unwrap_or("<desconhecida>"),
                            expected.name(),
                            value_ty.name()
                        ),
                        span: expr.span,
                    });
                }
                Ok(())
            }
        }
    }

    // Análise de alcançabilidade de retorno superficial: verifica se o bloco
    // contém um `mimo` direto ou um `talvez/senão` onde ambos os ramos retornam.
    // Não analisa fluxo complexo nem condições de laço — suficiente para a v0.
    fn block_returns(&self, block: &Block) -> bool {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Return(_) => return true,
                Stmt::If(if_stmt) if self.if_returns(if_stmt) => return true,
                _ => {}
            }
        }
        false
    }

    fn if_returns(&self, if_stmt: &IfStmt) -> bool {
        let then_returns = self.block_returns(&if_stmt.then_branch);
        let else_returns = match &if_stmt.else_branch {
            Some(ElseBlock::Block(block)) => self.block_returns(block),
            Some(ElseBlock::If(inner)) => self.if_returns(inner),
            None => false,
        };
        then_returns && else_returns
    }

    fn check_value_expr(&mut self, expr: &Expr, void_message: &str) -> Result<Type, PinkerError> {
        let ty = self.check_expr(expr)?;
        if ty.is_nulo() {
            return Err(PinkerError::Semantic {
                msg: void_message.to_string(),
                span: expr.span,
            });
        }
        Ok(ty)
    }

    // `Nulo` existe só internamente para a semântica da v0: função sem `-> tipo` retorna `Nulo`.
    // Esse tipo nunca pode aparecer em declaração de usuário.
    fn function_result_type(&self, function: &FunctionDecl, span: Span) -> Type {
        function
            .ret_type
            .clone()
            .unwrap_or(Type::Nulo(span))
            .with_span(span)
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(_) => Ok(Type::Bombom(expr.span)),
            ExprKind::BoolLit(_) => Ok(Type::Logica(expr.span)),
            ExprKind::Ident(name) => {
                self.resolve_var(name)
                    .map(|meta| meta.ty)
                    .ok_or_else(|| PinkerError::Semantic {
                        msg: format!("identificador '{}' não declarado", name),
                        span: expr.span,
                    })
            }
            ExprKind::Call(callee, args) => self.check_call_expr(expr.span, callee, args),
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_ty = self.check_value_expr(
                    lhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;
                let rhs_ty = self.check_value_expr(
                    rhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;

                if !Self::check_type_match(&lhs_ty, &rhs_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipos incompatíveis em operação binária: '{}' e '{}'",
                            lhs_ty.name(),
                            rhs_ty.name()
                        ),
                        span: expr.span,
                    });
                }

                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                        if matches!(lhs_ty, Type::Bombom(_)) {
                            Ok(Type::Bombom(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação aritmética requer operandos 'bombom'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    BinaryOp::Eq
                    | BinaryOp::Neq
                    | BinaryOp::Lt
                    | BinaryOp::Lte
                    | BinaryOp::Gt
                    | BinaryOp::Gte => Ok(Type::Logica(expr.span)),
                }
            }
            ExprKind::Unary(op, operand) => {
                let inner_ty = self.check_value_expr(
                    operand,
                    "resultado de função sem retorno não pode ser usado em operação unária",
                )?;
                match op {
                    UnaryOp::Neg => {
                        if matches!(inner_ty, Type::Bombom(_)) {
                            Ok(Type::Bombom(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação aritmética requer operando 'bombom'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::Not => {
                        if matches!(inner_ty, Type::Logica(_)) {
                            Ok(Type::Logica(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação lógica requer operando 'logica'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                }
            }
        }
    }

    fn check_call_expr(
        &mut self,
        expr_span: Span,
        callee: &Expr,
        args: &[Expr],
    ) -> Result<Type, PinkerError> {
        let ExprKind::Ident(name) = &callee.kind else {
            return Err(PinkerError::Semantic {
                msg: "apenas chamadas diretas por nome são suportadas na v0".to_string(),
                span: callee.span,
            });
        };

        let Some(function) = self.funcs.get(name).cloned() else {
            return Err(PinkerError::Semantic {
                msg: format!("função '{}' não declarada", name),
                span: callee.span,
            });
        };

        if args.len() != function.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                    name,
                    function.params.len(),
                    args.len()
                ),
                span: expr_span,
            });
        }

        for (index, (arg, param)) in args.iter().zip(function.params.iter()).enumerate() {
            let arg_ty = self.check_value_expr(
                arg,
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !Self::check_type_match(&param.ty, &arg_ty) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                        index + 1,
                        name,
                        param.ty.name(),
                        arg_ty.name()
                    ),
                    span: arg.span,
                });
            }
        }

        Ok(self.function_result_type(&function, expr_span))
    }
}

pub fn check_program(program: &Program) -> Result<(), PinkerError> {
    SemanticChecker::new().check_program(program)
}
