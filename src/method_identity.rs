//! Identidade estruturada de métodos e transporte provisório do parser.
//!
//! A identidade semântica nasce somente quando o chamador fornece a identidade
//! resolvida do tipo-alvo. O texto `__impl_*` não participa dessa decisão: ele
//! continua sendo apenas a forma injetiva com que o parser transporta os três
//! spellings até as fases seguintes.

// @pinker-nav:start tratos.metodos.identidade
// @pinker-nav:domain tratos
// @pinker-nav:layer identidade
// @pinker-nav:summary Identidade estruturada de método parametrizada pela identidade resolvida do alvo, compartilhada pela autoridade semântica e pela visão derivada da IR; também centraliza o codec injetivo dos nomes provisórios `__impl_*` e `__trait_default_check_*` — mesma gramática, prefixos distintos —, que preservam spellings para transporte e renderização mas nunca decidem coerência ou despacho, e a forma única que reconhece os dois como o mesmo corpo sintético de `trato`, para que a canonização e a materialização modular não precisem perguntar pelo prefixo literal.

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

/// Prefixo do método de `impl` materializado pelo parser.
pub const IMPL_PREFIX: &str = "__impl_";

pub fn render_provisional_function_name(
    trait_name: &str,
    target_spelling: &str,
    method_name: &str,
) -> String {
    format!(
        "{}{}_{}_{}_{}_{}",
        IMPL_PREFIX,
        trait_name.len(),
        trait_name,
        target_spelling.len(),
        target_spelling,
        method_name
    )
}

/// Prefixo do corpo default materializado apenas para checagem semântica.
///
/// Ele nasce quando um override explícito vence a seleção: o corpo do contrato
/// continua sendo checado, mas a função não entra em `method_index` nem em
/// vtable. O codec é o mesmo de `__impl_*`, com outro prefixo.
pub const TRAIT_DEFAULT_CHECK_PREFIX: &str = "__trait_default_check_";

pub fn render_trait_default_check_function_name(
    trait_name: &str,
    target_spelling: &str,
    method_name: &str,
) -> String {
    format!(
        "{}{}_{}_{}_{}_{}",
        TRAIT_DEFAULT_CHECK_PREFIX,
        trait_name.len(),
        trait_name,
        target_spelling.len(),
        target_spelling,
        method_name
    )
}

pub fn parse_provisional_function_name(name: &str) -> Option<(String, String, String)> {
    parse_com_prefixo(name, IMPL_PREFIX)
}

/// Corpo sintético materializado pela maquinaria de `trato`.
///
/// São duas formas do MESMO fato — um corpo que o parser copiou para dentro de
/// um `impl` — sob o mesmo codec e prefixos distintos: `__impl_*` quando o
/// corpo é o método selecionado, `__trait_default_check_*` quando um override
/// venceu e o corpo default continua devendo checagem. O prefixo decide apenas
/// se a função entra em `method_index`/vtable; a identidade e a obrigação de
/// materializá-la para validar são as mesmas.
///
/// Quem trata as duas formas como uma só não pode perguntar pelo prefixo
/// literal: era exatamente essa pergunta que fazia a checagem do default
/// desaparecer em unidade não-raiz e o nome dela colidir entre unidades.
pub fn parse_synthetic_trait_body_name(
    name: &str,
) -> Option<(&'static str, String, String, String)> {
    for prefixo in [IMPL_PREFIX, TRAIT_DEFAULT_CHECK_PREFIX] {
        if let Some((trait_name, target_spelling, method_name)) = parse_com_prefixo(name, prefixo) {
            return Some((prefixo, trait_name, target_spelling, method_name));
        }
    }
    None
}

/// Recompõe um corpo sintético sob o mesmo prefixo com que ele foi lido.
pub fn render_synthetic_trait_body_name(
    prefixo: &str,
    trait_name: &str,
    target_spelling: &str,
    method_name: &str,
) -> String {
    format!(
        "{}{}_{}_{}_{}_{}",
        prefixo,
        trait_name.len(),
        trait_name,
        target_spelling.len(),
        target_spelling,
        method_name
    )
}

fn parse_com_prefixo(name: &str, prefixo: &str) -> Option<(String, String, String)> {
    let rest = name.strip_prefix(prefixo)?;
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

    #[test]
    fn corpo_sintetico_reconhece_as_duas_formas_e_preserva_o_prefixo() {
        for prefixo in [IMPL_PREFIX, TRAIT_DEFAULT_CHECK_PREFIX] {
            let rendered = render_synthetic_trait_body_name(prefixo, "a.Marca", "bombom", "marcar");
            assert_eq!(
                parse_synthetic_trait_body_name(&rendered),
                Some((
                    prefixo,
                    "a.Marca".to_string(),
                    "bombom".to_string(),
                    "marcar".to_string()
                ))
            );
        }
    }

    #[test]
    fn tratos_homonimos_de_unidades_distintas_nao_compartilham_checagem_de_default() {
        assert_ne!(
            render_trait_default_check_function_name("a.Marca", "bombom", "marcar"),
            render_trait_default_check_function_name("b.Marca", "bombom", "marcar")
        );
    }
}
