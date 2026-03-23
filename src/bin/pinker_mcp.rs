#![allow(dead_code)]
/// pinker_mcp — MCP mínimo para a Pinker v0
///
/// Expõe a pipeline atual da Pinker via Model Context Protocol (JSON-RPC 2.0, stdio).
/// Ferramentas: pinker_checar, pinker_ast, pinker_ir, pinker_rodar, pinker_tokens.
///
/// Limitação intencional: código inline apenas (sem imports entre módulos).
/// Transporte: linha única por mensagem (newline-delimited JSON via stdio).
use pinker_v0::abstract_machine;
use pinker_v0::abstract_machine_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter;
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::lexer::Lexer;
use pinker_v0::parser::Parser;
use pinker_v0::printer;
use pinker_v0::semantic;
use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// JSON mínimo — valor, parser, builder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum JsonVal {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<JsonVal>),
    Obj(Vec<(String, JsonVal)>),
}

#[allow(dead_code)]
impl JsonVal {
    fn as_str(&self) -> Option<&str> {
        if let JsonVal::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn as_obj(&self) -> Option<&[(String, JsonVal)]> {
        if let JsonVal::Obj(fields) = self {
            Some(fields)
        } else {
            None
        }
    }
    fn as_arr(&self) -> Option<&[JsonVal]> {
        if let JsonVal::Arr(items) = self {
            Some(items)
        } else {
            None
        }
    }
    fn get(&self, key: &str) -> Option<&JsonVal> {
        self.as_obj()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    fn as_i64(&self) -> Option<i64> {
        if let JsonVal::Num(n) = self {
            Some(*n as i64)
        } else {
            None
        }
    }
}

struct JsonParser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.src.len()
            && matches!(self.src[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let b = self.src.get(self.pos).copied();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn parse(&mut self) -> Result<JsonVal, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => self.parse_string().map(JsonVal::Str),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') => {
                self.pos += 4;
                Ok(JsonVal::Bool(true))
            }
            Some(b'f') => {
                self.pos += 5;
                Ok(JsonVal::Bool(false))
            }
            Some(b'n') => {
                self.pos += 4;
                Ok(JsonVal::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            other => Err(format!("JSON inesperado: {:?}", other)),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.next(); // consume '"'
        let mut s = String::new();
        loop {
            match self.next() {
                Some(b'"') => return Ok(s),
                Some(b'\\') => {
                    match self.next() {
                        Some(b'"') => s.push('"'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'/') => s.push('/'),
                        Some(b'n') => s.push('\n'),
                        Some(b'r') => s.push('\r'),
                        Some(b't') => s.push('\t'),
                        Some(b'b') => s.push('\x08'),
                        Some(b'f') => s.push('\x0C'),
                        Some(b'u') => {
                            // minimal: read 4 hex digits
                            let mut hex = String::new();
                            for _ in 0..4 {
                                if let Some(c) = self.next() {
                                    hex.push(c as char);
                                }
                            }
                            let codepoint = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
                            s.push(char::from_u32(codepoint).unwrap_or('\u{FFFD}'));
                        }
                        _ => s.push('?'),
                    }
                }
                Some(b) => s.push(b as char),
                None => return Err("string não terminada".into()),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonVal, String> {
        self.next(); // consume '{'
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.next();
            return Ok(JsonVal::Obj(fields));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.next() != Some(b':') {
                return Err("esperado ':'".into());
            }
            let val = self.parse()?;
            fields.push((key, val));
            self.skip_ws();
            match self.next() {
                Some(b'}') => return Ok(JsonVal::Obj(fields)),
                Some(b',') => continue,
                other => return Err(format!("esperado ',' ou '}}', got {:?}", other)),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonVal, String> {
        self.next(); // consume '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.next();
            return Ok(JsonVal::Arr(items));
        }
        loop {
            items.push(self.parse()?);
            self.skip_ws();
            match self.next() {
                Some(b']') => return Ok(JsonVal::Arr(items)),
                Some(b',') => continue,
                other => return Err(format!("esperado ',' ou ']', got {:?}", other)),
            }
        }
    }

    fn parse_number(&mut self) -> Result<JsonVal, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.src[start..self.pos]).unwrap_or("0");
        s.parse::<f64>()
            .map(JsonVal::Num)
            .map_err(|e| e.to_string())
    }
}

fn json_parse(s: &str) -> Result<JsonVal, String> {
    let mut p = JsonParser::new(s);
    p.parse()
}

// ---------------------------------------------------------------------------
// JSON builder mínimo
// ---------------------------------------------------------------------------

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_obj(fields: &[(&str, String)]) -> String {
    let parts: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}:{}", json_str(k), v))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn json_arr(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

// ---------------------------------------------------------------------------
// Pipeline helpers — sem imports (inline apenas)
// ---------------------------------------------------------------------------

fn pipeline_check(code: &str) -> Result<(), String> {
    let mut lex = Lexer::new(code);
    let tokens = lex.tokenize().map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let prog = parser.parse().map_err(|e| e.to_string())?;
    semantic::check_program(&prog).map_err(|e| e.to_string())?;
    Ok(())
}

fn pipeline_tokens(code: &str) -> Result<String, String> {
    let mut lex = Lexer::new(code);
    let tokens = lex.tokenize().map_err(|e| e.to_string())?;
    let arr: Vec<String> = tokens
        .iter()
        .filter(|t| t.kind != pinker_v0::token::TokenKind::Eof)
        .map(|t| {
            json_obj(&[
                ("kind", json_str(t.kind.name())),
                ("lexeme", json_str(&t.lexeme)),
                ("span", json_str(&t.span.to_string())),
            ])
        })
        .collect();
    Ok(json_arr(&arr))
}

fn pipeline_ast(code: &str) -> Result<String, String> {
    let mut lex = Lexer::new(code);
    let tokens = lex.tokenize().map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let prog = parser.parse().map_err(|e| e.to_string())?;
    Ok(json_str(&printer::render_program_json(&prog)))
}

fn pipeline_ir(code: &str, modo: &str) -> Result<String, String> {
    let mut lex = Lexer::new(code);
    let tokens = lex.tokenize().map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let prog = parser.parse().map_err(|e| e.to_string())?;
    semantic::check_program(&prog).map_err(|e| e.to_string())?;

    let ir_prog = ir::lower_program(&prog).map_err(|e| e.to_string())?;
    ir_validate::validate_program(&ir_prog).map_err(|e| e.to_string())?;

    if modo == "ir" {
        return Ok(json_str(&ir::render_program(&ir_prog)));
    }

    let cfg = cfg_ir::lower_program(&ir_prog).map_err(|e| e.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|e| e.to_string())?;

    if modo == "cfg" {
        return Ok(json_str(&cfg_ir::render_program(&cfg)));
    }

    let sel = instr_select::lower_program(&cfg).map_err(|e| e.to_string())?;
    instr_select_validate::validate_program(&sel).map_err(|e| e.to_string())?;

    if modo == "selected" {
        return Ok(json_str(&instr_select::render_program(&sel)));
    }

    let machine = abstract_machine::lower_program(&sel).map_err(|e| e.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|e| e.to_string())?;
    Ok(json_str(&abstract_machine::render_program(&machine)))
}

fn pipeline_rodar(code: &str, args: &[String]) -> Result<(String, i32), String> {
    let mut lex = Lexer::new(code);
    let tokens = lex.tokenize().map_err(|e| e.to_string())?;
    let mut parser = Parser::new(tokens);
    let prog = parser.parse().map_err(|e| e.to_string())?;
    semantic::check_program(&prog).map_err(|e| e.to_string())?;

    let ir_prog = ir::lower_program(&prog).map_err(|e| e.to_string())?;
    ir_validate::validate_program(&ir_prog).map_err(|e| e.to_string())?;
    let cfg = cfg_ir::lower_program(&ir_prog).map_err(|e| e.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|e| e.to_string())?;
    let sel = instr_select::lower_program(&cfg).map_err(|e| e.to_string())?;
    instr_select_validate::validate_program(&sel).map_err(|e| e.to_string())?;
    let machine = abstract_machine::lower_program(&sel).map_err(|e| e.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|e| e.to_string())?;

    let outcome = interpreter::run_program_with_args(&machine, args).map_err(|e| e.to_string())?;
    let saida = match outcome.return_value {
        Some(interpreter::RuntimeValue::Int(v)) => v.to_string(),
        Some(interpreter::RuntimeValue::IntSigned(v)) => v.to_string(),
        Some(interpreter::RuntimeValue::Ptr(v)) => v.to_string(),
        Some(interpreter::RuntimeValue::Str(v)) => v,
        Some(interpreter::RuntimeValue::Bool(v)) => v.to_string(),
        None => String::new(),
    };
    let status = outcome.exit_status.unwrap_or(0);
    Ok((saida, status))
}

// ---------------------------------------------------------------------------
// MCP protocol
// ---------------------------------------------------------------------------

fn mcp_id_str(msg: &JsonVal) -> String {
    match msg.get("id") {
        Some(JsonVal::Num(n)) => (*n as i64).to_string(),
        Some(JsonVal::Str(s)) => json_str(s),
        Some(JsonVal::Null) | None => "null".to_string(),
        _ => "null".to_string(),
    }
}

fn ok_response(id: &str, result: String) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{}}}",
        id, result
    )
}

fn err_response(id: &str, code: i32, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{},\"message\":{}}}}}",
        id,
        code,
        json_str(message)
    )
}

