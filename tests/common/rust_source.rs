//! Leitura de fonte Rust para oráculos estruturais.
//!
//! Um oráculo que pergunta "esta camada ainda decide X por conta própria?"
//! precisa olhar código executável. Comentário e literal de texto mentem nos
//! dois sentidos: escondem a construção proibida quando interpostos, e acusam
//! quando apenas a mencionam.
//!
//! Limite declarado: identificador produzido por expansão de macro não aparece
//! no texto e nenhum oráculo textual o alcança.

/// Remove comentários, literais de texto e literais de caractere.
///
/// Cada trecho removido vira um espaço, então tokens vizinhos não se colam.
/// Um tempo de vida (`'a`) não fecha aspa e permanece como código.
pub fn codigo_executavel(fonte: &str) -> String {
    let bytes: Vec<char> = fonte.chars().collect();
    let mut saida = String::with_capacity(fonte.len());
    let mut i = 0;
    let mut profundidade_de_bloco = 0usize;
    while i < bytes.len() {
        let dois: String = bytes[i..(i + 2).min(bytes.len())].iter().collect();
        if profundidade_de_bloco > 0 {
            if dois == "/*" {
                profundidade_de_bloco += 1;
                i += 2;
            } else if dois == "*/" {
                profundidade_de_bloco -= 1;
                i += 2;
            } else {
                i += 1;
            }
            saida.push(' ');
            continue;
        }
        if dois == "/*" {
            profundidade_de_bloco = 1;
            i += 2;
            saida.push(' ');
            continue;
        }
        if dois == "//" {
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            saida.push(' ');
            continue;
        }
        // Literal de texto cru: `r"..."`, `r#"..."#`, com o mesmo número de `#`.
        if bytes[i] == 'r' {
            let mut cerquilhas = 0;
            while i + 1 + cerquilhas < bytes.len() && bytes[i + 1 + cerquilhas] == '#' {
                cerquilhas += 1;
            }
            if bytes.get(i + 1 + cerquilhas) == Some(&'"') {
                let fecho: String = std::iter::once('"')
                    .chain(std::iter::repeat('#').take(cerquilhas))
                    .collect();
                i += 2 + cerquilhas;
                while i < bytes.len() {
                    let janela: String = bytes[i..(i + fecho.len()).min(bytes.len())]
                        .iter()
                        .collect();
                    if janela == fecho {
                        i += fecho.len();
                        break;
                    }
                    i += 1;
                }
                saida.push(' ');
                continue;
            }
        }
        if bytes[i] == '"' {
            i += 1;
            while i < bytes.len() && bytes[i] != '"' {
                i += if bytes[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            saida.push(' ');
            continue;
        }
        if bytes[i] == '\'' {
            let escapado = bytes.get(i + 1) == Some(&'\\');
            let fim = if escapado { i + 3 } else { i + 2 };
            if bytes.get(fim) == Some(&'\'') {
                i = fim + 1;
                saida.push(' ');
                continue;
            }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    saida
}
