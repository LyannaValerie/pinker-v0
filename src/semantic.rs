//! Checagem semântica — validação antes do lowering para IR.
//!
//! `SemanticChecker` opera em duas passagens sobre o programa:
//! 1. **Declaração**: coleta todas as funções e constantes em tabelas globais (`funcs`, `consts`).
//!    Detecta duplicações e conflitos de nomes entre funções e constantes.
//! 2. **Verificação**: valida cada corpo de função e constante (tipos, escopos, retornos, aridade).
//!
//! Invariantes mantidas:
//! - Sombreamento de variável no mesmo escopo é proibido; escopos aninhados permitem sombra.
//! - `principal` é obrigatória, sem parâmetros, retorno `bombom`.
//! - Retorno de função com tipo declarado deve ser alcançável em todos os caminhos simples
//!   (análise superficial: sequência + talvez/senão — sem análise de fluxo completa).
//! - `Nulo` nunca aparece como tipo de usuário; representa ausência de retorno internamente.

use crate::ast::*;
use crate::error::PinkerError;
use crate::ir::TypeIR;
use crate::layout;
use crate::method_dispatch::{
    self, DispatchCandidate, DispatchRelation, MethodSelection, RepresentativeSelection,
};
use crate::method_identity::{self, MethodIdentity, QualifiedMethodResolution};
use crate::source_map::SourceId;
use crate::token::{Position, Span};
use crate::union_canon;
use std::collections::{BTreeMap, HashMap, HashSet};

mod calls;
mod expressions;
mod statements;
mod traits;

// @pinker-nav:start semantic.identificadores.namespace-produtor-de-simbolo
// @pinker-nav:domain identificadores
// @pinker-nav:layer semantic
// @pinker-nav:summary Fronteira única das definições top-level que produzem símbolo nativo (`carinho` e `eterno`): consulta os namespaces de escopo `SymbolDefinition` da autoridade `native_symbol` — hoje o prefixo `pinker_` do runtime e os símbolos de entrypoint de plataforma `main` e `_start` — e recusa com `E-SEMANTIC-RESERVED-NAMESPACE` e span da declaração, antes de qualquer assembler ou linker. Não é possível aplicá-los na fronteira léxica porque `main` é nome legítimo de pacote; e não é preciso repetir aqui as formas geradas pelo compilador, já recusadas no lexer. Nomes do host continuam legais: `malloc`, `memcpy`, `write`, `getenv`, `free` e `environ` passam por aqui sem diagnóstico e são isolados por STB_LOCAL na emissão.
/// Recusa, com span da declaração, um nome de definição que invadiria um
/// namespace de fato possuído pela Pinker.
pub fn validar_namespace_pinker_owned(name: &str, span: Span) -> Result<(), PinkerError> {
    if let Some(namespace) = crate::native_symbol::reserved_namespace(
        name,
        crate::native_symbol::ReservedScope::SymbolDefinition,
    ) {
        return Err(PinkerError::Semantic {
            msg: crate::native_symbol::reserved_namespace_message(name, namespace),
            span,
        });
    }
    Ok(())
}
// @pinker-nav:end semantic.identificadores.namespace-produtor-de-simbolo

/// #505 — o que a colisão de declaração ainda protege, e o que ela soltou.
///
/// Enquanto existia superfície global, TODA grafia intrínseca ocupava o
/// namespace callable de todo arquivo, e a política da PR #507 recusava a
/// declaração homônima em qualquer lugar. A #505 separou dois namespaces, e a
/// resposta passou a ser diferente em cada um.
///
/// **Grafia de membro** (`criar`, `tamanho`, `existe`, `obter`) só ocupa o
/// namespace do arquivo que a traz. Num arquivo sem `trazer`, `carinho
/// tamanho(...)` é declaração legítima do usuário — a proibição global perdeu
/// a razão junto com a superfície global, e não pode sobreviver por acidente.
/// No arquivo que traz o membro, a colisão é real e continua recusada.
///
/// **Grafia canônica** (`tamanho_verso`, `ler_arquivo`, `mapa_verso_verso_criar`)
/// esteve reservada, e não por inércia histórica: ela deixara de ser chamável,
/// mas continuava sendo a CHAVE DE DESPACHO que `semantic`, `ir`, `interpreter`
/// e `backend_s` usavam depois da canonicalização. Aceitar a declaração sem
/// reservar a grafia trocaria uma recusa explícita por sombreamento silencioso.
///
/// A #532 removeu a causa em vez do sintoma: a decisão "esta chamada é
/// intrínseca" passou a vir de `CalleeIdentity`, produzida só pela resolução de
/// um `trazer`. A grafia canônica deixou de ser chave de despacho e, com isso,
/// deixou de precisar de reserva textual. O que sobrou é a colisão REAL — o
/// arquivo que traz o membro e declara o homônimo:
///
/// ```text
/// MEMBER_SPELLING    -> LIVRE, SALVO IMPORT NESTA UNIDADE
/// CANONICAL_SPELLING -> LIVRE; A IDENTIDADE NÃO DISPUTA MAIS O NOME
/// ```
fn active_intrinsic_declaration_conflict(
    program: &Program,
    name: &str,
) -> Option<crate::intrinsics::identity::PublicIntrinsicSpelling> {
    program.imports.iter().find_map(|import| {
        let module = import.module.as_str();
        if !crate::intrinsics::public_surface::familia_conhecida(module) {
            return None;
        }
        match import.symbol.as_deref() {
            // Forma seletiva: liga a grafia do membro neste arquivo.
            Some(symbol) if symbol == name => {
                crate::intrinsics::identity::family_public_intrinsic_spelling(module, name)
            }
            _ => None,
        }
    })
}

fn validate_intrinsic_declaration_conflicts(program: &Program) -> Result<(), PinkerError> {
    for function in program.items.iter().filter_map(|item| match item {
        Item::Function(function) => Some(function),
        _ => None,
    }) {
        let Some(spelling) = active_intrinsic_declaration_conflict(program, &function.name) else {
            continue;
        };
        if crate::intrinsics::identity::declaration_conflict_policy(spelling)
            == crate::intrinsics::identity::DeclarationConflictPolicy::DeclarationIsRejected
        {
            // Duas recusas com causas diferentes precisam de mensagens
            // diferentes: uma diz que a grafia é da linguagem, a outra diz que
            // foi o import deste arquivo que criou a disputa.
            let msg = match spelling.origin {
                crate::intrinsics::identity::PublicIntrinsicOrigin::FamilyAlias { family } => {
                    format!(
                        "declaração callable '{}' colide com o membro '{}.{}' que este arquivo traz; remova o import ou renomeie a declaração",
                        function.name, family, function.name
                    )
                }
                // #532: a única causa restante é o import desta unidade. A
                // grafia canônica sozinha não gera mais conflito.
                _ => format!(
                    "declaração callable '{}' colide com um membro trazido por este arquivo; remova o import ou renomeie a declaração",
                    function.name
                ),
            };
            return Err(PinkerError::Semantic {
                msg,
                span: function.span,
            });
        }
    }
    Ok(())
}

// @pinker-nav:start semantic.importacoes.familias
// @pinker-nav:domain importacoes
// @pinker-nav:layer semantic
// @pinker-nav:summary Validação semântica de `trazer` sobre os módulos built-in, e dono único da política de colisão de import. A lista de módulos e a superfície que cada um exporta não moram aqui: são consultadas em `intrinsics::public_surface`, a autoridade única que o parser também consulta ao canonicalizar. Esta camada decide o que é decisão de import — módulo desconhecido, membro inexistente na forma seletiva e colisão do membro seletivo com item de topo (`validate_family_import_collision`, atravessada tanto pela CLI quanto pelo caminho de biblioteca). A mensagem de membro inexistente vem da própria autoridade. Depois da #505 a colisão de DECLARAÇÃO tem duas causas distintas e mensagens próprias: grafia canônica, reservada porque continua sendo a chave de despacho a jusante, e membro que esta unidade traz. Identidade homônima trazida por `trazer <modulo>;` não é recusada aqui nem em lugar nenhum: ela vence o módulo em silêncio, no parser.
/// Parte G: o membro trazido seletivamente colide com um item de topo?
///
/// A regra existia só em `main.rs`, o que deixava o caminho de biblioteca
/// (`parse` + `check_program`, que é o que a crate expõe e o que os testes
/// usam) aceitar em silêncio um `trazer arquivo.criar;` sobre um `carinho
/// criar` do próprio arquivo. Duas políticas para a mesma pergunta é uma
/// política a mais: a decisão mora aqui, na autoridade que todo caminho
/// atravessa, com a mesma mensagem que a CLI já dava.
pub fn validate_family_import_collision(
    import: &ImportDecl,
    items: &[Item],
) -> Result<(), PinkerError> {
    let Some(symbol) = import.symbol.as_deref() else {
        return Ok(());
    };
    if !crate::intrinsics::public_surface::familia_conhecida(import.module.as_str()) {
        return Ok(());
    }
    let colide = items.iter().any(|item| match item {
        Item::Function(function) => function.name == symbol,
        Item::Const(constant) => constant.name == symbol,
        Item::Struct(struct_decl) => struct_decl.name == symbol,
        Item::TypeAlias(alias) => alias.name == symbol,
        Item::Enum(enum_decl) => enum_decl.name == symbol,
        Item::Trait(trait_decl) => trait_decl.name == symbol,
    });
    if colide {
        return Err(PinkerError::Semantic {
            msg: format!(
                "colisão de nome no import: '{}' já existe no arquivo principal",
                symbol
            ),
            span: import.span,
        });
    }
    Ok(())
}

pub fn validate_builtin_family_import(import: &ImportDecl) -> Result<(), PinkerError> {
    if !crate::intrinsics::public_surface::familia_conhecida(import.module.as_str()) {
        return Err(PinkerError::Semantic {
            msg: format!(
                "família '{}' não é reconhecida como família importável; famílias disponíveis nesta fase: {}",
                import.module,
                crate::intrinsics::public_surface::familias_disponiveis()
            ),
            span: import.span,
        });
    }
    let Some(symbol) = import.symbol.as_deref() else {
        return Ok(());
    };
    // A recusa categórica de importação seletiva deixou de existir: o que se
    // recusa agora é um membro que a família não exporta. A família sem
    // exportações continua importável inteira e continua sem membro nenhum a
    // selecionar, e é isso que a mensagem diz.
    if !crate::intrinsics::public_surface::import_seletivo_valido(import.module.as_str(), symbol) {
        return Err(PinkerError::Semantic {
            msg: crate::intrinsics::public_surface::membro_inexistente(
                import.module.as_str(),
                symbol,
            ),
            span: import.span,
        });
    }
    Ok(())
}
// @pinker-nav:end semantic.importacoes.familias

#[derive(Clone)]
struct VarMeta {
    ty: Type,
    is_mut: bool,
}

struct Scope {
    vars: HashMap<String, VarMeta>,
}

#[derive(Clone)]
struct ImplMethodMeta {
    identity: MethodIdentity<String>,
    target_spelling: String,
    resolved_target_display: String,
    function_name: String,
    is_generated_default: bool,
    span: Span,
}

