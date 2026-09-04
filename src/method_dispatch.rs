//! Autoridade única da **seleção** de método de impl.
//!
//! Este módulo responde uma pergunta e só ela:
//!
//! ```text
//! dados candidatos já construídos por uma fase,
//! qual implementação concreta vence?
//! ```
//!
//! O que ele deliberadamente NÃO responde, e continua com seus donos:
//!
//! ```text
//! identidade do método            method_identity
//! alcance de trato/relação        module_resolve (TratosNoDespacho, nivel_de_despacho)
//! materialização de corpo default parser/semantic (a existência da função gerada)
//! span, mensagem e tipo do erro   a fase que chamou
//! representação e lowering        ir
//! ```
//!
//! A fase constrói candidatos a partir do índice da sua própria representação e
//! traduz o resultado para a sua superfície de diagnóstico. Nenhuma das duas
//! decide vencedor por conta própria — é isso que impede `--check` e o lowering
//! de discordarem sobre o mesmo programa.

use crate::module_resolve::{nivel_de_despacho, NivelDeDespacho, TratosNoDespacho};
use crate::source_map::SourceId;
use crate::token::Span;
use std::collections::HashMap;

/// A relação de `impl` que originou um candidato.
///
/// `fonte_da_relacao` é a unidade que DECLAROU o bloco `impl`, não a que
/// escreveu o método: corpo default materializado pode vir de outra unidade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRelation {
    pub trait_name: String,
    pub fonte_da_relacao: Option<SourceId>,
}

/// Um candidato de despacho já construído pela fase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchCandidate {
    pub function_name: String,
    /// `None` quando a fase não conhece a relação do candidato. O candidato
    /// entra no nível próprio, exatamente como sempre entrou.
    pub relation: Option<DispatchRelation>,
}

/// Resultado da seleção. A fase decide o span, a mensagem e o tipo do erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodSelection {
    Winner(String),
    NoMatch,
    Ambiguous,
}

/// Qual implementação concreta vence esta chamada?
///
/// Alcance vem de `module_resolve`: candidato que não alcança quem escreveu o
/// `span` não participa. Entre os que alcançam, vence o nível mais forte —
/// o trato próprio nunca perde para uma relação alcançada por importação
/// (#577). Sobrando exatamente um, ele é o vencedor; nenhum é `NoMatch`; mais
/// de um é `Ambiguous`.
pub fn select_impl_method(
    traits_visiveis_por_fonte: &HashMap<SourceId, TratosNoDespacho>,
    span: Span,
    candidates: impl IntoIterator<Item = DispatchCandidate>,
) -> MethodSelection {
    let alcancados: Vec<(NivelDeDespacho, String)> = candidates
        .into_iter()
        .filter_map(|candidate| match candidate.relation {
            Some(relation) => nivel_de_despacho(
                traits_visiveis_por_fonte,
                span,
                &relation.trait_name,
                relation.fonte_da_relacao,
            )
            .map(|nivel| (nivel, candidate.function_name)),
            None => Some((NivelDeDespacho::Proprio, candidate.function_name)),
        })
        .collect();

    let Some(mais_forte) = alcancados.iter().map(|(nivel, _)| *nivel).min() else {
        return MethodSelection::NoMatch;
    };
    let mut vencedores = alcancados
        .into_iter()
        .filter(|(nivel, _)| *nivel == mais_forte)
        .map(|(_, function_name)| function_name);

    match (vencedores.next(), vencedores.next()) {
        (Some(function_name), None) => MethodSelection::Winner(function_name),
        (Some(_), Some(_)) => MethodSelection::Ambiguous,
        (None, _) => MethodSelection::NoMatch,
    }
}

/// Qual função materializada REPRESENTA uma identidade de método?
///
/// A pergunta aparece uma vez por identidade, antes de qualquer chamada: uma
/// relação pode ter o método explícito do `impl` e o corpo default copiado do
/// trato. Não é materialização — quem cria a função gerada é outra autoridade;
/// aqui só se escolhe qual das já materializadas representa a identidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentativeSelection {
    /// Índice do representante na ordem canônica.
    Selected(usize),
    /// Dois métodos explícitos para a mesma identidade, em ordem canônica.
    ExplicitConflict { previous: usize, conflicting: usize },
}

/// Ordena `candidates` na ordem canônica e escolhe o representante.
///
/// A ordem canônica é a ordem total do símbolo provisório, nunca a ordem de
/// fonte ou de import. Um explícito vence os defaults materializados da mesma
/// identidade; dois explícitos são conflito, e a fase decide a mensagem.
pub fn select_representative<T>(
    candidates: &mut [T],
    symbol: impl Fn(&T) -> &str,
    generated_default: impl Fn(&T) -> bool,
) -> RepresentativeSelection {
    debug_assert!(
        !candidates.is_empty(),
        "identidade de método sem nenhuma função materializada"
    );
    candidates.sort_by(|left, right| symbol(left).cmp(symbol(right)));
    let mut explicitos = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| !generated_default(candidate))
        .map(|(index, _)| index);

    match (explicitos.next(), explicitos.next()) {
        (Some(previous), Some(conflicting)) => RepresentativeSelection::ExplicitConflict {
            previous,
            conflicting,
        },
        (Some(explicito), None) => RepresentativeSelection::Selected(explicito),
        (None, _) => RepresentativeSelection::Selected(0),
    }
}
