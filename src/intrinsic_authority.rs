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
    ProcessAccessor(&'static str),
}

impl IntrinsicIdentity {
    pub fn canonical_public_spelling(self) -> &'static str {
        match self {
            Self::Historical(spelling)
            | Self::Json(spelling)
            | Self::Sha256(spelling)
            | Self::ProcessAccessor(spelling) => spelling,
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
    ProcessAccessor,
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

/// Grafias históricas que são **alias público** de uma grafia adulta, não
/// identidade semântica própria.
///
/// Autoridade única da relação `alias -> identidade canônica`. A Founder
/// aprovou em #525 as três unificações levantadas pela revisão taxonômica de
/// #505: cada par abaixo já compartilhava semântica, assinatura, modelo de
/// falha e símbolo de runtime, e só permanecia como duas identidades porque
/// `IntrinsicIdentity::Historical` carrega a grafia.
///
/// ```text
/// LEGACY_PUBLIC_SPELLING != DISTINCT_CANONICAL_IDENTITY
/// MULTIPLE_PUBLIC_SPELLINGS -> ONE_CANONICAL_INTRINSIC_IDENTITY
/// ```
///
/// O alias continua público e reconhecido; o que ele deixa de ter é identidade
/// separada.
///
/// Escopo, para não prometer o que o diff não fez: esta tabela é a autoridade da
/// **identidade**. `semantic`, `interpreter` e `backend_s` continuam agrupando os
/// pares por grafia para efeito de despacho e de símbolo de runtime, como já
/// faziam — a #525 não os reescreveu. Quem precisar de identidade deve consultar
/// aqui em vez de reinventar equivalência nominal; consolidar aquele despacho na
/// autoridade central pertence à migração da #505. A gramática de argv continua
/// sendo dita por `runtime/pinker_argv_contract`.
///
/// Cada entrada é `(alias, grafia adulta)`. A grafia adulta nunca é ela mesma
/// um alias, e ambas as grafias precisam existir em
/// [`HISTORICAL_PUBLIC_SPELLINGS`] — as duas condições são verificadas por
/// teste, para que uma quarta equivalência não entre por descuido.
pub const HISTORICAL_CANONICAL_ALIASES: &[(&str, &str)] = &[
    ("argumento_nomeado_ou", "pedir_argumento"),
    ("argumento_nomeado_ou_ambiente_ou", "buscar_contexto"),
    ("tem_argumento_nomeado", "tem_chave"),
];

/// Grafia adulta representada por uma grafia histórica, quando ela é alias.
///
/// `None` significa que a grafia responde por si mesma — não que ela seja
/// desconhecida.
pub fn canonical_alias_target(spelling: &str) -> Option<&'static str> {
    HISTORICAL_CANONICAL_ALIASES
        .iter()
        .find(|(alias, _)| *alias == spelling)
        .map(|(_, canonical)| *canonical)
}

/// Único construtor de identidade histórica: colapsa alias na grafia adulta.
///
/// Toda entrada da superfície histórica passa por aqui, inclusive a que chega
/// por alias de família, para que a relação `grafia -> identidade` tenha uma
/// autoridade só.
fn historical_identity(spelling: &'static str) -> IntrinsicIdentity {
    IntrinsicIdentity::Historical(canonical_alias_target(spelling).unwrap_or(spelling))
}

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
    if let Some(spelling) = crate::saida_processo::ACESSORES
        .iter()
        .copied()
        .find(|candidate| *candidate == spelling)
    {
        return Some(PublicIntrinsicSpelling {
            spelling,
            identity: IntrinsicIdentity::ProcessAccessor(spelling),
            origin: PublicIntrinsicOrigin::ProcessAccessor,
        });
    }
    HISTORICAL_PUBLIC_SPELLINGS
        .iter()
        .copied()
        .find(|candidate| *candidate == spelling)
        .map(|spelling| PublicIntrinsicSpelling {
            spelling,
            identity: historical_identity(spelling),
            origin: PublicIntrinsicOrigin::Historical,
        })
}

