//! Trama Pinker — leitor mínimo de JSON de uma linha (JSONL).
//!
//! Os catálogos (`docs/navigation.jsonl`, `src/navigation.jsonl` e o ledger
//! `.pinker/changes/index.jsonl`) são gerados por esta ferramenta e consumidos
//! por ela. Este módulo faz o caminho de volta: interpreta cada linha JSON num
//! mapa determinístico de valores, para que as consultas leiam o catálogo em
//! vez de revarrer as fontes (especificação, §5).
//!
//! Suporta o subconjunto que a Trama emite: objetos com strings (com escapes
//! `\" \\ \n \r \t \uXXXX`), inteiros não-negativos, booleanos e arrays de
//! strings. Zero dependências externas.

use std::collections::BTreeMap;
use std::fmt;

/// Valor JSON mínimo aceito pelos catálogos da Trama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Array(Vec<JsonValue>),
}

impl JsonValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            JsonValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_str_array(&self) -> Option<Vec<String>> {
        match self {
            JsonValue::Array(items) => items
                .iter()
                .map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => None,
        }
    }
}

/// Objeto JSON interpretado, preservando acesso por chave.
pub type JsonObject = BTreeMap<String, JsonValue>;

/// Erro de parsing de uma linha JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub msg: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, JsonError> {
    Err(JsonError { msg: msg.into() })
}

/// Interpreta uma linha JSON como um objeto.
pub fn parse_object(line: &str) -> Result<JsonObject, JsonError> {
    let mut p = Parser::new(line);
    p.skip_ws();
    let obj = p.parse_object()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return err("conteúdo extra após o objeto JSON");
    }
    Ok(obj)
}

struct Parser<'a> {
    bytes: &'a [u8],
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Parser<'a> {
        Parser {
            bytes: src.as_bytes(),
            src,
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn parse_object(&mut self) -> Result<JsonObject, JsonError> {
        if self.peek() != Some(b'{') {
            return err("esperado '{' no início do objeto");
        }
        self.pos += 1;
        let mut map = JsonObject::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(map);
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return err(format!("esperado ':' após a chave '{}'", key));
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    continue;
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return err("esperado ',' ou '}' no objeto"),
            }
        }
        Ok(map)
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        match self.peek() {
            Some(b'"') => Ok(JsonValue::Str(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b't') | Some(b'f') => self.parse_bool(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_int(),
            _ => err("valor JSON inesperado"),
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.pos += 1; // consome '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    continue;
                }
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => return err("esperado ',' ou ']' no array"),
            }
        }
        Ok(JsonValue::Array(items))
    }

    fn parse_bool(&mut self) -> Result<JsonValue, JsonError> {
        if self.src[self.pos..].starts_with("true") {
            self.pos += 4;
            Ok(JsonValue::Bool(true))
        } else if self.src[self.pos..].starts_with("false") {
            self.pos += 5;
            Ok(JsonValue::Bool(false))
        } else {
            err("booleano malformado")
        }
    }

    fn parse_int(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while self.peek().map(|c| c.is_ascii_digit()) == Some(true) {
            self.pos += 1;
        }
        let text = &self.src[start..self.pos];
        text.parse::<i64>()
            .map(JsonValue::Int)
            .map_err(|_| JsonError {
                msg: format!("inteiro inválido: '{}'", text),
            })
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        if self.peek() != Some(b'"') {
            return err("esperada string entre aspas");
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return err("string sem aspas de fechamento");
            };
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(esc) = self.peek() else {
                        return err("escape truncado na string");
                    };
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'u' => {
                            let hex =
                                self.src
                                    .get(self.pos..self.pos + 4)
                                    .ok_or_else(|| JsonError {
                                        msg: "escape \\u truncado".to_string(),
                                    })?;
                            let code = u32::from_str_radix(hex, 16).map_err(|_| JsonError {
                                msg: format!("escape \\u inválido: '{}'", hex),
                            })?;
                            self.pos += 4;
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        other => {
                            return err(format!("escape desconhecido: '\\{}'", other as char));
                        }
                    }
                }
                _ => {
                    // Reconstrói o caractere UTF-8 a partir do byte inicial.
                    let ch_start = self.pos - 1;
                    let mut ch_end = self.pos;
                    while ch_end < self.bytes.len() && (self.bytes[ch_end] & 0xC0) == 0x80 {
                        ch_end += 1;
                    }
                    out.push_str(&self.src[ch_start..ch_end]);
                    self.pos = ch_end;
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_object() {
        let obj = parse_object(r#"{"schema":2,"id":"rosa.identity","start":44}"#).unwrap();
        assert_eq!(obj["schema"].as_int(), Some(2));
        assert_eq!(obj["id"].as_str(), Some("rosa.identity"));
        assert_eq!(obj["start"].as_int(), Some(44));
    }

    #[test]
    fn parses_arrays_and_bools() {
        let obj = parse_object(r#"{"tags":["a","b"],"flag":true}"#).unwrap();
        assert_eq!(
            obj["tags"].as_str_array(),
            Some(vec!["a".into(), "b".into()])
        );
        assert_eq!(obj["flag"], JsonValue::Bool(true));
    }

    #[test]
    fn handles_escapes_and_unicode() {
        let obj = parse_object(r#"{"t":"linha\nnova \"aspas\" e ç"}"#).unwrap();
        assert_eq!(obj["t"].as_str(), Some("linha\nnova \"aspas\" e ç"));
    }

    #[test]
    fn preserves_utf8() {
        let obj = parse_object(r#"{"t":"próxima ação"}"#).unwrap();
        assert_eq!(obj["t"].as_str(), Some("próxima ação"));
    }

    #[test]
    fn rejects_trailing_garbage() {
        assert!(parse_object(r#"{"a":1} extra"#).is_err());
        assert!(parse_object(r#"{"a":}"#).is_err());
    }
}
