use crate::ast::*;
use crate::token::Span;

pub fn render_program(program: &Program) -> String {
    let mut out = String::new();
    render_program_into(program, 0, &mut out);
    out
}

pub fn render_program_json(program: &Program) -> String {
    program.to_json_pretty()
}

fn render_program_into(program: &Program, indent: usize, out: &mut String) {
    line(
        out,
        indent,
        &format!("Program {}", format_span(program.span())),
    );
    if let Some(package) = &program.package {
        line(
            out,
            indent + 1,
            &format!("Package {} {}", package.name, format_span(package.span)),
        );
    }
    for item in &program.items {
        render_item(item, indent + 1, out);
    }
}

fn render_item(item: &Item, indent: usize, out: &mut String) {
    match item {
        Item::Function(function) => render_function(function, indent, out),
        Item::Const(constant) => {
            line(
                out,
                indent,
                &format!(
                    "Const {}: {} {}",
                    constant.name,
                    format_type(&constant.ty),
                    format_span(constant.span)
                ),
            );
            render_expr(&constant.init, indent + 1, out, "init");
        }
        Item::TypeAlias(alias) => {
            line(
                out,
                indent,
                &format!(
                    "TypeAlias {} = {} {}",
                    alias.name,
                    format_type(&alias.target),
                    format_span(alias.span)
                ),
            );
        }
        Item::Struct(struct_decl) => {
            line(
                out,
                indent,
                &format!(
                    "Struct {} {}",
                    struct_decl.name,
                    format_span(struct_decl.span)
                ),
            );
            for field in &struct_decl.fields {
                line(
                    out,
                    indent + 1,
                    &format!(
                        "{}: {} {}",
                        field.name,
                        format_type(&field.ty),
                        format_span(field.span)
                    ),
                );
            }
        }
    }
}

fn render_function(function: &FunctionDecl, indent: usize, out: &mut String) {
    let ret = function
        .ret_type
        .as_ref()
        .map(format_type)
        .unwrap_or_else(|| "nulo".to_string());
    line(
        out,
        indent,
        &format!(
            "Function {} -> {} {}",
            function.name,
            ret,
            format_span(function.span)
        ),
    );
    if function.params.is_empty() {
        line(out, indent + 1, "Params []");
    } else {
        line(out, indent + 1, "Params");
        for param in &function.params {
            line(
                out,
                indent + 2,
                &format!(
                    "{}: {} {}",
                    param.name,
                    format_type(&param.ty),
                    format_span(param.span)
                ),
            );
        }
    }
    render_block(&function.body, indent + 1, out, "Body");
}

fn render_block(block: &Block, indent: usize, out: &mut String, label: &str) {
    line(
        out,
        indent,
        &format!("{} {}", label, format_span(block.span)),
    );
    for stmt in &block.stmts {
        render_stmt(stmt, indent + 1, out);
    }
}

fn render_stmt(stmt: &Stmt, indent: usize, out: &mut String) {
    match stmt {
        Stmt::Let(let_stmt) => {
            let mutability = if let_stmt.is_mut { "mut " } else { "" };
            let annotation = let_stmt
                .ty
                .as_ref()
                .map(|ty| format!(": {}", format_type(ty)))
                .unwrap_or_default();
            line(
                out,
                indent,
                &format!(
                    "Let {}{}{} {}",
                    mutability,
                    let_stmt.name,
                    annotation,
                    format_span(let_stmt.span)
                ),
            );
            render_expr(&let_stmt.init, indent + 1, out, "init");
        }
        Stmt::Return(return_stmt) => {
            line(
                out,
                indent,
                &format!("Return {}", format_span(return_stmt.span)),
            );
            match &return_stmt.expr {
                Some(expr) => render_expr(expr, indent + 1, out, "value"),
                None => line(out, indent + 1, "value <vazio>"),
            }
        }
        Stmt::Assign(assign_stmt) => {
            line(
                out,
                indent,
                &format!(
                    "Assign {} {}",
                    assign_stmt.name,
                    format_span(assign_stmt.span)
                ),
            );
            render_expr(&assign_stmt.expr, indent + 1, out, "value");
        }
        Stmt::If(if_stmt) => render_if(if_stmt, indent, out, "If"),
        Stmt::While(while_stmt) => {
            line(
                out,
                indent,
                &format!("While {}", format_span(while_stmt.span)),
            );
            render_expr(&while_stmt.condition, indent + 1, out, "condition");
            render_block(&while_stmt.body, indent + 1, out, "body");
        }
        Stmt::Break(break_stmt) => {
            line(
                out,
                indent,
                &format!("Break {}", format_span(break_stmt.span)),
            );
        }
        Stmt::Continue(continue_stmt) => {
            line(
                out,
                indent,
                &format!("Continue {}", format_span(continue_stmt.span)),
            );
        }
        Stmt::Expr(expr) => {
            line(out, indent, &format!("ExprStmt {}", format_span(expr.span)));
            render_expr(expr, indent + 1, out, "expr");
        }
    }
}