/// Identidade real de um membro de módulo.
///
/// O registro de módulos endereça a identidade pela grafia canônica; quem
/// traduz grafia em identidade é esta autoridade, e só ela. Antes da #505 a
/// tradução era feita aqui por `historical_identity`, o que só dava a resposta
/// certa enquanto os módulos exportassem apenas superfície histórica e
/// falível. Com JSON, SHA-256 e acessores de processo dentro de módulos, uma
/// grafia como `json_tipo` produziria `Historical("json_tipo")` de um lado e
/// `Json("json_tipo")` do outro — duas identidades para a mesma grafia.
fn family_identity(identity: IdentidadeCanonica) -> IntrinsicIdentity {
    match identity {
        IdentidadeCanonica::PorGrafia(spelling) => {
            canonical_public_intrinsic_spelling(spelling)
                .expect("grafia canônica de membro registrada na autoridade de intrínsecas")
                .identity
        }
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
    canonical_public_intrinsic_spelling(spelling).is_some()
        || GRAFIAS_BUILTIN_NAO_PUBLICAS.contains(&spelling)
}

/// Membro público de um módulo importável.
///
/// Depois da #505 este é o **único** namespace público de intrínsecas. Ele é
/// endereçado por par, e não por grafia solta: dois módulos podem exportar
/// membros homônimos — `acaso.criar` e `arquivo.criar` são duas identidades
/// diferentes —, e achatá-los numa tabela por grafia era exatamente o modelo
/// global que esta Issue removeu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicIntrinsicMember {
    pub module: &'static str,
    pub member: &'static str,
    pub identity: IntrinsicIdentity,
}

/// `(módulo, membro)` -> identidade, ou ausência.
pub fn public_intrinsic_member(module: &str, member: &str) -> Option<PublicIntrinsicMember> {
    EXPORTACOES
        .iter()
        .find(|export| export.familia == module && export.membro() == member)
        .map(|export| PublicIntrinsicMember {
            module: export.familia,
            member: export.membro(),
            identity: family_identity(export.identidade),
        })
}

/// Toda a superfície pública, em ordem de declaração do registro de módulos.
pub fn all_public_intrinsic_members() -> Vec<PublicIntrinsicMember> {
    EXPORTACOES
        .iter()
        .map(|export| PublicIntrinsicMember {
            module: export.familia,
            member: export.membro(),
            identity: family_identity(export.identidade),
        })
        .collect()
}

