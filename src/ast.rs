use crate::token::{Span, TokenKind};

#[derive(Debug, Clone)]
pub struct Program {
    pub package: Option<PackageDecl>,
    pub items: Vec<Item>,
}

impl Program {
    pub fn to_json_pretty(&self) -> String {
        let mut out = String::new();
        let mut writer = JsonWriter::new(&mut out);
        self.write_json(&mut writer);
        out
    }

    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "Program");
        writer.field_span("span", self.span());
        match &self.package {
            Some(package) => writer.field_value("package", |writer| package.write_json(writer)),
            None => writer.field_null("package"),
        }
        writer.field_array("items", &self.items, |writer, item| item.write_json(writer));
        writer.end_object();
    }

    pub fn span(&self) -> Span {
        if let Some(package) = &self.package {
            if let Some(last) = self.items.last() {
                return package.span.merge(last.span());
            }
            return package.span;
        }

        self.items
            .first()
            .map(Item::span)
            .zip(self.items.last().map(Item::span))
            .map(|(start, end)| start.merge(end))
            .unwrap_or_else(|| Span::single(crate::token::Position::new(1, 1)))
    }
}

#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub name: String,
    pub span: Span,
}

impl PackageDecl {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "PackageDecl");
        writer.field_str("name", &self.name);
        writer.field_span("span", self.span);
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Function(FunctionDecl),
    Const(ConstDecl),
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Function(function) => function.span,
            Item::Const(constant) => constant.span,
        }
    }

    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        match self {
            Item::Function(function) => function.write_json(writer),
            Item::Const(constant) => constant.write_json(writer),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_type: Option<Type>,
    pub body: Block,
    pub span: Span,
}

impl FunctionDecl {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "FunctionDecl");
        writer.field_str("name", &self.name);
        writer.field_span("span", self.span);
        writer.field_array("params", &self.params, |writer, param| {
            param.write_json(writer)
        });
        match &self.ret_type {
            Some(ret_type) => writer.field_value("ret_type", |writer| ret_type.write_json(writer)),
            None => writer.field_null("ret_type"),
        }
        writer.field_value("body", |writer| self.body.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

impl Param {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "Param");
        writer.field_str("name", &self.name);
        writer.field_span("span", self.span);
        writer.field_value("ty", |writer| self.ty.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Type,
    pub init: Expr,
    pub span: Span,
}

impl ConstDecl {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "ConstDecl");
        writer.field_str("name", &self.name);
        writer.field_span("span", self.span);
        writer.field_value("ty", |writer| self.ty.write_json(writer));
        writer.field_value("init", |writer| self.init.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Bombom(Span),
    Logica(Span),
    Nulo(Span),
}

impl Type {
    pub fn span(&self) -> Span {
        match self {
            Type::Bombom(span) | Type::Logica(span) | Type::Nulo(span) => *span,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Type::Bombom(_) => "bombom",
            Type::Logica(_) => "logica",
            Type::Nulo(_) => "nulo",
        }
    }

    pub fn with_span(&self, span: Span) -> Self {
        match self {
            Type::Bombom(_) => Type::Bombom(span),
            Type::Logica(_) => Type::Logica(span),
            Type::Nulo(_) => Type::Nulo(span),
        }
    }

    pub fn is_nulo(&self) -> bool {
        matches!(self, Type::Nulo(_))
    }

    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "Type");
        writer.field_str("name", self.name());
        writer.field_span("span", self.span());
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

impl Block {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "Block");
        writer.field_span("span", self.span);
        writer.field_array("stmts", &self.stmts, |writer, stmt| stmt.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
}

impl AssignStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "AssignStmt");
        writer.field_str("name", &self.name);
        writer.field_span("span", self.span);
        writer.field_value("expr", |writer| self.expr.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    Return(ReturnStmt),
    Assign(AssignStmt),
    If(IfStmt),
    While(WhileStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
    Expr(Expr),
}

impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Let(stmt) => stmt.span,
            Stmt::Return(stmt) => stmt.span,
            Stmt::Assign(stmt) => stmt.span,
            Stmt::If(stmt) => stmt.span,
            Stmt::While(stmt) => stmt.span,
            Stmt::Break(stmt) => stmt.span,
            Stmt::Continue(stmt) => stmt.span,
            Stmt::Expr(expr) => expr.span,
        }
    }

    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        match self {
            Stmt::Let(stmt) => stmt.write_json(writer),
            Stmt::Return(stmt) => stmt.write_json(writer),
            Stmt::Assign(stmt) => stmt.write_json(writer),
            Stmt::If(stmt) => stmt.write_json(writer),
            Stmt::While(stmt) => stmt.write_json(writer),
            Stmt::Break(stmt) => stmt.write_json(writer),
            Stmt::Continue(stmt) => stmt.write_json(writer),
            Stmt::Expr(expr) => {
                writer.begin_object();
                writer.field_str("node", "ExprStmt");
                writer.field_span("span", expr.span);
                writer.field_value("expr", |writer| expr.write_json(writer));
                writer.end_object();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LetStmt {
    pub name: String,
    pub is_mut: bool,
    pub ty: Option<Type>,
    pub init: Expr,
    pub span: Span,
}

impl LetStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "LetStmt");
        writer.field_str("name", &self.name);
        writer.field_bool("is_mut", self.is_mut);
        writer.field_span("span", self.span);
        match &self.ty {
            Some(ty) => writer.field_value("ty", |writer| ty.write_json(writer)),
            None => writer.field_null("ty"),
        }
        writer.field_value("init", |writer| self.init.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub expr: Option<Expr>,
    pub span: Span,
}

impl ReturnStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "ReturnStmt");
        writer.field_span("span", self.span);
        match &self.expr {
            Some(expr) => writer.field_value("expr", |writer| expr.write_json(writer)),
            None => writer.field_null("expr"),
        }
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_branch: Block,
    pub else_branch: Option<ElseBlock>,
    pub span: Span,
}

