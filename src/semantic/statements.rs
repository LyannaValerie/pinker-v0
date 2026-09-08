//! Verificação de comandos de bloco da checagem semântica, movida de
//! `src/semantic.rs` pela unidade SEM-2 do inventário da #601 (Task #628).
//!
//! Só o arquivo mudou: a região cartografada `semantic.comandos.verificacao`, a
//! função que a compõe e a ordem em que ela decide continuam exatamente como
//! estavam. `super` mudou de significado ao descer um nível, e o `use` abaixo
//! devolve ao irmão o vocabulário do pai — `SemanticChecker`, os tipos da AST e
//! os helpers privados — sem promover nada: um filho enxerga os itens privados
//! do pai por privacidade de módulo, e este `use` é privado.
//!
//! O corte não atravessa autoridade nenhuma. A verificação de comandos não
//! consulta `method_dispatch` (C2), não lê o registry declarativo de
//! intrínsecas (C1), não reconstrói origem de default body por grafia (C5), não
//! reabre a conclusão arquitetural da #600 (C6) e não decide nada da política
//! de alcance ainda aberta da #579. O estado (`SemanticChecker`), a ordem das
//! duas passagens, os escopos, o sistema de tipos e as demais famílias
//! continuam no pai.
//!
//! Nenhum item do corte era `pub` antes do move e nenhum é agora.
//! `check_block` é o único símbolo que o pai chama e, por isso, o único que
//! passou de privado a `pub(super)`.

use super::*;

impl SemanticChecker {
    // @pinker-nav:start semantic.comandos.verificacao
    // @pinker-nav:domain comandos
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de comandos de um bloco: `mimo` (let) com inferência de `lista_criar`/`mapa_criar` pela anotação e checagem de tipo/faixa, retorno, atribuição a variável/deref/campo/índice (mutabilidade e tipos), `talvez`/`senão`, laço `sempre que` (com controle de profundidade), `quebrar`/`continuar`, `falar` (tipos imprimíveis), `sussurro` (asm) e expressão-comando.
    pub(super) fn check_block(
        &mut self,
        block: &Block,
        function_level: bool,
    ) -> Result<(), PinkerError> {
        if !function_level {
            self.push_scope();
        }

        for stmt in &block.stmts {
            match stmt {
                Stmt::Let(let_stmt) => {
                    // `nova l: lista<...> = lista_criar();` — a criação genérica
                    // recebe o tipo da anotação (única forma de inferência desta fase).
                    if let Some(declared_ty) = &let_stmt.ty {
                        if Self::expr_is_generic_list_create(&let_stmt.init) {
                            let resolved_declared_ty = self.resolve_type_or_error(declared_ty)?;
                            if Self::list_element_type(&resolved_declared_ty, let_stmt.span)
                                .is_none()
                            {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "'lista_criar()' exige anotação de tipo de lista em 'nova'; encontrado '{}'",
                                        resolved_declared_ty.name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            self.declare_var(
                                &let_stmt.name,
                                resolved_declared_ty,
                                let_stmt.is_mut,
                                let_stmt.span,
                            )?;
                            continue;
                        }
                        if Self::expr_is_generic_map_create(&let_stmt.init) {
                            let resolved_declared_ty = self.resolve_type_or_error(declared_ty)?;
                            if !Self::is_map_type(&resolved_declared_ty) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "'mapa_criar()' exige anotação de tipo de mapa em 'nova'; encontrado '{}'",
                                        resolved_declared_ty.name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            self.declare_var(
                                &let_stmt.name,
                                resolved_declared_ty,
                                let_stmt.is_mut,
                                let_stmt.span,
                            )?;
                            continue;
                        }
                    }
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
                                        resolved_declared_ty.display_name(),
                                        init_ty.display_name()
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
                                    if matches!(
                                        base.as_ref(),
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
                                    ) =>
                                {
                                    base.as_ref().clone()
                                }
                                Type::Pointer { .. } => {
                                    return Err(PinkerError::Semantic {
                                        msg: "escrita indireta aceita ponteiros para escalares públicos de uma palavra".to_string(),
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
                        AssignTarget::FieldDeref { base, field } => {
                            let base_ty = self.check_value_expr(
                                base,
                                "resultado de função sem retorno não pode ser base de escrita a campo",
                            )?;
                            let field_ty =
                                self.resolve_struct_field_type(&base_ty, field, assign_stmt.span)?;
                            if !Self::check_expected_type_for_expr(
                                &field_ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na escrita de campo '{}': esperado '{}', encontrado '{}'",
                                        field,
                                        field_ty.name(),
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(&field_ty, &assign_stmt.expr)?;
                        }
                        AssignTarget::Index { base, index } => {
                            let base_ty = self.check_value_expr(
                                base,
                                "resultado de função sem retorno não pode ser base de escrita por índice",
                            )?;
                            match &base_ty {
                                Type::FixedArray { element, .. } => {
                                    if !matches!(element.as_ref(), Type::Bombom(_)) {
                                        return Err(PinkerError::Semantic {
                                            msg: "escrita por índice nesta fase aceita apenas '[bombom; N]'".to_string(),
                                            span: assign_stmt.span,
                                        });
                                    }
                                }
                                _ => {
                                    return Err(PinkerError::Semantic {
                                        msg:
                                            "escrita por índice exige base de array fixo nesta fase"
                                                .to_string(),
                                        span: assign_stmt.span,
                                    });
                                }
                            }
                            let index_ty = self.check_value_expr(
                                index,
                                "resultado de função sem retorno não pode ser índice de escrita",
                            )?;
                            if !matches!(index_ty, Type::Bombom(_)) {
                                return Err(PinkerError::Semantic {
                                    msg: "índice de escrita nesta fase deve ser 'bombom'"
                                        .to_string(),
                                    span: index.span,
                                });
                            }
                            let expected_ty = Type::Bombom(assign_stmt.span);
                            if !Self::check_expected_type_for_expr(
                                &expected_ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na escrita por índice: esperado 'bombom', encontrado '{}'",
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(&expected_ty, &assign_stmt.expr)?;
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
                    for arg in &falar_stmt.args {
                        let ty = self.check_value_expr(
                            arg,
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
                }
                Stmt::InlineAsm(inline_asm_stmt) => self.check_inline_asm(inline_asm_stmt)?,
                Stmt::EnumMatch(enum_match) => self.check_enum_match(enum_match)?,
                Stmt::UnionMatch(union_match) => self.check_union_match(union_match)?,
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
    // @pinker-nav:end semantic.comandos.verificacao
}