pub struct SemanticChecker {
    funcs: HashMap<String, FunctionDecl>,
    consts: HashMap<String, ConstDecl>,
    type_aliases: HashMap<String, Type>,
    structs: HashMap<String, StructDecl>,
    enums: HashMap<String, EnumDecl>,
    traits: HashMap<String, TraitDecl>,
    // Registro autoritativo aceito pela semântica. Cada entrada carrega uma
    // MethodIdentity cujo alvo é a chave canônica do tipo já resolvido.
    impl_methods: Vec<ImplMethodMeta>,
    // Visão derivada para lookup não qualificado; a chave vem exclusivamente
    // do registro acima e nunca de spelling do receiver.
    method_index: HashMap<(String, String), Vec<String>>,
    scopes: Vec<Scope>,
    current_func_name: Option<String>,
    current_func_ret: Option<Type>,
    loop_depth: usize,
    // Fase 243: closures (`__anon_carinho_*`) já resolvidas (corpo checado
    // com o ambiente correto) e suas capturas — nome da closure -> lista
    // (nome capturado, tipo), em ordem determinística de primeira
    // referência no corpo. Uma closure é resolvida exatamente uma vez, no
    // ponto de criação (onde seu `Ident` sintético aparece como valor).
    checked_closures: HashSet<String>,
    closure_captures: HashMap<String, Vec<(String, Type)>>,
    /// Tratos que cada unidade-fonte pode enxergar, por `SourceId`.
    ///
    /// Vazio quando não houve composição modular — e vazio significa "não há a
    /// quem restringir", não "ninguém enxerga nada". Uma chamada de método não
    /// nomeia o trato, então o despacho não é alcançado pela resolução nominal
    /// canônica; é aqui que ele passa a respeitar o ambiente de quem escreveu a
    /// chamada.
    traits_visiveis_por_fonte: HashMap<SourceId, crate::module_resolve::TratosNoDespacho>,
    /// #577 — unidade que DECLAROU cada relação `(trato canônico, alvo
    /// canônico)`, pelo `SourceId` do próprio bloco `impl`.
    ///
    /// O span do bloco é do arquivo que o escreveu: ele não é corpo copiado, e
    /// portanto responde pela origem da relação sem depender de proveniência de
    /// corpo default. É o que permite ao nível subordinado do despacho admitir
    /// exatamente as relações das unidades importadas, e não toda relação do
    /// trato.
    fontes_das_relacoes: HashMap<(String, String), SourceId>,
    /// Unidades-fonte que são módulo, por `SourceId`.
    ///
    /// Depois da resolução nominal canônica, TODA referência legítima de um
    /// módulo a uma entidade de usuário está qualificada — inclusive às
    /// próprias, que se chamam `M.x`. Uma grafia crua vinda de um módulo é,
    /// portanto, ou builtin (despachado antes daqui) ou tentativa de alcançar a
    /// raiz. É a última fronteira da não-interferência, e existe porque a
    /// resolução deixa passar a grafia builtin de propósito: sem ela, um módulo
    /// que chamasse `mapa_criar(1)` — aridade que o builtin não atende — cairia
    /// na função de mesmo nome declarada na raiz.
    fontes_de_modulo: HashSet<SourceId>,
}

impl Default for SemanticChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticChecker {
    /// Verificador ciente da composição modular.
    ///
    /// Recebe, por unidade-fonte, os tratos que aquela unidade autorizou.
    pub fn com_visibilidade_de_tratos(
        traits_visiveis_por_fonte: HashMap<SourceId, crate::module_resolve::TratosNoDespacho>,
        fontes_de_modulo: HashSet<SourceId>,
    ) -> Self {
        Self {
            traits_visiveis_por_fonte,
            fontes_de_modulo,
            ..Self::new()
        }
    }

    /// A grafia crua vem de um módulo?
    ///
    /// Nome possuído pelo compilador e nome já qualificado não contam: os dois
    /// são identidade resolvida, não grafia.
    fn grafia_crua_de_modulo(&self, span: Span, name: &str) -> bool {
        // Autoridade única de identidade gerada. `starts_with("__")` recusaria
        // `__usuario`, que é identificador de usuário legal.
        !crate::native_symbol::is_compiler_generated(name)
            && !name.contains('.')
            && self.fontes_de_modulo.contains(&span.source)
    }
}

impl SemanticChecker {
    pub fn new() -> Self {
        Self {
            funcs: HashMap::new(),
            consts: HashMap::new(),
            type_aliases: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            impl_methods: Vec::new(),
            fontes_das_relacoes: HashMap::new(),
            method_index: HashMap::new(),
            scopes: Vec::new(),
            current_func_name: None,
            current_func_ret: None,
            loop_depth: 0,
            checked_closures: HashSet::new(),
            traits_visiveis_por_fonte: HashMap::new(),
            fontes_de_modulo: HashSet::new(),
            closure_captures: HashMap::new(),
        }
    }

    /// Nome do leque por trás de um identificador, atravessando a cadeia de
    /// apelidos.
    ///
    /// A cadeia é transparente: `apelido A = Leque; apelido B = A;` faz `B.X`
    /// significar exatamente `Leque.X`, sem criar uma identidade nominal nova.
    fn resolve_enum_base_name(&self, base_name: &str) -> Option<String> {
        let mut current = base_name.to_string();
        // O teto é o número de apelidos declarados: uma cadeia mais longa que
        // isso só pode ser cíclica, e a recursão de apelidos já é diagnosticada
        // por `resolve_type_named`.
        for _ in 0..=self.type_aliases.len() {
            if self.enums.contains_key(&current) {
                return Some(current);
            }
            match self.type_aliases.get(&current) {
                Some(Type::Enum { name, .. }) | Some(Type::Alias { name, .. }) => {
                    current.clone_from(name)
                }
                _ => return None,
            }
        }
        None
    }

    /// Projeção da fase: o tipo declarativo do registry como `Type` desta fase.
    ///
    /// É uma VISTA, não autoridade. O contrato continua sendo dito uma vez em
    /// `intrinsics::registry`; aqui ele só ganha o vocabulário e o span que a
    /// checagem semântica usa. Só as representações que a superfície histórica
    /// declara chegam aqui — as demais pertencem a contratos próprios de fase.
    fn tipo_de_intrinseca(ty: crate::ir::TypeIR, span: Span) -> Type {
        use crate::ir::TypeIR;
        match ty {
            TypeIR::Bombom => Type::Bombom(span),
            TypeIR::Verso => Type::Verso(span),
            TypeIR::Logica => Type::Logica(span),
            TypeIR::Nulo => Type::Nulo(span),
            TypeIR::ListBombom => Type::ListBombom(span),
            TypeIR::ListVerso => Type::ListVerso(span),
            TypeIR::MapVersoBombom => Type::MapVersoBombom(span),
            TypeIR::MapVersoVerso => Type::MapVersoVerso(span),
            TypeIR::MapBombomBombom => Type::MapBombomBombom(span),
            TypeIR::MapBombomVerso => Type::MapBombomVerso(span),
            outro => unreachable!(
                "representação {outro:?} não pertence ao contrato declarado de intrínseca histórica"
            ),
        }
    }

    fn type_key(ty: &Type) -> String {
        match ty {
            Type::Alias { name, .. }
            | Type::Struct { name, .. }
            | Type::OpaqueHandle { name, .. }
            | Type::Enum { name, .. } => name.clone(),
            Type::Function { params, ret, .. } => {
                let params = params
                    .iter()
                    .map(Self::type_key)
                    .collect::<Vec<_>>()
                    .join(",");
                format!("carinho({})->{}", params, Self::type_key(ret))
            }
            Type::Union { members, .. } => {
                let mut keys = members.iter().map(Self::type_key).collect::<Vec<_>>();
                keys.sort();
                keys.dedup();
                format!("uniao<{}>", keys.join(","))
            }
            Type::Applied { .. } => Self::trait_object_name(ty)
                .map(|trait_name| format!("trato<{}>", trait_name))
                .unwrap_or_else(|| ty.name().to_string()),
            Type::Map { key, value, .. } => {
                format!("mapa<{},{}>", Self::type_key(key), Self::type_key(value))
            }
            _ => ty.name().to_string(),
        }
    }

    fn raw_function_abi_type_supported(ty: &Type, allow_nulo: bool) -> bool {
        match ty {
            Type::Bombom(_)
            | Type::U8(_)
            | Type::U16(_)
            | Type::U32(_)
            | Type::U64(_)
            | Type::I8(_)
            | Type::I16(_)
            | Type::I32(_)
            | Type::I64(_)
            | Type::Logica(_)
            | Type::Verso(_)
            | Type::ListBombom(_)
            | Type::ListVerso(_)
            | Type::ListEnum { .. }
            | Type::MapVersoBombom(_)
            | Type::MapVersoVerso(_)
            | Type::MapBombomBombom(_)
            | Type::MapBombomVerso(_)
            | Type::Map { .. }
            | Type::Enum { .. }
            | Type::Union { .. }
            | Type::Pointer { .. }
            | Type::Function { .. }
            | Type::OpaqueHandle { .. } => true,
            Type::Applied { .. } => Self::trait_object_name(ty).is_some(),
            Type::Nulo(_) => allow_nulo,
            Type::FixedArray { .. } | Type::Struct { .. } | Type::Alias { .. } => false,
        }
    }

