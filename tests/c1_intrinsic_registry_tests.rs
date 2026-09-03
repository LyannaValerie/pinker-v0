//! #442/C1 — a autoridade declarativa das intrínsecas históricas é uma só.
//!
//! Antes da consolidação, existência, contrato de parâmetros, contrato de
//! retorno, política de aridade e roteamento de runtime eram enumerados
//! independentemente por sete camadas. Estes testes provam que a enumeração
//! passou a ser única e que reintroduzir uma cópia local é detectável.

use pinker_v0::intrinsic_authority::{
    intrinsic_from_public_spelling, CalleeIdentity, IntrinsicIdentity, HISTORICAL_CANONICAL_ALIASES,
};
use pinker_v0::intrinsics::registry::{self, ArityPolicy, RuntimeRouting, Signature};
use std::collections::BTreeSet;

/// Grafias históricas que cada fase ainda cita literalmente, e por quê.
///
/// A auditoria é o gate contra a regressão que motivou a consolidação: se uma
/// fase voltar a listar grafias históricas por conta própria, o conjunto muda e
/// o teste fica vermelho. Cada exceção abaixo é implementação de fase — corpo,
/// monomorfização ou efeito de pilha —, nunca binding declarativo.
const CITACOES_AUTORIZADAS: &[(&str, &str, &[&str])] = &[
    (
        "src/ir_validate.rs",
        "tipagem da variádica: modelo `verso` seguido de N valores; o mínimo vem do registry",
        &["formatar_verso"],
    ),
    (
        "src/cfg_ir_validate.rs",
        "tipagem da variádica, idem",
        &["formatar_verso"],
    ),
    ("src/instr_select_validate.rs", "nenhuma", &[]),
    (
        "src/abstract_machine_validate.rs",
        "efeito de pilha das duas operações de aridade não fixa",
        &["afirmar", "formatar_verso"],
    ),
    (
        "src/backend_s.rs",
        "empacotamento próprio de `formatar_verso` e a forma de ponteiro de `alocar`",
        &["alocar", "formatar_verso"],
    ),
];

fn fonte_sem_testes(caminho: &str) -> String {
    let bruto = std::fs::read_to_string(format!("{}/{caminho}", env!("CARGO_MANIFEST_DIR")))
        .expect("fonte da fase legível");
    match bruto.find("\n#[cfg(test)]") {
        Some(corte) => bruto[..corte].to_string(),
        None => bruto,
    }
}

fn grafias_citadas(fonte: &str) -> BTreeSet<&'static str> {
    registry::grafias()
        .filter(|grafia| fonte.contains(&format!("\"{grafia}\"")))
        .collect()
}

#[test]
fn nenhuma_fase_de_validacao_reintroduz_enumeracao_historica() {
    for (caminho, motivo, autorizadas) in CITACOES_AUTORIZADAS {
        let citadas = grafias_citadas(&fonte_sem_testes(caminho));
        let esperadas: BTreeSet<&str> = autorizadas.iter().copied().collect();
        assert_eq!(
            citadas, esperadas,
            "{caminho}: citação literal de grafia histórica fora do autorizado ({motivo})"
        );
    }
}

#[test]
fn nenhuma_fase_reconstroi_a_tabela_de_simbolos_de_runtime() {
    let backend = fonte_sem_testes("src/backend_s.rs");
    for entrada in registry::HISTORICAL {
        if let RuntimeRouting::Symbol(simbolo) = entrada.runtime {
            assert!(
                !backend.contains(&format!("\"{}\" => Some(\"{simbolo}\")", entrada.spelling)),
                "{}: símbolo de runtime voltou a ser decidido no backend",
                entrada.spelling
            );
        }
    }
}

#[test]
fn registry_e_identidade_enxergam_a_mesma_superficie() {
    let grafias: Vec<&str> = registry::grafias().collect();
    assert_eq!(grafias.len(), 131);
    for grafia in &grafias {
        assert!(registry::e_historica(grafia));
        assert!(
            matches!(
                intrinsic_from_public_spelling(grafia),
                Some(IntrinsicIdentity::Historical(_))
            ),
            "{grafia}"
        );
    }
    // Uma grafia de usuário não entra na autoridade por parecer com uma.
    assert!(!registry::e_historica("tamanho_verso_do_usuario"));
    assert_eq!(
        intrinsic_from_public_spelling("tamanho_verso_do_usuario"),
        None
    );
}