fn render_if(if_stmt: &IfStmt, indent: usize, out: &mut String, label: &str) {
    line(
        out,
        indent,
        &format!("{} {}", label, format_span(if_stmt.span)),
    );
    render_expr(&if_stmt.condition, indent + 1, out, "condition");
    render_block(&if_stmt.then_branch, indent + 1, out, "then");
    match &if_stmt.else_branch {
        Some(ElseBlock::Block(block)) => render_block(block, indent + 1, out, "else"),
        Some(ElseBlock::If(nested_if)) => render_if(nested_if, indent + 1, out, "else-if"),
        None => line(out, indent + 1, "else <ausente>"),
    }
}

fn render_expr(expr: &Expr, indent: usize, out: &mut String, label: &str) {
    match &expr.kind {
        ExprKind::Binary(lhs, op, rhs) => {
            line(
                out,
                indent,
                &format!("{} Binary({}) {}", label, op.name(), format_span(expr.span)),
            );
            render_expr(lhs, indent + 1, out, "lhs");
            render_expr(rhs, indent + 1, out, "rhs");
        }
        ExprKind::Unary(op, operand) => {
            line(
                out,
                indent,
                &format!("{} Unary({}) {}", label, op.name(), format_span(expr.span)),
            );
            render_expr(operand, indent + 1, out, "operand");
        }
        ExprKind::Call(callee, args) => {
            line(
                out,
                indent,
                &format!("{} Call {}", label, format_span(expr.span)),
            );
            render_expr(callee, indent + 1, out, "callee");
            for arg in args {
                render_expr(arg, indent + 1, out, "arg");
            }
        }
        ExprKind::FieldAccess { base, field } => {
            line(
                out,
                indent,
                &format!(
                    "{} FieldAccess({}) {}",
                    label,
                    field,
                    format_span(expr.span)
                ),
            );
            render_expr(base, indent + 1, out, "base");
        }
        ExprKind::Index { base, index } => {
            line(
                out,
                indent,
                &format!("{} Index {}", label, format_span(expr.span)),
            );
            render_expr(base, indent + 1, out, "base");
            render_expr(index, indent + 1, out, "index");
        }
        ExprKind::Cast {
            expr: inner,
            target,
        } => {
            line(
                out,
                indent,
                &format!(
                    "{} Cast({}) {}",
                    label,
                    format_type(target),
                    format_span(expr.span)
                ),
            );
            render_expr(inner, indent + 1, out, "expr");
        }
        ExprKind::SizeOfType { target } => {
            line(
                out,
                indent,
                &format!(
                    "{} SizeOfType({}) {}",
                    label,
                    format_type(target),
                    format_span(expr.span)
                ),
            );
        }
        ExprKind::AlignOfType { target } => {
            line(
                out,
                indent,
                &format!(
                    "{} AlignOfType({}) {}",
                    label,
                    format_type(target),
                    format_span(expr.span)
                ),
            );
        }
        ExprKind::Ident(name) => {
            line(
                out,
                indent,
                &format!("{} Ident({}) {}", label, name, format_span(expr.span)),
            );
        }
        ExprKind::IntLit(value) => {
            line(
                out,
                indent,
                &format!("{} IntLit({}) {}", label, value, format_span(expr.span)),
            );
        }
        ExprKind::BoolLit(value) => {
            line(
                out,
                indent,
                &format!("{} BoolLit({}) {}", label, value, format_span(expr.span)),
            );
        }
    }
}

fn format_type(ty: &Type) -> String {
    match ty {
        Type::Alias { name, .. } => name.clone(),
        Type::Struct { name, .. } => name.clone(),
        Type::FixedArray { element, size, .. } => format!("[{}; {}]", format_type(element), size),
        Type::Pointer { base, .. } => format!("seta<{}>", format_type(base)),
        _ => ty.name().to_string(),
    }
}

fn format_span(span: Span) -> String {
    format!("[{}]", span)
}

fn line(out: &mut String, indent: usize, text: &str) {
    for _ in 0..indent {
        out.push_str("  ");
    }
    out.push_str(text);
    out.push('\n');
}
