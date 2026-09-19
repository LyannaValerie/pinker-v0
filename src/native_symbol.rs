//! Autoridade única da fronteira nativa de símbolos.
//!
//! Três decisões que antes viviam espalhadas como literais independentes no
//! backend passam a ter um dono só:
//!
//! - qual identidade Pinker produz qual símbolo nativo (entrypoint);
//! - qual ligação ELF cada definição emitida recebe (`STB_LOCAL`/`STB_GLOBAL`);
//! - quais namespaces pertencem à Pinker e são recusados cedo na fonte.
//!
//! O módulo também concentra o encoding injetivo dos rótulos locais gerados,
//! que antes era concatenação textual crua e podia definir o mesmo rótulo duas
//! vezes num programa válido.
//!
//! O que este módulo **não** faz: não mangla nomes de usuário, não conhece
//! libc, não decide política de intrínsecas públicas e não fala sobre
//! visibilidade dinâmica (`STV_HIDDEN`), que governa exportação e não resolve
//! captura no link estático.

// @pinker-nav:start native.symbol.entrypoint
// @pinker-nav:domain identity
// @pinker-nav:layer native
// @pinker-nav:summary Explicit authority of the entrypoint: ENTRYPOINT_SOURCE_IDENTITY (`principal`) is the only source identity that produces a platform symbol, ENTRYPOINT_NATIVE_SYMBOL (`main`) is the symbol of the assemblable surface and FREESTANDING_ENTRYPOINT_SYMBOL (`_start`) that of the free surface. NativeSurface explicitly models the deliberate difference between the assemblable surface, where the identity becomes an ABI symbol, and the annotative textual surface `pinker.text.v0`, which preserves the Pinker spelling. `function_symbol` is the only point that answers `principal -> main`; `is_entrypoint` is the only point that recognizes the entrypoint's identity.
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Identidade Pinker do entrypoint na fonte. Não muda a sintaxe de `principal`.
pub const ENTRYPOINT_SOURCE_IDENTITY: &str = "principal";

/// Símbolo de plataforma do entrypoint na superfície montável (ABI C).
pub const ENTRYPOINT_NATIVE_SYMBOL: &str = "main";

/// Símbolo de boot do entrypoint na superfície livre (freestanding).
pub const FREESTANDING_ENTRYPOINT_SYMBOL: &str = "_start";

/// Superfície de renderização. A diferença entre as duas é deliberada e
/// modelada aqui, e não um acidente de literais repetidos em dois renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSurface {
    /// `.s` montável entregue à toolchain externa (`pink build`, `--nativo`).
    /// A identidade Pinker vira símbolo de ABI: `principal` produz `main`.
    Assemblable,
    /// `.s` textual `pinker.text.v0`. Superfície anotativa: nada é montado ou
    /// ligado a partir dela, então a grafia Pinker é preservada como está.
    TextualAbi,
}

/// `true` somente para a identidade de fonte do entrypoint.
pub fn is_entrypoint(source_name: &str) -> bool {
    source_name == ENTRYPOINT_SOURCE_IDENTITY
}

/// Símbolo nativo de uma função, por superfície.
///
/// Único ponto do compilador que decide `principal -> main`.
pub fn function_symbol(surface: NativeSurface, source_name: &str) -> String {
    match surface {
        NativeSurface::Assemblable if is_entrypoint(source_name) => {
            ENTRYPOINT_NATIVE_SYMBOL.to_string()
        }
        _ => source_name.to_string(),
    }
}
// @pinker-nav:end native.symbol.entrypoint

// @pinker-nav:start native.symbol.wiring
// @pinker-nav:domain abi
// @pinker-nav:layer native
// @pinker-nav:summary Single authority over the linkage of the definitions emitted by the program object: NativeDefinition classifies the definition (entrypoint, user function, compiler-generated function, `eterno` global, backend local helper) and `native_binding` answers LOCAL or GLOBAL per class. Only the entrypoint is GLOBAL, because it is the object's only definition consumed from outside (by the CRT); everything else is STB_LOCAL and therefore stops satisfying external runtime references that should go to the host. `NativeBinding::directive` is the only producer of `.globl`/`.local`; `.hidden` is not used, because STV_HIDDEN governs dynamic export and does not prevent capture at static link time.

/// Ligação ELF de uma definição emitida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeBinding {
    /// `STB_LOCAL`. Não atravessa a unidade de link e não pode satisfazer
    /// referência externa de outro objeto ou archive.
    Local,
    /// `STB_GLOBAL`. Reservado às definições realmente consumidas de fora.
    Global,
}

