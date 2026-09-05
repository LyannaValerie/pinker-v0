use super::*;

impl Parser {
    // @pinker-nav:start parser.genericos.inferencia-local
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Inferência genérica local e determinística para chamadas sem argumentos de tipo explícitos: sintetiza somente tipos locais de argumentos, unifica recursivamente posições formais com parâmetros de tipo, exige substituição única, diagnostica conflito/ausência de fonte e registra a mesma instanciação monomórfica usada pelo caminho explícito. Não usa tipo de retorno esperado, não executa coercion e não contém dispatch nominal por função.
    pub(super) fn push_value_param_scope(&mut self, params: &[Param]) {
        self.value_type_scopes.push(
            params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .collect(),
        );
    }

    pub(super) fn register_value_type(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.value_type_scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn resolve_value_type(&self, name: &str) -> Option<Type> {
        self.value_type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn inference_type_eq(lhs: &Type, rhs: &Type) -> bool {
        match (lhs, rhs) {
            (Type::Bombom(_), Type::Bombom(_))
            | (Type::U8(_), Type::U8(_))
            | (Type::U16(_), Type::U16(_))
            | (Type::U32(_), Type::U32(_))
            | (Type::U64(_), Type::U64(_))
            | (Type::I8(_), Type::I8(_))
            | (Type::I16(_), Type::I16(_))
            | (Type::I32(_), Type::I32(_))
            | (Type::I64(_), Type::I64(_))
            | (Type::Logica(_), Type::Logica(_))
            | (Type::Verso(_), Type::Verso(_))
            | (Type::ListBombom(_), Type::ListBombom(_))
            | (Type::ListVerso(_), Type::ListVerso(_))
            | (Type::MapVersoBombom(_), Type::MapVersoBombom(_))
            | (Type::MapVersoVerso(_), Type::MapVersoVerso(_))
            | (Type::MapBombomBombom(_), Type::MapBombomBombom(_))
            | (Type::MapBombomVerso(_), Type::MapBombomVerso(_))
            | (Type::Nulo(_), Type::Nulo(_)) => true,
            (
                Type::Alias { name: lhs, .. }
                | Type::Struct { name: lhs, .. }
                | Type::Enum { name: lhs, .. },
                Type::Alias { name: rhs, .. }
                | Type::Struct { name: rhs, .. }
                | Type::Enum { name: rhs, .. },
            ) => lhs == rhs,
            (Type::ListEnum { element: lhs, .. }, Type::ListEnum { element: rhs, .. }) => {
                lhs == rhs
            }
            (
                Type::Map {
                    key: lhs_key,
                    value: lhs_value,
                    ..
                },
                Type::Map {
                    key: rhs_key,
                    value: rhs_value,
                    ..
                },
            ) => {
                Self::inference_type_eq(lhs_key, rhs_key)
                    && Self::inference_type_eq(lhs_value, rhs_value)
            }
            (
                Type::FixedArray {
                    element: lhs,
                    size: lhs_size,
                    ..
                },
                Type::FixedArray {
                    element: rhs,
                    size: rhs_size,
                    ..
                },
            ) => lhs_size == rhs_size && Self::inference_type_eq(lhs, rhs),
            (
                Type::Pointer {
                    base: lhs,
                    is_volatile: lhs_volatile,
                    ..
                },
                Type::Pointer {
                    base: rhs,
                    is_volatile: rhs_volatile,
                    ..
                },
            ) => lhs_volatile == rhs_volatile && Self::inference_type_eq(lhs, rhs),
            (
                Type::Function {
                    params: lhs_params,
                    ret: lhs_ret,
                    ..
                },
                Type::Function {
                    params: rhs_params,
                    ret: rhs_ret,
                    ..
                },
            ) => {
                lhs_params.len() == rhs_params.len()
                    && lhs_params
                        .iter()
                        .zip(rhs_params)
                        .all(|(lhs, rhs)| Self::inference_type_eq(lhs, rhs))
                    && Self::inference_type_eq(lhs_ret, rhs_ret)
            }
            (
                Type::Applied {
                    name: lhs_name,
                    args: lhs_args,
                    ..
                },
                Type::Applied {
                    name: rhs_name,
                    args: rhs_args,
                    ..
                },
            ) => {
                lhs_name == rhs_name
                    && lhs_args.len() == rhs_args.len()
                    && lhs_args
                        .iter()
                        .zip(rhs_args)
                        .all(|(lhs, rhs)| Self::inference_type_eq(lhs, rhs))
            }
            (
                Type::Union {
                    members: lhs_members,
                    ..
                },
                Type::Union {
                    members: rhs_members,
                    ..
                },
            ) => {
                lhs_members.len() == rhs_members.len()
                    && lhs_members
                        .iter()
                        .zip(rhs_members)
                        .all(|(lhs, rhs)| Self::inference_type_eq(lhs, rhs))
            }
            _ => false,
        }
    }

    fn formal_contains_type_param(formal: &Type, type_params: &HashSet<String>) -> bool {
        match formal {
            Type::Alias { name, .. } => type_params.contains(name),
            Type::ListEnum { element, .. } => type_params.contains(element),
            Type::FixedArray { element, .. } => {
                Self::formal_contains_type_param(element, type_params)
            }
            Type::Pointer { base, .. } => Self::formal_contains_type_param(base, type_params),
            Type::Function { params, ret, .. } => {
                params
                    .iter()
                    .any(|param| Self::formal_contains_type_param(param, type_params))
                    || Self::formal_contains_type_param(ret, type_params)
            }
            Type::Applied { args, .. } => args
                .iter()
                .any(|arg| Self::formal_contains_type_param(arg, type_params)),
            Type::Map { key, value, .. } => {
                Self::formal_contains_type_param(key, type_params)
                    || Self::formal_contains_type_param(value, type_params)
            }
            Type::Union { members, .. } => members
                .iter()
                .any(|member| Self::formal_contains_type_param(member, type_params)),
            _ => false,
        }
    }

    fn bind_inferred_type(
        name: &str,
        actual: &Type,
        substitutions: &mut HashMap<String, Type>,
        span: Span,
    ) -> Result<(), PinkerError> {
        if let Some(previous) = substitutions.get(name) {
            if !Self::inference_type_eq(previous, actual) {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "E-GENERIC-CONFLICTING-INFERENCE: parâmetro '{}' recebeu evidências incompatíveis '{}' e '{}'",
                        name,
                        previous.display_name(),
                        actual.display_name()
                    ),
                    span,
                });
            }
        } else {
            substitutions.insert(name.to_string(), actual.clone());
        }
        Ok(())
    }

    fn infer_substitutions_from_types(
        formal: &Type,
        actual: &Type,
        type_params: &HashSet<String>,
        substitutions: &mut HashMap<String, Type>,
        span: Span,
    ) -> Result<(), PinkerError> {
        if let Type::Alias { name, .. } = formal {
            if type_params.contains(name) {
                return Self::bind_inferred_type(name, actual, substitutions, span);
            }
        }

        match (formal, actual) {
            (
                Type::ListEnum { element, .. },
                Type::ListBombom(actual_span),
            ) if type_params.contains(element) => Self::bind_inferred_type(
                element,
                &Type::Bombom(*actual_span),
                substitutions,
                span,
            ),
            (
                Type::ListEnum { element, .. },
                Type::ListVerso(actual_span),
            ) if type_params.contains(element) => Self::bind_inferred_type(
                element,
                &Type::Verso(*actual_span),
                substitutions,
                span,
            ),
            (
                Type::ListEnum { element, .. },
                Type::ListEnum {
                    element: actual_element,
                    span: actual_span,
                },
            ) if type_params.contains(element) => Self::bind_inferred_type(
                element,
                &Type::Alias {
                    name: actual_element.clone(),
                    span: *actual_span,
                },
                substitutions,
                span,
            ),
            (
                Type::FixedArray {
                    element: formal_element,
                    size: formal_size,
                    ..
                },
                Type::FixedArray {
                    element: actual_element,
                    size: actual_size,
                    ..
                },
            ) if formal_size == actual_size => Self::infer_substitutions_from_types(
                formal_element,
                actual_element,
                type_params,
                substitutions,
                span,
            ),
            (
                Type::Pointer {
                    base: formal_base,
                    is_volatile: formal_volatile,
                    ..
                },
                Type::Pointer {
                    base: actual_base,
                    is_volatile: actual_volatile,
                    ..
                },
            ) if formal_volatile == actual_volatile => Self::infer_substitutions_from_types(
                formal_base,
                actual_base,
                type_params,
                substitutions,
                span,
            ),
            (
                Type::Function {
                    params: formal_params,
                    ret: formal_ret,
                    ..
                },
                Type::Function {
                    params: actual_params,
                    ret: actual_ret,
                    ..
                },
            ) if formal_params.len() == actual_params.len() => {
                for (formal, actual) in formal_params.iter().zip(actual_params) {
                    Self::infer_substitutions_from_types(
                        formal,
                        actual,
                        type_params,
                        substitutions,
                        span,
                    )?;
                }
                Self::infer_substitutions_from_types(
                    formal_ret,
                    actual_ret,
                    type_params,
                    substitutions,
                    span,
                )
            }
            (
                Type::Applied {
                    name: formal_name,
                    args: formal_args,
                    ..
                },
                Type::Applied {
                    name: actual_name,
                    args: actual_args,
                    ..
                },
            ) if formal_name == actual_name && formal_args.len() == actual_args.len() => {
                for (formal, actual) in formal_args.iter().zip(actual_args) {
                    Self::infer_substitutions_from_types(
                        formal,
                        actual,
                        type_params,
                        substitutions,
                        span,
                    )?;
                }
                Ok(())
            }
            (
                Type::Map {
                    key: formal_key,
                    value: formal_value,
                    ..
                },
                Type::Map {
                    key: actual_key,
                    value: actual_value,
                    ..
                },
            ) => {
                Self::infer_substitutions_from_types(
                    formal_key,
                    actual_key,
                    type_params,
                    substitutions,
                    span,
                )?;
                Self::infer_substitutions_from_types(
                    formal_value,
                    actual_value,
                    type_params,
                    substitutions,
                    span,
                )
            }
            (
                Type::Union {
                    members: formal_members,
                    ..
                },
                Type::Union {
                    members: actual_members,
                    ..
                },
            ) if formal_members.len() == actual_members.len() => {
                for (formal, actual) in formal_members.iter().zip(actual_members) {
                    Self::infer_substitutions_from_types(
                        formal,
                        actual,
                        type_params,
                        substitutions,
                        span,
                    )?;
                }
                Ok(())
            }
            _ if !Self::formal_contains_type_param(formal, type_params) => Ok(()),
            _ => Err(PinkerError::Parse {
                msg: format!(
                    "E-GENERIC-NO-INFERENCE-SOURCE: tipo real '{}' não corresponde à estrutura inferível '{}'",
                    actual.display_name(),
                    formal.display_name()
                ),
                span,
            }),
        }
    }

    fn inferred_generic_result_type(&self, name: &str, span: Span) -> Option<Type> {
        self.generic_instantiations
            .iter()
            .rev()
            .find_map(|instantiation| {
                (self.generic_function_name(&instantiation.name, &instantiation.type_args) == name)
                    .then(|| {
                        let template = self.generic_templates.get(&instantiation.name)?;
                        let substitutions = template
                            .type_params
                            .iter()
                            .cloned()
                            .zip(instantiation.type_args.iter().cloned())
                            .collect::<HashMap<_, _>>();
                        template
                            .ret_type
                            .as_ref()
                            .map(|ret| Self::substitute_type(ret, &substitutions).with_span(span))
                    })
                    .flatten()
            })
    }

    pub(super) fn infer_local_expr_type(&self, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::IntLit(_) => Some(Type::Bombom(expr.span)),
            ExprKind::BoolLit(_) => Some(Type::Logica(expr.span)),
            ExprKind::StringLit(_) => Some(Type::Verso(expr.span)),
            ExprKind::Ident(name) => self.resolve_value_type(name),
            ExprKind::Unary(UnaryOp::Not, _) => Some(Type::Logica(expr.span)),
            ExprKind::Unary(_, inner) => self
                .infer_local_expr_type(inner)
                .map(|ty| ty.with_span(expr.span)),
            ExprKind::AddressOf(inner) => {
                self.infer_local_expr_type(inner).map(|base| Type::Pointer {
                    base: Box::new(base),
                    is_volatile: false,
                    span: expr.span,
                })
            }
            ExprKind::Cast { target, .. } => Some(target.with_span(expr.span)),
            ExprKind::SizeOfType { .. } | ExprKind::AlignOfType { .. } => {
                Some(Type::Bombom(expr.span))
            }
            ExprKind::Binary(
                _,
                BinaryOp::LogicalAnd
                | BinaryOp::LogicalOr
                | BinaryOp::Eq
                | BinaryOp::Neq
                | BinaryOp::Lt
                | BinaryOp::Lte
                | BinaryOp::Gt
                | BinaryOp::Gte,
                _,
            ) => Some(Type::Logica(expr.span)),
            ExprKind::Binary(lhs, _, _) => self
                .infer_local_expr_type(lhs)
                .map(|ty| ty.with_span(expr.span)),
            ExprKind::Call(callee, _) => match &callee.kind {
                ExprKind::Ident(name) => self.inferred_generic_result_type(name, expr.span),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn infer_generic_call_type_args(
        &self,
        template: &FunctionDecl,
        args: &[Expr],
        span: Span,
    ) -> Result<Vec<Type>, PinkerError> {
        if args.len() != template.params.len() {
            return Err(PinkerError::Parse {
                msg: format!(
                    "E-GENERIC-CALL-ARITY: função genérica '{}' exige {} argumento(s), recebido {}",
                    template.name,
                    template.params.len(),
                    args.len()
                ),
                span,
            });
        }

        let type_params = template.type_params.iter().cloned().collect::<HashSet<_>>();
        let mut substitutions = HashMap::new();
        for (index, (param, arg)) in template.params.iter().zip(args).enumerate() {
            if !Self::formal_contains_type_param(&param.ty, &type_params) {
                continue;
            }
            let Some(actual) = self.infer_local_expr_type(arg) else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "E-GENERIC-NO-INFERENCE-SOURCE: argumento {} da chamada '{}' não possui tipo local sintetizável",
                        index + 1,
                        template.name
                    ),
                    span: arg.span,
                });
            };
            Self::infer_substitutions_from_types(
                &param.ty,
                &actual,
                &type_params,
                &mut substitutions,
                arg.span,
            )?;
        }

        let missing = template
            .type_params
            .iter()
            .filter(|param| !substitutions.contains_key(*param))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(PinkerError::Parse {
                msg: format!(
                    "E-GENERIC-NO-INFERENCE-SOURCE: chamada '{}' não fornece evidência para {}",
                    template.name,
                    missing.join(", ")
                ),
                span,
            });
        }

        Ok(template
            .type_params
            .iter()
            .filter_map(|param| substitutions.remove(param))
            .collect())
    }

    // @pinker-nav:end parser.genericos.inferencia-local

    // @pinker-nav:start parser.genericos.substituicao-ast
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Substituição recursiva de parâmetros de tipo numa AST-template: aplica a tabela parâmetro-de-tipo → tipo concreto percorrendo `Type` (inclusive `ListEnum` que colapsa para lista concreta), `Expr`, `AssignTarget`, `Block`, `ElseBlock`, `IfStmt` e `Stmt`, produzindo uma cópia concreta com os spans preservados. É uma única operação recursiva distribuída por vários helpers `substitute_*`; não executa checagem semântica nem lowering para IR.
    pub(super) fn substitute_type(ty: &Type, substitutions: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Alias { name, span } => substitutions
                .get(name)
                .map(|ty| ty.with_span(*span))
                .unwrap_or_else(|| ty.clone()),
            Type::FixedArray {
                element,
                size,
                span,
            } => Type::FixedArray {
                element: Box::new(Self::substitute_type(element, substitutions)),
                size: *size,
                span: *span,
            },
            Type::Pointer {
                base,
                is_volatile,
                span,
            } => Type::Pointer {
                base: Box::new(Self::substitute_type(base, substitutions)),
                is_volatile: *is_volatile,
                span: *span,
            },
            Type::Function { params, ret, span } => Type::Function {
                params: params
                    .iter()
                    .map(|param| Self::substitute_type(param, substitutions))
                    .collect(),
                ret: Box::new(Self::substitute_type(ret, substitutions)),
                span: *span,
            },
            Type::Applied { name, args, span } => Type::Applied {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| Self::substitute_type(arg, substitutions))
                    .collect(),
                span: *span,
            },
            Type::ListEnum { element, span } => {
                if let Some(ty) = substitutions.get(element) {
                    match ty {
                        Type::Bombom(_) => Type::ListBombom(*span),
                        Type::Verso(_) => Type::ListVerso(*span),
                        Type::Alias { name, .. }
                        | Type::Enum { name, .. }
                        | Type::Struct { name, .. } => Type::ListEnum {
                            element: name.clone(),
                            span: *span,
                        },
                        _ => ty.clone(),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Map { key, value, span } => Type::Map {
                key: Box::new(Self::substitute_type(key, substitutions)),
                value: Box::new(Self::substitute_type(value, substitutions)),
                span: *span,
            },
            _ => ty.clone(),
        }
    }

    fn substitute_expr(expr: &Expr, substitutions: &HashMap<String, Type>) -> Expr {
        let kind = match &expr.kind {
            ExprKind::Binary(lhs, op, rhs) => ExprKind::Binary(
                Box::new(Self::substitute_expr(lhs, substitutions)),
                *op,
                Box::new(Self::substitute_expr(rhs, substitutions)),
            ),
            ExprKind::Unary(op, operand) => {
                ExprKind::Unary(*op, Box::new(Self::substitute_expr(operand, substitutions)))
            }
            ExprKind::AddressOf(operand) => {
                ExprKind::AddressOf(Box::new(Self::substitute_expr(operand, substitutions)))
            }
            ExprKind::Intrinsic(identity) => ExprKind::Intrinsic(*identity),
            ExprKind::Call(callee, args) => ExprKind::Call(
                Box::new(Self::substitute_expr(callee, substitutions)),
                args.iter()
                    .map(|arg| Self::substitute_expr(arg, substitutions))
                    .collect(),
            ),
            ExprKind::InternalMapIterCreate(map) => {
                ExprKind::InternalMapIterCreate(Box::new(Self::substitute_expr(map, substitutions)))
            }
            ExprKind::InternalMapIterNextKey(iterator) => ExprKind::InternalMapIterNextKey(
                Box::new(Self::substitute_expr(iterator, substitutions)),
            ),
            ExprKind::FieldAccess { base, field } => ExprKind::FieldAccess {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                field: field.clone(),
            },
            ExprKind::Index { base, index } => ExprKind::Index {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                index: Box::new(Self::substitute_expr(index, substitutions)),
            },
            ExprKind::Cast { expr, target } => ExprKind::Cast {
                expr: Box::new(Self::substitute_expr(expr, substitutions)),
                target: Self::substitute_type(target, substitutions),
            },
            ExprKind::SizeOfType { target } => ExprKind::SizeOfType {
                target: Self::substitute_type(target, substitutions),
            },
            ExprKind::AlignOfType { target } => ExprKind::AlignOfType {
                target: Self::substitute_type(target, substitutions),
            },
            ExprKind::Ident(_)
            | ExprKind::IntLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_) => expr.kind.clone(),
        };
        Expr {
            kind,
            span: expr.span,
        }
    }

    fn substitute_assign_target(
        target: &AssignTarget,
        substitutions: &HashMap<String, Type>,
    ) -> AssignTarget {
        match target {
            AssignTarget::Ident(name) => AssignTarget::Ident(name.clone()),
            AssignTarget::Deref(expr) => {
                AssignTarget::Deref(Box::new(Self::substitute_expr(expr, substitutions)))
            }
            AssignTarget::FieldDeref { base, field } => AssignTarget::FieldDeref {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                field: field.clone(),
            },
            AssignTarget::Index { base, index } => AssignTarget::Index {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                index: Box::new(Self::substitute_expr(index, substitutions)),
            },
        }
    }

    fn substitute_block(block: &Block, substitutions: &HashMap<String, Type>) -> Block {
        Block {
            stmts: block
                .stmts
                .iter()
                .map(|stmt| Self::substitute_stmt(stmt, substitutions))
                .collect(),
            span: block.span,
        }
    }

    fn substitute_else_block(
        else_block: &ElseBlock,
        substitutions: &HashMap<String, Type>,
    ) -> ElseBlock {
        match else_block {
            ElseBlock::Block(block) => {
                ElseBlock::Block(Self::substitute_block(block, substitutions))
            }
            ElseBlock::If(if_stmt) => {
                ElseBlock::If(Box::new(Self::substitute_if_stmt(if_stmt, substitutions)))
            }
        }
    }

    fn substitute_if_stmt(if_stmt: &IfStmt, substitutions: &HashMap<String, Type>) -> IfStmt {
        IfStmt {
            condition: Self::substitute_expr(&if_stmt.condition, substitutions),
            then_branch: Self::substitute_block(&if_stmt.then_branch, substitutions),
            else_branch: if_stmt
                .else_branch
                .as_ref()
                .map(|else_branch| Self::substitute_else_block(else_branch, substitutions)),
            span: if_stmt.span,
        }
    }

    fn substitute_stmt(stmt: &Stmt, substitutions: &HashMap<String, Type>) -> Stmt {
        match stmt {
            Stmt::Let(let_stmt) => Stmt::Let(LetStmt {
                name: let_stmt.name.clone(),
                is_mut: let_stmt.is_mut,
                ty: let_stmt
                    .ty
                    .as_ref()
                    .map(|ty| Self::substitute_type(ty, substitutions)),
                init: Self::substitute_expr(&let_stmt.init, substitutions),
                span: let_stmt.span,
            }),
            Stmt::Return(return_stmt) => Stmt::Return(ReturnStmt {
                expr: return_stmt
                    .expr
                    .as_ref()
                    .map(|expr| Self::substitute_expr(expr, substitutions)),
                span: return_stmt.span,
            }),
            Stmt::Assign(assign_stmt) => Stmt::Assign(AssignStmt {
                target: Self::substitute_assign_target(&assign_stmt.target, substitutions),
                expr: Self::substitute_expr(&assign_stmt.expr, substitutions),
                span: assign_stmt.span,
            }),
            Stmt::If(if_stmt) => Stmt::If(Self::substitute_if_stmt(if_stmt, substitutions)),
            Stmt::While(while_stmt) => Stmt::While(WhileStmt {
                condition: Self::substitute_expr(&while_stmt.condition, substitutions),
                body: Self::substitute_block(&while_stmt.body, substitutions),
                span: while_stmt.span,
            }),
            Stmt::Break(stmt) => Stmt::Break(stmt.clone()),
            Stmt::Continue(stmt) => Stmt::Continue(stmt.clone()),
            Stmt::Falar(falar) => Stmt::Falar(FalarStmt {
                args: falar
                    .args
                    .iter()
                    .map(|arg| Self::substitute_expr(arg, substitutions))
                    .collect(),
                span: falar.span,
            }),
            Stmt::InlineAsm(stmt) => Stmt::InlineAsm(InlineAsmStmt {
                chunks: stmt.chunks.clone(),
                operands: stmt
                    .operands
                    .iter()
                    .map(|operand| InlineAsmOperand {
                        name: operand.name.clone(),
                        direction: operand.direction.clone(),
                        constraint: operand.constraint.clone(),
                        value: Self::substitute_expr(&operand.value, substitutions),
                        span: operand.span,
                    })
                    .collect(),
                clobbers: stmt.clobbers.clone(),
                span: stmt.span,
            }),
            Stmt::EnumMatch(enum_match) => Stmt::EnumMatch(EnumMatchStmt {
                scrutinee: Self::substitute_expr(&enum_match.scrutinee, substitutions),
                arms: enum_match
                    .arms
                    .iter()
                    .map(|arm| EnumMatchArm {
                        pattern: arm.pattern.clone(),
                        body: Self::substitute_block(&arm.body, substitutions),
                        span: arm.span,
                    })
                    .collect(),
                otherwise: enum_match
                    .otherwise
                    .as_ref()
                    .map(|block| Self::substitute_block(block, substitutions)),
                span: enum_match.span,
            }),
            Stmt::UnionMatch(union_match) => Stmt::UnionMatch(UnionMatchStmt {
                scrutinee: Self::substitute_expr(&union_match.scrutinee, substitutions),
                arms: union_match
                    .arms
                    .iter()
                    .map(|arm| UnionMatchArm {
                        member_type: Self::substitute_type(&arm.member_type, substitutions),
                        binding: arm.binding.clone(),
                        body: Self::substitute_block(&arm.body, substitutions),
                        span: arm.span,
                    })
                    .collect(),
                span: union_match.span,
            }),
            Stmt::Expr(expr) => Stmt::Expr(Self::substitute_expr(expr, substitutions)),
        }
    }
    // @pinker-nav:end parser.genericos.substituicao-ast

    // @pinker-nav:start parser.callbacks.substituicao-estatica
    // @pinker-nav:domain callbacks
    // @pinker-nav:layer parser
    // @pinker-nav:summary Reescrita de chamadas a parâmetros-função por chamadas diretas: percorre recursivamente `Expr`, `AssignTarget`, `Block`, `ElseBlock`, `IfStmt` e `Stmt` de um corpo-template e, quando o callee de uma chamada é um identificador ligado a um callback (tabela nome-do-parâmetro → função concreta), troca-o pelo nome da função concreta, preservando as demais expressões e spans. É especialização de callbacks estáticos, não substituição de tipos genéricos.
    fn substitute_function_param_expr(expr: &Expr, replacements: &HashMap<String, String>) -> Expr {
        let kind = match &expr.kind {
            ExprKind::Call(callee, args) => {
                let substituted_callee = Self::substitute_function_param_expr(callee, replacements);
                let callee = if let ExprKind::Ident(name) = &substituted_callee.kind {
                    if let Some(function_name) = replacements.get(name) {
                        Expr {
                            kind: ExprKind::Ident(function_name.clone()),
                            span: substituted_callee.span,
                        }
                    } else {
                        substituted_callee
                    }
                } else {
                    substituted_callee
                };
                ExprKind::Call(
                    Box::new(callee),
                    args.iter()
                        .map(|arg| Self::substitute_function_param_expr(arg, replacements))
                        .collect(),
                )
            }
            ExprKind::Binary(lhs, op, rhs) => ExprKind::Binary(
                Box::new(Self::substitute_function_param_expr(lhs, replacements)),
                *op,
                Box::new(Self::substitute_function_param_expr(rhs, replacements)),
            ),
            ExprKind::Unary(op, operand) => ExprKind::Unary(
                *op,
                Box::new(Self::substitute_function_param_expr(operand, replacements)),
            ),
            ExprKind::AddressOf(operand) => ExprKind::AddressOf(Box::new(
                Self::substitute_function_param_expr(operand, replacements),
            )),
            ExprKind::FieldAccess { base, field } => ExprKind::FieldAccess {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                field: field.clone(),
            },
            ExprKind::Index { base, index } => ExprKind::Index {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                index: Box::new(Self::substitute_function_param_expr(index, replacements)),
            },
            ExprKind::Cast { expr, target } => ExprKind::Cast {
                expr: Box::new(Self::substitute_function_param_expr(expr, replacements)),
                target: target.clone(),
            },
            ExprKind::InternalMapIterCreate(map) => ExprKind::InternalMapIterCreate(Box::new(
                Self::substitute_function_param_expr(map, replacements),
            )),
            ExprKind::InternalMapIterNextKey(iterator) => ExprKind::InternalMapIterNextKey(
                Box::new(Self::substitute_function_param_expr(iterator, replacements)),
            ),
            ExprKind::SizeOfType { target } => ExprKind::SizeOfType {
                target: target.clone(),
            },
            ExprKind::AlignOfType { target } => ExprKind::AlignOfType {
                target: target.clone(),
            },
            ExprKind::Ident(name) => ExprKind::Ident(
                replacements
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
            ),
            // #532: identidade intrínseca não é nome de parâmetro-função e
            // nunca é substituída por callback estático.
            ExprKind::Intrinsic(_)
            | ExprKind::IntLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_) => expr.kind.clone(),
        };
        Expr {
            kind,
            span: expr.span,
        }
    }

    fn substitute_function_param_assign_target(
        target: &AssignTarget,
        replacements: &HashMap<String, String>,
    ) -> AssignTarget {
        match target {
            AssignTarget::Ident(name) => AssignTarget::Ident(name.clone()),
            AssignTarget::Deref(expr) => AssignTarget::Deref(Box::new(
                Self::substitute_function_param_expr(expr, replacements),
            )),
            AssignTarget::FieldDeref { base, field } => AssignTarget::FieldDeref {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                field: field.clone(),
            },
            AssignTarget::Index { base, index } => AssignTarget::Index {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                index: Box::new(Self::substitute_function_param_expr(index, replacements)),
            },
        }
    }

    pub(super) fn substitute_function_param_block(
        block: &Block,
        replacements: &HashMap<String, String>,
    ) -> Block {
        Block {
            stmts: block
                .stmts
                .iter()
                .map(|stmt| Self::substitute_function_param_stmt(stmt, replacements))
                .collect(),
            span: block.span,
        }
    }

    fn substitute_function_param_else_block(
        else_block: &ElseBlock,
        replacements: &HashMap<String, String>,
    ) -> ElseBlock {
        match else_block {
            ElseBlock::Block(block) => {
                ElseBlock::Block(Self::substitute_function_param_block(block, replacements))
            }
            ElseBlock::If(if_stmt) => ElseBlock::If(Box::new(
                Self::substitute_function_param_if_stmt(if_stmt, replacements),
            )),
        }
    }

    fn substitute_function_param_if_stmt(
        if_stmt: &IfStmt,
        replacements: &HashMap<String, String>,
    ) -> IfStmt {
        IfStmt {
            condition: Self::substitute_function_param_expr(&if_stmt.condition, replacements),
            then_branch: Self::substitute_function_param_block(&if_stmt.then_branch, replacements),
            else_branch: if_stmt.else_branch.as_ref().map(|else_branch| {
                Self::substitute_function_param_else_block(else_branch, replacements)
            }),
            span: if_stmt.span,
        }
    }

    fn substitute_function_param_stmt(stmt: &Stmt, replacements: &HashMap<String, String>) -> Stmt {
        match stmt {
            Stmt::Let(let_stmt) => Stmt::Let(LetStmt {
                name: let_stmt.name.clone(),
                is_mut: let_stmt.is_mut,
                ty: let_stmt.ty.clone(),
                init: Self::substitute_function_param_expr(&let_stmt.init, replacements),
                span: let_stmt.span,
            }),
            Stmt::Return(return_stmt) => Stmt::Return(ReturnStmt {
                expr: return_stmt
                    .expr
                    .as_ref()
                    .map(|expr| Self::substitute_function_param_expr(expr, replacements)),
                span: return_stmt.span,
            }),
            Stmt::Assign(assign_stmt) => Stmt::Assign(AssignStmt {
                target: Self::substitute_function_param_assign_target(
                    &assign_stmt.target,
                    replacements,
                ),
                expr: Self::substitute_function_param_expr(&assign_stmt.expr, replacements),
                span: assign_stmt.span,
            }),
            Stmt::If(if_stmt) => Stmt::If(Self::substitute_function_param_if_stmt(
                if_stmt,
                replacements,
            )),
            Stmt::While(while_stmt) => Stmt::While(WhileStmt {
                condition: Self::substitute_function_param_expr(
                    &while_stmt.condition,
                    replacements,
                ),
                body: Self::substitute_function_param_block(&while_stmt.body, replacements),
                span: while_stmt.span,
            }),
            Stmt::Break(stmt) => Stmt::Break(stmt.clone()),
            Stmt::Continue(stmt) => Stmt::Continue(stmt.clone()),
            Stmt::Falar(falar) => Stmt::Falar(FalarStmt {
                args: falar
                    .args
                    .iter()
                    .map(|arg| Self::substitute_function_param_expr(arg, replacements))
                    .collect(),
                span: falar.span,
            }),
            Stmt::InlineAsm(stmt) => Stmt::InlineAsm(InlineAsmStmt {
                chunks: stmt.chunks.clone(),
                operands: stmt
                    .operands
                    .iter()
                    .map(|operand| InlineAsmOperand {
                        name: operand.name.clone(),
                        direction: operand.direction.clone(),
                        constraint: operand.constraint.clone(),
                        value: Self::substitute_function_param_expr(&operand.value, replacements),
                        span: operand.span,
                    })
                    .collect(),
                clobbers: stmt.clobbers.clone(),
                span: stmt.span,
            }),
            Stmt::EnumMatch(enum_match) => Stmt::EnumMatch(EnumMatchStmt {
                scrutinee: Self::substitute_function_param_expr(
                    &enum_match.scrutinee,
                    replacements,
                ),
                arms: enum_match
                    .arms
                    .iter()
                    .map(|arm| EnumMatchArm {
                        pattern: arm.pattern.clone(),
                        body: Self::substitute_function_param_block(&arm.body, replacements),
                        span: arm.span,
                    })
                    .collect(),
                otherwise: enum_match
                    .otherwise
                    .as_ref()
                    .map(|block| Self::substitute_function_param_block(block, replacements)),
                span: enum_match.span,
            }),
            Stmt::UnionMatch(union_match) => Stmt::UnionMatch(UnionMatchStmt {
                scrutinee: Self::substitute_function_param_expr(
                    &union_match.scrutinee,
                    replacements,
                ),
                arms: union_match
                    .arms
                    .iter()
                    .map(|arm| UnionMatchArm {
                        member_type: arm.member_type.clone(),
                        binding: arm.binding.clone(),
                        body: Self::substitute_function_param_block(&arm.body, replacements),
                        span: arm.span,
                    })
                    .collect(),
                span: union_match.span,
            }),
            Stmt::Expr(expr) => {
                Stmt::Expr(Self::substitute_function_param_expr(expr, replacements))
            }
        }
    }
    // @pinker-nav:end parser.callbacks.substituicao-estatica

    // @pinker-nav:start parser.callbacks.instanciacao-estatica
    // @pinker-nav:domain callbacks
    // @pinker-nav:layer parser
    // @pinker-nav:summary Materializa as especializações de callback estático solicitadas: localiza a função concreta (entre itens e funções pendentes via `function_decl_by_name`), exige que todo parâmetro-função receba um callback, valida posição e compatibilidade de assinatura de cada vínculo (erros `Parse` locais), gera o nome monomórfico (`__fnparam_*`), remove os parâmetros-função da assinatura, reescreve as chamadas no corpo e deduplica pelas instâncias já emitidas. Produz `FunctionDecl` concretos; não faz checagem semântica.
    fn function_decl_by_name<'a>(
        name: &str,
        items: &'a [Item],
        pending_functions: &'a [FunctionDecl],
    ) -> Option<&'a FunctionDecl> {
        items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .or_else(|| {
                pending_functions
                    .iter()
                    .find(|function| function.name == name)
            })
    }

    pub(super) fn instantiate_function_param_functions(
        &self,
        items: &[Item],
    ) -> Result<Vec<FunctionDecl>, PinkerError> {
        let mut out = Vec::new();
        let mut emitted = HashSet::new();
        for instantiation in &self.function_param_instantiations {
            let Some(template) = self.function_param_templates.get(&instantiation.name) else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "função '{}' não aceita callback estático nesta fase",
                        instantiation.name
                    ),
                    span: instantiation.span,
                });
            };
            let mono_name = Self::function_param_specialization_name(
                &instantiation.name,
                &instantiation.bindings,
            );
            if !emitted.insert(mono_name.clone()) {
                continue;
            }
            let binding_indices = instantiation
                .bindings
                .iter()
                .map(|binding| binding.index)
                .collect::<HashSet<_>>();
            for (index, param) in template.params.iter().enumerate() {
                if matches!(param.ty, Type::Function { .. }) && !binding_indices.contains(&index) {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "parâmetro função '{}' de '{}' exige callback estático na chamada",
                            param.name, template.name
                        ),
                        span: param.span,
                    });
                }
            }
            let mut replacements = HashMap::new();
            for binding in &instantiation.bindings {
                let Some(param) = template.params.get(binding.index) else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "callback estático em posição inválida na chamada '{}'",
                            instantiation.name
                        ),
                        span: binding.span,
                    });
                };
                let Type::Function { .. } = &param.ty else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "argumento {} da chamada '{}' não é parâmetro função",
                            binding.index + 1,
                            instantiation.name
                        ),
                        span: binding.span,
                    });
                };
                let Some(function) = Self::function_decl_by_name(
                    &binding.function_name,
                    items,
                    &self.pending_functions,
                ) else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "callback '{}' não encontrado para especialização de '{}'",
                            binding.function_name, instantiation.name
                        ),
                        span: binding.span,
                    });
                };
                let actual_ty = Self::function_type_for_decl(function, binding.span)?;
                if param.ty != actual_ty {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "callback '{}' é incompatível com parâmetro '{}'",
                            binding.function_name, param.name
                        ),
                        span: binding.span,
                    });
                }
                replacements.insert(param.name.clone(), binding.function_name.clone());
            }
            out.push(FunctionDecl {
                name: mono_name,
                impl_facts: None,
                trait_default_body: template.trait_default_body.clone(),
                type_params: Vec::new(),
                params: template
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !binding_indices.contains(index))
                    .map(|(_, param)| param.clone())
                    .collect(),
                ret_type: template.ret_type.clone(),
                body: Self::substitute_function_param_block(&template.body, &replacements),
                span: template.span,
            });
        }
        Ok(out)
    }
    // @pinker-nav:end parser.callbacks.instanciacao-estatica

    // @pinker-nav:start parser.genericos.funcoes-instanciacao
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Materializa as funções genéricas solicitadas durante o parsing: localiza o template (erro `Parse` se ausente), confere a aridade dos argumentos de tipo, gera o nome monomórfico e deduplica por ele, monta a tabela de substituições e substitui tipos de parâmetros, tipo de retorno e corpo, produzindo `FunctionDecl` concretos sem parâmetros de tipo. Não valida semanticamente nem anexa ao `Program`.
    pub(super) fn instantiate_generic_functions(&self) -> Result<Vec<FunctionDecl>, PinkerError> {
        let mut out = Vec::new();
        let mut emitted = HashSet::new();
        for instantiation in &self.generic_instantiations {
            let Some(template) = self.generic_templates.get(&instantiation.name) else {
                return Err(PinkerError::Parse {
                    msg: format!("função genérica '{}' não declarada", instantiation.name),
                    span: instantiation.span,
                });
            };
            if template.type_params.len() != instantiation.type_args.len() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "função genérica '{}' exige {} argumento(s) de tipo, recebido {}",
                        instantiation.name,
                        template.type_params.len(),
                        instantiation.type_args.len()
                    ),
                    span: instantiation.span,
                });
            }
            let mono_name =
                self.generic_function_name(&instantiation.name, &instantiation.type_args);
            if !emitted.insert(mono_name.clone()) {
                continue;
            }
            let substitutions = template
                .type_params
                .iter()
                .cloned()
                .zip(instantiation.type_args.iter().cloned())
                .collect::<HashMap<_, _>>();
            out.push(FunctionDecl {
                name: mono_name,
                impl_facts: None,
                trait_default_body: template.trait_default_body.clone(),
                type_params: Vec::new(),
                params: template
                    .params
                    .iter()
                    .map(|param| Param {
                        name: param.name.clone(),
                        ty: Self::substitute_type(&param.ty, &substitutions),
                        span: param.span,
                    })
                    .collect(),
                ret_type: template
                    .ret_type
                    .as_ref()
                    .map(|ty| Self::substitute_type(ty, &substitutions)),
                body: Self::substitute_block(&template.body, &substitutions),
                span: template.span,
            });
        }
        Ok(out)
    }
    // @pinker-nav:end parser.genericos.funcoes-instanciacao

    // @pinker-nav:start parser.genericos.leques-instanciacao
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Percorre as solicitações de leque genérico registradas, localiza cada template (erro `Parse` se ausente), gera o nome monomórfico e deduplica por ele, e delega a `instantiate_generic_enum_decl` a construção da declaração especializada, produzindo os `EnumDecl` concretos que a passagem de entrada anexa ao `Program`. Não valida semanticamente.
    pub(super) fn instantiate_generic_enums(&self) -> Result<Vec<EnumDecl>, PinkerError> {
        let mut out = Vec::new();
        let mut emitted = HashSet::new();
        for instantiation in &self.enum_generic_instantiations {
            let Some(template) = self.enum_generic_templates.get(&instantiation.name) else {
                return Err(PinkerError::Parse {
                    msg: format!("leque genérico '{}' não declarado", instantiation.name),
                    span: instantiation.span,
                });
            };
            let mono_name = self.generic_enum_name(&instantiation.name, &instantiation.type_args);
            if !emitted.insert(mono_name) {
                continue;
            }
            out.push(self.instantiate_generic_enum_decl(
                template,
                &instantiation.type_args,
                instantiation.span,
            )?);
        }
        Ok(out)
    }
    // @pinker-nav:end parser.genericos.leques-instanciacao
}