fn tool_result_ok(text: &str) -> String {
    let item = json_obj(&[("type", json_str("text")), ("text", json_str(text))]);
    json_obj(&[("content", json_arr(&[item]))])
}

fn tool_result_err(text: &str) -> String {
    let item = json_obj(&[("type", json_str("text")), ("text", json_str(text))]);
    json_obj(&[
        ("content", json_arr(&[item])),
        ("isError", "true".to_string()),
    ])
}

fn handle_initialize(id: &str) -> String {
    let result = json_obj(&[
        ("protocolVersion", json_str("2024-11-05")),
        ("capabilities", json_obj(&[("tools", json_obj(&[]))])),
        (
            "serverInfo",
            json_obj(&[
                ("name", json_str("pinker-mcp")),
                ("version", json_str("0.1.0")),
            ]),
        ),
    ]);
    ok_response(id, result)
}

fn handle_tools_list(id: &str) -> String {
    let tools = vec![
        json_obj(&[
            ("name", json_str("pinker_checar")),
            ("description", json_str("Verifica léxico, parser e semântica de código Pinker. Retorna ok ou mensagem de erro.")),
            ("inputSchema", json_obj(&[
                ("type", json_str("object")),
                ("properties", json_obj(&[
                    ("codigo", json_obj(&[
                        ("type", json_str("string")),
                        ("description", json_str("Código-fonte Pinker inline.")),
                    ])),
                ])),
                ("required", json_arr(&[json_str("codigo")])),
            ])),
        ]),
        json_obj(&[
            ("name", json_str("pinker_tokens")),
            ("description", json_str("Retorna a lista de tokens do código Pinker como array JSON.")),
            ("inputSchema", json_obj(&[
                ("type", json_str("object")),
                ("properties", json_obj(&[
                    ("codigo", json_obj(&[
                        ("type", json_str("string")),
                        ("description", json_str("Código-fonte Pinker inline.")),
                    ])),
                ])),
                ("required", json_arr(&[json_str("codigo")])),
            ])),
        ]),
        json_obj(&[
            ("name", json_str("pinker_ast")),
            ("description", json_str("Retorna a AST do código Pinker em formato JSON.")),
            ("inputSchema", json_obj(&[
                ("type", json_str("object")),
                ("properties", json_obj(&[
                    ("codigo", json_obj(&[
                        ("type", json_str("string")),
                        ("description", json_str("Código-fonte Pinker inline.")),
                    ])),
                ])),
                ("required", json_arr(&[json_str("codigo")])),
            ])),
        ]),
        json_obj(&[
            ("name", json_str("pinker_ir")),
            ("description", json_str("Retorna representação intermediária do código Pinker. modo: 'ir' (padrão), 'cfg', 'selected', 'machine'.")),
            ("inputSchema", json_obj(&[
                ("type", json_str("object")),
                ("properties", json_obj(&[
                    ("codigo", json_obj(&[
                        ("type", json_str("string")),
                        ("description", json_str("Código-fonte Pinker inline.")),
                    ])),
                    ("modo", json_obj(&[
                        ("type", json_str("string")),
                        ("description", json_str("Nível de IR: 'ir', 'cfg', 'selected', 'machine'. Padrão: 'ir'.")),
                    ])),
                ])),
                ("required", json_arr(&[json_str("codigo")])),
            ])),
        ]),
        json_obj(&[
            ("name", json_str("pinker_rodar")),
            ("description", json_str("Executa código Pinker via interpretador. Retorna saída, status e resultado.")),
            ("inputSchema", json_obj(&[
                ("type", json_str("object")),
                ("properties", json_obj(&[
                    ("codigo", json_obj(&[
                        ("type", json_str("string")),
                        ("description", json_str("Código-fonte Pinker inline.")),
                    ])),
                    ("args", json_obj(&[
                        ("type", json_str("array")),
                        ("items", json_obj(&[("type", json_str("string"))])),
                        ("description", json_str("Argumentos posicionais para argumento(i) em --run.")),
                    ])),
                ])),
                ("required", json_arr(&[json_str("codigo")])),
            ])),
        ]),
    ];
    let result = json_obj(&[("tools", json_arr(&tools))]);
    ok_response(id, result)
}