impl NativeBinding {
    /// Diretiva GAS correspondente. Único produtor de `.globl`/`.local`.
    pub fn directive(self, symbol: &str) -> String {
        match self {
            Self::Local => format!(".local {symbol}"),
            Self::Global => format!(".globl {symbol}"),
        }
    }
}

/// Classe de definição emitida no objeto do programa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDefinition {
    /// `principal`, que produz o símbolo de plataforma.
    Entrypoint,
    /// Função top-level escrita pelo usuário que não é o entrypoint.
    UserFunction,
    /// Função sintetizada pelo compilador (`__impl_`, `__gen_`,
    /// `__anon_carinho_`, `__fnref_env_`, ...).
    GeneratedFunction,
    /// Global `eterno`.
    UserGlobal,
    /// Helper emitido pelo próprio backend (adapter de trato, implementação
    /// local de intrínseca no subset hospedado).
    BackendHelper,
}

/// Classifica uma função pelo nome de identidade já materializado.
///
/// A identidade gerada não é reconhecida por superprefixo: quem responde é a
/// mesma autoridade canônica que reserva os namespaces, via
/// [`is_compiler_generated`].
pub fn classify_function(source_name: &str) -> NativeDefinition {
    if is_entrypoint(source_name) {
        NativeDefinition::Entrypoint
    } else if is_compiler_generated(source_name) {
        NativeDefinition::GeneratedFunction
    } else {
        NativeDefinition::UserFunction
    }
}

/// Ligação de uma definição emitida. Ponto testável exigido por R1.
pub fn native_binding(definition: NativeDefinition) -> NativeBinding {
    match definition {
        // Consumido pelo CRT: é a única definição do objeto que precisa
        // atravessar a unidade de link.
        NativeDefinition::Entrypoint => NativeBinding::Global,
        // Nenhum consumidor externo demonstrado no produto atual (#496).
        NativeDefinition::UserFunction
        | NativeDefinition::GeneratedFunction
        | NativeDefinition::UserGlobal
        | NativeDefinition::BackendHelper => NativeBinding::Local,
    }
}

/// Ligação da definição de uma função, a partir do nome de identidade.
pub fn function_binding(source_name: &str) -> NativeBinding {
    native_binding(classify_function(source_name))
}
// @pinker-nav:end native.symbol.wiring

// @pinker-nav:start native.symbol.reserved-namespace
// @pinker-nav:domain identifiers
// @pinker-nav:layer native
// @pinker-nav:summary Targeted reservation of the namespaces Pinker actually owns, derived from a single table: the nineteen forms of synthetic identity the compiler really materializes (seventeen prefixes, from `__pinker_internal_` to `__propagar_falha_`, and two exact names, `__env` and `__ternario`), the `pinker_` prefix (symbols defined and consumed by `libpinker_rt.a`) and the platform entrypoint symbols `main` and `_start`. The reservation is of the form actually owned, not of the superprefix common to it: `__` remains free, so `__usuario` is a legal Pinker name, and an `Exact` entry never becomes a `Prefix` out of convenience — `__env` is reserved and `__envio` is not. Each entry declares its owner (`NamespaceOwner`) and the exact boundary at which it is applied — `AnyIdentifier` at the source's lexical boundary, `SymbolDefinition` at the symbol-producing definition boundary — and each boundary consults only the entries of its scope, because `main` is a legitimate package name and the synthetic identities are created by the compiler itself after the lexer. `is_compiler_generated` is the only point that recognizes a generated identity, and `classify_function` consumes it instead of testing a prefix on its own. Host names (`malloc`, `memcpy`, `write`, `getenv`, `free`, `environ`, ...) are NOT reserved: they remain legal as Pinker names and are isolated by STB_LOCAL.

/// Prefixo histórico das intrínsecas internas materializadas pelo lowering.
/// Continua reservado; hoje é uma das dezenove formas da tabela canônica.
pub const COMPILER_INTERNAL_PREFIX: &str = "__pinker_internal_";

/// Prefixo do namespace ABI do runtime nativo.
pub const RUNTIME_ABI_PREFIX: &str = "pinker_";

/// Quem, dentro da Pinker, materializa nomes num namespace reservado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceOwner {
    /// O compilador materializa identidades sintéticas nesta forma.
    CompilerGenerated,
    /// O runtime nativo define e consome estes símbolos em `libpinker_rt.a`.
    RuntimeAbi,
    /// A plataforma consome este símbolo de entrypoint.
    PlatformEntrypoint,
}

