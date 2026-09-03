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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// #532 — quem é o callee de um call site, decidido UMA vez.
///
/// A grafia canônica de uma intrínseca é texto de diagnóstico e de sintaxe. Ela
/// era também a autoridade executiva: `semantic`, `ir`, `interpreter` e
/// `backend_s` perguntavam, cada um por conta própria, "este nome está na minha
/// tabela de intrínsecas?", e a resposta valia mesmo quando o nome era de uma
/// função do usuário.
///
/// ```text
/// SYMBOL_NAME        != SYMBOL_IDENTITY
/// TEXTUAL_SPELLING   != INTRINSIC_IDENTITY
/// ```
///
/// Esta é a decisão que a resolução produz e que todas as camadas a jusante
/// consomem em vez de reconstruir. Ela não é derivável do texto do usuário: só
/// a canonicalização de um import — `trazer M;` ou `trazer M.x;` — pode
/// produzir [`CalleeIdentity::Intrinsic`], e nada no programa do usuário pode
/// produzir [`CalleeIdentity::CompilerInternal`].
///
/// A grafia canônica continua viajando DENTRO da identidade, e é ela que
/// escolhe QUAL intrínseca é. O que ela deixou de fazer é decidir SE a chamada
/// é intrínseca — e é essa segunda pergunta que competia com o usuário.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CalleeIdentity {
    /// Função declarada por alguma unidade-fonte. Nenhuma tabela de intrínseca
    /// responde por ela.
    #[default]
    User,
    /// Intrínseca resolvida pela autoridade a partir de `(módulo, membro)`.
    Intrinsic(IntrinsicIdentity),
    /// Operação materializada pelo próprio compilador (`__pinker_internal_*`,
    /// `__ternario`). Não tem grafia de usuário possível — `native_symbol`
    /// recusa o namespace na declaração —, então não disputa nome com ninguém.
    CompilerInternal,
}

impl CalleeIdentity {
    /// A chamada é de função do usuário?
    pub fn is_user(self) -> bool {
        matches!(self, Self::User)
    }

    /// A chamada pode ser atendida pelas tabelas de builtin das camadas a
    /// jusante?
    ///
    /// É o único predicado que `interpreter`, `backend_s` e os validadores
    /// precisam para não capturar função de usuário.
    pub fn dispatches_as_builtin(self) -> bool {
        !self.is_user()
    }

    /// Grafia canônica da intrínseca, quando há uma.
    ///
    /// Serve para ESCOLHER entre intrínsecas, nunca para descobrir que a
    /// chamada é intrínseca.
    pub fn canonical_spelling(self) -> Option<&'static str> {
        match self {
            Self::Intrinsic(identity) => Some(identity.canonical_public_spelling()),
            Self::User | Self::CompilerInternal => None,
        }
    }
}

/// Identidade de um callee escrito como identificador simples.
///
/// Um identificador de fonte é do USUÁRIO. A única exceção é a identidade que o
/// próprio compilador materializa (`__pinker_internal_*`, `__ternario`, ...):
/// `native_symbol` recusa esse namespace na fronteira léxica, então nenhum
/// programa pode produzi-lo, e as camadas a jusante continuam podendo atendê-lo
/// pelas suas tabelas internas.
///
/// A pergunta é de `native_symbol::is_compiler_generated`, e NÃO de
/// `starts_with("__")`: `__usuario` é nome de usuário legal.
pub fn callee_identity_de_ident(name: &str) -> CalleeIdentity {
    if crate::native_symbol::is_compiler_generated(name) {
        CalleeIdentity::CompilerInternal
    } else {
        CalleeIdentity::User
    }
}

/// Identidade de uma grafia que a canonicalização já resolveu como intrínseca.
///
/// Ponto único em que uma grafia canônica vira [`CalleeIdentity::Intrinsic`].
/// Quem chama já provou que a grafia veio de um import resolvido, nunca do
/// texto cru do usuário.
pub fn callee_identity_da_grafia_canonica(spelling: &str) -> CalleeIdentity {
    match intrinsic_from_public_spelling(spelling) {
        Some(identity) => CalleeIdentity::Intrinsic(identity),
        // Grafia interna do compilador (`__pinker_internal_*`, `__ternario`) e
        // formas monomórficas que a IR materializa fora da superfície pública.
        None => CalleeIdentity::CompilerInternal,
    }
}

/// #532 — tabela consultada pela identidade do callee, não só pela grafia.
///
/// Os validadores de IR, CFG, seleção e máquina mantêm cada um a sua tabela de
/// assinaturas, e todas misturavam num mapa só as funções do programa e as
/// intrínsecas, endereçadas pela mesma chave textual. Com a grafia canônica
/// liberada para o usuário, esse mapa passa a poder ter duas respostas para a
/// mesma chave, e quem escolhe é a identidade já decidida.
///
/// A tabela não decide nada: ela apenas obedece à decisão que recebe.
#[derive(Debug, Default)]
pub struct TabelaPorIdentidade<T> {
    /// Assinaturas das funções declaradas pelo programa.
    pub usuario: std::collections::HashMap<String, T>,
    /// Assinaturas das intrínsecas e das operações internas do compilador.
    pub intrinsecas: std::collections::HashMap<String, T>,
}

