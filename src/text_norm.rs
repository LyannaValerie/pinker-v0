//! Trama Pinker — normalização determinística de texto para consultas.
//!
//! Implementa a política de normalização da especificação (§7.1) usada tanto
//! pelas consultas documentais (`pink doc`) quanto pelas de código (`pink nav`):
//!
//! 1. converter para minúsculas;
//! 2. remover diacríticos (acentos);
//! 3. substituir pontuação por espaço;
//! 4. colapsar espaços;
//! 5. separar em termos;
//! 6. preservar os valores originais apenas para exibição (feito pelo chamador).
//!
//! Sem fuzzy search, embeddings ou stemming — coerente com a filosofia
//! zero-dependência do compilador. A remoção de diacríticos cobre o alfabeto
//! latino usado no repositório (português) por mapeamento explícito e auditável.

// @pinker-nav:start trama.queries.normalization
// @pinker-nav:domain queries
// @pinker-nav:layer trama
// @pinker-nav:summary Deterministic text normalization for the Trama's queries (lowercasing, diacritic removal, punctuation becoming whitespace, collapsing and splitting into terms), with no stemming and no fuzzy matching — a shared basis for `pink doc` and `pink nav`.
/// Normaliza uma string para uma forma canônica comparável: minúsculas, sem
/// acentos, com pontuação virada em espaço e espaços colapsados em um único
/// separador. O resultado tem no máximo um espaço entre termos e não tem
/// espaços nas bordas.
pub fn normalize(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pending_space = false;
    for ch in input.chars() {
        let folded = fold_char(ch);
        for f in folded.chars() {
            if f.is_alphanumeric() {
                if pending_space && !out.is_empty() {
                    out.push(' ');
                }
                pending_space = false;
                // `to_lowercase` pode devolver múltiplos chars; preserva todos.
                for lower in f.to_lowercase() {
                    out.push(lower);
                }
            } else {
                // Qualquer não-alfanumérico (pontuação, espaço, símbolo) vira
                // um separador; a colapsação acontece pelo `pending_space`.
                pending_space = true;
            }
        }
    }
    out
}

/// Termos normalizados de uma consulta, na ordem, sem vazios.
pub fn terms(input: &str) -> Vec<String> {
    normalize(input)
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// Remove o diacrítico de um caractere latino comum. Retorna o próprio
/// caractere (como `char`) quando não há mapeamento. O tipo de retorno é
/// `&'static str` para caso raros de expansão, mas hoje todos os mapeamentos
/// são 1:1.
fn fold_char(ch: char) -> &'static str {
    match ch {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'É' | 'È' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'í' | 'ì' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => "i",
        'Í' | 'Ì' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' => "I",
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' | 'ø' => "o",
        'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' | 'Ō' | 'Ŏ' | 'Ő' | 'Ø' => "O",
        'ú' | 'ù' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ú' | 'Ù' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'ý' | 'ÿ' => "y",
        'Ý' | 'Ÿ' => "Y",
        // Caso geral: devolve o caractere sem alteração. Precisamos de um
        // `&'static str`; para caracteres não mapeados usamos um buffer por
        // meio de `char_to_static` — mas como Rust não permite vazar aqui,
        // tratamos o caso comum ASCII diretamente e delegamos o restante.
        _ => passthrough(ch),
    }
}

/// Para caracteres sem mapeamento de diacrítico, devolvemos a fatia estática
/// correspondente quando é ASCII; caracteres não-ASCII sem acento conhecido são
/// preservados via tabela mínima. Como não podemos produzir `&'static str` de
/// um `char` arbitrário, ASCII cobre o essencial e o resto cai como espaço
/// (tratado como separador), o que é seguro para a normalização de consultas.
fn passthrough(ch: char) -> &'static str {
    // ASCII imprimível é o caso dominante em ids, chaves e consultas.
    const ASCII: &str = concat!(
        " !\"#$%&'()*+,-./0123456789:;<=>?@",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`",
        "abcdefghijklmnopqrstuvwxyz{|}~"
    );
    if ch.is_ascii() && !ch.is_control() {
        let idx = (ch as usize) - 0x20;
        return &ASCII[idx..idx + 1];
    }
    // Não-ASCII sem diacrítico conhecido: trata como separador (espaço). Isso
    // é conservador e determinístico; nenhum termo do repositório depende disso.
    " "
}
// @pinker-nav:end trama.queries.normalization

// @pinker-nav:start evidence.queries.normalization
// @pinker-nav:domain queries
// @pinker-nav:layer evidence
// @pinker-nav:summary Proofs of query normalization: lowercasing with diacritic removal, punctuation becoming whitespace with collapsing of repetitions, splitting into terms discarding empty ones and folding of every Portuguese diacritic.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_strips_accents() {
        assert_eq!(normalize("Próxima Fase"), "proxima fase");
        assert_eq!(normalize("QUEM É ROSA?"), "quem e rosa");
        assert_eq!(normalize("estado atual"), "estado atual");
    }

    #[test]
    fn punctuation_becomes_space_and_collapses() {
        assert_eq!(normalize("a.b_c-d"), "a b c d");
        assert_eq!(normalize("  muitos    espaços  "), "muitos espacos");
        assert_eq!(normalize("engine.state.current"), "engine state current");
    }

    #[test]
    fn terms_splits_and_drops_empty() {
        assert_eq!(
            terms("qual é a próxima fase?"),
            vec!["qual", "e", "a", "proxima", "fase"]
        );
        assert!(terms("   ").is_empty());
        assert!(terms("").is_empty());
    }

    #[test]
    fn all_portuguese_diacritics_fold() {
        assert_eq!(normalize("ãâáàéêíóôõúüç"), "aaaaeeiooouuc");
        assert_eq!(normalize("Ãâ Éê Çç"), "aa ee cc");
    }
}
// @pinker-nav:end evidence.queries.normalization