/// Fronteira em que uma reserva é aplicada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservedScope {
    /// Toda posição de identificador originado da fonte (fronteira do lexer).
    AnyIdentifier,
    /// Definição top-level que produz um símbolo nativo: `carinho` e `eterno`.
    SymbolDefinition,
}

/// Forma da reserva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservedShape {
    Prefix(&'static str),
    Exact(&'static str),
}

impl ReservedShape {
    fn matches(self, name: &str) -> bool {
        match self {
            Self::Prefix(prefix) => name.starts_with(prefix),
            Self::Exact(exact) => name == exact,
        }
    }

    /// Como o namespace aparece no diagnóstico.
    pub fn rendered(self) -> String {
        match self {
            Self::Prefix(prefix) => format!("usa o prefixo '{prefix}', reservado à Pinker"),
            Self::Exact(exact) => format!("é o símbolo '{exact}', reservado à Pinker"),
        }
    }
}

/// Namespace possuído pela Pinker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinkerOwnedNamespace {
    pub shape: ReservedShape,
    pub scope: ReservedScope,
    /// Quem materializa nomes neste espaço.
    pub owner: NamespaceOwner,
    /// Por que a Pinker é dona deste espaço.
    pub reason: &'static str,
}

/// Tabela canônica. Cobre exatamente o que a Pinker produz: compilador,
/// runtime e entrypoint. Não congela lista de libc.
pub const PINKER_OWNED_NAMESPACES: &[PinkerOwnedNamespace] = &[
    // Identidades sintéticas do compilador. Cada entrada é uma forma que o
    // compilador de fato materializa; o superprefixo `__` que todas
    // compartilham NÃO é reservado, porque a Pinker não o possui.
    // `__gen_leque_` precede `__gen_` só para o diagnóstico nomear a família
    // exata; a reserva seria a mesma em qualquer ordem.
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix(COMPILER_INTERNAL_PREFIX),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o lowering materializa as intrínsecas internas de leque e mapa sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix(crate::anonymous_identity::ANONYMOUS_CALLABLE_PREFIX),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa cada `carinho` anônimo sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__impl_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa cada método de `trato` implementado sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__trait_default_check_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa a checagem de método padrão de `trato` sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__gen_leque_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "a identidade genérica materializa cada `leque` monomorfizado sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__gen_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "a identidade genérica materializa cada função monomorfizada sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__fnref_env_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o IR materializa o wrapper de referência a função top-level sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__fnparam_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa cada especialização de callback estático sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__iter_lista_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de lista da iteração sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__iter_mapa_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de mapa da iteração sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__iter_indice_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de índice da iteração sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__iter_tamanho_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de tamanho da iteração sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__iter_cursor_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de cursor da iteração sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__range_limite_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de limite do intervalo sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__tentar_alvo_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o alvo de `tentar` sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__propagar_alvo_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o alvo da propagação de falha sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix("__propagar_falha_"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "o parser materializa o slot de falha da propagação sob este prefixo",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Exact("__env"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "é o parâmetro oculto de ambiente que o IR injeta em cada closure",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Exact("__ternario"),
        scope: ReservedScope::AnyIdentifier,
        owner: NamespaceOwner::CompilerGenerated,
        reason: "é a chamada sintética que o lowering emite para a escolha ternária",
    },
    // Runtime e plataforma.
    PinkerOwnedNamespace {
        shape: ReservedShape::Prefix(RUNTIME_ABI_PREFIX),
        scope: ReservedScope::SymbolDefinition,
        owner: NamespaceOwner::RuntimeAbi,
        reason: "o runtime nativo define e consome os símbolos deste prefixo em 'libpinker_rt.a'",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Exact(ENTRYPOINT_NATIVE_SYMBOL),
        scope: ReservedScope::SymbolDefinition,
        owner: NamespaceOwner::PlatformEntrypoint,
        reason: "é o símbolo de plataforma produzido exclusivamente por 'principal'",
    },
    PinkerOwnedNamespace {
        shape: ReservedShape::Exact(FREESTANDING_ENTRYPOINT_SYMBOL),
        scope: ReservedScope::SymbolDefinition,
        owner: NamespaceOwner::PlatformEntrypoint,
        reason: "é o símbolo de boot produzido exclusivamente por 'principal' no modo livre",
    },
];

/// Consulta única da reserva.
///
/// Cada entrada é aplicada em exatamente uma fronteira, e a fronteira é parte
/// da política: as formas geradas já são recusadas quando o texto da fonte
/// vira `Ident`, então todo nome dessas formas que chega à fronteira de
/// definição foi criado pelo próprio compilador; e `main` é nome legítimo de
/// pacote, então só pode ser recusado onde de fato produziria um símbolo.
pub fn reserved_namespace(name: &str, scope: ReservedScope) -> Option<PinkerOwnedNamespace> {
    namespaces_possuidos().find(|entry| entry.scope == scope && entry.shape.matches(name))
}

/// A tabela canônica, mais — só no build de teste — a grafia contrafactual do
/// contrafactual de U-05. A entrada de prova existe para que renomear o
/// namespace anônimo não vire, sem querer, um teste sobre namespace NÃO
/// reservado: sem ela a renomeação mudaria duas coisas ao mesmo tempo e uma
/// falha não distinguiria as causas. Em produção o iterador é a tabela.
fn namespaces_possuidos() -> impl Iterator<Item = PinkerOwnedNamespace> {
    let possuidos = PINKER_OWNED_NAMESPACES.iter().copied();
    #[cfg(test)]
    let possuidos = possuidos.chain(NAMESPACES_DE_PROVA.iter().copied());
    possuidos
}

#[cfg(test)]
const NAMESPACES_DE_PROVA: &[PinkerOwnedNamespace] = &[PinkerOwnedNamespace {
    shape: ReservedShape::Prefix(crate::anonymous_identity::PREFIXO_CONTRAFACTUAL),
    scope: ReservedScope::AnyIdentifier,
    owner: NamespaceOwner::CompilerGenerated,
    reason: "é a grafia contrafactual do callable anônimo, existente só no build de teste",
}];

/// `true` quando o nome pertence a uma forma que o compilador materializa.
///
/// Único ponto que reconhece identidade gerada. Não é `starts_with("__")`: o
/// superprefixo comum às famílias não é propriedade da Pinker, e um nome de
/// usuário como `__usuario` não é identidade gerada.
pub fn is_compiler_generated(name: &str) -> bool {
    namespaces_possuidos()
        .any(|entry| entry.owner == NamespaceOwner::CompilerGenerated && entry.shape.matches(name))
}

/// Mensagem única do diagnóstico de namespace reservado.
pub fn reserved_namespace_message(name: &str, namespace: PinkerOwnedNamespace) -> String {
    format!(
        "E-SEMANTIC-RESERVED-NAMESPACE\nidentificador '{name}' {}: {}",
        namespace.shape.rendered(),
        namespace.reason
    )
}
// @pinker-nav:end native.symbol.reserved-namespace

// @pinker-nav:start native.symbol.injective-label
// @pinker-nav:domain rendering
// @pinker-nav:layer native
// @pinker-nav:summary Injective encoding of the generated local labels, by the same principle already used by the generic identity and by the vtable symbol: each component enters with a byte-length prefix, so that the concatenation is recoverable and `components(A) != components(B)` implies `encode(A) != encode(B)`. `injective_local_label` produces the label and `decode_injective_local_label` recovers the components — recoverability is the proof of injectivity, not a convenience. It replaces the textual concatenation `.L{fn}_{label}`, which collapsed `('f','loop_join_1')` and `('f_loop','join_1')` into the same `.Lf_loop_join_1`.

/// Prefixo dos rótulos locais injetivos emitidos pelo backend.
pub const INJECTIVE_LOCAL_LABEL_PREFIX: &str = ".Lp";

/// Rótulo local injetivo para uma sequência de componentes estruturais.
///
/// Cada componente é emitido como `<comprimento em bytes>_<componente>`. A
/// concatenação de componentes prefixados por comprimento é injetiva, e o
/// resultado continua legível o suficiente para inspeção manual do `.s`.
pub fn injective_local_label(components: &[&str]) -> String {
    let mut label = String::from(INJECTIVE_LOCAL_LABEL_PREFIX);
    for component in components {
        write!(&mut label, "{}_{}", component.len(), component)
            .expect("escrita em String não falha");
    }
    label
}

/// Recupera os componentes de um rótulo produzido por
/// [`injective_local_label`]. Devolve `None` para qualquer texto que não seja
/// um rótulo bem formado desta autoridade.
pub fn decode_injective_local_label(label: &str) -> Option<Vec<String>> {
    let mut rest = label.strip_prefix(INJECTIVE_LOCAL_LABEL_PREFIX)?;
    let mut components = Vec::new();
    while !rest.is_empty() {
        let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 {
            return None;
        }
        let len: usize = rest[..digits].parse().ok()?;
        rest = rest[digits..].strip_prefix('_')?;
        if rest.len() < len || !rest.is_char_boundary(len) {
            return None;
        }
        let (component, tail) = rest.split_at(len);
        components.push(component.to_string());
        rest = tail;
    }
    Some(components)
}
// @pinker-nav:end native.symbol.injective-label

// @pinker-nav:start native.symbol.emitted-set
// @pinker-nav:domain validation
// @pinker-nav:layer native
// @pinker-nav:summary Verification of the set the renderer is about to emit, before handing the `.s` to the external toolchain (R2). `EmittedDefinitions` records each definition as a `(symbol, identity that produced it)` pair: two distinct identities on the same symbol are a collision and become a deterministic Pinker diagnostic — ordered by BTreeMap, never by HashMap order — while the same identity repeated on the same symbol is deliberate many-to-one and remains legal. It closes GNU as's raw error class ('symbol already defined') for the collisions the compiler can already know about.

/// Colisão entre duas identidades distintas que renderizam para a mesma
/// definição emitida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCollision {
    pub symbol: String,
    pub first_identity: String,
    pub second_identity: String,
}

