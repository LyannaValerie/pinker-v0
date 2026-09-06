//! Fonte de um módulo Rust decomposto fisicamente (`mod.rs` mais irmãos).
//!
//! Depois da decomposição física da #602 a implementação do `parser` deixou de
//! morar num arquivo e passou a morar no diretório `src/parser/`. Um oráculo
//! estrutural que continuasse lendo um único arquivo seguiria compilando e
//! passando, e pararia de observar o código que foi para os irmãos — a falha
//! silenciosa que o inventário da #601 registrou como OG-1. Todos os oráculos
//! leem o módulo inteiro por aqui, numa definição só.
//!
//! A lista é explícita porque `include_str!` resolve em tempo de compilação:
//! remover um irmão quebra a compilação, e acrescentar um sem registrar aqui é
//! o que `tests/parser_module_layout_tests.rs` recusa.

// Cada alvo de teste consome só a parte do helper que lhe interessa; o outro
// módulo fica sem uso naquele binário sem que isso seja código morto.
#![allow(dead_code)]

/// Arquivos que compõem o módulo `parser`, na ordem declarada em `mod.rs`.
pub const PARSER_ARQUIVOS: &[(&str, &str)] = &[
    ("mod.rs", include_str!("../../src/parser/mod.rs")),
    ("comandos.rs", include_str!("../../src/parser/comandos.rs")),
    (
        "expressoes.rs",
        include_str!("../../src/parser/expressoes.rs"),
    ),
    (
        "genericos.rs",
        include_str!("../../src/parser/genericos.rs"),
    ),
    ("lacos.rs", include_str!("../../src/parser/lacos.rs")),
    (
        "resultado.rs",
        include_str!("../../src/parser/resultado.rs"),
    ),
];

