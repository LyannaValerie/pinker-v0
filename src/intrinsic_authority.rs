//! Autoridade da superfície pública de intrínsecas.
//!
//! Este módulo responde somente três perguntas de linguagem: se uma grafia
//! pública pertence a uma intrínseca, qual identidade ela representa e qual
//! política vale para uma declaração callable homônima. Assinaturas,
//! execução no interpretador e símbolos de runtime continuam com seus donos
//! de fase.

use crate::falha_operacional::{OperacaoFalivel, SUPERFICIES_FALIVEIS};
use crate::familia_superficie::{IdentidadeCanonica, EXPORTACOES};
use std::collections::BTreeMap;

/// Identidade de linguagem de uma intrínseca, sem promover ABI a identidade.
///
/// As variantes classificam autoridades já existentes; não há uma variante
/// por intrínseca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntrinsicIdentity {
    Historical(&'static str),
    Fallible(OperacaoFalivel),
    Json(&'static str),
    Sha256(&'static str),
}

impl IntrinsicIdentity {
    pub fn canonical_public_spelling(self) -> &'static str {
        match self {
            Self::Historical(spelling) | Self::Json(spelling) | Self::Sha256(spelling) => spelling,
            Self::Fallible(operation) => {
                SUPERFICIES_FALIVEIS
                    .iter()
                    .find(|surface| surface.operacao == operation)
                    .expect("operação falível registrada na autoridade")
                    .intrinseca
            }
        }
    }
}

/// Origem da grafia pública. A origem classifica ownership, não execução.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicIntrinsicOrigin {
    Historical,
    Fallible,
    Json,
    Sha256,
    FamilyAlias { family: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicIntrinsicSpelling {
    pub spelling: &'static str,
    pub identity: IntrinsicIdentity,
    pub origin: PublicIntrinsicOrigin,
}

/// Política congelada pela Founder para o namespace callable compartilhado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationConflictPolicy {
    DeclarationIsRejected,
}

/// Lacuna histórica: estas identidades ainda não possuem registry estrutural
/// anterior a U3. A lista é a autoridade canônica da superfície histórica;
/// semantic/interpreter/backend permanecem consumidores de fase até trabalho
/// posterior explicitamente autorizado.
pub const HISTORICAL_PUBLIC_SPELLINGS: &[&str] = &[
    "abrir",
    "abrir_anexo",
    "afirmar",
    "aleatorio_criar",
    "aleatorio_entre",
    "aleatorio_proximo",
    "alocar",
    "ambiente_ou",
    "anexar_verso",
    "aparar_verso",
    "argumento",
    "argumento_nomeado_ou",
    "argumento_nomeado_ou_ambiente_ou",
    "argumento_ou",
    "arquivo_ou",
    "bombom_para_verso",
    "buscar_contexto",
    "buscar_verso",
    "caminho_existe",
    "capturar_stderr",
    "capturar_stdout",
    "comeca_com",
    "contem_verso",
    "copiar_arquivo",
    "criar_arquivo",
    "criar_diretorio",
    "diretorio_atual",
    "dividir_verso_contar",
    "dividir_verso_em",
    "dormir",
    "e_arquivo",
    "e_diretorio",
    "e_vazio",
    "emitir_json_plano_bombom",
    "emitir_linha_csv_bombom",
    "escrever",
    "escrever_verso",
    "executar_com_entrada",
    "executar_processo",
    "fatiar_verso",
    "fechar",
    "formatar_tempo_unix",
    "formatar_verso",
    "igual_verso",
    "indice_verso",
    "indice_verso_em",
    "juntar_caminho",
    "juntar_verso",
    "juntar_verso_com",
    "ler_arquivo",
    "ler_arquivo_verso",
    "ler_json_plano_bombom",
    "ler_linha_csv_bombom",
    "ler_verso_arquivo",
    "liberar",
    "lista_anexar",
    "lista_bombom_anexar",
    "lista_bombom_criar",
    "lista_bombom_definir",
    "lista_bombom_inserir",
    "lista_bombom_obter",
    "lista_bombom_tamanho",
    "lista_bombom_tirar_ultimo",
    "lista_criar",
    "lista_definir",
    "lista_inserir",
    "lista_obter",
    "lista_tamanho",
    "lista_tirar_ultimo",
    "lista_verso_anexar",
    "lista_verso_criar",
    "lista_verso_definir",
    "lista_verso_inserir",
    "lista_verso_obter",
    "lista_verso_tamanho",
    "lista_verso_tirar_ultimo",
    "maiusculo_verso",
    "mapa_bombom_bombom_criar",
    "mapa_bombom_bombom_definir",
    "mapa_bombom_bombom_obter",
    "mapa_bombom_bombom_remover",
    "mapa_bombom_bombom_tamanho",
    "mapa_bombom_bombom_tem",
    "mapa_bombom_verso_criar",
    "mapa_bombom_verso_definir",
    "mapa_bombom_verso_obter",
    "mapa_bombom_verso_remover",
    "mapa_bombom_verso_tamanho",
    "mapa_bombom_verso_tem",
    "mapa_definir",
    "mapa_obter",
    "mapa_remover",
    "mapa_tamanho",
    "mapa_tem",
    "mapa_verso_bombom_criar",
    "mapa_verso_bombom_definir",
    "mapa_verso_bombom_obter",
    "mapa_verso_bombom_remover",
    "mapa_verso_bombom_tamanho",
    "mapa_verso_bombom_tem",
    "mapa_verso_verso_criar",
    "mapa_verso_verso_definir",
    "mapa_verso_verso_obter",
    "mapa_verso_verso_remover",
    "mapa_verso_verso_tamanho",
    "mapa_verso_verso_tem",
    "minusculo_verso",
    "nao_vazio_verso",
    "ouvir",
    "ouvir_verso",
    "ouvir_verso_ou",
    "pedir_argumento",
    "pipeline_minimo",
    "quantos_argumentos",
    "remover_arquivo",
    "remover_diretorio",
    "renomear_arquivo",
    "sair",
    "substituir_verso",
    "tamanho_arquivo",
    "tamanho_verso",
    "tem_argumento",
    "tem_argumento_nomeado",
    "tem_chave",
    "tem_flag",
    "tempo_unix",
    "termina_com",
    "truncar_arquivo",
    "vazio_verso",
    "verso_para_bombom",
];

/// Resolve uma grafia canônica global, sem aliases ativados por import de família.
pub fn canonical_public_intrinsic_spelling(spelling: &str) -> Option<PublicIntrinsicSpelling> {
    if let Some(surface) = SUPERFICIES_FALIVEIS
        .iter()
        .find(|surface| surface.intrinseca == spelling)
    {
        return Some(PublicIntrinsicSpelling {
            spelling: surface.intrinseca,
            identity: IntrinsicIdentity::Fallible(surface.operacao),
            origin: PublicIntrinsicOrigin::Fallible,
        });
    }
    if let Some(spelling) = crate::valor_json::ACESSORES
        .iter()
        .copied()
        .find(|candidate| *candidate == spelling)
    {
        return Some(PublicIntrinsicSpelling {
            spelling,
            identity: IntrinsicIdentity::Json(spelling),
            origin: PublicIntrinsicOrigin::Json,
        });
    }
    if let Some(spelling) = crate::sha256::ACESSORES
        .iter()
        .copied()
        .find(|candidate| *candidate == spelling)
    {
        return Some(PublicIntrinsicSpelling {
            spelling,
            identity: IntrinsicIdentity::Sha256(spelling),
            origin: PublicIntrinsicOrigin::Sha256,
        });
    }
    HISTORICAL_PUBLIC_SPELLINGS
        .iter()
        .copied()
        .find(|candidate| *candidate == spelling)
        .map(|spelling| PublicIntrinsicSpelling {
            spelling,
            identity: IntrinsicIdentity::Historical(spelling),
            origin: PublicIntrinsicOrigin::Historical,
        })
}

fn family_identity(identity: IdentidadeCanonica) -> IntrinsicIdentity {
    match identity {
        IdentidadeCanonica::Historica(spelling) => IntrinsicIdentity::Historical(spelling),
        IdentidadeCanonica::Falivel(operation) => IntrinsicIdentity::Fallible(operation),
    }
}

/// Grafias que a autoridade semântica resolve como chamada builtin ANTES de
/// procurar função de usuário, e que esta autoridade de intrínsecas públicas
/// não possui.
///
/// A política da PR #507 cobre a superfície pública; ela não cobre toda grafia
/// builtin. `mapa_criar` é a criação genérica de mapa: o checador a despacha
/// pelo nome, mas ela não é redeclarável-rejeitável como intrínseca pública.
///
/// Quem consome esta lista é a resolução modular, que precisa distinguir
/// "grafia builtin" de "entidade declarada por alguma unidade". Sem isso, um
/// módulo que declare `mapa_criar` faria a raiz perder a chamada builtin.
/// O teste de deriva em `tests/issue_514_module_composition_tests.rs` recusa
/// qualquer grafia builtin de `src/semantic.rs` que esta autoridade não
/// reconheça.
const GRAFIAS_BUILTIN_NAO_PUBLICAS: &[&str] = &["mapa_criar"];

/// A grafia é resolvida como chamada builtin pela autoridade semântica?
///
/// `GRAFIA_BUILTIN != ENTIDADE_DE_UNIDADE`: builtin não pertence a
/// unidade-fonte alguma e por isso nunca é capturado nem capturável.
pub fn e_grafia_builtin_chamavel(spelling: &str) -> bool {
    public_intrinsic_spelling(spelling).is_some()
        || GRAFIAS_BUILTIN_NAO_PUBLICAS.contains(&spelling)
}

/// Resolve uma grafia pública vigente para a sua identidade intrínseca.
pub fn public_intrinsic_spelling(spelling: &str) -> Option<PublicIntrinsicSpelling> {
    canonical_public_intrinsic_spelling(spelling).or_else(|| {
        EXPORTACOES
            .iter()
            .find(|export| export.membro() == spelling)
            .map(|export| PublicIntrinsicSpelling {
                spelling: export.membro(),
                identity: family_identity(export.identidade),
                origin: PublicIntrinsicOrigin::FamilyAlias {
                    family: export.familia,
                },
            })
    })
}

/// Resolve um membro público no contexto da família que o ativa.
pub fn family_public_intrinsic_spelling(
    family: &str,
    spelling: &str,
) -> Option<PublicIntrinsicSpelling> {
    EXPORTACOES
        .iter()
        .find(|export| export.familia == family && export.membro() == spelling)
        .map(|export| PublicIntrinsicSpelling {
            spelling: export.membro(),
            identity: family_identity(export.identidade),
            origin: PublicIntrinsicOrigin::FamilyAlias {
                family: export.familia,
            },
        })
}

/// Forma direta de Q1+Q2: grafia pública para identidade, ou ausência.
pub fn intrinsic_from_public_spelling(spelling: &str) -> Option<IntrinsicIdentity> {
    public_intrinsic_spelling(spelling).map(|entry| entry.identity)
}

/// Q3: somente grafias intrínsecas possuem a política de conflito congelada.
pub fn declaration_conflict_policy(
    _spelling: PublicIntrinsicSpelling,
) -> DeclarationConflictPolicy {
    DeclarationConflictPolicy::DeclarationIsRejected
}

fn authority_entries() -> Vec<PublicIntrinsicSpelling> {
    let mut entries = Vec::new();
    entries.extend(HISTORICAL_PUBLIC_SPELLINGS.iter().copied().map(|spelling| {
        PublicIntrinsicSpelling {
            spelling,
            identity: IntrinsicIdentity::Historical(spelling),
            origin: PublicIntrinsicOrigin::Historical,
        }
    }));
    entries.extend(
        SUPERFICIES_FALIVEIS
            .iter()
            .map(|surface| PublicIntrinsicSpelling {
                spelling: surface.intrinseca,
                identity: IntrinsicIdentity::Fallible(surface.operacao),
                origin: PublicIntrinsicOrigin::Fallible,
            }),
    );
    entries.extend(
        crate::valor_json::ACESSORES
            .iter()
            .copied()
            .map(|spelling| PublicIntrinsicSpelling {
                spelling,
                identity: IntrinsicIdentity::Json(spelling),
                origin: PublicIntrinsicOrigin::Json,
            }),
    );
    entries.extend(crate::sha256::ACESSORES.iter().copied().map(|spelling| {
        PublicIntrinsicSpelling {
            spelling,
            identity: IntrinsicIdentity::Sha256(spelling),
            origin: PublicIntrinsicOrigin::Sha256,
        }
    }));
    entries.extend(EXPORTACOES.iter().map(|export| PublicIntrinsicSpelling {
        spelling: export.membro(),
        identity: family_identity(export.identidade),
        origin: PublicIntrinsicOrigin::FamilyAlias {
            family: export.familia,
        },
    }));
    entries
}

/// Todas as grafias públicas, únicas e em ordem lexicográfica.
pub fn all_public_intrinsic_spellings() -> Vec<PublicIntrinsicSpelling> {
    let mut unique = BTreeMap::new();
    for entry in authority_entries() {
        if let Some(previous) = unique.insert(entry.spelling, entry) {
            debug_assert_eq!(previous.identity, entry.identity);
        }
    }
    unique.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_spelling_never_resolves_to_two_identities() {
        let mut seen = BTreeMap::new();
        for entry in authority_entries() {
            if let Some(previous) = seen.insert(entry.spelling, entry.identity) {
                assert_eq!(
                    previous, entry.identity,
                    "grafia pública ambígua: {}",
                    entry.spelling
                );
            }
        }
    }

    #[test]
    fn authority_is_complete_nonempty_and_classified() {
        assert_eq!(HISTORICAL_PUBLIC_SPELLINGS.len(), 130);
        let spellings = all_public_intrinsic_spellings();
        assert_eq!(spellings.len(), 163);
        assert!(spellings.iter().all(|entry| !entry.spelling.is_empty()));
        assert!(spellings.iter().all(|entry| {
            matches!(
                entry.origin,
                PublicIntrinsicOrigin::Historical
                    | PublicIntrinsicOrigin::Fallible
                    | PublicIntrinsicOrigin::Json
                    | PublicIntrinsicOrigin::Sha256
                    | PublicIntrinsicOrigin::FamilyAlias { .. }
            )
        }));
    }

    #[test]
    fn deliberate_aliases_are_n_to_one() {
        assert_eq!(
            intrinsic_from_public_spelling("ler_bombom"),
            intrinsic_from_public_spelling("ler_arquivo")
        );
        assert_eq!(
            intrinsic_from_public_spelling("sha256"),
            Some(IntrinsicIdentity::Fallible(OperacaoFalivel::HashArquivo))
        );
    }

    #[test]
    fn measured_representatives_and_ordinary_control_are_separated() {
        for spelling in HISTORICAL_PUBLIC_SPELLINGS
            .iter()
            .copied()
            .chain(
                SUPERFICIES_FALIVEIS
                    .iter()
                    .map(|surface| surface.intrinseca),
            )
            .chain(crate::valor_json::ACESSORES)
            .chain(crate::sha256::ACESSORES)
        {
            assert!(
                intrinsic_from_public_spelling(spelling).is_some(),
                "{spelling}"
            );
        }
        assert_eq!(intrinsic_from_public_spelling("minha_funcao_normal"), None);
    }

    #[test]
    fn runtime_and_native_namespaces_are_not_public_spellings() {
        for spelling in [
            "pinker_verso_tamanho",
            "pinker_sha256_verso",
            "pinker_usuario",
            "main",
            "_start",
            "malloc",
            "__pinker_internal_leque_tag",
        ] {
            assert_eq!(intrinsic_from_public_spelling(spelling), None, "{spelling}");
        }
    }
}