fn handle_tools_call(id: &str, params: &JsonVal) -> String {
    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return err_response(id, -32602, "parâmetro 'name' ausente"),
    };
    let args = params.get("arguments");

    match name.as_str() {
        "pinker_checar" => {
            let codigo = match args.and_then(|a| a.get("codigo")).and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return err_response(id, -32602, "argumento 'codigo' ausente"),
            };
            let result = match pipeline_check(&codigo) {
                Ok(()) => tool_result_ok("ok"),
                Err(e) => tool_result_err(&e),
            };
            ok_response(id, result)
        }
        "pinker_tokens" => {
            let codigo = match args.and_then(|a| a.get("codigo")).and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return err_response(id, -32602, "argumento 'codigo' ausente"),
            };
            let result = match pipeline_tokens(&codigo) {
                Ok(tokens_json) => {
                    let text = format!("tokens: {}", tokens_json);
                    tool_result_ok(&text)
                }
                Err(e) => tool_result_err(&e),
            };
            ok_response(id, result)
        }
        "pinker_ast" => {
            let codigo = match args.and_then(|a| a.get("codigo")).and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return err_response(id, -32602, "argumento 'codigo' ausente"),
            };
            let result = match pipeline_ast(&codigo) {
                Ok(ast_json) => tool_result_ok(&ast_json),
                Err(e) => tool_result_err(&e),
            };
            ok_response(id, result)
        }
        "pinker_ir" => {
            let codigo = match args.and_then(|a| a.get("codigo")).and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return err_response(id, -32602, "argumento 'codigo' ausente"),
            };
            let modo = args
                .and_then(|a| a.get("modo"))
                .and_then(|v| v.as_str())
                .unwrap_or("ir")
                .to_string();
            let result = match pipeline_ir(&codigo, &modo) {
                Ok(ir_text) => tool_result_ok(&ir_text),
                Err(e) => tool_result_err(&e),
            };
            ok_response(id, result)
        }
        "pinker_rodar" => {
            let codigo = match args.and_then(|a| a.get("codigo")).and_then(|v| v.as_str()) {
                Some(c) => c.to_string(),
                None => return err_response(id, -32602, "argumento 'codigo' ausente"),
            };
            let run_args: Vec<String> = args
                .and_then(|a| a.get("args"))
                .and_then(|v| v.as_arr())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let result = match pipeline_rodar(&codigo, &run_args) {
                Ok((saida, status)) => {
                    let text = if saida.is_empty() {
                        format!("status: {}", status)
                    } else {
                        format!("saida: {}\nstatus: {}", saida, status)
                    };
                    tool_result_ok(&text)
                }
                Err(e) => tool_result_err(&e),
            };
            ok_response(id, result)
        }
        other => err_response(id, -32601, &format!("ferramenta desconhecida: {}", other)),
    }
}

fn handle_message(line: &str) -> Option<String> {
    let msg = match json_parse(line) {
        Ok(v) => v,
        Err(e) => {
            return Some(format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{{\"code\":-32700,\"message\":{}}}}}",
                json_str(&format!("parse error: {}", e))
            ));
        }
    };

    let id = mcp_id_str(&msg);
    let method = match msg.get("method").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        None => return None, // notification ou resposta — ignorar
    };

    match method.as_str() {
        "initialize" => Some(handle_initialize(&id)),
        "initialized" => None, // notificação, sem resposta
        "tools/list" => Some(handle_tools_list(&id)),
        "tools/call" => {
            let params = msg.get("params").unwrap_or(&JsonVal::Null);
            Some(handle_tools_call(&id, params))
        }
        _ => Some(err_response(
            &id,
            -32601,
            &format!("method not found: {}", method),
        )),
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_message(trimmed) {
            let _ = writeln!(out, "{}", response);
            let _ = out.flush();
        }
    }
}