impl<T> TabelaPorIdentidade<T> {
    /// Assinatura do callee segundo a identidade que a resolução decidiu.
    ///
    /// Callee de usuário só enxerga o programa. Callee builtin consulta a
    /// tabela de intrínsecas e, se ela não responder, recai no programa — é por
    /// aí que as identidades geradas pelo compilador (`__gen_*`, `__impl_*`)
    /// continuam encontrando a própria assinatura.
    pub fn resolver(&self, identidade: CalleeIdentity, callee: &str) -> Option<&T> {
        if identidade.is_user() {
            return self.usuario.get(callee);
        }
        self.intrinsecas
            .get(callee)
            .or_else(|| self.usuario.get(callee))
    }

    /// A grafia existe em alguma das duas tabelas?
    pub fn contem_grafia(&self, callee: &str) -> bool {
        self.intrinsecas.contains_key(callee) || self.usuario.contains_key(callee)
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

/// Grafias da superfície histórica, em ordem estável.
///
/// A lista literal que morava aqui era a chave de sete tabelas de fase que a
/// repetiam. Desde a consolidação C1 ela é a chave de
/// [`crate::intrinsics::registry`], que declara também aridade, contrato de
/// parâmetros, contrato de retorno e roteamento de runtime. Esta autoridade
/// continua dona da IDENTIDADE — o que a grafia significa e qual alias colapsa
/// em qual grafia adulta —, e consome a enumeração em vez de manter a sua.
fn historical_public_spellings() -> impl Iterator<Item = &'static str> + Clone {
    crate::intrinsics::registry::grafias()
}

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
/// [`crate::intrinsics::registry`] — as duas condições são verificadas por
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
    historical_public_spellings()
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
/// **Vazia desde a #532.** `mapa_criar` era a última entrada: criação genérica
/// de mapa chamável sem import enquanto classificada como não-pública. Ela
/// passou a ser `mapa.criar`, pela mesma tradução que `lista_criar` ->
/// `lista.criar` já tinha recebido na #505, e com isso
/// `GLOBAL_CALLABLE_BUILTIN_EXCEPTIONS = 0`.
///
/// A lista permanece como fronteira declarada, não como lugar de despejo: quem
/// consome é a resolução modular, que precisa distinguir "grafia builtin" de
/// "entidade declarada por alguma unidade". O teste de deriva em
/// `tests/issue_514_module_composition_tests.rs` recusa qualquer grafia builtin
/// de `src/semantic.rs` que esta autoridade não reconheça — é ele que impede a
/// lista de voltar a crescer em silêncio.
const GRAFIAS_BUILTIN_NAO_PUBLICAS: &[&str] = &[];

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
    entries.extend(
        historical_public_spellings().map(|spelling| PublicIntrinsicSpelling {
            spelling,
            identity: historical_identity(spelling),
            origin: PublicIntrinsicOrigin::Historical,
        }),
    );
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
        assert_eq!(historical_public_spellings().count(), 131);
        let spellings = all_canonical_intrinsic_spellings();
        // 131 históricas + 9 falíveis + 11 acessores JSON + 1 SHA-256 +
        // 3 acessores de processo, sem interseção entre as cinco listas.
        // Membro de módulo não entra: ele é endereçado por par, não por
        // grafia.
        assert_eq!(spellings.len(), 155);
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
                historical_public_spellings().any(|grafia| grafia == *alias),
                "alias fora da superfície histórica pública: {alias}"
            );
            assert!(
                historical_public_spellings().any(|grafia| grafia == *canonical),
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
        for spelling in historical_public_spellings() {
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
            historical_public_spellings().count() - HISTORICAL_CANONICAL_ALIASES.len()
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
        // O nome público da superfície falível vem da autoridade que o
        // declara, e não é reescrito aqui: `falha_operacional` é o único lugar
        // do `src/` onde essa grafia pode aparecer.
        let hash_arquivo =
            crate::falha_operacional::superficie_por_operacao(OperacaoFalivel::HashArquivo)
                .expect("superfície falível registrada")
                .intrinseca;
        assert_eq!(
            public_intrinsic_member("integridade", hash_arquivo).map(|entry| entry.identity),
            Some(IntrinsicIdentity::Fallible(OperacaoFalivel::HashArquivo))
        );
    }

    #[test]
    fn measured_representatives_and_ordinary_control_are_separated() {
        for spelling in historical_public_spellings()
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