#[test]
fn politica_de_alias_historico_continua_congelada() {
    assert_eq!(HISTORICAL_CANONICAL_ALIASES.len(), 3);
    for (alias, adulta) in HISTORICAL_CANONICAL_ALIASES {
        let alias_entry = registry::entrada(alias).expect("alias no registry");
        let adulta_entry = registry::entrada(adulta).expect("grafia adulta no registry");
        assert_eq!(alias_entry.signature, adulta_entry.signature);
        assert_eq!(alias_entry.runtime, adulta_entry.runtime);
        assert_eq!(
            registry::simbolo_runtime(alias),
            registry::simbolo_runtime(adulta)
        );
    }
    // Nenhum alias novo entra por descuido: a relação é exatamente esta.
    let aliases: BTreeSet<&str> = HISTORICAL_CANONICAL_ALIASES
        .iter()
        .map(|(alias, _)| *alias)
        .collect();
    assert_eq!(
        aliases,
        [
            "argumento_nomeado_ou",
            "argumento_nomeado_ou_ambiente_ou",
            "tem_argumento_nomeado"
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn simbolo_declarado_existe_no_runtime_nativo() {
    let runtime = include_str!("../runtime/pinker_rt/src/lib.rs");
    for entrada in registry::HISTORICAL {
        match entrada.runtime {
            RuntimeRouting::Symbol(simbolo) => assert!(
                runtime.contains(&format!("fn {simbolo}(")),
                "{}: símbolo {simbolo} ausente do runtime nativo",
                entrada.spelling
            ),
            RuntimeRouting::ByArity { .. } => {
                let aridades =
                    registry::aridades_aceitas(entrada.spelling).expect("recorte declarado");
                for argc in aridades {
                    let simbolo = registry::simbolo_runtime_por_aridade(entrada.spelling, *argc)
                        .expect("aridade dentro do recorte tem símbolo");
                    assert!(
                        runtime.contains(&format!("fn {simbolo}(")),
                        "{}: símbolo {simbolo} ausente do runtime nativo",
                        entrada.spelling
                    );
                }
            }
            RuntimeRouting::NotRouted => {}
        }
    }
}

#[test]
fn contrato_declarado_cobre_toda_a_superficie_das_fases_de_assinatura() {
    let declaradas: BTreeSet<&str> = registry::HISTORICAL
        .iter()
        .filter(|entrada| entrada.assinatura_ir().is_some())
        .map(|entrada| entrada.spelling)
        .collect();
    let genericas: BTreeSet<&str> = registry::HISTORICAL
        .iter()
        .filter(|entrada| entrada.signature == Signature::GenericMonomorphized)
        .map(|entrada| entrada.spelling)
        .collect();
    assert_eq!(declaradas.len(), 118);
    assert_eq!(genericas.len(), 13);
    assert!(declaradas.is_disjoint(&genericas));
    for grafia in &genericas {
        assert!(
            grafia.starts_with("lista_") || grafia.starts_with("mapa_"),
            "{grafia}: só as coleções genéricas são monomorfizadas antes das assinaturas"
        );
    }
}

#[test]
fn aridade_exata_derivada_do_contrato_de_parametros() {
    for entrada in registry::HISTORICAL {
        let Signature::Declared { arity, params, .. } = entrada.signature else {
            continue;
        };
        if arity == ArityPolicy::Exact {
            assert!(
                !registry::aridade_no_recorte(entrada.spelling, params.len()),
                "{}: aridade exata não abre recorte",
                entrada.spelling
            );
        }
    }
}

#[test]
fn identidade_do_callee_continua_decidindo_quem_e_intrinseca() {
    // A grafia sozinha nunca prova que a chamada é intrínseca: a decisão é da
    // identidade resolvida, e o registry não a substitui.
    assert!(CalleeIdentity::User.is_user());
    assert!(!CalleeIdentity::User.dispatches_as_builtin());
    let intrinseca = intrinsic_from_public_spelling("tamanho_verso").expect("grafia histórica");
    assert!(CalleeIdentity::Intrinsic(intrinseca).dispatches_as_builtin());
    assert_eq!(
        CalleeIdentity::Intrinsic(intrinseca).canonical_spelling(),
        Some("tamanho_verso")
    );
    assert_eq!(CalleeIdentity::User.canonical_spelling(), None);
}