impl IfStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "IfStmt");
        writer.field_span("span", self.span);
        writer.field_value("condition", |writer| self.condition.write_json(writer));
        writer.field_value("then_branch", |writer| self.then_branch.write_json(writer));
        match &self.else_branch {
            Some(else_branch) => {
                writer.field_value("else_branch", |writer| else_branch.write_json(writer))
            }
            None => writer.field_null("else_branch"),
        }
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Block,
    pub span: Span,
}

impl WhileStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "WhileStmt");
        writer.field_span("span", self.span);
        writer.field_value("condition", |writer| self.condition.write_json(writer));
        writer.field_value("body", |writer| self.body.write_json(writer));
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct BreakStmt {
    pub span: Span,
}

impl BreakStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "BreakStmt");
        writer.field_span("span", self.span);
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub struct ContinueStmt {
    pub span: Span,
}

impl ContinueStmt {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        writer.begin_object();
        writer.field_str("node", "ContinueStmt");
        writer.field_span("span", self.span);
        writer.end_object();
    }
}

#[derive(Debug, Clone)]
pub enum ElseBlock {
    Block(Block),
    If(Box<IfStmt>),
}

impl ElseBlock {
    pub fn span(&self) -> Span {
        match self {
            ElseBlock::Block(block) => block.span,
            ElseBlock::If(if_stmt) => if_stmt.span,
        }
    }

    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        match self {
            ElseBlock::Block(block) => {
                writer.begin_object();
                writer.field_str("node", "ElseBlock");
                writer.field_str("kind", "Block");
                writer.field_span("span", block.span);
                writer.field_value("block", |writer| block.write_json(writer));
                writer.end_object();
            }
            ElseBlock::If(if_stmt) => {
                writer.begin_object();
                writer.field_str("node", "ElseBlock");
                writer.field_str("kind", "If");
                writer.field_span("span", if_stmt.span);
                writer.field_value("if_stmt", |writer| if_stmt.write_json(writer));
                writer.end_object();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    fn write_json(&self, writer: &mut JsonWriter<'_>) {
        match &self.kind {
            ExprKind::Binary(lhs, op, rhs) => {
                writer.begin_object();
                writer.field_str("node", "BinaryExpr");
                writer.field_span("span", self.span);
                writer.field_str("op", op.name());
                writer.field_value("lhs", |writer| lhs.write_json(writer));
                writer.field_value("rhs", |writer| rhs.write_json(writer));
                writer.end_object();
            }
            ExprKind::Unary(op, operand) => {
                writer.begin_object();
                writer.field_str("node", "UnaryExpr");
                writer.field_span("span", self.span);
                writer.field_str("op", op.name());
                writer.field_value("operand", |writer| operand.write_json(writer));
                writer.end_object();
            }
            ExprKind::Call(callee, args) => {
                writer.begin_object();
                writer.field_str("node", "CallExpr");
                writer.field_span("span", self.span);
                writer.field_value("callee", |writer| callee.write_json(writer));
                writer.field_array("args", args, |writer, arg| arg.write_json(writer));
                writer.end_object();
            }
            ExprKind::Ident(name) => {
                writer.begin_object();
                writer.field_str("node", "IdentExpr");
                writer.field_span("span", self.span);
                writer.field_str("name", name);
                writer.end_object();
            }
            ExprKind::IntLit(value) => {
                writer.begin_object();
                writer.field_str("node", "IntLit");
                writer.field_span("span", self.span);
                writer.field_u64("value", *value);
                writer.end_object();
            }
            ExprKind::BoolLit(value) => {
                writer.begin_object();
                writer.field_str("node", "BoolLit");
                writer.field_span("span", self.span);
                writer.field_bool("value", *value);
                writer.end_object();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Ident(String),
    IntLit(u64),
    BoolLit(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
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

impl BinaryOp {
    pub fn from_token(kind: TokenKind) -> Option<Self> {
        match kind {
            TokenKind::AmpAmp => Some(Self::LogicalAnd),
            TokenKind::PipePipe => Some(Self::LogicalOr),
            TokenKind::Amp => Some(Self::BitAnd),
            TokenKind::Pipe => Some(Self::BitOr),
            TokenKind::Caret => Some(Self::BitXor),
            TokenKind::LessLess => Some(Self::Shl),
            TokenKind::GreaterGreater => Some(Self::Shr),
            TokenKind::Plus => Some(Self::Add),
            TokenKind::Minus => Some(Self::Sub),
            TokenKind::Star => Some(Self::Mul),
            TokenKind::Slash => Some(Self::Div),
            TokenKind::EqEq => Some(Self::Eq),
            TokenKind::BangEq => Some(Self::Neq),
            TokenKind::Less => Some(Self::Lt),
            TokenKind::LessEq => Some(Self::Lte),
            TokenKind::Greater => Some(Self::Gt),
            TokenKind::GreaterEq => Some(Self::Gte),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::LogicalAnd => "LogicalAnd",
            Self::LogicalOr => "LogicalOr",
            Self::BitAnd => "BitAnd",
            Self::BitOr => "BitOr",
            Self::BitXor => "BitXor",
            Self::Shl => "Shl",
            Self::Shr => "Shr",
            Self::Add => "Add",
            Self::Sub => "Sub",
            Self::Mul => "Mul",
            Self::Div => "Div",
            Self::Eq => "Eq",
            Self::Neq => "Neq",
            Self::Lt => "Lt",
            Self::Lte => "Lte",
            Self::Gt => "Gt",
            Self::Gte => "Gte",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl UnaryOp {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Neg => "Neg",
            Self::Not => "Not",
        }
    }
}

enum JsonContainer {
    Object { first: bool },
    Array { first: bool },
}

struct JsonWriter<'a> {
    out: &'a mut String,
    indent: usize,
    stack: Vec<JsonContainer>,
    pending_field_value: bool,
}

impl<'a> JsonWriter<'a> {
    fn new(out: &'a mut String) -> Self {
        Self {
            out,
            indent: 0,
            stack: Vec::new(),
            pending_field_value: false,
        }
    }

    fn begin_object(&mut self) {
        self.start_value();
        self.out.push('{');
        self.indent += 1;
        self.stack.push(JsonContainer::Object { first: true });
    }

    fn end_object(&mut self) {
        self.indent -= 1;
        if matches!(
            self.stack.last(),
            Some(JsonContainer::Object { first: false })
        ) {
            self.newline();
        }
        self.stack.pop();
        self.out.push('}');
        self.mark_value_written();
    }

    fn begin_array(&mut self) {
        self.start_value();
        self.out.push('[');
        self.indent += 1;
        self.stack.push(JsonContainer::Array { first: true });
    }

    fn end_array(&mut self) {
        self.indent -= 1;
        if matches!(
            self.stack.last(),
            Some(JsonContainer::Array { first: false })
        ) {
            self.newline();
        }
        self.stack.pop();
        self.out.push(']');
        self.mark_value_written();
    }

    fn field_str(&mut self, name: &str, value: &str) {
        self.field_name(name);
        self.string(value);
        self.mark_field_written();
    }

    fn field_bool(&mut self, name: &str, value: bool) {
        self.field_name(name);
        self.out.push_str(if value { "true" } else { "false" });
        self.mark_field_written();
    }

    fn field_u64(&mut self, name: &str, value: u64) {
        self.field_name(name);
        self.out.push_str(&value.to_string());
        self.mark_field_written();
    }

    fn field_null(&mut self, name: &str) {
        self.field_name(name);
        self.out.push_str("null");
        self.mark_field_written();
    }

    fn field_span(&mut self, name: &str, span: Span) {
        self.field_name(name);
        self.begin_object();
        self.field_value("start", |writer| writer.write_position(span.start));
        self.field_value("end", |writer| writer.write_position(span.end));
        self.end_object();
        self.mark_field_written();
    }

    fn field_array<T>(
        &mut self,
        name: &str,
        values: &[T],
        mut write_item: impl FnMut(&mut Self, &T),
    ) {
        self.field_name(name);
        self.begin_array();
        for value in values {
            write_item(self, value);
        }
        self.end_array();
        self.mark_field_written();
    }

    fn field_value(&mut self, name: &str, write_value: impl FnOnce(&mut Self)) {
        self.field_name(name);
        write_value(self);
        self.mark_field_written();
    }

    fn write_position(&mut self, pos: crate::token::Position) {
        self.begin_object();
        self.field_u64("line", pos.line as u64);
        self.field_u64("col", pos.col as u64);
        self.end_object();
    }

    fn before_value(&mut self) {
        let should_prepare_array_item =
            matches!(self.stack.last(), Some(JsonContainer::Array { .. }));
        if should_prepare_array_item {
            let is_first = match self.stack.last() {
                Some(JsonContainer::Array { first }) => *first,
                _ => true,
            };
            if !is_first {
                self.out.push(',');
            }
            self.newline();
            if let Some(JsonContainer::Array { first }) = self.stack.last_mut() {
                *first = false;
            }
        }
    }

    fn field_name(&mut self, name: &str) {
        if matches!(self.stack.last(), Some(JsonContainer::Object { .. })) {
            let is_first = match self.stack.last() {
                Some(JsonContainer::Object { first }) => *first,
                _ => true,
            };
            if !is_first {
                self.out.push(',');
            }
            self.newline();
            if let Some(JsonContainer::Object { first }) = self.stack.last_mut() {
                *first = false;
            }
        }
        self.string(name);
        self.out.push_str(": ");
        self.pending_field_value = true;
    }

    fn mark_field_written(&mut self) {
        self.pending_field_value = false;
    }

    fn mark_value_written(&mut self) {}

    fn newline(&mut self) {
        self.out.push('\n');
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    fn start_value(&mut self) {
        if self.pending_field_value {
            self.pending_field_value = false;
            return;
        }
        self.before_value();
    }

    fn string(&mut self, value: &str) {
        self.out.push('"');
        for ch in value.chars() {
            match ch {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                c if c.is_control() => self.out.push_str(&format!("\\u{:04x}", c as u32)),
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }
}
