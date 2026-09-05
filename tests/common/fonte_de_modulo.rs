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