/// Concatena o módulo `parser` inteiro, `mod.rs` primeiro.
pub fn parser() -> String {
    PARSER_ARQUIVOS
        .iter()
        .map(|(_, fonte)| *fonte)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Arquivos que compõem o binário `pink`, na ordem declarada em `main.rs`.
///
/// A decomposição física da #605 tirou de `src/main.rs` as famílias de parsing
/// da CLI, dos comandos `doc` e da carga de módulos. O entrypoint continua
/// sendo `src/main.rs` — ele é a raiz do crate binário, não virou `mod.rs` —, e
/// os irmãos moram em `src/pink_cli/`, declarados por `#[path]`. Um oráculo que
/// continuasse lendo só `src/main.rs` seguiria verde e pararia de observar o
/// que foi para os irmãos: é a mesma falha silenciosa OG-1 da #601.
pub const PINK_CLI_ARQUIVOS: &[(&str, &str)] = &[
    ("main.rs", include_str!("../../src/main.rs")),
    (
        "cli_parsing.rs",
        include_str!("../../src/pink_cli/cli_parsing.rs"),
    ),
    ("doc_cli.rs", include_str!("../../src/pink_cli/doc_cli.rs")),
    ("modules.rs", include_str!("../../src/pink_cli/modules.rs")),
];

/// Concatena o binário `pink` inteiro, `main.rs` primeiro.
pub fn pink_cli() -> String {
    PINK_CLI_ARQUIVOS
        .iter()
        .map(|(_, fonte)| *fonte)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Arquivos que compõem o módulo `interpreter`, na ordem declarada no pai.
///
/// A decomposição física da #608 (unidade INT-1) tirou de `src/interpreter.rs`
/// a porta `try_call_intrinsic` inteira. O pai continua sendo um arquivo — ele
/// não virou `mod.rs` —, e o irmão mora em `src/interpreter/`, declarado pelo
/// `mod` do próprio pai. Um oráculo que continuasse lendo só
/// `src/interpreter.rs` seguiria verde e pararia de observar o despacho
/// hospedado inteiro: é a mesma falha silenciosa OG-1 da #601.
pub const INTERPRETER_ARQUIVOS: &[(&str, &str)] = &[
    ("interpreter.rs", include_str!("../../src/interpreter.rs")),
    (
        "hosted_intrinsics.rs",
        include_str!("../../src/interpreter/hosted_intrinsics.rs"),
    ),
];

/// Concatena o módulo `interpreter` inteiro, o pai primeiro.
pub fn interpreter() -> String {
    INTERPRETER_ARQUIVOS
        .iter()
        .map(|(_, fonte)| *fonte)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Os mesmos arquivos como caminhos relativos à raiz, para censos que leem do
/// disco em vez de `include_str!`. Fonte única com [`INTERPRETER_ARQUIVOS`]:
/// registrar um irmão novo lá já o coloca sob esses censos.
pub fn interpreter_caminhos() -> Vec<String> {
    INTERPRETER_ARQUIVOS
        .iter()
        .map(|(nome, _)| {
            if *nome == "interpreter.rs" {
                "src/interpreter.rs".to_string()
            } else {
                format!("src/interpreter/{nome}")
            }
        })
        .collect()
}

/// Arquivos que compõem o módulo `backend_s`, na ordem declarada no pai.
///
/// A decomposição física da #610 (unidade BS-3) tirou de `src/backend_s.rs` os
/// dois módulos de teste do caminho montável, e a da #612 (unidade BS-2) tirou
/// a renderização ABI textual. O pai continua sendo um arquivo — ele não virou
/// `mod.rs` —, e os irmãos moram em `src/backend_s/`, declarados pelos `mod` do
/// próprio pai. Os oráculos que censuram o arquivo inteiro leem por aqui; os
/// que querem só a produção leem [`backend_s_producao`], que desce nos irmãos
/// pelo mesmo caminho — ler só o pai deixou de ser ler a produção quando a
/// BS-2 mudou produção de arquivo.
pub const BACKEND_S_ARQUIVOS: &[(&str, &str)] = &[
    ("backend_s.rs", include_str!("../../src/backend_s.rs")),
    (
        "render_abi.rs",
        include_str!("../../src/backend_s/render_abi.rs"),
    ),
    ("tests.rs", include_str!("../../src/backend_s/tests.rs")),
];

/// Concatena o módulo `backend_s` inteiro, o pai primeiro.
pub fn backend_s() -> String {
    BACKEND_S_ARQUIVOS
        .iter()
        .map(|(_, fonte)| *fonte)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Só a produção do módulo `backend_s`: cada arquivo cortado no primeiro
/// `#[cfg(test)]`, como os censos de autoridade sempre cortaram, e depois
/// concatenados.
///
/// Antes da BS-2 cortar o pai era cortar o módulo, porque toda a produção
/// morava nele. Deixou de ser: um censo que continuasse lendo só
/// `src/backend_s.rs` seguiria verde e pararia de observar `render_abi.rs` —
/// a falha silenciosa OG-1 do inventário da #601.
pub fn backend_s_producao() -> String {
    BACKEND_S_ARQUIVOS
        .iter()
        .map(|(_, fonte)| producao(fonte))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Os mesmos arquivos como caminhos relativos à raiz, para censos que leem do
/// disco em vez de `include_str!`. Fonte única com [`BACKEND_S_ARQUIVOS`]:
/// registrar um irmão novo lá já o coloca sob esses censos.
pub fn backend_s_caminhos() -> Vec<String> {
    BACKEND_S_ARQUIVOS
        .iter()
        .map(|(nome, _)| {
            if *nome == "backend_s.rs" {
                "src/backend_s.rs".to_string()
            } else {
                format!("src/backend_s/{nome}")
            }
        })
        .collect()
}

/// A parte produtiva de um arquivo: tudo antes do primeiro `#[cfg(test)]`.
fn producao(fonte: &str) -> &str {
    match fonte.find("\n#[cfg(test)]") {
        Some(corte) => &fonte[..corte],
        None => fonte,
    }
}