/// Conjunto de definições que o renderer vai emitir.
#[derive(Debug, Default)]
pub struct EmittedDefinitions {
    // BTreeMap: a ordem do diagnóstico não pode depender de hashing.
    entries: BTreeMap<String, String>,
    collisions: Vec<EmittedCollision>,
}

impl EmittedDefinitions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra uma definição emitida e a identidade Pinker que a produziu.
    pub fn define(&mut self, symbol: &str, identity: &str) {
        match self.entries.get(symbol) {
            // Muitos-para-um deliberado da mesma identidade: legal.
            Some(previous) if previous == identity => {}
            Some(previous) => self.collisions.push(EmittedCollision {
                symbol: symbol.to_string(),
                first_identity: previous.clone(),
                second_identity: identity.to_string(),
            }),
            None => {
                self.entries
                    .insert(symbol.to_string(), identity.to_string());
            }
        }
    }

    /// Primeira colisão em ordem determinística, se houver.
    pub fn first_collision(&self) -> Option<&EmittedCollision> {
        self.collisions
            .iter()
            .min_by(|a, b| (&a.symbol, &a.second_identity).cmp(&(&b.symbol, &b.second_identity)))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Mensagem única do diagnóstico de colisão do conjunto emitido.
pub fn emitted_collision_message(collision: &EmittedCollision) -> String {
    format!(
        "E-BACKEND-SYMBOL-COLLISION\nas identidades '{}' e '{}' renderizam para a mesma definição nativa '{}'",
        collision.first_identity, collision.second_identity, collision.symbol
    )
}
// @pinker-nav:end native.symbol.emitted-set

// @pinker-nav:start evidence.native.symbol
// @pinker-nav:domain identity
// @pinker-nav:layer evidence
// @pinker-nav:summary Local evidence of the native symbol authority: it fixes `principal -> main` only on the assemblable surface, linkage by class, the exact scope of each reserved namespace, the injectivity and recoverability of the label encoding (including the historical `f`/`f_loop` pair of F-04) and the separation between many-to-one of the same identity and a collision between distinct identities.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrypoint_e_a_unica_identidade_que_produz_main() {
        assert_eq!(
            function_symbol(NativeSurface::Assemblable, ENTRYPOINT_SOURCE_IDENTITY),
            ENTRYPOINT_NATIVE_SYMBOL
        );
        assert_eq!(
            function_symbol(NativeSurface::Assemblable, "somar"),
            "somar"
        );
        // A superfície textual é anotativa: preserva a grafia Pinker.
        assert_eq!(
            function_symbol(NativeSurface::TextualAbi, ENTRYPOINT_SOURCE_IDENTITY),
            ENTRYPOINT_SOURCE_IDENTITY
        );
    }

    #[test]
    fn ligacao_por_classe() {
        assert_eq!(
            native_binding(NativeDefinition::Entrypoint),
            NativeBinding::Global
        );
        for definition in [
            NativeDefinition::UserFunction,
            NativeDefinition::GeneratedFunction,
            NativeDefinition::UserGlobal,
            NativeDefinition::BackendHelper,
        ] {
            assert_eq!(native_binding(definition), NativeBinding::Local);
        }
        assert_eq!(
            NativeBinding::Local.directive("malloc"),
            ".local malloc",
            "STB_LOCAL é emitido como .local; .hidden não resolve captura em link estático"
        );
        assert_eq!(NativeBinding::Global.directive("main"), ".globl main");
    }

    #[test]
    fn nomes_do_host_continuam_legais_na_pinker() {
        for host in ["malloc", "memcpy", "write", "getenv", "free", "environ"] {
            assert!(reserved_namespace(host, ReservedScope::AnyIdentifier).is_none());
            assert!(reserved_namespace(host, ReservedScope::SymbolDefinition).is_none());
            assert_eq!(function_binding(host), NativeBinding::Local);
        }
    }

    #[test]
    fn namespaces_reservados_valem_na_fronteira_declarada() {
        // Forma gerada real: recusada em qualquer posição de identificador.
        assert!(reserved_namespace("__impl_x", ReservedScope::AnyIdentifier).is_some());
        // `main` é nome legítimo de pacote: só é recusado onde produz símbolo.
        assert!(reserved_namespace("main", ReservedScope::AnyIdentifier).is_none());
        assert!(reserved_namespace("main", ReservedScope::SymbolDefinition).is_some());
        assert!(reserved_namespace("_start", ReservedScope::SymbolDefinition).is_some());
        assert!(reserved_namespace("pinker_rt_iniciar", ReservedScope::SymbolDefinition).is_some());
        // O entrypoint em si nunca é reservado contra o usuário.
        assert!(
            reserved_namespace(ENTRYPOINT_SOURCE_IDENTITY, ReservedScope::SymbolDefinition)
                .is_none()
        );
    }

    #[test]
    fn a_reserva_e_da_forma_possuida_e_nao_do_superprefixo() {
        // O superprefixo comum às famílias não pertence à Pinker.
        for livre in ["__usuario", "__coisa", "__abc123", "__", "___", "__x"] {
            assert!(
                reserved_namespace(livre, ReservedScope::AnyIdentifier).is_none(),
                "'{livre}' não pertence a nenhuma família realmente gerada"
            );
            assert!(!is_compiler_generated(livre));
            assert_eq!(classify_function(livre), NativeDefinition::UserFunction);
            assert_eq!(function_binding(livre), NativeBinding::Local);
        }
    }

    #[test]
    fn entrada_exata_nunca_vira_prefixo() {
        for exato in ["__env", "__ternario"] {
            assert!(reserved_namespace(exato, ReservedScope::AnyIdentifier).is_some());
        }
        // Extensões da grafia exata continuam livres.
        for extensao in ["__envio", "__env_", "__ternarios", "__ternario_x"] {
            assert!(
                reserved_namespace(extensao, ReservedScope::AnyIdentifier).is_none(),
                "'{extensao}' estende um nome exato e não é forma possuída"
            );
        }
    }

    #[test]
    fn classificacao_de_gerada_consome_a_autoridade_canonica() {
        for entry in PINKER_OWNED_NAMESPACES {
            if entry.owner != NamespaceOwner::CompilerGenerated {
                continue;
            }
            let representante = match entry.shape {
                ReservedShape::Prefix(prefix) => format!("{prefix}amostra"),
                ReservedShape::Exact(exact) => exact.to_string(),
            };
            assert!(is_compiler_generated(&representante), "{representante}");
            assert_eq!(
                classify_function(&representante),
                NativeDefinition::GeneratedFunction,
                "{representante}"
            );
            assert_eq!(entry.scope, ReservedScope::AnyIdentifier);
        }
    }

    #[test]
    fn rotulo_injetivo_separa_o_caso_historico_da_f04() {
        let a = injective_local_label(&["f", "loop_join_1"]);
        let b = injective_local_label(&["f_loop", "join_1"]);
        assert_ne!(a, b);
        assert_eq!(
            decode_injective_local_label(&a).unwrap(),
            vec!["f".to_string(), "loop_join_1".to_string()]
        );
        assert_eq!(
            decode_injective_local_label(&b).unwrap(),
            vec!["f_loop".to_string(), "join_1".to_string()]
        );
    }

    #[test]
    fn conjunto_emitido_separa_muitos_para_um_de_colisao() {
        let mut set = EmittedDefinitions::new();
        set.define("pinker_falar_fim", "intrínseca falar");
        set.define("pinker_falar_fim", "intrínseca falar");
        assert!(set.first_collision().is_none());

        set.define("main", "principal");
        set.define("main", "carinho main");
        let collision = set.first_collision().expect("colisão registrada");
        assert_eq!(collision.symbol, "main");
    }
}
// @pinker-nav:end evidence.native.symbol
