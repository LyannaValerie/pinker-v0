//! Identidade estruturada de métodos e transporte provisório do parser.
//!
//! A identidade semântica nasce somente quando o chamador fornece a identidade
//! resolvida do tipo-alvo. O texto `__impl_*` não participa dessa decisão: ele
//! continua sendo apenas a forma injetiva com que o parser transporta os três
//! spellings até as fases seguintes.

// @pinker-nav:start tratos.metodos.identidade
// @pinker-nav:domain tratos
// @pinker-nav:layer identidade
// @pinker-nav:summary Identidade estruturada de método parametrizada pela identidade resolvida do alvo, compartilhada pela autoridade semântica e pela visão derivada da IR; também centraliza o codec injetivo dos nomes provisórios `__impl_*`, que preservam spellings para transporte e renderização mas nunca decidem coerência ou despacho.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MethodIdentity<T> {
    pub trait_name: String,
    pub target: T,
    pub method_name: String,
}

impl<T> MethodIdentity<T> {
    pub fn new(trait_name: String, target: T, method_name: String) -> Self {
        Self {
            trait_name,
            target,
            method_name,
        }
    }
}

pub fn render_provisional_function_name(
    trait_name: &str,
    target_spelling: &str,
    method_name: &str,
) -> String {
    format!(
        "__impl_{}_{}_{}_{}_{}",
        trait_name.len(),
        trait_name,
        target_spelling.len(),
        target_spelling,
        method_name
    )
}

pub fn parse_provisional_function_name(name: &str) -> Option<(String, String, String)> {
    let rest = name.strip_prefix("__impl_")?;
    let (trait_len, rest) = rest.split_once('_')?;
    let trait_len: usize = trait_len.parse().ok()?;
    if rest.len() < trait_len + 1 || !rest.is_char_boundary(trait_len) {
        return None;
    }
    let (trait_name, rest) = rest.split_at(trait_len);
    let rest = rest.strip_prefix('_')?;
    let (target_len, rest) = rest.split_once('_')?;
    let target_len: usize = target_len.parse().ok()?;
    if rest.len() < target_len + 1 || !rest.is_char_boundary(target_len) {
        return None;
    }
    let (target_spelling, rest) = rest.split_at(target_len);
    let method_name = rest.strip_prefix('_')?;
    if method_name.is_empty() {
        return None;
    }
    Some((
        trait_name.to_string(),
        target_spelling.to_string(),
        method_name.to_string(),
    ))
}

// @pinker-nav:end tratos.metodos.identidade

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transporte_provisorio_e_injetivo_para_componentes_com_sublinhado() {
        let rendered = render_provisional_function_name("Meu_Trato", "Meu_Tipo", "meu_metodo");
        assert_eq!(
            parse_provisional_function_name(&rendered),
            Some((
                "Meu_Trato".to_string(),
                "Meu_Tipo".to_string(),
                "meu_metodo".to_string()
            ))
        );
    }
}
