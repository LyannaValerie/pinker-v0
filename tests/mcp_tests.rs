/// Testes da Rodada Paralela-1: MCP mínimo (pinker_mcp).
///
/// Testa as ferramentas expostas pelo binário `pinker_mcp` via stdin/stdout JSON-RPC.
use std::io::Write;
use std::process::{Command, Stdio};

fn mcp_path() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    // em modo test, os binários ficam em target/debug/deps ou target/debug
    // tentamos target/debug direto
    if p.ends_with("deps") {
        p.pop();
    }
    p.push("pinker_mcp");
    p
}

fn mcp_send(inputs: &[&str]) -> String {
    let bin = mcp_path();
    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("falha ao iniciar pinker_mcp em {:?}: {}", bin, e));

    {
        let stdin = child.stdin.as_mut().unwrap();
        for msg in inputs {
            stdin.write_all(msg.as_bytes()).unwrap();
            stdin.write_all(b"\n").unwrap();
        }
    }

    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn json_contains(output: &str, key_value: &str) -> bool {
    output.contains(key_value)
}

#[test]
fn mcp_initialize_retorna_capacidades() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#,
    ]);
    assert!(json_contains(&resp, "pinker-mcp"), "resp: {}", resp);
    assert!(json_contains(&resp, "protocolVersion"), "resp: {}", resp);
}

#[test]
fn mcp_tools_list_retorna_ferramentas() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    ]);
    assert!(json_contains(&resp, "pinker_checar"), "resp: {}", resp);
    assert!(json_contains(&resp, "pinker_ast"), "resp: {}", resp);
    assert!(json_contains(&resp, "pinker_ir"), "resp: {}", resp);
    assert!(json_contains(&resp, "pinker_rodar"), "resp: {}", resp);
    assert!(json_contains(&resp, "pinker_tokens"), "resp: {}", resp);
}

#[test]
fn mcp_pinker_checar_codigo_valido() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"pinker_checar","arguments":{"codigo":"pacote main; carinho principal() -> bombom { mimo 1; }"}}}"#,
    ]);
    assert!(json_contains(&resp, "\"ok\""), "resp: {}", resp);
}

#[test]
fn mcp_pinker_checar_codigo_invalido() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"pinker_checar","arguments":{"codigo":"pacote main; carinho principal() -> bombom { mimo b; }"}}}"#,
    ]);
    assert!(json_contains(&resp, "isError"), "resp: {}", resp);
}

#[test]
fn mcp_pinker_checar_bitnot_til() {
    // verifica que ~ funciona no MCP
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"pinker_checar","arguments":{"codigo":"pacote main; carinho principal() -> bombom { nova x: bombom = 5; mimo ~x; }"}}}"#,
    ]);
    assert!(json_contains(&resp, "\"ok\""), "resp: {}", resp);
}

#[test]
fn mcp_pinker_checar_bitnot_nope() {
    // verifica que nope funciona no MCP
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"pinker_checar","arguments":{"codigo":"pacote main; carinho principal() -> bombom { nova x: bombom = 5; mimo nope x; }"}}}"#,
    ]);
    assert!(json_contains(&resp, "\"ok\""), "resp: {}", resp);
}

#[test]
fn mcp_pinker_rodar_retorna_resultado() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"pinker_rodar","arguments":{"codigo":"pacote main; carinho principal() -> bombom { mimo 42; }"}}}"#,
    ]);
    assert!(json_contains(&resp, "42"), "resp: {}", resp);
}

#[test]
fn mcp_pinker_tokens_retorna_lista() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"pinker_tokens","arguments":{"codigo":"pacote main;"}}}"#,
    ]);
    assert!(json_contains(&resp, "KwPacote"), "resp: {}", resp);
}

#[test]
fn mcp_metodo_desconhecido_retorna_erro() {
    let resp = mcp_send(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"metodo_inexistente","params":{}}"#,
    ]);
    assert!(json_contains(&resp, "method not found"), "resp: {}", resp);
}