    fn validate_raw_function_signature(
        params: &[Type],
        ret: &Type,
        span: Span,
    ) -> Result<(), PinkerError> {
        if let Some((index, ty)) = params
            .iter()
            .enumerate()
            .find(|(_, ty)| !Self::raw_function_abi_type_supported(ty, false))
        {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "assinatura de ponteiro cru de função usa tipo ABI não suportado no parâmetro {}: '{}'",
                    index + 1,
                    Self::type_key(ty)
                ),
                span,
            });
        }
        if !Self::raw_function_abi_type_supported(ret, true) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "assinatura de ponteiro cru de função usa tipo ABI de retorno não suportado: '{}'",
                    Self::type_key(ret)
                ),
                span,
            });
        }
        Ok(())
    }

    /// Unidade que DECLAROU a relação `(trato canônico, alvo canônico)`.
    ///
    /// Adaptador único da fase para a proveniência de relação; a pergunta de
    /// alcance em si é de `module_resolve`. Mora no pai porque as duas
    /// superfícies que a fazem — chamada qualificada em `calls` e formação de
    /// objeto de trato em `expressions` — são irmãs, e duplicar o adaptador
    /// devolveria a cada uma a chance de responder por conta própria.
    fn fonte_da_relacao(&self, trait_name: &str, target: &str) -> Option<SourceId> {
        self.fontes_das_relacoes
            .get(&(trait_name.to_string(), target.to_string()))
            .copied()
    }

    /// A relação `(trato, alvo)` alcança quem escreveu `span`?
    ///
    /// #649/`POLICY_B_RELATION_REACHABILITY_ALWAYS_MATTERS` — a autoridade é
    /// `module_resolve`, a MESMA que o despacho não qualificado consulta por
    /// dentro de `method_dispatch`. Esta fase só traz a sua representação de
    /// alvo e traduz o veredito: nenhuma regra de alcance nasce aqui, e a
    /// resposta não depende de o chamador poder nomear o trato.
    fn relacao_alcanca(&self, trait_name: &str, target: &str, span: Span) -> bool {
        crate::module_resolve::relacao_alcanca(
            &self.traits_visiveis_por_fonte,
            span,
            self.fonte_da_relacao(trait_name, target),
        )
    }

    fn trait_object_name(ty: &Type) -> Option<&str> {
        match ty {
            Type::Applied {
                name,
                args,
                span: _,
            } if name == "trato" => match args.as_slice() {
                [Type::Alias {
                    name: trait_name, ..
                }] => Some(trait_name.as_str()),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_contextual_self_type(ty: &Type) -> bool {
        matches!(ty, Type::Alias { name, .. } if name == "si")
    }

    fn type_contains_contextual_self(ty: &Type) -> bool {
        match ty {
            Type::Alias { name, .. } => name == "si",
            Type::ListEnum { element, .. } => element == "si",
            Type::FixedArray { element, .. } => {
                Self::type_contains_contextual_self(element.as_ref())
            }
            Type::Pointer { base, .. } => Self::type_contains_contextual_self(base.as_ref()),
            Type::Function { params, ret, .. } => {
                params.iter().any(Self::type_contains_contextual_self)
                    || Self::type_contains_contextual_self(ret.as_ref())
            }
            Type::Applied { args, .. } => args.iter().any(Self::type_contains_contextual_self),
            _ => false,
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope {
            vars: HashMap::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn root_span(program: &Program) -> Span {
        program
            .package
            .as_ref()
            .map(|package| package.span)
            .or_else(|| program.imports.first().map(|import| import.span))
            .or_else(|| program.items.first().map(Item::span))
            .unwrap_or_else(|| Span::single(Position::new(1, 1)))
    }

    // @pinker-nav:start semantic.tipos.sistema
    // @pinker-nav:domain tipos
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Sistema de tipos da checagem: compatibilidade estrutural (`check_type_match`), resolução de tipos nomeados/aliases com detecção de recursão (`resolve_type_named`/`resolve_type_or_error`), validação de struct, regras de inteiro/cast e verificação de faixa de literais inteiros contra o tipo-alvo.
    fn check_type_match(expected: &Type, actual: &Type) -> bool {
        match (expected, actual) {
            (Type::Bombom(_), Type::Bombom(_))
            | (Type::Bombom(_), Type::U64(_))
            | (Type::U64(_), Type::Bombom(_))
            | (Type::U8(_), Type::U8(_))
            | (Type::U16(_), Type::U16(_))
            | (Type::U32(_), Type::U32(_))
            | (Type::U64(_), Type::U64(_))
            | (Type::I8(_), Type::I8(_))
            | (Type::I16(_), Type::I16(_))
            | (Type::I32(_), Type::I32(_))
            | (Type::I64(_), Type::I64(_))
            | (Type::Logica(_), Type::Logica(_))
            | (Type::Verso(_), Type::Verso(_))
            | (Type::ListBombom(_), Type::ListBombom(_))
            | (Type::ListVerso(_), Type::ListVerso(_))
            | (Type::MapVersoBombom(_), Type::MapVersoBombom(_))
            | (Type::MapVersoVerso(_), Type::MapVersoVerso(_))
            | (Type::MapBombomBombom(_), Type::MapBombomBombom(_))
            | (Type::MapBombomVerso(_), Type::MapBombomVerso(_))
            | (Type::Nulo(_), Type::Nulo(_)) => true,
            (Type::Struct { name: lhs_name, .. }, Type::Struct { name: rhs_name, .. }) => {
                lhs_name == rhs_name
            }
            (
                Type::OpaqueHandle { name: lhs_name, .. },
                Type::OpaqueHandle { name: rhs_name, .. },
            ) => lhs_name == rhs_name,
            (Type::Enum { name: lhs_name, .. }, Type::Enum { name: rhs_name, .. }) => {
                lhs_name == rhs_name
            }
            (
                Type::Union {
                    members: lhs_members,
                    ..
                },
                Type::Union {
                    members: rhs_members,
                    ..
                },
            ) => {
                lhs_members.len() == rhs_members.len()
                    && lhs_members
                        .iter()
                        .zip(rhs_members)
                        .all(|(lhs, rhs)| Self::check_type_match(lhs, rhs))
            }
            (
                Type::ListEnum {
                    element: lhs_element,
                    ..
                },
                Type::ListEnum {
                    element: rhs_element,
                    ..
                },
            ) => lhs_element == rhs_element,
            (
                Type::Map {
                    key: lhs_key,
                    value: lhs_value,
                    ..
                },
                Type::Map {
                    key: rhs_key,
                    value: rhs_value,
                    ..
                },
            ) => {
                Self::check_type_match(lhs_key, rhs_key)
                    && Self::check_type_match(lhs_value, rhs_value)
            }
            (
                Type::FixedArray {
                    element: lhs_element,
                    size: lhs_size,
                    ..
                },
                Type::FixedArray {
                    element: rhs_element,
                    size: rhs_size,
                    ..
                },
            ) => {
                lhs_size == rhs_size
                    && Self::check_type_match(lhs_element.as_ref(), rhs_element.as_ref())
            }
            (
                Type::Pointer {
                    base: lhs_base,
                    is_volatile: lhs_volatile,
                    ..
                },
                Type::Pointer {
                    base: rhs_base,
                    is_volatile: rhs_volatile,
                    ..
                },
            ) => {
                lhs_volatile == rhs_volatile
                    && Self::check_type_match(lhs_base.as_ref(), rhs_base.as_ref())
            }
            (Type::Applied { .. }, Type::Applied { .. }) => {
                let expected_trait = Self::trait_object_name(expected);
                expected_trait.is_some() && expected_trait == Self::trait_object_name(actual)
            }
            // Fase 242: tipo função é comparado estruturalmente por assinatura
            // (aridade + tipo de cada parâmetro + tipo de retorno).
            (
                Type::Function {
                    params: lhs_params,
                    ret: lhs_ret,
                    ..
                },
                Type::Function {
                    params: rhs_params,
                    ret: rhs_ret,
                    ..
                },
            ) => {
                lhs_params.len() == rhs_params.len()
                    && lhs_params
                        .iter()
                        .zip(rhs_params.iter())
                        .all(|(l, r)| Self::check_type_match(l, r))
                    && Self::check_type_match(lhs_ret.as_ref(), rhs_ret.as_ref())
            }
            _ => false,
        }
    }

    fn resolve_type_named(
        &self,
        ty: &Type,
        resolving: &mut Vec<String>,
    ) -> Result<Type, PinkerError> {
        match ty {
            Type::Alias { name, span } => {
                if self.structs.contains_key(name) {
                    return Ok(Type::Struct {
                        name: name.clone(),
                        span: *span,
                    });
                }
                if self.enums.contains_key(name) {
                    return Ok(Type::Enum {
                        name: name.clone(),
                        span: *span,
                    });
                }
                if resolving.iter().any(|entry| entry == name) {
                    return Err(PinkerError::Semantic {
                        msg: format!("alias de tipo recursivo detectado em '{}'", name),
                        span: *span,
                    });
                }
                let Some(target) = self.type_aliases.get(name) else {
                    return Err(PinkerError::Semantic {
                        msg: format!("tipo '{}' não existe", name),
                        span: *span,
                    });
                };
                resolving.push(name.clone());
                let resolved = self.resolve_type_named(target, resolving)?;
                resolving.pop();
                Ok(resolved.with_span(*span))
            }
            Type::FixedArray {
                element,
                size,
                span,
            } => {
                if *size == 0 {
                    return Err(PinkerError::Semantic {
                        msg: "array fixo deve ter tamanho maior que zero".to_string(),
                        span: *span,
                    });
                }

                let resolved_element = self.resolve_type_named(element.as_ref(), resolving)?;
                if matches!(resolved_element, Type::Nulo(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "tipo base de array fixo não pode ser 'nulo'".to_string(),
                        span: resolved_element.span(),
                    });
                }
                if matches!(resolved_element, Type::FixedArray { .. }) {
                    return Err(PinkerError::Semantic {
                        msg: "array fixo aninhado ainda não é suportado nesta fase".to_string(),
                        span: resolved_element.span(),
                    });
                }

                Ok(Type::FixedArray {
                    element: Box::new(resolved_element),
                    size: *size,
                    span: *span,
                })
            }
            Type::Map { key, value, span } => {
                let key = self.resolve_type_named(key, resolving)?;
                let value = self.resolve_type_named(value, resolving)?;
                if !matches!(key, Type::Bombom(_) | Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo de chave de mapa incompatível: '{}' não possui igualdade e representação estáveis no contrato vigente",
                            Self::type_key(&key)
                        ),
                        span: key.span(),
                    });
                }
                if !matches!(
                    value,
                    Type::Bombom(_)
                        | Type::U8(_)
                        | Type::U16(_)
                        | Type::U32(_)
                        | Type::U64(_)
                        | Type::I8(_)
                        | Type::I16(_)
                        | Type::I32(_)
                        | Type::I64(_)
                        | Type::Logica(_)
                        | Type::Verso(_)
                        | Type::Enum { .. }
                ) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "representação de valor de mapa não suportada: '{}' não possui armazenamento/lifetime aprovado",
                            Self::type_key(&value)
                        ),
                        span: value.span(),
                    });
                }
                Ok(Type::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                    span: *span,
                })
            }
            Type::Pointer {
                base,
                is_volatile,
                span,
            } => {
                let resolved_base = if let Type::Function {
                    params,
                    ret,
                    span: function_span,
                } = base.as_ref()
                {
                    let resolved_params = params
                        .iter()
                        .map(|param| self.resolve_type_named(param, resolving))
                        .collect::<Result<Vec<_>, _>>()?;
                    let resolved_ret = self.resolve_type_named(ret.as_ref(), resolving)?;
                    Self::validate_raw_function_signature(
                        &resolved_params,
                        &resolved_ret,
                        *function_span,
                    )?;
                    Type::Function {
                        params: resolved_params,
                        ret: Box::new(resolved_ret),
                        span: *function_span,
                    }
                } else {
                    self.resolve_type_named(base.as_ref(), resolving)?
                };
                if matches!(resolved_base, Type::Nulo(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "tipo base de 'seta' não pode ser 'nulo'".to_string(),
                        span: resolved_base.span(),
                    });
                }
                if matches!(resolved_base, Type::Pointer { .. }) {
                    return Err(PinkerError::Semantic {
                        msg: "seta de seta ainda não é suportada nesta fase".to_string(),
                        span: resolved_base.span(),
                    });
                }
                Ok(Type::Pointer {
                    base: Box::new(resolved_base),
                    is_volatile: *is_volatile,
                    span: *span,
                })
            }
            Type::Function { params, ret, span } => {
                let resolved_params = params
                    .iter()
                    .map(|param| self.resolve_type_named(param, resolving))
                    .collect::<Result<Vec<_>, _>>()?;
                let resolved_ret = self.resolve_type_named(ret.as_ref(), resolving)?;
                if matches!(resolved_ret, Type::Nulo(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "tipo função público exige retorno declarado nesta fase".to_string(),
                        span: resolved_ret.span(),
                    });
                }
                Ok(Type::Function {
                    params: resolved_params,
                    ret: Box::new(resolved_ret),
                    span: *span,
                })
            }
            Type::Applied { name, args, span } if name == "trato" => {
                let Some(trait_name) = Self::trait_object_name(ty) else {
                    return Err(PinkerError::Semantic {
                        msg: "tipo de objeto de trato exige exatamente um nome nominal".to_string(),
                        span: *span,
                    });
                };

                let Some(trait_decl) = self.traits.get(trait_name) else {
                    return Err(PinkerError::Semantic {
                        msg: format!("trato '{}' não declarado", trait_name),
                        span: *span,
                    });
                };

                if !self.validate_object_trait_shape(trait_decl)? {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "trato '{}' não é objetificável: declare 'si' como receiver contextual de todos os métodos",
                            trait_name
                        ),
                        span: *span,
                    });
                }

                Ok(Type::Applied {
                    name: name.clone(),
                    args: args.clone(),
                    span: *span,
                })
            }
            Type::Union { members, span } => {
                // A canonicalização (achatamento, deduplicação e ordem) vem do
                // contrato compartilhado em `union_canon`, o mesmo consumido
                // pelo lowering ao internar o `UnionTypeIR`. Não há chave nem
                // ordem próprias desta camada.
                let mut resolved_members = Vec::with_capacity(members.len());
                for member in members {
                    resolved_members.push(self.resolve_type_named(member, resolving)?);
                }
                let canonical = union_canon::canonicalize_resolved_members(resolved_members);
                if canonical.len() < 2 {
                    return Err(PinkerError::Semantic {
                        msg: "união estrutural exige ao menos dois membros distintos após canonicalização"
                            .to_string(),
                        span: *span,
                    });
                }
                // HR3: a representação de payload de cada membro é decidida
                // **aqui**, antes da IR validada. Um membro sem layout
                // conhecido, com tamanho zero, acima do limite ou com
                // alinhamento não suportado é recusado com código estável em
                // vez de virar metadata falsa que só falharia na criação
                // nativa do descritor.
                for member in &canonical {
                    crate::union_payload::classify_union_payload(
                        member,
                        &self.type_aliases,
                        &self.structs,
                    )
                    .map_err(|rejection| PinkerError::Semantic {
                        msg: rejection.message(),
                        span: *span,
                    })?;
                }
                Ok(Type::Union {
                    members: canonical,
                    span: *span,
                })
            }
            Type::Struct { .. } => Ok(ty.clone()),
            Type::ListEnum { element, span } => {
                if self.enums.contains_key(element) {
                    return Ok(ty.clone());
                }
                // O elemento passa pela mesma resolução dos demais tipos:
                // `lista<Apelido>` é a lista do **alvo** do apelido, e não uma
                // lista de um tipo nominal novo chamado `Apelido`. Sem isto,
                // `apelido CorAlias = Cor; lista<CorAlias>` teria identidade
                // distinta de `lista<Cor>`.
                let resolved_element = self.type_aliases.get(element).and_then(|_| {
                    self.resolve_type_named(
                        &Type::Alias {
                            name: element.clone(),
                            span: *span,
                        },
                        resolving,
                    )
                    .ok()
                });
                match resolved_element {
                    Some(Type::Bombom(_)) => Ok(Type::ListBombom(*span)),
                    Some(Type::Verso(_)) => Ok(Type::ListVerso(*span)),
                    Some(Type::Enum { name, .. }) => Ok(Type::ListEnum {
                        element: name,
                        span: *span,
                    }),
                    _ => Err(PinkerError::Semantic {
                        msg: format!(
                            "lista genérica exige leque declarado como elemento; '{}' não é um leque",
                            element
                        ),
                        span: *span,
                    }),
                }
            }
            _ => Ok(ty.clone()),
        }
    }

    /// #532 — a criação genérica é reconhecida pela IDENTIDADE do callee.
    ///
    /// `lista.criar` e `mapa.criar` chegam aqui como identidade resolvida; uma
    /// função do usuário com a mesma grafia é `Ident` e nunca satisfaz esta
    /// pergunta.
    fn expr_is_intrinsic_call_without_args(expr: &Expr, canonica: &str) -> bool {
        let ExprKind::Call(callee, args) = &expr.kind else {
            return false;
        };
        let ExprKind::Intrinsic(identity) = &callee.kind else {
            return false;
        };
        identity.canonical_public_spelling() == canonica && args.is_empty()
    }

    fn expr_is_generic_list_create(expr: &Expr) -> bool {
        Self::expr_is_intrinsic_call_without_args(expr, "lista_criar")
    }

    fn expr_is_generic_map_create(expr: &Expr) -> bool {
        Self::expr_is_intrinsic_call_without_args(expr, "mapa_criar")
    }

    fn is_map_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::MapVersoBombom(_)
                | Type::MapVersoVerso(_)
                | Type::MapBombomBombom(_)
                | Type::MapBombomVerso(_)
                | Type::Map { .. }
        )
    }

    /// Tipo do elemento de um tipo de lista (legado ou genérico).
    fn list_element_type(list_ty: &Type, span: Span) -> Option<Type> {
        match list_ty {
            Type::ListBombom(_) => Some(Type::Bombom(span)),
            Type::ListVerso(_) => Some(Type::Verso(span)),
            Type::ListEnum { element, .. } => Some(Type::Enum {
                name: element.clone(),
                span,
            }),
            _ => None,
        }
    }

    fn resolve_type_or_error(&self, ty: &Type) -> Result<Type, PinkerError> {
        let mut resolving = Vec::new();
        self.resolve_type_named(ty, &mut resolving)
    }

    fn resolved_type_identity(&self, ty: &Type) -> Result<String, PinkerError> {
        let resolved = self.resolve_type_or_error(ty)?;
        let identity = union_canon::canonical_type_key(&resolved);
        if union_canon::is_poisoned_key(&identity) {
            return Err(PinkerError::Semantic {
                msg: "identidade semântica de tipo perdida após resolução".to_string(),
                span: ty.span(),
            });
        }
        Ok(identity)
    }

    fn validate_struct_decl(&self, struct_decl: &StructDecl) -> Result<(), PinkerError> {
        let mut field_names = HashSet::new();
        for field in &struct_decl.fields {
            if !field_names.insert(field.name.as_str()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "campo '{}' duplicado na struct '{}'",
                        field.name, struct_decl.name
                    ),
                    span: field.span,
                });
            }
            let resolved = self.resolve_type_or_error(&field.ty)?;
            if matches!(
                resolved,
                Type::Struct { name, .. } if name == struct_decl.name
            ) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "struct '{}' não pode conter recursão direta nesta fase",
                        struct_decl.name
                    ),
                    span: field.span,
                });
            }
        }
        Ok(())
    }

    fn is_integer_type(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Bombom(_)
                | Type::U8(_)
                | Type::U16(_)
                | Type::U32(_)
                | Type::U64(_)
                | Type::I8(_)
                | Type::I16(_)
                | Type::I32(_)
                | Type::I64(_)
        )
    }

    fn expr_is_int_literal(expr: &Expr) -> bool {
        matches!(expr.kind, ExprKind::IntLit(_))
            || matches!(
                &expr.kind,
                ExprKind::Unary(UnaryOp::Neg, inner) if matches!(inner.kind, ExprKind::IntLit(_))
            )
    }

    fn expr_is_zero_literal(expr: &Expr) -> bool {
        matches!(expr.kind, ExprKind::IntLit(0))
    }

    /// Classifica uma carga de variante pela autoridade única (D1).
    ///
    /// A semântica não reimplementa a resolução: fornece apenas as tabelas de
    /// apelidos, leques e ninhos que possui, e recebe de volta representação
    /// operacional e tipo resolvido acoplados.
    fn classify_enum_payload(
        &self,
        ty: &Type,
    ) -> Result<crate::enum_payload::EnumPayloadShape, crate::enum_payload::EnumPayloadRejection>
    {
        let enums: HashSet<String> = self.enums.keys().cloned().collect();
        let structs: HashSet<String> = self.structs.keys().cloned().collect();
        crate::enum_payload::classify_enum_payload(ty, &self.type_aliases, &enums, &structs)
    }

    fn enum_has_payload(&self, enum_name: &str) -> bool {
        self.enums
            .get(enum_name)
            .map(|decl| {
                decl.variants
                    .iter()
                    .any(|variant| !variant.payloads.is_empty())
            })
            .unwrap_or(false)
    }

    fn is_cast_allowed(source: &Type, target: &Type) -> bool {
        if Self::is_integer_type(source) && Self::is_integer_type(target) {
            return true;
        }
        let is_bombom_ptr = |ty: &Type| {
            matches!(
                ty,
                Type::Pointer {
                    base,
                    is_volatile: _,
                    span: _,
                } if matches!(base.as_ref(), Type::Bombom(_))
            )
        };
        let is_data_ptr = |ty: &Type| {
            matches!(
                ty,
                Type::Pointer { base, .. }
                    if !matches!(base.as_ref(), Type::Function { .. })
            )
        };

        (matches!(source, Type::Bombom(_)) && is_bombom_ptr(target))
            || (is_bombom_ptr(source) && matches!(target, Type::Bombom(_)))
            || (is_data_ptr(source) && is_data_ptr(target))
            // Leitura do discriminante de um leque; o caminho inverso continua fechado.
            || (matches!(source, Type::Enum { .. }) && matches!(target, Type::Bombom(_)))
    }

    fn check_expected_type_for_expr(expected: &Type, actual: &Type, expr: &Expr) -> bool {
        Self::check_type_match(expected, actual)
            || matches!(
                expected,
                Type::Pointer { base, .. } if matches!(base.as_ref(), Type::Function { .. })
            ) && Self::expr_is_zero_literal(expr)
            || matches!(
                expected,
                Type::Pointer { base, .. } if !matches!(base.as_ref(), Type::Function { .. })
            ) && Self::expr_is_int_literal(expr)
            || (Self::is_integer_type(expected) && Self::expr_is_int_literal(expr))
    }

    /// Valida que um literal inteiro cabe no tipo-alvo esperado.
    /// Retorna `Ok(())` se o literal couber ou se o tipo não impõe restrição de faixa.
    /// Retorna erro semântico se o literal exceder o intervalo válido do tipo.
    fn validate_int_literal_range(expected: &Type, expr: &Expr) -> Result<(), PinkerError> {
        if let ExprKind::Unary(UnaryOp::Neg, inner) = &expr.kind {
            if let ExprKind::IntLit(value) = &inner.kind {
                let value = *value;
                let (type_name, fits) = match expected {
                    Type::U8(_) | Type::U16(_) | Type::U32(_) | Type::U64(_) | Type::Bombom(_) => {
                        return Ok(())
                    }
                    Type::I8(_) => ("i8", value <= 128),
                    Type::I16(_) => ("i16", value <= 32768),
                    Type::I32(_) => ("i32", value <= 2147483648),
                    Type::I64(_) => ("i64", value <= 9223372036854775808),
                    _ => return Ok(()),
                };
                return if fits {
                    Ok(())
                } else {
                    Err(PinkerError::Semantic {
                        msg: format!("literal -{} excede a faixa do tipo '{}'", value, type_name),
                        span: expr.span,
                    })
                };
            }
        }
        let ExprKind::IntLit(value) = &expr.kind else {
            return Ok(());
        };
        let value = *value;
        let (type_name, fits) = match expected {
            Type::U8(_) => ("u8", value <= u8::MAX as u64),
            Type::U16(_) => ("u16", value <= u16::MAX as u64),
            Type::U32(_) => ("u32", value <= u32::MAX as u64),
            Type::U64(_) | Type::Bombom(_) => return Ok(()),
            Type::I8(_) => ("i8", value <= i8::MAX as u64),
            Type::I16(_) => ("i16", value <= i16::MAX as u64),
            Type::I32(_) => ("i32", value <= i32::MAX as u64),
            Type::I64(_) => ("i64", value <= i64::MAX as u64),
            _ => return Ok(()),
        };
        if fits {
            Ok(())
        } else {
            Err(PinkerError::Semantic {
                msg: format!(
                    "literal {} excede a faixa do tipo '{}' (máximo: {})",
                    value,
                    type_name,
                    match expected {
                        Type::U8(_) => u8::MAX as u64,
                        Type::U16(_) => u16::MAX as u64,
                        Type::U32(_) => u32::MAX as u64,
                        Type::I8(_) => i8::MAX as u64,
                        Type::I16(_) => i16::MAX as u64,
                        Type::I32(_) => i32::MAX as u64,
                        Type::I64(_) => i64::MAX as u64,
                        _ => unreachable!(),
                    }
                ),
                span: expr.span,
            })
        }
    }
    // @pinker-nav:end semantic.tipos.sistema

    // @pinker-nav:start semantic.escopos.variaveis
    // @pinker-nav:domain escopos
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Tabela de escopos léxicos: declaração de variável com proibição de sombreamento no mesmo escopo (`declare_var`) e resolução de nome subindo a pilha de escopos, com fallback para constantes globais (`resolve_var`).
    fn declare_var(
        &mut self,
        name: &str,
        ty: Type,
        is_mut: bool,
        span: Span,
    ) -> Result<(), PinkerError> {
        let scope = self.scopes.last_mut().expect("escopo ativo ausente");
        if scope.vars.contains_key(name) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "variável '{}' já declarada no escopo atual; sombreamento no mesmo escopo é proibido",
                    name
                ),
                span,
            });
        }
        scope.vars.insert(name.to_string(), VarMeta { ty, is_mut });
        Ok(())
    }

    fn resolve_var(&self, name: &str) -> Option<VarMeta> {
        for scope in self.scopes.iter().rev() {
            if let Some(meta) = scope.vars.get(name) {
                return Some(meta.clone());
            }
        }

        if let Some(meta) = self.consts.get(name).map(|constant| VarMeta {
            ty: self
                .resolve_type_or_error(&constant.ty)
                .unwrap_or_else(|_| constant.ty.clone()),
            is_mut: false,
        }) {
            return Some(meta);
        }

        // Fase 242: nome solto de função top-level materializa um valor
        // callable — precedência mais baixa (só depois de escopos locais e
        // constantes), sem alterar shadowing existente. Função genérica não
        // concretizada (type_params não vazio) não pode virar valor.
        self.function_value_type(name)
            .map(|ty| VarMeta { ty, is_mut: false })
    }

    // Fase 242: busca só em `self.scopes` (parâmetros/`nova` locais), sem
    // cair para `consts` ou funções top-level — usada em posição de chamada
    // para decidir precedência de sombreamento local sobre função global.
    fn resolve_local_var_type(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(meta) = scope.vars.get(name) {
                return Some(meta.ty.clone());
            }
        }
        None
    }

    fn function_value_type(&self, name: &str) -> Option<Type> {
        let function = self.funcs.get(name)?;
        if !function.type_params.is_empty() {
            return None;
        }
        let span = function.span;
        let params = function.params.iter().map(|p| p.ty.clone()).collect();
        let ret = function
            .ret_type
            .clone()
            .unwrap_or_else(|| Type::Nulo(span));
        Some(Type::Function {
            params,
            ret: Box::new(ret),
            span,
        })
    }
    // @pinker-nav:end semantic.escopos.variaveis

    fn resolve_struct_field_type(
        &self,
        base_ty: &Type,
        field: &str,
        span: Span,
    ) -> Result<Type, PinkerError> {
        let Type::Struct { name, .. } = base_ty else {
            return Err(PinkerError::Semantic {
                msg: "acesso de campo exige base do tipo 'ninho'".to_string(),
                span,
            });
        };
        let struct_decl = self
            .structs
            .get(name)
            .ok_or_else(|| PinkerError::Semantic {
                msg: format!("tipo de struct '{}' não declarado", name),
                span,
            })?;
        let struct_field = struct_decl
            .fields
            .iter()
            .find(|candidate| candidate.name == field)
            .ok_or_else(|| PinkerError::Semantic {
                msg: format!("campo '{}' não existe em '{}'", field, name),
                span,
            })?;
        self.resolve_type_or_error(&struct_field.ty)
            .map(|ty| ty.with_span(span))
    }

    /// Parte B1: nenhum leque em que o runtime deposita tags pode ter chegado
    /// aqui com outra taxonomia.
    ///
    /// Complementa — não substitui — a conjunção do parser, que enxerga o nome
    /// de origem e produz o diagnóstico no span da declaração do usuário. Esta
    /// passagem olha o programa completo e por isso alcança o que a outra não
    /// pode alcançar: identidade reivindicada em **outro módulo** e nome
    /// monomórfico composto por um leque genérico de outro nome.
    ///
    /// A decisão continua sendo da autoridade única: aqui só se pergunta a ela,
    /// para cada superfície falível, se o leque materializado com o nome que ela
    /// declara diverge da taxonomia builtin.
    fn check_runtime_result_identity(
        &self,
        superficie: &crate::falha_operacional::SuperficieFalivel,
        span: Span,
    ) -> Result<(), PinkerError> {
        // A identidade injetiva impede que o template de usuário substitua o
        // leque builtin no mapa. A guarda #475 ainda precisa rejeitar a
        // coexistência quando o runtime efetivamente produz as tags. Como o
        // renderer é lossless, a própria autoridade recupera a proveniência;
        // a semântica não interpreta spelling nem mantém outro encoder.
        if let Some((nome, decl)) = self.enums.iter().find(|(nome, _)| {
            matches!(
                crate::generic_identity::specialization_template_identity(nome),
                Some(crate::generic_identity::GenericTemplateIdentity {
                    kind: crate::generic_identity::GenericKind::Enum,
                    origin,
                    ref local_name,
                }) if origin != crate::generic_identity::GenericOrigin::Builtin
                    && local_name == superficie.identidade()
            )
        }) {
            return Err(PinkerError::Semantic {
                msg: crate::falha_operacional::conflito_de_taxonomia(
                    &superficie.leque_monomorfico(),
                    &format!(
                        "a especialização '{nome}' veio de um template declarado pelo usuário"
                    ),
                ),
                span: decl.span,
            });
        }
        let monomorfico = superficie.leque_monomorfico();
        let Some(enum_decl) = self.enums.get(&monomorfico) else {
            // Sem leque materializado não há onde depositar a tag; o programa
            // falha adiante por tipo indefinido, com o diagnóstico daquela causa.
            return Ok(());
        };
        let Some(detalhe) = superficie.taxonomia_divergente(enum_decl) else {
            return Ok(());
        };
        // Span da declaração quando ela é do usuário; o predeclarado usa a
        // posição sintética 0:0, que não descreve nada — nesse caso o uso é a
        // melhor localização disponível.
        let posicao = if enum_decl.span == crate::falha_operacional::span_sintetico() {
            span
        } else {
            enum_decl.span
        };
        Err(PinkerError::Semantic {
            msg: crate::falha_operacional::conflito_de_taxonomia(&monomorfico, &detalhe),
            span: posicao,
        })
    }

    // @pinker-nav:start semantic.programa.duas-passagens
    // @pinker-nav:domain programa
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Entrada em duas passagens sobre o `Program`: passagem 1 valida importações e coleta funções, constantes, aliases, structs, leques e tratos em tabelas globais (detectando duplicações e conflitos de nome entre categorias, cargas de variante e recursão de alias/struct); passagem 2 dispara a verificação de contratos e de todos os corpos.
    /// Parte G: o identificador não resolve para identidade alguma?
    ///
    /// Só isto autoriza falar de família em posição de base: enquanto qualquer
    /// leitura histórica do nome existir, ela vence e a família se cala.
    fn nome_sem_identidade(&self, nome: &str) -> bool {
        self.resolve_var(nome).is_none()
            && self.resolve_enum_base_name(nome).is_none()
            && !self.funcs.contains_key(nome)
            && !self.consts.contains_key(nome)
            && !self.structs.contains_key(nome)
            && !self.enums.contains_key(nome)
            && !self.traits.contains_key(nome)
            && !self.type_aliases.contains_key(nome)
    }

    /// Parte G: dica de família não importada, emitida só como último recurso.
    ///
    /// A dica nasceu no parser e quebrava programa legado: lá não se sabe se o
    /// nome tem dono, e a Parte G recusava `x.campo` de qualquer ligação cujo
    /// tipo o parser não tivesse inferido. Aqui a pergunta é respondível — se
    /// nada reivindica o nome e a família exporta o membro, o programador quis
    /// a superfície nova e esqueceu o `trazer`.
    fn dica_de_familia_nao_importada(
        &self,
        base: &str,
        campo: &str,
        span: Span,
    ) -> Option<PinkerError> {
        if !crate::intrinsics::public_surface::forma_qualificada_valida(base, campo) {
            return None;
        }
        if !self.nome_sem_identidade(base) {
            return None;
        }
        // #532: quando existe módulo real homônimo da família, ele governa o
        // nome — e os itens dele entram COMO GRAFIA CRUA, não pela forma
        // qualificada. Mandar o leitor escrever `trazer <base>;` seria mandá-lo
        // repetir o que já escreveu: a autoridade de import consumiu esse
        // `trazer` como import de módulo, então o nome chega aqui sem
        // identidade e a dica de família mentiria o remédio.
        //
        // A entidade canônica `base.campo` é o que prova qual das duas leituras
        // está em jogo, e ela existe no programa projetado.
        let canonico = format!("{base}.{campo}");
        if self.funcs.contains_key(&canonico) {
            return Some(PinkerError::Semantic {
                msg: format!(
                    "'{base}' é um módulo Pinker, não uma família built-in; os itens de '{base}' entram com a própria grafia — escreva '{campo}(...)'"
                ),
                span,
            });
        }
        Some(PinkerError::Semantic {
            msg: crate::intrinsics::public_surface::familia_nao_importada(base, campo),
            span,
        })
    }

    // --- Passagem 1: declaração global ---
    // Registra funções e constantes antes de verificar qualquer corpo.
    // Erros aqui interrompem antes da passagem 2.
    pub fn check_program(&mut self, program: &Program) -> Result<(), PinkerError> {
        validate_intrinsic_declaration_conflicts(program)?;
        // Fases 186–188 — validação mínima de importações por família.
        // Recorte atual: apenas `trazer tempo;`, `trazer ambiente;`
        // e `trazer acaso;` são reconhecidos.
        // Importação seletiva (`trazer familia.simbolo;`) e demais famílias continuam rejeitadas.
        for import in &program.imports {
            validate_builtin_family_import(import)?;
            validate_family_import_collision(import, &program.items)?;
            // `trazer tempo;`, `trazer ambiente;` e `trazer acaso;` são válidos
            // — as intrínsecas dessas famílias já estão disponíveis globalmente.
        }

        for item in &program.items {
            match item {
                Item::Function(function) => {
                    validar_namespace_pinker_owned(&function.name, function.span)?;
                    if self.funcs.contains_key(&function.name)
                        && method_identity::parse_provisional_function_name(&function.name)
                            .is_none()
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!("função '{}' já declarada", function.name),
                            span: function.span,
                        });
                    }
                    if self.consts.contains_key(&function.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por uma constante global",
                                function.name
                            ),
                            span: function.span,
                        });
                    }
                    self.funcs.insert(function.name.clone(), function.clone());
                }
                Item::Const(constant) => {
                    validar_namespace_pinker_owned(&constant.name, constant.span)?;
                    if self.consts.contains_key(&constant.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("constante '{}' já declarada", constant.name),
                            span: constant.span,
                        });
                    }
                    if self.funcs.contains_key(&constant.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("nome '{}' já utilizado por uma função", constant.name),
                            span: constant.span,
                        });
                    }
                    self.consts.insert(constant.name.clone(), constant.clone());
                }
                Item::TypeAlias(alias) => {
                    if self.type_aliases.contains_key(&alias.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("alias de tipo '{}' já declarado", alias.name),
                            span: alias.span,
                        });
                    }
                    if self.funcs.contains_key(&alias.name)
                        || self.consts.contains_key(&alias.name)
                        || self.enums.contains_key(&alias.name)
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por função/constante/leque",
                                alias.name
                            ),
                            span: alias.span,
                        });
                    }
                    self.type_aliases
                        .insert(alias.name.clone(), alias.target.clone());
                }
                Item::Struct(struct_decl) => {
                    if self.structs.contains_key(&struct_decl.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("struct '{}' já declarada", struct_decl.name),
                            span: struct_decl.span,
                        });
                    }
                    if self.funcs.contains_key(&struct_decl.name)
                        || self.consts.contains_key(&struct_decl.name)
                        || self.type_aliases.contains_key(&struct_decl.name)
                        || self.enums.contains_key(&struct_decl.name)
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por função/constante/alias de tipo/leque",
                                struct_decl.name
                            ),
                            span: struct_decl.span,
                        });
                    }
                    self.structs
                        .insert(struct_decl.name.clone(), struct_decl.clone());
                }
                Item::Enum(enum_decl) => {
                    if self.enums.contains_key(&enum_decl.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("leque '{}' já declarado", enum_decl.name),
                            span: enum_decl.span,
                        });
                    }
                    if self.funcs.contains_key(&enum_decl.name)
                        || self.consts.contains_key(&enum_decl.name)
                        || self.type_aliases.contains_key(&enum_decl.name)
                        || self.structs.contains_key(&enum_decl.name)
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por função/constante/alias/struct",
                                enum_decl.name
                            ),
                            span: enum_decl.span,
                        });
                    }
                    if enum_decl.variants.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "leque '{}' deve ter ao menos uma variante",
                                enum_decl.name
                            ),
                            span: enum_decl.span,
                        });
                    }
                    let mut seen_variants = HashSet::new();
                    for variant in &enum_decl.variants {
                        if !seen_variants.insert(variant.name.as_str()) {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "variante '{}' duplicada no leque '{}'",
                                    variant.name, enum_decl.name
                                ),
                                span: variant.span,
                            });
                        }
                    }
                    self.enums.insert(enum_decl.name.clone(), enum_decl.clone());
                }
                Item::Trait(trait_decl) => {
                    if self.traits.contains_key(&trait_decl.name) {
                        return Err(PinkerError::Semantic {
                            msg: format!("trato '{}' já declarado", trait_decl.name),
                            span: trait_decl.span,
                        });
                    }
                    if self.funcs.contains_key(&trait_decl.name)
                        || self.consts.contains_key(&trait_decl.name)
                        || self.type_aliases.contains_key(&trait_decl.name)
                        || self.structs.contains_key(&trait_decl.name)
                        || self.enums.contains_key(&trait_decl.name)
                    {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "nome '{}' já utilizado por função/constante/alias/struct/leque",
                                trait_decl.name
                            ),
                            span: trait_decl.span,
                        });
                    }
                    let mut seen_methods = HashSet::new();
                    for method in &trait_decl.methods {
                        if !seen_methods.insert(method.name.as_str()) {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "método '{}' duplicado no trato '{}'",
                                    method.name, trait_decl.name
                                ),
                                span: method.span,
                            });
                        }
                    }
                    self.traits
                        .insert(trait_decl.name.clone(), trait_decl.clone());
                }
            }
        }

        for alias_target in self.type_aliases.values() {
            self.resolve_type_or_error(alias_target)?;
        }
        for struct_decl in self.structs.values() {
            self.validate_struct_decl(struct_decl)?;
        }
        // Cargas de variantes são validadas após a coleta completa para
        // permitir referência a leque declarado depois (inclusive recursiva).
        for enum_decl in self.enums.values() {
            for variant in &enum_decl.variants {
                for payload in &variant.payloads {
                    // A validade da carga vem da autoridade única de
                    // classificação (D1), nunca de um `match` parcial local:
                    // ela resolve apelidos em profundidade, resolve o elemento
                    // de `lista<E>` e recusa com motivo estável.
                    if let Err(rejection) = self.classify_enum_payload(payload) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "carga da variante '{}' deve ser {}; {}",
                                variant.name,
                                crate::enum_payload::CONTRATO_CARGAS,
                                rejection.message()
                            ),
                            span: payload.span(),
                        });
                    }
                }
            }
        }

        // A relação nominal é decidida antes de qualquer método ser
        // materializado: cardinalidade da declaração primeiro, cobertura do
        // contrato depois.
        self.validate_impl_relations(program)?;
        self.register_impl_methods(program)?;
        self.validate_impl_contracts(program)?;
        self.validate_trait_contracts()?;

        // --- Passagem 2: verificação de corpos ---
        self.check_principal(program)?;

        for item in &program.items {
            match item {
                // Fase 243: closures (`__anon_carinho_*`) são checadas
                // lazily no ponto de criação (`resolve_closure_value`), com
                // o ambiente léxico correto — não aqui, isoladas.
                Item::Function(function)
                    if crate::anonymous_identity::is_anonymous_callable_name(&function.name) => {}
                Item::Function(function) => self.check_function(function)?,
                Item::Const(constant) => self.check_const_body(constant)?,
                Item::TypeAlias(_) | Item::Struct(_) | Item::Enum(_) | Item::Trait(_) => {}
            }
        }

        // Fase 243: closure sintética nunca resolvida como valor (idioma de
        // chamada imediata `carinho(...) {...}(x)`, Fase 225) nunca passa
        // por `resolve_closure_value` — permanece uma função comum, sem
        // `__env`, igual ao comportamento anterior à Fase 243. Só closures
        // genuinamente usadas como valor recebem a convenção uniforme.
        for item in &program.items {
            if let Item::Function(function) = item {
                if crate::anonymous_identity::is_anonymous_callable_name(&function.name)
                    && !self.checked_closures.contains(&function.name)
                {
                    self.checked_closures.insert(function.name.clone());
                    self.check_function(function)?;
                }
            }
        }

        Ok(())
    }
    // @pinker-nav:end semantic.programa.duas-passagens

    // @pinker-nav:start semantic.funcoes.verificacao
    // @pinker-nav:domain funcoes
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de corpos de topo: política fixa de `principal` (sem parâmetros, retorno `bombom`), checagem de constante (tipo do inicializador e faixa) e de função (parâmetros no escopo, corpo, e alcançabilidade de retorno em todos os caminhos simples quando há retorno declarado), redigindo identidades sintéticas de callables anônimos nos diagnósticos.
    // `principal` é a política fixa de entrada da v0: sem parâmetros e retorno bombom.
    fn check_principal(&self, program: &Program) -> Result<(), PinkerError> {
        let Some(main_fn) = self.funcs.get("principal") else {
            let msg = if program.freestanding.is_some() {
                "função 'principal' (boot entry desta fase em modo `livre`) não encontrada"
                    .to_string()
            } else {
                "função 'principal' (entry point) não encontrada".to_string()
            };
            return Err(PinkerError::Semantic {
                msg,
                span: Self::root_span(program),
            });
        };

        if !main_fn.params.is_empty() {
            return Err(PinkerError::Semantic {
                msg: "a função 'principal' não deve ter parâmetros".to_string(),
                span: main_fn.span,
            });
        }

        let resolved_ret = main_fn
            .ret_type
            .as_ref()
            .map(|ty| self.resolve_type_or_error(ty))
            .transpose()?;
        match resolved_ret {
            Some(Type::Bombom(_)) => Ok(()),
            _ => Err(PinkerError::Semantic {
                msg: "a função 'principal' deve declarar retorno 'bombom'".to_string(),
                span: main_fn.span,
            }),
        }
    }

    fn check_const_body(&mut self, constant: &ConstDecl) -> Result<(), PinkerError> {
        let resolved_const_ty = self.resolve_type_or_error(&constant.ty)?;
        self.push_scope();
        let init_ty = self.check_value_expr(
            &constant.init,
            "resultado de função sem retorno não pode inicializar constante",
        )?;
        self.pop_scope();

        if !Self::check_expected_type_for_expr(&resolved_const_ty, &init_ty, &constant.init) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "tipo incompatível na constante '{}': esperado '{}', encontrado '{}'",
                    constant.name,
                    resolved_const_ty.name(),
                    init_ty.name()
                ),
                span: constant.init.span,
            });
        }
        Self::validate_int_literal_range(&resolved_const_ty, &constant.init)?;

        Ok(())
    }

    fn check_function(&mut self, function: &FunctionDecl) -> Result<(), PinkerError> {
        self.current_func_name = Some(Self::function_name_for_diagnostic(&function.name));
        self.current_func_ret = function
            .ret_type
            .as_ref()
            .map(|ty| self.resolve_type_or_error(ty))
            .transpose()?;
        self.loop_depth = 0;
        self.push_scope();

        // Parâmetros entram no escopo da função antes do corpo (não são mutáveis).
        for param in &function.params {
            let resolved_param_ty = self.resolve_type_or_error(&param.ty)?;
            self.declare_var(&param.name, resolved_param_ty, false, param.span)?;
        }

        self.check_block(&function.body, true)?;

        // A v0 só resolve fluxo simples: sequência, blocos e cadeias de talvez/senao.
        if self.current_func_ret.is_some() && !self.block_returns(&function.body) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "função '{}' com retorno declarado não retorna em todos os caminhos simples",
                    Self::function_name_for_diagnostic(&function.name)
                ),
                span: function.body.span,
            });
        }

        self.pop_scope();
        self.current_func_name = None;
        self.current_func_ret = None;
        self.loop_depth = 0;
        Ok(())
    }

    // Fase 243: resolve um literal `carinho` (Fase 225) no ponto exato onde
    // seu `Ident` sintético aparece como valor — momento em que `self.scopes`
    // reflete o escopo léxico realmente vigente na criação. Cada identificador
    // livre do corpo (varredura sintática de `ast::free_identifiers_in_function`)
    // que resolve para uma variável local (`resolve_local_var_type`, não
    // `self.funcs`/`self.consts`) é uma captura por valor; os demais resolvem
    // normalmente dentro do próprio corpo (função top-level, constante,
    // variante de leque) e não geram captura alguma. Resolvida uma única vez
    // por closure — chamadas repetidas devolvem o tipo já calculado sem
    // recomputar nem re-checar o corpo.
    fn resolve_closure_value(&mut self, name: &str, span: Span) -> Result<Type, PinkerError> {
        if self.checked_closures.contains(name) {
            return self
                .function_value_type(name)
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!("closure '{}' não encontrada após resolução", name),
                    span,
                });
        }
        let function = self
            .funcs
            .get(name)
            .cloned()
            .ok_or_else(|| PinkerError::Semantic {
                msg: format!("closure '{}' não declarada", name),
                span,
            })?;
        let param_names: HashSet<String> = function.params.iter().map(|p| p.name.clone()).collect();
        let free = transitive_free_identifiers_in_function(&function, |name| {
            self.funcs.get(name).cloned()
        });
        let mut captures = Vec::new();
        for candidate in &free {
            if param_names.contains(candidate) {
                continue;
            }
            let Some(meta) = self.resolve_local_var_type(candidate) else {
                continue;
            };
            // A admissibilidade da captura segue a representação canônica
            // já transportável pela ABI. Isso inclui handles opacos de uma
            // palavra como callables e objetos de trato, sem aceitar por
            // acidente todo `Type::Applied`.
            let struct_names = self.structs.keys().cloned().collect::<HashSet<_>>();
            let capture_ir =
                TypeIR::from_ast_with_context(&meta, &self.type_aliases, &struct_names).map_err(
                    |error| PinkerError::Semantic {
                        msg: format!(
                            "captura de '{}' não possui representação nativa válida: {}",
                            candidate, error
                        ),
                        span,
                    },
                )?;
            if !capture_ir.is_closure_environment_word() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "captura de '{}' com tipo '{}' não suportada nesta fase (apenas tipos de 1 palavra)",
                        candidate,
                        meta.name()
                    ),
                    span,
                });
            }
            captures.push((candidate.clone(), meta));
        }
        self.closure_captures
            .insert(name.to_string(), captures.clone());
        self.checked_closures.insert(name.to_string());
        self.check_closure_function(&function, &captures)?;
        self.function_value_type(name)
            .ok_or_else(|| PinkerError::Semantic {
                msg: format!("closure '{}' sem tipo função válido", name),
                span,
            })
    }

    // Checa o corpo de uma closure com duas camadas de escopo: capturas
    // (imutáveis, resolvidas no momento da criação) por baixo, parâmetros da
    // própria closure por cima — permitindo que um parâmetro sombreie uma
    // captura homônima (§14.3) sem violar a proibição de sombreamento no
    // mesmo escopo. Atribuir a uma captura já é rejeitado pela checagem de
    // mutabilidade existente (`declare_var(..., is_mut=false, ...)`), sem
    // diagnóstico dedicado.
    fn check_closure_function(
        &mut self,
        function: &FunctionDecl,
        captures: &[(String, Type)],
    ) -> Result<(), PinkerError> {
        let saved_func_name = self.current_func_name.take();
        let saved_func_ret = self.current_func_ret.take();
        let saved_loop_depth = self.loop_depth;

        self.current_func_name = Some(Self::function_name_for_diagnostic(&function.name));
        self.current_func_ret = function
            .ret_type
            .as_ref()
            .map(|ty| self.resolve_type_or_error(ty))
            .transpose()?;
        self.loop_depth = 0;

        self.push_scope();
        for (capture_name, capture_ty) in captures {
            self.declare_var(capture_name, capture_ty.clone(), false, function.span)?;
        }
        self.push_scope();
        for param in &function.params {
            let resolved_param_ty = self.resolve_type_or_error(&param.ty)?;
            self.declare_var(&param.name, resolved_param_ty, false, param.span)?;
        }

        self.check_block(&function.body, true)?;

        if self.current_func_ret.is_some() && !self.block_returns(&function.body) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "função '{}' com retorno declarado não retorna em todos os caminhos simples",
                    Self::function_name_for_diagnostic(&function.name)
                ),
                span: function.body.span,
            });
        }

        self.pop_scope();
        self.pop_scope();
        self.current_func_name = saved_func_name;
        self.current_func_ret = saved_func_ret;
        self.loop_depth = saved_loop_depth;
        Ok(())
    }

    fn function_name_for_diagnostic(name: &str) -> String {
        if crate::anonymous_identity::is_anonymous_callable_name(name) {
            "<anônima>".to_string()
        } else {
            name.to_string()
        }
    }
    // @pinker-nav:end semantic.funcoes.verificacao

    fn check_inline_asm(&mut self, stmt: &InlineAsmStmt) -> Result<(), PinkerError> {
        let semantic_error = |msg: String, span: Span| PinkerError::Semantic { msg, span };
        if stmt.chunks.is_empty() {
            return Err(semantic_error(
                "'sussurro' exige ao menos uma string literal".to_string(),
                stmt.span,
            ));
        }
        if stmt.chunks.iter().any(|chunk| chunk.trim().is_empty()) {
            return Err(semantic_error(
                "bloco de 'sussurro' não pode conter string vazia".to_string(),
                stmt.span,
            ));
        }

        let mut template_references = HashSet::new();
        for chunk in &stmt.chunks {
            validate_inline_asm_chunk(chunk, stmt.span)?;
            let parts = crate::inline_asm::parse_template(chunk)
                .map_err(|error| semantic_error(error.to_string(), stmt.span))?;
            for part in parts {
                if let crate::inline_asm::AsmTemplatePart::Operand(name) = part {
                    template_references.insert(name);
                }
            }
        }

        let mut clobber_names = HashSet::new();
        let mut clobbers = Vec::new();
        for clobber in &stmt.clobbers {
            if !clobber_names.insert(clobber.name.clone()) {
                return Err(semantic_error(
                    format!(
                        "E-SEMANTIC-ASM-CLOBBER-CONFLICT\nclobber '{}' duplicado em 'sussurro'",
                        clobber.name
                    ),
                    clobber.span,
                ));
            }
            clobbers.push(
                crate::inline_asm::parse_clobber(&clobber.name)
                    .map_err(|error| semantic_error(error.to_string(), clobber.span))?,
            );
        }

        let mut binding_names = HashSet::new();
        let mut output_targets = HashSet::new();
        let mut constraints = Vec::new();
        for operand in &stmt.operands {
            if !binding_names.insert(operand.name.clone()) {
                return Err(semantic_error(
                    format!(
                        "E-SEMANTIC-ASM-DUPLICATE-OPERAND\noperando '{}' duplicado em 'sussurro'",
                        operand.name
                    ),
                    operand.span,
                ));
            }
            let constraint = crate::inline_asm::parse_constraint(&operand.constraint)
                .map_err(|error| semantic_error(error.to_string(), operand.span))?;
            constraints.push((operand.name.clone(), constraint));

            let ty = match &operand.direction {
                InlineAsmDirection::Input => self.check_value_expr(
                    &operand.value,
                    "resultado de função sem retorno não pode ser operando de entrada de 'sussurro'",
                )?,
                InlineAsmDirection::Output => {
                    let ExprKind::Ident(target) = &operand.value.kind else {
                        return Err(semantic_error(
                            "E-SEMANTIC-ASM-INVALID-OUTPUT\nsaida de 'sussurro' exige variável mutável simples como alvo".to_string(),
                            operand.value.span,
                        ));
                    };
                    let Some(meta) = self.resolve_var(target) else {
                        return Err(semantic_error(
                            format!(
                                "E-SEMANTIC-ASM-INVALID-OUTPUT\nvariável de saída '{}' não declarada",
                                target
                            ),
                            operand.value.span,
                        ));
                    };
                    if !meta.is_mut {
                        return Err(semantic_error(
                            format!(
                                "E-SEMANTIC-ASM-INVALID-OUTPUT\nvariável de saída '{}' não é mutável",
                                target
                            ),
                            operand.value.span,
                        ));
                    }
                    if !output_targets.insert(target.clone()) {
                        return Err(semantic_error(
                            format!(
                                "E-SEMANTIC-ASM-AMBIGUOUS-BINDING\nalvo de saída '{}' aparece mais de uma vez",
                                target
                            ),
                            operand.value.span,
                        ));
                    }
                    meta.ty
                }
                InlineAsmDirection::Unknown(direction) => {
                    return Err(semantic_error(
                        format!(
                            "E-SEMANTIC-ASM-DIRECTION\ndireção de operando desconhecida: '{}'",
                            direction
                        ),
                        operand.span,
                    ));
                }
            };
            let ty = self.resolve_type_or_error(&ty)?;
            if !is_inline_asm_operand_type(&ty) {
                return Err(semantic_error(
                    format!(
                        "E-SEMANTIC-ASM-UNSUPPORTED-TYPE\ntipo '{}' não possui representação nativa autorizada como operando de 'sussurro'",
                        ty.name()
                    ),
                    operand.value.span,
                ));
            }
        }

        for reference in &template_references {
            if !binding_names.contains(reference) {
                return Err(semantic_error(
                    format!(
                        "{}\noperando '{{{}}}' não foi declarado em 'sussurro'",
                        crate::inline_asm::E_ASM_UNKNOWN_OPERAND,
                        reference
                    ),
                    stmt.span,
                ));
            }
        }
        for binding in &binding_names {
            if !template_references.contains(binding) {
                return Err(semantic_error(
                    format!(
                        "E-SEMANTIC-ASM-AMBIGUOUS-BINDING\noperando '{}' foi declarado mas não aparece no template",
                        binding
                    ),
                    stmt.span,
                ));
            }
        }

        crate::inline_asm::allocate_registers(&constraints, &clobbers)
            .map_err(|error| semantic_error(error.to_string(), stmt.span))?;
        crate::inline_asm::validate_abi_contract(
            &stmt.chunks,
            !stmt.operands.is_empty() || !stmt.clobbers.is_empty(),
            &clobbers,
        )
        .map_err(|error| semantic_error(error.to_string(), stmt.span))?;
        Ok(())
    }

    fn check_enum_match(&mut self, enum_match: &EnumMatchStmt) -> Result<(), PinkerError> {
        if let Some(EnumPattern::Variant {
            enum_name, span, ..
        }) = enum_match.arms.first().map(|arm| &arm.pattern)
        {
            if self.resolve_enum_base_name(enum_name).is_none() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "encaixe usa leque '{}' não declarado antes deste ponto",
                        enum_name
                    ),
                    span: *span,
                });
            }
        }
        let scrutinee_ty = self.check_value_expr(
            &enum_match.scrutinee,
            "resultado de função sem retorno não pode ser inspecionado por 'encaixe'",
        )?;
        let scrutinee_ty = self.resolve_type_or_error(&scrutinee_ty)?;
        if !matches!(scrutinee_ty, Type::Enum { .. }) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "PATTERN_NOT_APPLICABLE_TO_PAYLOAD: 'encaixe' de leque exige scrutinee de leque; encontrado '{}'",
                    scrutinee_ty.name()
                ),
                span: enum_match.scrutinee.span,
            });
        }

        let mut previous = Vec::<&EnumPattern>::new();
        for arm in &enum_match.arms {
            let mut bindings = Vec::new();
            let mut binding_names = HashSet::new();
            self.check_enum_pattern(
                &arm.pattern,
                &scrutinee_ty,
                &mut bindings,
                &mut binding_names,
                0,
            )?;
            if let Some(earlier) = previous
                .iter()
                .find(|earlier| Self::enum_pattern_covers(earlier, &arm.pattern))
            {
                let message = if Self::enum_pattern_covers(&arm.pattern, earlier) {
                    let variant = match &arm.pattern {
                        EnumPattern::Variant { variant, .. } => variant.as_str(),
                        EnumPattern::Binding { .. } => "_",
                    };
                    format!("variante '{}' repetida no encaixe", variant)
                } else {
                    "UNREACHABLE_PATTERN: padrão de 'caso' já coberto por braço anterior"
                        .to_string()
                };
                return Err(PinkerError::Semantic {
                    msg: message,
                    span: arm.span,
                });
            }
            previous.push(&arm.pattern);

            self.push_scope();
            let checked = bindings
                .into_iter()
                .try_for_each(|(name, ty, span)| self.declare_var(&name, ty, false, span))
                .and_then(|()| self.check_block(&arm.body, true));
            self.pop_scope();
            checked?;
        }

        if enum_match.otherwise.is_none() {
            if let Some(gap) = self.enum_pattern_coverage_gap(&scrutinee_ty, &previous)? {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "NON_EXHAUSTIVE_NESTED_MATCH: encaixe não cobre {gap}; adicione o caso ou um 'senao'"
                    ),
                    span: enum_match.span,
                });
            }
        }
        if let Some(otherwise) = &enum_match.otherwise {
            self.check_block(otherwise, false)?;
        }
        Ok(())
    }

    fn check_enum_pattern(
        &self,
        pattern: &EnumPattern,
        expected: &Type,
        bindings: &mut Vec<(String, Type, Span)>,
        binding_names: &mut HashSet<String>,
        depth: usize,
    ) -> Result<(), PinkerError> {
        match pattern {
            EnumPattern::Binding { name, span } => {
                if !binding_names.insert(name.clone()) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "UNREACHABLE_PATTERN: binding '{}' repetido no mesmo padrão",
                            name
                        ),
                        span: *span,
                    });
                }
                bindings.push((name.clone(), expected.clone().with_span(*span), *span));
                Ok(())
            }
            EnumPattern::Variant {
                enum_name,
                variant,
                payloads,
                span,
            } => {
                let expected = self.resolve_type_or_error(expected)?;
                let Type::Enum {
                    name: expected_name,
                    ..
                } = expected
                else {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "PATTERN_NOT_APPLICABLE_TO_PAYLOAD: padrão '{}.{}' não se aplica à carga '{}'",
                            enum_name,
                            variant,
                            expected.name()
                        ),
                        span: *span,
                    });
                };
                let Some(pattern_enum_name) = self.resolve_enum_base_name(enum_name) else {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "INVALID_NESTED_PATTERN_TYPE: leque '{}' do padrão não declarado",
                            enum_name
                        ),
                        span: *span,
                    });
                };
                let expected_enum_name = self
                    .resolve_enum_base_name(&expected_name)
                    .unwrap_or(expected_name.clone());
                if pattern_enum_name != expected_enum_name {
                    if depth == 0 {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "encaixe mistura leques diferentes: '{}' e '{}'",
                                expected_enum_name, enum_name
                            ),
                            span: *span,
                        });
                    }
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "INVALID_NESTED_PATTERN_TYPE: esperado padrão do leque '{}', encontrado '{}.{}'",
                            expected_enum_name, enum_name, variant
                        ),
                        span: *span,
                    });
                }
                let enum_decl = self
                    .enums
                    .get(&expected_enum_name)
                    .expect("nome de leque resolvido acima");
                let Some(variant_decl) = enum_decl
                    .variants
                    .iter()
                    .find(|candidate| candidate.name == *variant)
                else {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "INVALID_NESTED_PATTERN_TYPE: variante '{}' não existe no leque '{}'",
                            variant, expected_enum_name
                        ),
                        span: *span,
                    });
                };
                if variant_decl.payloads.len() != payloads.len() {
                    let legacy_message = if depth == 0 {
                        match (variant_decl.payloads.len(), payloads.len()) {
                        (0, actual) if actual > 0 => Some(format!(
                            "variante '{}' não carrega valor; use 'caso {}.{}' sem parênteses",
                            variant, expected_enum_name, variant
                        )),
                        (expected, 0) if expected > 0 => Some(format!(
                            "variante '{}' carrega {} valor(es); use 'caso {}.{}(...)' com {} nome(s)",
                            variant, expected, expected_enum_name, variant, expected
                        )),
                        (expected, actual)
                            if payloads
                                .iter()
                                .all(|payload| matches!(payload, EnumPattern::Binding { .. })) =>
                        {
                            Some(format!(
                                "variante '{}' carrega {} valor(es), mas o caso liga {} nome(s)",
                                variant, expected, actual
                            ))
                        }
                        _ => None,
                        }
                    } else {
                        None
                    };
                    return Err(PinkerError::Semantic {
                        msg: legacy_message.unwrap_or_else(|| format!(
                                "INVALID_PATTERN_PAYLOAD_ARITY: variante '{}.{}' carrega {} valor(es), mas o padrão possui {}",
                                expected_enum_name,
                                variant,
                                variant_decl.payloads.len(),
                                payloads.len()
                            )),
                        span: *span,
                    });
                }
                if payloads.len() > 1
                    && payloads
                        .iter()
                        .any(|payload| matches!(payload, EnumPattern::Variant { .. }))
                {
                    return Err(PinkerError::Semantic {
                        msg: "PATTERN_NOT_APPLICABLE_TO_PAYLOAD: decomposição aninhada de variante com múltiplas cargas permanece fora do contrato D10"
                            .to_string(),
                        span: *span,
                    });
                }
                for (payload, payload_ty) in payloads.iter().zip(&variant_decl.payloads) {
                    let shape = self.classify_enum_payload(payload_ty).map_err(|rejection| {
                        PinkerError::Semantic {
                            msg: format!(
                                "PATTERN_NOT_APPLICABLE_TO_PAYLOAD: carga de '{}.{}' não é decomponível: {}",
                                expected_enum_name,
                                variant,
                                rejection.message()
                            ),
                            span: payload.span(),
                        }
                    })?;
                    self.check_enum_pattern(
                        payload,
                        &shape.resolved,
                        bindings,
                        binding_names,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn enum_pattern_covers(earlier: &EnumPattern, later: &EnumPattern) -> bool {
        match (earlier, later) {
            (EnumPattern::Binding { .. }, _) => true,
            (
                EnumPattern::Variant {
                    variant: earlier_variant,
                    payloads: earlier_payloads,
                    ..
                },
                EnumPattern::Variant {
                    variant: later_variant,
                    payloads: later_payloads,
                    ..
                },
            ) => {
                earlier_variant == later_variant
                    && earlier_payloads.len() == later_payloads.len()
                    && earlier_payloads
                        .iter()
                        .zip(later_payloads)
                        .all(|(earlier, later)| Self::enum_pattern_covers(earlier, later))
            }
            _ => false,
        }
    }

    fn enum_pattern_coverage_gap(
        &self,
        expected: &Type,
        patterns: &[&EnumPattern],
    ) -> Result<Option<String>, PinkerError> {
        if patterns
            .iter()
            .any(|pattern| matches!(pattern, EnumPattern::Binding { .. }))
        {
            return Ok(None);
        }
        let expected = self.resolve_type_or_error(expected)?;
        let Type::Enum { name, .. } = expected else {
            return Ok(Some(format!("a carga de tipo '{}'", expected.name())));
        };
        let enum_name = self.resolve_enum_base_name(&name).unwrap_or(name);
        let enum_decl = self
            .enums
            .get(&enum_name)
            .expect("nome de leque resolvido para cobertura");
        for variant_decl in &enum_decl.variants {
            let matching = patterns
                .iter()
                .filter_map(|pattern| match pattern {
                    EnumPattern::Variant {
                        variant, payloads, ..
                    } if *variant == variant_decl.name => Some(payloads),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if matching.is_empty() {
                return Ok(Some(format!(
                    "a variante '{}' do leque '{}'",
                    variant_decl.name, enum_name
                )));
            }
            if variant_decl.payloads.is_empty()
                || matching.iter().any(|payloads| {
                    payloads
                        .iter()
                        .all(|payload| matches!(payload, EnumPattern::Binding { .. }))
                })
            {
                continue;
            }
            if variant_decl.payloads.len() == 1 {
                let shape = self
                    .classify_enum_payload(&variant_decl.payloads[0])
                    .map_err(|rejection| PinkerError::Semantic {
                        msg: format!(
                            "PATTERN_NOT_APPLICABLE_TO_PAYLOAD: cobertura de '{}.{}': {}",
                            enum_name,
                            variant_decl.name,
                            rejection.message()
                        ),
                        span: variant_decl.span,
                    })?;
                let children = matching
                    .iter()
                    .filter_map(|payloads| payloads.first())
                    .collect::<Vec<_>>();
                if let Some(inner) = self.enum_pattern_coverage_gap(&shape.resolved, &children)? {
                    return Ok(Some(format!(
                        "o subpadrão '{}.{} -> {}'",
                        enum_name, variant_decl.name, inner
                    )));
                }
            } else {
                return Ok(Some(format!(
                    "todas as cargas da variante '{}.{}'",
                    enum_name, variant_decl.name
                )));
            }
        }
        Ok(None)
    }
}

/// Valida um pedaço de `sussurro` pela política estrutural de statements.
///
/// A completude não vem de uma lista de diretivas proibidas: depois da remoção
/// de labels e comentários, toda diretiva do assembler começa um statement com
/// `.` e é rejeitada por construção.
fn validate_inline_asm_chunk(chunk: &str, span: Span) -> Result<(), PinkerError> {
    crate::inline_asm::scan_chunk(chunk)
        .map(|_| ())
        .map_err(|error| PinkerError::Semantic {
            msg: error.to_string(),
            span,
        })
}

fn is_inline_asm_operand_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Bombom(_)
            | Type::U8(_)
            | Type::U16(_)
            | Type::U32(_)
            | Type::U64(_)
            | Type::I8(_)
            | Type::I16(_)
            | Type::I32(_)
            | Type::I64(_)
            | Type::Logica(_)
            | Type::Pointer { .. }
    )
}

pub fn check_program(program: &Program) -> Result<(), PinkerError> {
    SemanticChecker::new().check_program(program)
}

/// Verifica um programa composto, respeitando o ambiente de cada unidade-fonte
/// no despacho de método.
pub fn check_program_composto(
    program: &Program,
    traits_visiveis_por_fonte: HashMap<SourceId, crate::module_resolve::TratosNoDespacho>,
    fontes_de_modulo: HashSet<SourceId>,
) -> Result<(), PinkerError> {
    SemanticChecker::com_visibilidade_de_tratos(traits_visiveis_por_fonte, fontes_de_modulo)
        .check_program(program)
}

// @pinker-nav:start semantic.modulos.validacao-local
// @pinker-nav:domain modulos
// @pinker-nav:layer semantica
// @pinker-nav:summary check_module_unit valida uma unidade-fonte COMO MÓDULO, sem exigir `principal`: aplica à unidade as regras de declaração que dependem de dados do próprio Program — a política de redeclaração de intrínsecas públicas da PR #507, a validação de import de família built-in e a colisão entre import de família e item homônimo. Sao exatamente as obrigacoes cujo gatilho desaparecia quando `imports` e `items` do modulo eram descartados antes de qualquer validacao, fazendo com que a mesma fonte recusada como raiz passasse a ser aceita ao virar modulo.
/// Valida uma unidade-fonte **como módulo**.
///
/// `MODULE_VALIDATION_INPUT_PRESERVATION`: para toda regra V aplicável a um
/// módulo M, se V depende de informação I presente na unidade-fonte M, então V
/// roda antes de qualquer transformação que descarte I.
///
/// Aqui ficam as regras cujo GATILHO é a própria unidade e que, portanto,
/// desapareciam junto com ela: se `imports` do módulo somem, a regra de import
/// de família nunca nasce; se um item do módulo não é materializado, a política
/// de propriedade de grafia nunca é consultada sobre ele. Não é uma obrigação
/// criada e depois perdida — é uma obrigação que deixava de ser criada.
///
/// Esta entrada NÃO exige `principal`. Um módulo não é um programa raiz e
/// exigir dele o ponto de entrada era a razão pela qual não existia modo
/// algum de validar uma unidade como módulo.
pub fn check_module_unit(program: &Program) -> Result<(), PinkerError> {
    // Política de intrínsecas públicas da PR #507. Mover o mesmo código para
    // dentro de um módulo deixava de disparar a regra.
    validate_intrinsic_declaration_conflicts(program)?;

    for import in &program.imports {
        // Import de família inválido dentro de módulo deixava de ser validado.
        validate_builtin_family_import(import)?;
        // Colisão entre import de família e item homônimo do módulo, idem.
        validate_family_import_collision(import, &program.items)?;
    }

    Ok(())
}
// @pinker-nav:end semantic.modulos.validacao-local