/// A grafia é membro público de **algum** módulo?
///
/// Responde sobre o namespace público sem escolher módulo por ela: serve a
/// quem precisa saber que a grafia pertence à superfície de intrínsecas, nunca
/// a quem precisa resolver uma chamada. Resolver exige o par.
pub fn e_membro_publico_de_algum_modulo(spelling: &str) -> bool {
    EXPORTACOES.iter().any(|export| export.membro() == spelling)
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

/// Forma direta de Q1+Q2: grafia canônica para identidade, ou ausência.
///
/// A grafia canônica endereça a identidade; ela deixou de ser chamável sem
/// import quando a #505 removeu a superfície global. Quem resolve uma chamada
/// usa [`public_intrinsic_member`].
pub fn intrinsic_from_public_spelling(spelling: &str) -> Option<IntrinsicIdentity> {
    canonical_public_intrinsic_spelling(spelling).map(|entry| entry.identity)
}

/// Q3: somente grafias intrínsecas possuem a política de conflito congelada.
pub fn declaration_conflict_policy(
    _spelling: PublicIntrinsicSpelling,
) -> DeclarationConflictPolicy {
    DeclarationConflictPolicy::DeclarationIsRejected
}

/// As grafias que endereçam identidade diretamente.
///
/// Membro de módulo não entra aqui: ele é endereçado por par em
/// [`all_public_intrinsic_members`], e misturar os dois namespaces numa lista
/// só é o que fazia `acaso.criar` e `arquivo.criar` colidirem.
fn canonical_authority_entries() -> Vec<PublicIntrinsicSpelling> {
    let mut entries = Vec::new();
    entries.extend(HISTORICAL_PUBLIC_SPELLINGS.iter().copied().map(|spelling| {
        PublicIntrinsicSpelling {
            spelling,
            identity: historical_identity(spelling),
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
    entries.extend(
        crate::saida_processo::ACESSORES
            .iter()
            .copied()
            .map(|spelling| PublicIntrinsicSpelling {
                spelling,
                identity: IntrinsicIdentity::ProcessAccessor(spelling),
                origin: PublicIntrinsicOrigin::ProcessAccessor,
            }),
    );
    entries
}

/// Todas as grafias canônicas, únicas e em ordem lexicográfica.
pub fn all_canonical_intrinsic_spellings() -> Vec<PublicIntrinsicSpelling> {
    let mut unique = BTreeMap::new();
    for entry in canonical_authority_entries() {
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
        for entry in canonical_authority_entries() {
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
        let spellings = all_canonical_intrinsic_spellings();
        // 130 históricas + 9 falíveis + 11 acessores JSON + 1 SHA-256 +
        // 3 acessores de processo, sem interseção entre as cinco listas.
        // Membro de módulo não entra: ele é endereçado por par, não por
        // grafia.
        assert_eq!(spellings.len(), 154);
        assert!(spellings.iter().all(|entry| !entry.spelling.is_empty()));
        assert!(spellings.iter().all(|entry| {
            matches!(
                entry.origin,
                PublicIntrinsicOrigin::Historical
                    | PublicIntrinsicOrigin::Fallible
                    | PublicIntrinsicOrigin::Json
                    | PublicIntrinsicOrigin::Sha256
                    | PublicIntrinsicOrigin::ProcessAccessor
                    | PublicIntrinsicOrigin::FamilyAlias { .. }
            )
        }));
    }

    #[test]
    fn alias_registry_is_structurally_sound() {
        let mut seen = BTreeMap::new();
        for (alias, canonical) in HISTORICAL_CANONICAL_ALIASES {
            assert!(
                HISTORICAL_PUBLIC_SPELLINGS.contains(alias),
                "alias fora da superfície histórica pública: {alias}"
            );
            assert!(
                HISTORICAL_PUBLIC_SPELLINGS.contains(canonical),
                "grafia adulta fora da superfície histórica pública: {canonical}"
            );
            assert_ne!(alias, canonical, "alias não pode apontar para si mesmo");
            assert!(
                canonical_alias_target(canonical).is_none(),
                "grafia adulta {canonical} é ela mesma um alias; a relação precisa ter um nível só"
            );
            assert!(
                seen.insert(*alias, *canonical).is_none(),
                "alias declarado duas vezes: {alias}"
            );
        }
    }

    #[test]
    fn founder_unifications_are_the_only_historical_collapses() {
        // #525 unifica exatamente três pares. Uma quarta equivalência entrando
        // por descuido quebra aqui antes de chegar a qualquer consumidor.
        assert_eq!(HISTORICAL_CANONICAL_ALIASES.len(), 3);
        assert_eq!(
            HISTORICAL_CANONICAL_ALIASES,
            &[
                ("argumento_nomeado_ou", "pedir_argumento"),
                ("argumento_nomeado_ou_ambiente_ou", "buscar_contexto"),
                ("tem_argumento_nomeado", "tem_chave"),
            ]
        );

        let mut historical_identities = BTreeMap::new();
        for spelling in HISTORICAL_PUBLIC_SPELLINGS.iter().copied() {
            let identity = historical_identity(spelling);
            let IntrinsicIdentity::Historical(canonical) = identity else {
                panic!("grafia histórica {spelling} produziu identidade não histórica");
            };
            historical_identities
                .entry(canonical)
                .or_insert_with(Vec::new)
                .push(spelling);
        }
        let collapsed: Vec<_> = historical_identities
            .iter()
            .filter(|(_, spellings)| spellings.len() > 1)
            .map(|(canonical, spellings)| (*canonical, spellings.clone()))
            .collect();
        assert_eq!(
            collapsed,
            vec![
                (
                    "buscar_contexto",
                    vec!["argumento_nomeado_ou_ambiente_ou", "buscar_contexto"]
                ),
                (
                    "pedir_argumento",
                    vec!["argumento_nomeado_ou", "pedir_argumento"]
                ),
                ("tem_chave", vec!["tem_argumento_nomeado", "tem_chave"]),
            ]
        );
        assert_eq!(
            historical_identities.len(),
            HISTORICAL_PUBLIC_SPELLINGS.len() - HISTORICAL_CANONICAL_ALIASES.len()
        );
    }

    #[test]
    fn deliberate_aliases_are_n_to_one() {
        // O membro é endereçado pelo par, e resolve para a mesma identidade
        // que a grafia canônica endereça.
        assert_eq!(
            public_intrinsic_member("arquivo", "ler_bombom").map(|entry| entry.identity),
            intrinsic_from_public_spelling("ler_arquivo")
        );
        assert_eq!(
            public_intrinsic_member("integridade", "sha256_arquivo").map(|entry| entry.identity),
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
            .chain(crate::saida_processo::ACESSORES)
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
