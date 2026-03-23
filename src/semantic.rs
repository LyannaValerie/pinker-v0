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
use crate::layout;
use crate::token::{Position, Span};
use std::collections::{HashMap, HashSet};

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
    type_aliases: HashMap<String, Type>,
    structs: HashMap<String, StructDecl>,
    scopes: Vec<Scope>,
    current_func_name: Option<String>,
    current_func_ret: Option<Type>,
    loop_depth: usize,
}

impl Default for SemanticChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticChecker {
    pub fn new() -> Self {
        Self {
            funcs: HashMap::new(),
            consts: HashMap::new(),
            type_aliases: HashMap::new(),
            structs: HashMap::new(),
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
            .or_else(|| program.imports.first().map(|import| import.span))
            .or_else(|| program.items.first().map(Item::span))
            .unwrap_or_else(|| Span::single(Position::new(1, 1)))
    }

    fn check_type_match(expected: &Type, actual: &Type) -> bool {
        match (expected, actual) {
            (Type::Bombom(_), Type::Bombom(_))
            | (Type::Bombom(_), Type::U64(_))
            | (Type::U64(_), Type::Bombom(_))
            | (Type::U8(_), Type::U8(_))
            | (Type::U16(_), Type::U16(_))
            | (Type::U32(_), Type::U32(_))
            | (Type::U64(_), Type::U64(_))
            | (Type::I8(_), Type::I8(_))
            | (Type::I16(_), Type::I16(_))
            | (Type::I32(_), Type::I32(_))
            | (Type::I64(_), Type::I64(_))
            | (Type::Logica(_), Type::Logica(_))
            | (Type::Verso(_), Type::Verso(_)) => true,
            (Type::Struct { name: lhs_name, .. }, Type::Struct { name: rhs_name, .. }) => {
                lhs_name == rhs_name
            }
            (
                Type::FixedArray {
                    element: lhs_element,
                    size: lhs_size,
                    ..
                },
                Type::FixedArray {
                    element: rhs_element,
                    size: rhs_size,
                    ..
                },
            ) => {
                lhs_size == rhs_size
                    && Self::check_type_match(lhs_element.as_ref(), rhs_element.as_ref())
            }
            (
                Type::Pointer {
                    base: lhs_base,
                    is_volatile: lhs_volatile,
                    ..
                },
                Type::Pointer {
                    base: rhs_base,
                    is_volatile: rhs_volatile,
                    ..
                },
            ) => {
                lhs_volatile == rhs_volatile
                    && Self::check_type_match(lhs_base.as_ref(), rhs_base.as_ref())
            }
            _ => false,
        }
    }

    fn resolve_type_named(
        &self,
        ty: &Type,
        resolving: &mut Vec<String>,
    ) -> Result<Type, PinkerError> {
        match ty {
            Type::Alias { name, span } => {
                if self.structs.contains_key(name) {
                    return Ok(Type::Struct {
                        name: name.clone(),
                        span: *span,
                    });
                }
                if resolving.iter().any(|entry| entry == name) {
                    return Err(PinkerError::Semantic {
                        msg: format!("alias de tipo recursivo detectado em '{}'", name),
                        span: *span,
                    });
                }
                let Some(target) = self.type_aliases.get(name) else {
                    return Err(PinkerError::Semantic {
                        msg: format!("tipo '{}' não existe", name),
                        span: *span,
                    });
                };
                resolving.push(name.clone());
                let resolved = self.resolve_type_named(target, resolving)?;
                resolving.pop();
                Ok(resolved.with_span(*span))
            }
            Type::FixedArray {
                element,
                size,
                span,
            } => {
                if *size == 0 {
                    return Err(PinkerError::Semantic {
                        msg: "array fixo deve ter tamanho maior que zero".to_string(),
                        span: *span,
                    });
                }

                let resolved_element = self.resolve_type_named(element.as_ref(), resolving)?;
                if matches!(resolved_element, Type::Nulo(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "tipo base de array fixo não pode ser 'nulo'".to_string(),
                        span: resolved_element.span(),
                    });
                }
                if matches!(resolved_element, Type::FixedArray { .. }) {
                    return Err(PinkerError::Semantic {
                        msg: "array fixo aninhado ainda não é suportado nesta fase".to_string(),
                        span: resolved_element.span(),
                    });
                }

                Ok(Type::FixedArray {
                    element: Box::new(resolved_element),
                    size: *size,
                    span: *span,
                })
            }
            Type::Pointer {
                base,
                is_volatile,
                span,
            } => {
                let resolved_base = self.resolve_type_named(base.as_ref(), resolving)?;
                if matches!(resolved_base, Type::Nulo(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "tipo base de 'seta' não pode ser 'nulo'".to_string(),
                        span: resolved_base.span(),
                    });
                }
                if matches!(resolved_base, Type::Pointer { .. }) {
                    return Err(PinkerError::Semantic {
                        msg: "seta de seta ainda não é suportada nesta fase".to_string(),
                        span: resolved_base.span(),
                    });
                }
                Ok(Type::Pointer {
                    base: Box::new(resolved_base),
                    is_volatile: *is_volatile,
                    span: *span,
                })
            }
            Type::Struct { .. } => Ok(ty.clone()),
            _ => Ok(ty.clone()),
        }
    }

    fn resolve_type_or_error(&self, ty: &Type) -> Result<Type, PinkerError> {
        let mut resolving = Vec::new();
        self.resolve_type_named(ty, &mut resolving)
    }

    fn validate_struct_decl(&self, struct_decl: &StructDecl) -> Result<(), PinkerError> {
        let mut field_names = HashSet::new();
        for field in &struct_decl.fields {
            if !field_names.insert(field.name.as_str()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "campo '{}' duplicado na struct '{}'",
                        field.name, struct_decl.name
                    ),
                    span: field.span,
                });
            }
            let resolved = self.resolve_type_or_error(&field.ty)?;
            if matches!(
                resolved,
                Type::Struct { name, .. } if name == struct_decl.name
            ) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "struct '{}' não pode conter recursão direta nesta fase",
                        struct_decl.name
                    ),
                    span: field.span,
                });
            }
        }
        Ok(())
    }

    fn is_integer_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Bombom(_)
                | Type::U8(_)
                | Type::U16(_)
                | Type::U32(_)
                | Type::U64(_)
                | Type::I8(_)
                | Type::I16(_)
                | Type::I32(_)
                | Type::I64(_)
        )
    }

    fn expr_is_int_literal(expr: &Expr) -> bool {
        matches!(expr.kind, ExprKind::IntLit(_))
    }

    fn is_cast_allowed(source: &Type, target: &Type) -> bool {
        if Self::is_integer_type(source) && Self::is_integer_type(target) {
            return true;
        }
        let is_bombom_ptr = |ty: &Type| {
            matches!(
                ty,
                Type::Pointer {
                    base,
                    is_volatile: _,
                    span: _,
                } if matches!(base.as_ref(), Type::Bombom(_))
            )
        };

        (matches!(source, Type::Bombom(_)) && is_bombom_ptr(target))
            || (is_bombom_ptr(source) && matches!(target, Type::Bombom(_)))
    }

    fn check_expected_type_for_expr(expected: &Type, actual: &Type, expr: &Expr) -> bool {
        Self::check_type_match(expected, actual)
            || matches!(expected, Type::Pointer { .. }) && Self::expr_is_int_literal(expr)
            || (Self::is_integer_type(expected) && Self::expr_is_int_literal(expr))
    }

    /// Valida que um literal inteiro cabe no tipo-alvo esperado.
    /// Retorna `Ok(())` se o literal couber ou se o tipo não impõe restrição de faixa.
    /// Retorna erro semântico se o literal exceder o intervalo válido do tipo.
    fn validate_int_literal_range(expected: &Type, expr: &Expr) -> Result<(), PinkerError> {
        let ExprKind::IntLit(value) = &expr.kind else {
            return Ok(());
        };
        let value = *value;
        let (type_name, fits) = match expected {
            Type::U8(_) => ("u8", value <= u8::MAX as u64),
            Type::U16(_) => ("u16", value <= u16::MAX as u64),
            Type::U32(_) => ("u32", value <= u32::MAX as u64),
            Type::U64(_) | Type::Bombom(_) => return Ok(()),
            Type::I8(_) => ("i8", value <= i8::MAX as u64),
            Type::I16(_) => ("i16", value <= i16::MAX as u64),
            Type::I32(_) => ("i32", value <= i32::MAX as u64),
            Type::I64(_) => ("i64", value <= i64::MAX as u64),
            _ => return Ok(()),
        };
        if fits {
            Ok(())
        } else {
            Err(PinkerError::Semantic {
                msg: format!(
                    "literal {} excede a faixa do tipo '{}' (máximo: {})",
                    value,
                    type_name,
                    match expected {
                        Type::U8(_) => u8::MAX as u64,
                        Type::U16(_) => u16::MAX as u64,
                        Type::U32(_) => u32::MAX as u64,
                        Type::I8(_) => i8::MAX as u64,
                        Type::I16(_) => i16::MAX as u64,
                        Type::I32(_) => i32::MAX as u64,
                        Type::I64(_) => i64::MAX as u64,
                        _ => unreachable!(),
                    }
                ),
                span: expr.span,
            })
        }
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
            ty: self
                .resolve_type_or_error(&constant.ty)
                .unwrap_or_else(|_| constant.ty.clone()),
            is_mut: false,
        })
    }

    fn resolve_struct_field_type(
        &self,
        base_ty: &Type,
        field: &str,
        span: Span,
    ) -> Result<Type, PinkerError> {
        let Type::Struct { name, .. } = base_ty else {
            return Err(PinkerError::Semantic {
                msg: "acesso de campo exige base do tipo 'ninho'".to_string(),
                span,
            });
        };
        let struct_decl = self
            .structs
            .get(name)
            .ok_or_else(|| PinkerError::Semantic {
                msg: format!("tipo de struct '{}' não declarado", name),
                span,
            })?;
        let struct_field = struct_decl
            .fields
            .iter()
            .find(|candidate| candidate.name == field)
            .ok_or_else(|| PinkerError::Semantic {
                msg: format!("campo '{}' não existe em '{}'", field, name),
                span,
            })?;
        self.resolve_type_or_error(&struct_field.ty)
            .map(|ty| ty.with_span(span))
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
                Item::TypeAlias(alias) => {
                    if self.type_aliases.contains_key(&alias.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("alias de tipo '{}' já declarado", alias.name),
                            span: alias.span,
                        });
                    }
                    if self.funcs.contains_key(&alias.name) || self.consts.contains_key(&alias.name)
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por função/constante global",
                                alias.name
                            ),
                            span: alias.span,
                        });
                    }
                    self.type_aliases
                        .insert(alias.name.clone(), alias.target.clone());
                }
                Item::Struct(struct_decl) => {
                    if self.structs.contains_key(&struct_decl.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("struct '{}' já declarada", struct_decl.name),
                            span: struct_decl.span,
                        });
                    }
                    if self.funcs.contains_key(&struct_decl.name)
                        || self.consts.contains_key(&struct_decl.name)
                        || self.type_aliases.contains_key(&struct_decl.name)
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por função/constante/alias de tipo",
                                struct_decl.name
                            ),
                            span: struct_decl.span,
                        });
                    }
                    self.structs
                        .insert(struct_decl.name.clone(), struct_decl.clone());
                }
            }
        }

        for alias_target in self.type_aliases.values() {
            self.resolve_type_or_error(alias_target)?;
        }
        for struct_decl in self.structs.values() {
            self.validate_struct_decl(struct_decl)?;
        }

        // --- Passagem 2: verificação de corpos ---
        self.check_principal(program)?;

        for item in &program.items {
            match item {
                Item::Function(function) => self.check_function(function)?,
                Item::Const(constant) => self.check_const_body(constant)?,
                Item::TypeAlias(_) | Item::Struct(_) => {}
            }
        }

        Ok(())
    }

    // `principal` é a política fixa de entrada da v0: sem parâmetros e retorno bombom.
    fn check_principal(&self, program: &Program) -> Result<(), PinkerError> {
        let Some(main_fn) = self.funcs.get("principal") else {
            let msg = if program.freestanding.is_some() {
                "função 'principal' (boot entry desta fase em modo `livre`) não encontrada"
                    .to_string()
            } else {
                "função 'principal' (entry point) não encontrada".to_string()
            };
            return Err(PinkerError::Semantic {
                msg,
                span: Self::root_span(program),
            });
        };

        if !main_fn.params.is_empty() {
            return Err(PinkerError::Semantic {
                msg: "a função 'principal' não deve ter parâmetros".to_string(),
                span: main_fn.span,
            });
        }

        let resolved_ret = main_fn
            .ret_type
            .as_ref()
            .map(|ty| self.resolve_type_or_error(ty))
            .transpose()?;
        match resolved_ret {
            Some(Type::Bombom(_)) => Ok(()),
            _ => Err(PinkerError::Semantic {
                msg: "a função 'principal' deve declarar retorno 'bombom'".to_string(),
                span: main_fn.span,
            }),
        }
    }

    fn check_const_body(&mut self, constant: &ConstDecl) -> Result<(), PinkerError> {
        let resolved_const_ty = self.resolve_type_or_error(&constant.ty)?;
        self.push_scope();
        let init_ty = self.check_value_expr(
            &constant.init,
            "resultado de função sem retorno não pode inicializar constante",
        )?;
        self.pop_scope();

        if !Self::check_expected_type_for_expr(&resolved_const_ty, &init_ty, &constant.init) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "tipo incompatível na constante '{}': esperado '{}', encontrado '{}'",
                    constant.name,
                    resolved_const_ty.name(),
                    init_ty.name()
                ),
                span: constant.init.span,
            });
        }
        Self::validate_int_literal_range(&resolved_const_ty, &constant.init)?;

        Ok(())
    }

    fn check_function(&mut self, function: &FunctionDecl) -> Result<(), PinkerError> {
        self.current_func_name = Some(function.name.clone());
        self.current_func_ret = function
            .ret_type
            .as_ref()
            .map(|ty| self.resolve_type_or_error(ty))
            .transpose()?;
        self.loop_depth = 0;
        self.push_scope();

        // Parâmetros entram no escopo da função antes do corpo (não são mutáveis).
        for param in &function.params {
            let resolved_param_ty = self.resolve_type_or_error(&param.ty)?;
            self.declare_var(&param.name, resolved_param_ty, false, param.span)?;
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
                            let resolved_declared_ty = self.resolve_type_or_error(declared_ty)?;
                            if !Self::check_expected_type_for_expr(
                                &resolved_declared_ty,
                                &init_ty,
                                &let_stmt.init,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo de inicialização incompatível para '{}': esperado '{}', encontrado '{}'",
                                        let_stmt.name,
                                        resolved_declared_ty.name(),
                                        init_ty.name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            Self::validate_int_literal_range(
                                &resolved_declared_ty,
                                &let_stmt.init,
                            )?;
                            resolved_declared_ty
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
                    match &assign_stmt.target {
                        AssignTarget::Ident(name) => {
                            let Some(var_meta) = self.resolve_var(name) else {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "variável '{}' não declarada para atribuição",
                                        name
                                    ),
                                    span: assign_stmt.span,
                                });
                            };

                            if !var_meta.is_mut {
                                return Err(PinkerError::Semantic {
                                    msg: format!("reatribuição inválida: '{}' não é mutável", name),
                                    span: assign_stmt.span,
                                });
                            }

                            if !Self::check_expected_type_for_expr(
                                &var_meta.ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na atribuição para '{}': esperado '{}', encontrado '{}'",
                                        name,
                                        var_meta.ty.name(),
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(&var_meta.ty, &assign_stmt.expr)?;
                        }
                        AssignTarget::Deref(ptr_expr) => {
                            let ptr_ty = self.check_value_expr(
                                ptr_expr,
                                "resultado de função sem retorno não pode ser usado como ponteiro de escrita indireta",
                            )?;
                            let expected_value_ty = match ptr_ty {
                                Type::Pointer { base, .. }
                                    if matches!(base.as_ref(), Type::Bombom(_)) =>
                                {
                                    Type::Bombom(ptr_expr.span)
                                }
                                Type::Pointer { .. } => {
                                    return Err(PinkerError::Semantic {
                                        msg: "escrita indireta nesta fase aceita apenas 'seta<bombom>'".to_string(),
                                        span: ptr_expr.span,
                                    });
                                }
                                _ => {
                                    return Err(PinkerError::Semantic {
                                        msg: "escrita indireta requer operando do tipo 'seta<T>'"
                                            .to_string(),
                                        span: ptr_expr.span,
                                    });
                                }
                            };

                            if !Self::check_expected_type_for_expr(
                                &expected_value_ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na escrita indireta: esperado '{}', encontrado '{}'",
                                        expected_value_ty.name(),
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(
                                &expected_value_ty,
                                &assign_stmt.expr,
                            )?;
                        }
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
                Stmt::Falar(falar_stmt) => {
                    let ty = self.check_value_expr(
                        &falar_stmt.expr,
                        "'falar' exige expressão com valor (não nulo)",
                    )?;
                    let is_printable = matches!(
                        ty,
                        Type::Bombom(_)
                            | Type::U8(_)
                            | Type::U16(_)
                            | Type::U32(_)
                            | Type::U64(_)
                            | Type::I8(_)
                            | Type::I16(_)
                            | Type::I32(_)
                            | Type::I64(_)
                            | Type::Logica(_)
                            | Type::Verso(_)
                    );
                    if !is_printable {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "'falar' não suporta tipo '{}'; apenas bombom, u8, u16, u32, u64, i8, i16, i32, i64, logica e verso são imprimíveis",
                                ty.name()
                            ),
                            span: falar_stmt.span,
                        });
                    }
                }
                Stmt::InlineAsm(inline_asm_stmt) => {
                    if inline_asm_stmt.chunks.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: "'sussurro' exige ao menos uma string literal".to_string(),
                            span: inline_asm_stmt.span,
                        });
                    }
                    if inline_asm_stmt
                        .chunks
                        .iter()
                        .any(|chunk| chunk.trim().is_empty())
                    {
                        return Err(PinkerError::Semantic {
                            msg: "bloco de 'sussurro' não pode conter string vazia".to_string(),
                            span: inline_asm_stmt.span,
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
                if !Self::check_expected_type_for_expr(&expected, &value_ty, expr) {
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
                Self::validate_int_literal_range(&expected, expr)?;
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
        let base = function
            .ret_type
            .as_ref()
            .and_then(|ty| self.resolve_type_or_error(ty).ok())
            .unwrap_or(Type::Nulo(span));
        base.with_span(span)
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(_) => Ok(Type::Bombom(expr.span)),
            ExprKind::BoolLit(_) => Ok(Type::Logica(expr.span)),
            ExprKind::StringLit(_) => Ok(Type::Verso(expr.span)),
            ExprKind::Ident(name) => {
                self.resolve_var(name)
                    .map(|meta| meta.ty)
                    .ok_or_else(|| PinkerError::Semantic {
                        msg: format!("identificador '{}' não declarado", name),
                        span: expr.span,
                    })
            }
            ExprKind::Call(callee, args) => self.check_call_expr(expr.span, callee, args),
            ExprKind::FieldAccess { base, field } => {
                let base_ty = self.check_value_expr(
                    base,
                    "resultado de função sem retorno não pode ser base de acesso a campo",
                )?;
                self.resolve_struct_field_type(&base_ty, field, expr.span)
            }
            ExprKind::Index { base, index } => {
                let base_ty = self.check_value_expr(
                    base,
                    "resultado de função sem retorno não pode ser base de indexação",
                )?;
                let index_ty = self.check_value_expr(
                    index,
                    "resultado de função sem retorno não pode ser índice",
                )?;
                if !matches!(index_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "índice nesta fase deve ser 'bombom'".to_string(),
                        span: index.span,
                    });
                }
                match base_ty {
                    Type::FixedArray { element, .. } => Ok(element.as_ref().with_span(expr.span)),
                    _ => Err(PinkerError::Semantic {
                        msg: "indexação exige base de array fixo nesta fase".to_string(),
                        span: expr.span,
                    }),
                }
            }
            ExprKind::Cast {
                expr: source_expr,
                target,
            } => {
                let source_ty = self.check_value_expr(
                    source_expr,
                    "resultado de função sem retorno não pode ser convertido com 'virar'",
                )?;
                let target_ty = self.resolve_type_or_error(target)?.with_span(expr.span);
                if !Self::is_cast_allowed(&source_ty, &target_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "cast explícito inválido nesta fase: '{}' virar '{}'",
                            source_ty.name(),
                            target_ty.name()
                        ),
                        span: expr.span,
                    });
                }
                Ok(target_ty)
            }
            ExprKind::SizeOfType { target } => {
                let resolved = self.resolve_type_or_error(target)?.with_span(expr.span);
                layout::layout_of_type(&resolved, &self.type_aliases, &self.structs).map_err(
                    |msg| PinkerError::Semantic {
                        msg: format!("consulta de peso inválida: {}", msg),
                        span: expr.span,
                    },
                )?;
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::AlignOfType { target } => {
                let resolved = self.resolve_type_or_error(target)?.with_span(expr.span);
                layout::layout_of_type(&resolved, &self.type_aliases, &self.structs).map_err(
                    |msg| PinkerError::Semantic {
                        msg: format!("consulta de alinhamento inválida: {}", msg),
                        span: expr.span,
                    },
                )?;
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_ty = self.check_value_expr(
                    lhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;
                let rhs_ty = self.check_value_expr(
                    rhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;

                if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                    if let Some(pointer_result) =
                        Self::check_pointer_arithmetic(expr.span, *op, &lhs_ty, &rhs_ty)
                    {
                        return pointer_result;
                    }
                }

                let binary_types_compatible = Self::check_type_match(&lhs_ty, &rhs_ty)
                    || (Self::expr_is_int_literal(lhs) && Self::is_integer_type(&rhs_ty))
                    || (Self::expr_is_int_literal(rhs) && Self::is_integer_type(&lhs_ty));
                if !binary_types_compatible {
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
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                        if matches!(lhs_ty, Type::Logica(_)) {
                            Ok(Type::Logica(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação lógica requer operandos 'logica'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
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
                        if Self::is_integer_type(&lhs_ty) {
                            if Self::expr_is_int_literal(lhs)
                                && !Self::expr_is_int_literal(rhs)
                                && Self::is_integer_type(&rhs_ty)
                            {
                                Ok(rhs_ty.with_span(expr.span))
                            } else {
                                Ok(lhs_ty.with_span(expr.span))
                            }
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação aritmética/bitwise requer operandos inteiros compatíveis"
                                    .to_string(),
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
                        if Self::is_integer_type(&inner_ty) {
                            Ok(inner_ty.with_span(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação aritmética requer operando inteiro".to_string(),
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
                    UnaryOp::Deref => match inner_ty {
                        Type::Pointer { base, .. } => match base.as_ref() {
                            Type::Bombom(_) => Ok(Type::Bombom(expr.span)),
                            Type::FixedArray { element, size, .. }
                                if matches!(element.as_ref(), Type::Bombom(_)) =>
                            {
                                Ok(Type::FixedArray {
                                    element: Box::new(Type::Bombom(expr.span)),
                                    size: *size,
                                    span: expr.span,
                                })
                            }
                            Type::Struct { name, .. } => Ok(Type::Struct {
                                name: name.clone(),
                                span: expr.span,
                            }),
                            _ => Err(PinkerError::Semantic {
                                msg: "dereferência nesta fase aceita apenas 'seta<bombom>', 'seta<[bombom; N]>' ou 'seta<ninho>'".to_string(),
                                span: expr.span,
                            }),
                        },
                        _ => Err(PinkerError::Semantic {
                            msg: "dereferência requer operando do tipo 'seta<T>'".to_string(),
                            span: expr.span,
                        }),
                    },
                }
            }
        }
    }

    fn check_pointer_arithmetic(
        expr_span: Span,
        op: BinaryOp,
        lhs_ty: &Type,
        rhs_ty: &Type,
    ) -> Option<Result<Type, PinkerError>> {
        let is_ptr_bombom = |ty: &Type| {
            matches!(
                ty,
                Type::Pointer { base, .. } if matches!(base.as_ref(), Type::Bombom(_))
            )
        };
        let is_bombom = |ty: &Type| matches!(ty, Type::Bombom(_));

        if is_ptr_bombom(lhs_ty) && is_bombom(rhs_ty) {
            return Some(Ok(lhs_ty.with_span(expr_span)));
        }
        if is_bombom(lhs_ty) && is_ptr_bombom(rhs_ty) {
            let msg = match op {
                BinaryOp::Add => {
                    "aritmética de ponteiro nesta fase suporta apenas 'ptr + bombom' e 'ptr - bombom'"
                }
                BinaryOp::Sub => {
                    "subtração de ponteiro nesta fase suporta apenas 'ptr - bombom'"
                }
                _ => unreachable!("check_pointer_arithmetic só recebe add/sub"),
            };
            return Some(Err(PinkerError::Semantic {
                msg: msg.to_string(),
                span: expr_span,
            }));
        }
        if matches!(lhs_ty, Type::Pointer { .. }) || matches!(rhs_ty, Type::Pointer { .. }) {
            let msg = match op {
                BinaryOp::Add => "aritmética de ponteiro nesta fase exige 'seta<bombom> + bombom'",
                BinaryOp::Sub => "aritmética de ponteiro nesta fase exige 'seta<bombom> - bombom'",
                _ => unreachable!("check_pointer_arithmetic só recebe add/sub"),
            };
            return Some(Err(PinkerError::Semantic {
                msg: msg.to_string(),
                span: expr_span,
            }));
        }
        None
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

        if name == "ouvir" {
            if !args.is_empty() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'ouvir' com aridade inválida: esperado 0, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "abrir" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'abrir' com aridade inválida: esperado 1, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(arg_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'abrir': esperado 'verso', encontrado '{}'",
                        arg_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "ler_arquivo" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'ler_arquivo' com aridade inválida: esperado 1, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(arg_ty, Type::Bombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'ler_arquivo': esperado 'bombom', encontrado '{}'",
                        arg_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "fechar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'fechar' com aridade inválida: esperado 1, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(arg_ty, Type::Bombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'fechar': esperado 'bombom', encontrado '{}'",
                        arg_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Nulo(expr_span));
        }
        if name == "escrever" {
            if args.len() != 2 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'escrever' com aridade inválida: esperado 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let handle_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(handle_ty, Type::Bombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'escrever': esperado 'bombom', encontrado '{}'",
                        handle_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            let value_ty = self.check_value_expr(
                &args[1],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(value_ty, Type::Bombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 2 da chamada 'escrever': esperado 'bombom', encontrado '{}'",
                        value_ty.name()
                    ),
                    span: args[1].span,
                });
            }
            return Ok(Type::Nulo(expr_span));
        }

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
            let expected_param_ty = self.resolve_type_or_error(&param.ty)?;
            if !Self::check_expected_type_for_expr(&expected_param_ty, &arg_ty, arg) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                        index + 1,
                        name,
                        expected_param_ty.name(),
                        arg_ty.name()
                    ),
                    span: arg.span,
                });
            }
            Self::validate_int_literal_range(&expected_param_ty, arg)?;
        }

        Ok(self.function_result_type(&function, expr_span))
    }
}

pub fn check_program(program: &Program) -> Result<(), PinkerError> {
    SemanticChecker::new().check_program(program)
}
