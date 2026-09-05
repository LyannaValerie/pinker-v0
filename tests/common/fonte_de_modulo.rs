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
