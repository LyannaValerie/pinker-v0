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

use std::collections::BTreeMap;
use std::fmt::Write as _;

// @pinker-nav:start nativo.simbolo.entrypoint
// @pinker-nav:domain identidade
// @pinker-nav:layer nativo
// @pinker-nav:summary Autoridade explícita do entrypoint: ENTRYPOINT_SOURCE_IDENTITY (`principal`) é a única identidade de fonte que produz um símbolo de plataforma, ENTRYPOINT_NATIVE_SYMBOL (`main`) é o símbolo da superfície montável e FREESTANDING_ENTRYPOINT_SYMBOL (`_start`) o da superfície livre. NativeSurface modela explicitamente a diferença deliberada entre a superfície montável, onde a identidade vira símbolo de ABI, e a superfície textual `pinker.text.v0`, anotativa, que preserva a grafia Pinker. `function_symbol` é o único ponto que responde `principal -> main`; `is_entrypoint` é o único ponto que reconhece a identidade do entrypoint.

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
// @pinker-nav:end nativo.simbolo.entrypoint

// @pinker-nav:start nativo.simbolo.ligacao
// @pinker-nav:domain abi
// @pinker-nav:layer nativo
// @pinker-nav:summary Autoridade única de ligação das definições emitidas pelo objeto do programa: NativeDefinition classifica a definição (entrypoint, função de usuário, função gerada pelo compilador, global `eterno`, helper local do backend) e `native_binding` responde LOCAL ou GLOBAL por classe. Só o entrypoint é GLOBAL, porque é a única definição do objeto consumida de fora (pelo CRT); todo o resto é STB_LOCAL e por isso deixa de satisfazer referências externas do runtime que deveriam ir ao host. `NativeBinding::directive` é o único produtor de `.globl`/`.local`; `.hidden` não é usado, porque STV_HIDDEN governa exportação dinâmica e não impede a captura no link estático.

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
// @pinker-nav:end nativo.simbolo.ligacao

// @pinker-nav:start nativo.simbolo.namespace-reservado
// @pinker-nav:domain identificadores
// @pinker-nav:layer nativo
// @pinker-nav:summary Reserva dirigida dos namespaces que a Pinker realmente possui, derivada de uma tabela única: as dezenove formas de identidade sintética que o compilador de fato materializa (dezessete prefixos, de `__pinker_internal_` a `__propagar_falha_`, e dois nomes exatos, `__env` e `__ternario`), o prefixo `pinker_` (símbolos definidos e consumidos por `libpinker_rt.a`) e os símbolos de entrypoint de plataforma `main` e `_start`. A reserva é da forma realmente possuída, não do superprefixo comum a ela: `__` continua livre, então `__usuario` é nome Pinker legal, e uma entrada `Exact` nunca vira `Prefix` por conveniência — `__env` é reservado e `__envio` não. Cada entrada declara seu dono (`NamespaceOwner`) e a fronteira exata em que é aplicada — `AnyIdentifier` na fronteira léxica de fonte, `SymbolDefinition` na fronteira de definição produtora de símbolo — e cada fronteira consulta só as entradas do seu escopo, porque `main` é nome legítimo de pacote e as identidades sintéticas são criadas pelo próprio compilador depois do lexer. `is_compiler_generated` é o único ponto que reconhece identidade gerada, e `classify_function` o consome em vez de testar prefixo por conta própria. Nomes do host (`malloc`, `memcpy`, `write`, `getenv`, `free`, `environ`, ...) NÃO são reservados: continuam legais como nomes Pinker e são isolados por STB_LOCAL.

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
        shape: ReservedShape::Prefix("__anon_carinho_"),
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
    PINKER_OWNED_NAMESPACES
        .iter()
        .copied()
        .find(|entry| entry.scope == scope && entry.shape.matches(name))
}

/// `true` quando o nome pertence a uma forma que o compilador materializa.
///
/// Único ponto que reconhece identidade gerada. Não é `starts_with("__")`: o
/// superprefixo comum às famílias não é propriedade da Pinker, e um nome de
/// usuário como `__usuario` não é identidade gerada.
pub fn is_compiler_generated(name: &str) -> bool {
    PINKER_OWNED_NAMESPACES
        .iter()
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
// @pinker-nav:end nativo.simbolo.namespace-reservado

// @pinker-nav:start nativo.simbolo.rotulo-injetivo
// @pinker-nav:domain renderizacao
// @pinker-nav:layer nativo
// @pinker-nav:summary Encoding injetivo dos rótulos locais gerados, pelo mesmo princípio já usado pela identidade genérica e pelo símbolo de vtable: cada componente entra com prefixo de comprimento em bytes, de modo que a concatenação é recuperável e `components(A) != components(B)` implica `encode(A) != encode(B)`. `injective_local_label` produz o rótulo e `decode_injective_local_label` recupera os componentes — a recuperabilidade é a prova de injetividade, não uma conveniência. Substitui a concatenação textual `.L{fn}_{label}`, que colapsava `('f','loop_join_1')` e `('f_loop','join_1')` no mesmo `.Lf_loop_join_1`.

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
// @pinker-nav:end nativo.simbolo.rotulo-injetivo

// @pinker-nav:start nativo.simbolo.conjunto-emitido
// @pinker-nav:domain validacao
// @pinker-nav:layer nativo
// @pinker-nav:summary Verificação do conjunto que o renderer vai emitir, antes de entregar o `.s` à toolchain externa (R2). `EmittedDefinitions` registra cada definição como par `(símbolo, identidade que a produziu)`: duas identidades distintas no mesmo símbolo são colisão e viram diagnóstico Pinker determinístico — ordenado por BTreeMap, nunca por ordem de HashMap — enquanto a mesma identidade repetida no mesmo símbolo é muitos-para-um deliberado e permanece legal. Fecha a classe de erro cru de GNU as ('symbol already defined') para as colisões que o compilador já pode conhecer.

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
// @pinker-nav:end nativo.simbolo.conjunto-emitido

// @pinker-nav:start evidencia.nativo.simbolo
// @pinker-nav:domain identidade
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência local da autoridade nativa de símbolos: fixa `principal -> main` só na superfície montável, a ligação por classe, o escopo exato de cada namespace reservado, a injetividade e a recuperabilidade do encoding de rótulo (incluindo o par histórico `f`/`f_loop` da F-04) e a separação entre muitos-para-um da mesma identidade e colisão entre identidades distintas.
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
// @pinker-nav:end evidencia.nativo.simbolo
