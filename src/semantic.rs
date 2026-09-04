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
use crate::method_identity::{self, MethodIdentity};
use crate::module_resolve::NivelDeDespacho;
use crate::source_map::SourceId;
use crate::token::{Position, Span};
use crate::union_canon;
use std::collections::{BTreeMap, HashMap, HashSet};

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

    /// Por qual nível um trato alcança o ponto em que a chamada foi escrita?
    ///
    /// Sem índice não há composição modular e nada é filtrado. Span sintético
    /// não reivindica fonte e também não é filtrado: restringir por ausência
    /// de alegação recusaria o que o compilador ele mesmo materializou.
    fn nivel_de_despacho(
        &self,
        span: Span,
        trait_name: &str,
        fonte_da_relacao: Option<SourceId>,
    ) -> Option<NivelDeDespacho> {
        crate::module_resolve::nivel_de_despacho(
            &self.traits_visiveis_por_fonte,
            span,
            trait_name,
            fonte_da_relacao,
        )
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
                Item::Function(function) if function.name.starts_with("__anon_carinho_") => {}
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
                if function.name.starts_with("__anon_carinho_")
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

    // @pinker-nav:start semantic.tratos.contratos
    // @pinker-nav:domain tratos
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Autoridade semântica de relações, métodos e contratos de tratos. `validate_impl_relations` vem primeiro e é a única autoridade de cardinalidade da relação nominal: cada `ImplDecl` de `program.impls` vira a identidade `(trato canônico, alvo canônico)` — o mesmo `union_canon` que a identidade de método usa — e a segunda declaração da mesma identidade é recusada, sem olhar quantos métodos explícitos cada bloco materializou; bloco vazio continua sendo declaração da relação. Depois, `register_impl_methods` resolve integralmente o tipo-alvo declarado transportado em `ImplFunctionFacts`, deriva sua chave por `union_canon`, registra `MethodIdentity(trato, tipo resolvido, método)` e compara separadamente o receiver resolvido; `method_index` é somente a visão derivada para chamadas não qualificadas, e a recusa de método repetido continua endereçando repetição dentro do mesmo bloco. Por último, `validate_impl_contracts` agrupa os métodos já materializados pela identidade resolvida e cobra cobertura do contrato do trato: ausência de método requerido é erro de cobertura, nunca duplicata.
    /// Cardinalidade da relação nominal de `impl`, antes de qualquer
    /// materialização de método.
    ///
    /// A relação existe porque a declaração existe: `impl T para X {}` é a
    /// mesma relação que `impl T para X { ... }`, e duas declarações da mesma
    /// identidade canônica `(trato, alvo)` são uma duplicata mesmo quando uma
    /// delas não escreve método algum. Derivar isto da contagem de métodos
    /// materializados era o que fazia um bloco sem método explícito
    /// desaparecer da coerência.
    ///
    /// A identidade vem das autoridades canônicas já existentes: o
    /// `trait_name` que a resolução modular canonizou e a chave de
    /// `union_canon` do alvo resolvido. Nome sintético (`__impl_*`,
    /// `__trait_default_check_*`) é transporte e não participa desta decisão.
    fn validate_impl_relations(&mut self, program: &Program) -> Result<(), PinkerError> {
        // (trato canônico, alvo canônico) -> (grafia resolvida, grafia
        // declarada, span da primeira declaração)
        let mut declared: BTreeMap<(String, String), (String, String, Span)> = BTreeMap::new();
        let mut origens: HashMap<(String, String), SourceId> = HashMap::new();
        for impl_decl in &program.impls {
            let declared_spelling = Self::type_key(&impl_decl.target_ty);
            let resolved = self.resolve_type_or_error(&impl_decl.target_ty)?;
            let canonical = union_canon::canonical_type_key(&resolved);
            if union_canon::is_poisoned_key(&canonical) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "identidade resolvida do alvo '{}' do impl '{}' foi perdida",
                        declared_spelling, impl_decl.trait_name
                    ),
                    span: impl_decl.target_ty.span(),
                });
            }
            let resolved_display = Self::type_key(&resolved);
            match declared.entry((impl_decl.trait_name.clone(), canonical)) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    // #577: a mesma identidade que decide cardinalidade decide
                    // origem. Uma segunda declaração nunca chega aqui, então a
                    // relação tem exatamente uma unidade declarante.
                    origens.insert(
                        (impl_decl.trait_name.clone(), slot.key().1.clone()),
                        impl_decl.span.source,
                    );
                    slot.insert((resolved_display, declared_spelling, impl_decl.span));
                }
                std::collections::btree_map::Entry::Occupied(slot) => {
                    let (previous_resolved, previous_spelling, previous_span) = slot.get();
                    if previous_spelling == &declared_spelling {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "impl do trato '{}' para tipo '{}' já declarado; outra declaração em {}",
                                impl_decl.trait_name, declared_spelling, previous_span
                            ),
                            span: impl_decl.span,
                        });
                    }
                    let equivalence = format!(
                        "'{}' e '{}' resolvem para '{}'",
                        declared_spelling, previous_spelling, previous_resolved
                    );
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl do trato '{}' para tipo '{}' conflita com impl para '{}'; {} (outra declaração em {})",
                            impl_decl.trait_name,
                            declared_spelling,
                            previous_spelling,
                            equivalence,
                            previous_span
                        ),
                        span: impl_decl.span,
                    });
                }
            }
        }
        self.fontes_das_relacoes = origens;
        Ok(())
    }

    fn register_impl_methods(&mut self, program: &Program) -> Result<(), PinkerError> {
        let mut candidates: BTreeMap<MethodIdentity<String>, Vec<ImplMethodMeta>> = BTreeMap::new();
        for item in &program.items {
            let Item::Function(function) = item else {
                continue;
            };
            let Some((trait_name, _target_transport, method_name)) =
                method_identity::parse_provisional_function_name(&function.name)
            else {
                continue;
            };
            let impl_facts = function
                .impl_facts
                .as_ref()
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!(
                        "método provisório '{}.{}' perdeu o alvo declarado do impl",
                        trait_name, method_name
                    ),
                    span: function.span,
                })?;
            let target_spelling = Self::type_key(&impl_facts.target_ty);
            let resolved_target = self.resolve_type_or_error(&impl_facts.target_ty)?;
            let canonical_target = union_canon::canonical_type_key(&resolved_target);
            if union_canon::is_poisoned_key(&canonical_target) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "identidade resolvida do alvo '{}' do impl '{}' foi perdida",
                        target_spelling, trait_name
                    ),
                    span: impl_facts.target_ty.span(),
                });
            }
            let resolved_target_display = Self::type_key(&resolved_target);
            let identity = MethodIdentity::new(trait_name, canonical_target, method_name);
            candidates
                .entry(identity.clone())
                .or_default()
                .push(ImplMethodMeta {
                    identity,
                    target_spelling,
                    resolved_target_display,
                    function_name: function.name.clone(),
                    is_generated_default: impl_facts.generated_default,
                    span: function.span,
                });
        }

        for (identity, mut candidates) in candidates {
            candidates.sort_by(|left, right| left.function_name.cmp(&right.function_name));
            let explicit = candidates
                .iter()
                .filter(|candidate| !candidate.is_generated_default)
                .collect::<Vec<_>>();
            if explicit.len() > 1 {
                let previous = explicit[0];
                let conflicting = explicit[1];
                if previous.target_spelling == conflicting.target_spelling {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "método '{}' do trato '{}' para tipo '{}' já implementado; outra implementação já declarada em {}",
                            identity.method_name,
                            identity.trait_name,
                            conflicting.target_spelling,
                            previous.span
                        ),
                        span: conflicting.span,
                    });
                }
                let equivalence = format!(
                    "'{}' e '{}' resolvem para '{}'",
                    conflicting.target_spelling,
                    previous.target_spelling,
                    conflicting.resolved_target_display
                );
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "método '{}' do trato '{}' para tipo '{}' conflita com implementação para '{}'; {} (outra declaração em {})",
                        identity.method_name,
                        identity.trait_name,
                        conflicting.target_spelling,
                        previous.target_spelling,
                        equivalence,
                        previous.span
                    ),
                    span: conflicting.span,
                });
            }

            // Um override explícito vence defaults materializados da mesma
            // relação resolvida. Sem override, escolhemos pelo símbolo
            // provisório (ordem total), nunca pela ordem de fonte/import.
            let selected = explicit
                .first()
                .copied()
                .unwrap_or_else(|| &candidates[0])
                .clone();
            self.method_index
                .entry((identity.target.clone(), identity.method_name.clone()))
                .or_default()
                .push(selected.function_name.clone());
            self.impl_methods.push(selected);
        }
        Ok(())
    }

    fn validate_impl_contracts(&self, program: &Program) -> Result<(), PinkerError> {
        let mut groups: BTreeMap<(String, String), (String, Vec<&ImplMethodMeta>)> =
            BTreeMap::new();
        for impl_decl in &program.impls {
            let resolved = self.resolve_type_or_error(&impl_decl.target_ty)?;
            let canonical = union_canon::canonical_type_key(&resolved);
            groups
                .entry((impl_decl.trait_name.clone(), canonical))
                .or_insert_with(|| (Self::type_key(&resolved), Vec::new()));
        }
        for meta in &self.impl_methods {
            groups
                .entry((
                    meta.identity.trait_name.clone(),
                    meta.identity.target.clone(),
                ))
                .or_insert_with(|| (meta.resolved_target_display.clone(), Vec::new()))
                .1
                .push(meta);
        }

        for ((trait_name, _canonical_target), (target_type, methods)) in groups {
            let Some(trait_decl) = self.traits.get(&trait_name) else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "impl '{}' para '{}' referencia trato não declarado",
                        trait_name, target_type
                    ),
                    span: Span::new(Position::new(0, 0), Position::new(0, 0)),
                });
            };
            // Preserve the trait's contextual-`si` diagnostics before
            // contextualizing and comparing concrete impl signatures.
            self.validate_object_trait_shape(trait_decl)?;
            let mut seen = HashSet::new();

            for meta in &methods {
                let Some(method) = trait_decl
                    .methods
                    .iter()
                    .find(|method| method.name == meta.identity.method_name)
                else {
                    let span = self
                        .funcs
                        .get(&meta.function_name)
                        .map(|function| function.span)
                        .unwrap_or(trait_decl.span);
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl '{}' para '{}' declara método '{}' que não existe no trato",
                            trait_name, target_type, meta.identity.method_name
                        ),
                        span,
                    });
                };
                if !seen.insert(meta.identity.method_name.as_str()) {
                    let span = self
                        .funcs
                        .get(&meta.function_name)
                        .map(|function| function.span)
                        .unwrap_or(trait_decl.span);
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl '{}' para '{}' declara método '{}' mais de uma vez",
                            trait_name, target_type, meta.identity.method_name
                        ),
                        span,
                    });
                }

                let function = self
                    .funcs
                    .get(&meta.function_name)
                    .expect("impl method metadata always references a collected function");
                self.validate_impl_trait_method_function(trait_decl, method, meta, function)?;
            }

            for method in &trait_decl.methods {
                if !seen.contains(method.name.as_str()) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "impl '{}' para '{}' não implementa método '{}'",
                            trait_name, target_type, method.name
                        ),
                        span: method.span,
                    });
                }
            }
        }

        Ok(())
    }

    fn validate_object_trait_shape(&self, trait_decl: &TraitDecl) -> Result<bool, PinkerError> {
        let uses_contextual_self = trait_decl.methods.iter().any(|method| {
            method
                .params
                .iter()
                .any(|param| Self::type_contains_contextual_self(&param.ty))
                || method
                    .ret_type
                    .as_ref()
                    .map(Self::type_contains_contextual_self)
                    .unwrap_or(false)
        });

        if !uses_contextual_self {
            return Ok(false);
        }

        for method in &trait_decl.methods {
            let Some(receiver) = method.params.first() else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "trato '{}' usa receiver contextual 'si'; método '{}' deve declarar 'si' como primeiro parâmetro",
                        trait_decl.name, method.name
                    ),
                    span: method.span,
                });
            };

            if !Self::is_contextual_self_type(&receiver.ty) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "trato '{}' usa receiver contextual 'si'; método '{}' deve declarar 'si' como primeiro parâmetro",
                        trait_decl.name, method.name
                    ),
                    span: receiver.span,
                });
            }

            for param in method.params.iter().skip(1) {
                if Self::type_contains_contextual_self(&param.ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "método '{}' do trato '{}' só pode usar 'si' como primeiro parâmetro receiver",
                            method.name, trait_decl.name
                        ),
                        span: param.span,
                    });
                }

                let struct_names = self.structs.keys().cloned().collect::<HashSet<_>>();
                let ir_type = TypeIR::from_ast_with_context(
                    &param.ty,
                    &self.type_aliases,
                    &struct_names,
                )
                .map_err(|error| PinkerError::Semantic {
                    msg: format!(
                        "parâmetro '{}' do método '{}' no trato '{}' não possui representação nativa válida: {}",
                        param.name, method.name, trait_decl.name, error
                    ),
                    span: param.span,
                })?;
                if !ir_type.is_native_abi_word() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "parâmetro '{}' do método '{}' no trato '{}' exige representação multi-palavra sem transporte nativo nesta fase",
                            param.name, method.name, trait_decl.name
                        ),
                        span: param.span,
                    });
                }
            }

            if let Some(ret_type) = &method.ret_type {
                if Self::type_contains_contextual_self(ret_type) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "método '{}' do trato '{}' não pode retornar 'si' em objeto de trato nesta fase",
                            method.name, trait_decl.name
                        ),
                        span: ret_type.span(),
                    });
                }
            }
        }

        Ok(true)
    }

    fn validate_impl_trait_method_function(
        &self,
        trait_decl: &TraitDecl,
        method: &TraitMethodSig,
        meta: &ImplMethodMeta,
        function: &FunctionDecl,
    ) -> Result<(), PinkerError> {
        if function.params.len() != method.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' do trato '{}' espera {} parâmetro(s), mas impl para '{}' tem {}",
                    method.name,
                    trait_decl.name,
                    method.params.len(),
                    meta.target_spelling,
                    function.params.len()
                ),
                span: function.span,
            });
        }

        let Some(receiver) = function.params.first() else {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' do trato '{}' exige receiver no impl para '{}'",
                    method.name, trait_decl.name, meta.target_spelling
                ),
                span: function.span,
            });
        };

        let receiver_direct = Self::type_key(&receiver.ty);
        let receiver_identity = self.resolved_type_identity(&receiver.ty)?;

        if meta.identity.target != receiver_identity {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "receiver do método '{}' no impl '{}' para '{}' usa '{}'",
                    method.name, trait_decl.name, meta.target_spelling, receiver_direct
                ),
                span: receiver.span,
            });
        }

        let expected_receiver = method
            .params
            .first()
            .expect("aridade já foi comparada e impl possui receiver");
        if !Self::is_contextual_self_type(&expected_receiver.ty) {
            let expected_ty = self.resolve_type_or_error(&expected_receiver.ty)?;
            let found_ty = self.resolve_type_or_error(&receiver.ty)?;
            if union_canon::canonical_type_key(&expected_ty)
                != union_canon::canonical_type_key(&found_ty)
            {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "receiver do método '{}' no trato '{}' espera '{}', mas impl para '{}' usa '{}'",
                        method.name,
                        trait_decl.name,
                        Self::type_key(&expected_ty),
                        meta.target_spelling,
                        Self::type_key(&found_ty)
                    ),
                    span: receiver.span,
                });
            }
        }

        for (expected, found) in method
            .params
            .iter()
            .skip(1)
            .zip(function.params.iter().skip(1))
        {
            let expected_ty = self.resolve_type_or_error(&expected.ty)?;
            let found_ty = self.resolve_type_or_error(&found.ty)?;

            if union_canon::canonical_type_key(&expected_ty)
                != union_canon::canonical_type_key(&found_ty)
            {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "parâmetro '{}' do método '{}' no trato '{}' espera '{}', mas impl para '{}' usa '{}'",
                        expected.name,
                        method.name,
                        trait_decl.name,
                        Self::type_key(&expected_ty),
                        meta.target_spelling,
                        Self::type_key(&found_ty)
                    ),
                    span: found.span,
                });
            }
        }

        match (&method.ret_type, &function.ret_type) {
            (None, None) => {}
            (Some(expected), Some(found)) => {
                let expected_ty = self.resolve_type_or_error(expected)?;
                let found_ty = self.resolve_type_or_error(found)?;

                if union_canon::canonical_type_key(&expected_ty)
                    != union_canon::canonical_type_key(&found_ty)
                {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno do método '{}' no trato '{}' espera '{}', mas impl para '{}' usa '{}'",
                            method.name,
                            trait_decl.name,
                            Self::type_key(&expected_ty),
                            meta.target_spelling,
                            Self::type_key(&found_ty)
                        ),
                        span: found.span(),
                    });
                }
            }
            _ => {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "retorno do método '{}' no trato '{}' é incompatível no impl para '{}'",
                        method.name, trait_decl.name, meta.target_spelling
                    ),
                    span: function.span,
                });
            }
        }

        Ok(())
    }

    fn validate_trait_contracts(&self) -> Result<(), PinkerError> {
        for trait_decl in self.traits.values() {
            if trait_decl.methods.is_empty() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "trato '{}' deve declarar ao menos um método",
                        trait_decl.name
                    ),
                    span: trait_decl.span,
                });
            }

            let objectifiable = self.validate_object_trait_shape(trait_decl)?;

            for method in &trait_decl.methods {
                if objectifiable {
                    let candidates: Vec<(&ImplMethodMeta, &FunctionDecl)> = self
                        .impl_methods
                        .iter()
                        .filter(|meta| {
                            meta.identity.trait_name == trait_decl.name
                                && meta.identity.method_name == method.name
                        })
                        .filter_map(|meta| {
                            self.funcs
                                .get(&meta.function_name)
                                .map(|function| (meta, function))
                        })
                        .collect();

                    if candidates.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "trato objetificável '{}' exige ao menos um impl completo para o método '{}'",
                                trait_decl.name, method.name
                            ),
                            span: method.span,
                        });
                    }

                    for (meta, function) in candidates {
                        self.validate_impl_trait_method_function(
                            trait_decl, method, meta, function,
                        )?;
                    }

                    continue;
                }

                let mut candidates = Vec::new();

                if let Some(function) = self.funcs.get(&method.name) {
                    candidates.push(function);
                }

                for meta in &self.impl_methods {
                    if meta.identity.trait_name == trait_decl.name
                        && meta.identity.method_name == method.name
                    {
                        if let Some(function) = self.funcs.get(&meta.function_name) {
                            candidates.push(function);
                        }
                    }
                }

                if candidates.is_empty() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "trato '{}' exige função '{}' compatível declarada no topo",
                            trait_decl.name, method.name
                        ),
                        span: method.span,
                    });
                }

                let mut first_error = None;

                for function in candidates {
                    match self.validate_trait_method_function(trait_decl, method, function) {
                        Ok(()) => {
                            first_error = None;
                            break;
                        }
                        Err(err) if first_error.is_none() => first_error = Some(err),
                        Err(_) => {}
                    }
                }

                if let Some(err) = first_error {
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    fn validate_trait_method_function(
        &self,
        trait_decl: &TraitDecl,
        method: &TraitMethodSig,
        function: &FunctionDecl,
    ) -> Result<(), PinkerError> {
        if function.params.len() != method.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' do trato '{}' espera {} parâmetro(s), mas função declarada tem {}",
                    method.name,
                    trait_decl.name,
                    method.params.len(),
                    function.params.len()
                ),
                span: function.span,
            });
        }
        for (expected, found) in method.params.iter().zip(function.params.iter()) {
            let expected_ty = self.resolve_type_or_error(&expected.ty)?;
            let found_ty = self.resolve_type_or_error(&found.ty)?;
            if Self::type_key(&expected_ty) != Self::type_key(&found_ty) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "parâmetro '{}' do método '{}' no trato '{}' espera '{}', mas função usa '{}'",
                        expected.name,
                        method.name,
                        trait_decl.name,
                        Self::type_key(&expected_ty),
                        Self::type_key(&found_ty)
                    ),
                    span: found.span,
                });
            }
        }
        match (&method.ret_type, &function.ret_type) {
            (None, None) => {}
            (Some(expected), Some(found)) => {
                let expected_ty = self.resolve_type_or_error(expected)?;
                let found_ty = self.resolve_type_or_error(found)?;
                if Self::type_key(&expected_ty) != Self::type_key(&found_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno do método '{}' no trato '{}' espera '{}', mas função usa '{}'",
                            method.name,
                            trait_decl.name,
                            Self::type_key(&expected_ty),
                            Self::type_key(&found_ty)
                        ),
                        span: found.span(),
                    });
                }
            }
            _ => {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "retorno do método '{}' no trato '{}' é incompatível com a função declarada",
                        method.name, trait_decl.name
                    ),
                    span: function.span,
                });
            }
        }
        Ok(())
    }
    // @pinker-nav:end semantic.tratos.contratos

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
        if name.starts_with(crate::anonymous_identity::ANONYMOUS_CALLABLE_PREFIX) {
            "<anônima>".to_string()
        } else {
            name.to_string()
        }
    }
    // @pinker-nav:end semantic.funcoes.verificacao

    // @pinker-nav:start semantic.comandos.verificacao
    // @pinker-nav:domain comandos
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de comandos de um bloco: `mimo` (let) com inferência de `lista_criar`/`mapa_criar` pela anotação e checagem de tipo/faixa, retorno, atribuição a variável/deref/campo/índice (mutabilidade e tipos), `talvez`/`senão`, laço `sempre que` (com controle de profundidade), `quebrar`/`continuar`, `falar` (tipos imprimíveis), `sussurro` (asm) e expressão-comando.
    fn check_block(&mut self, block: &Block, function_level: bool) -> Result<(), PinkerError> {
        if !function_level {
            self.push_scope();
        }

        for stmt in &block.stmts {
            match stmt {
                Stmt::Let(let_stmt) => {
                    // `nova l: lista<...> = lista_criar();` — a criação genérica
                    // recebe o tipo da anotação (única forma de inferência desta fase).
                    if let Some(declared_ty) = &let_stmt.ty {
                        if Self::expr_is_generic_list_create(&let_stmt.init) {
                            let resolved_declared_ty = self.resolve_type_or_error(declared_ty)?;
                            if Self::list_element_type(&resolved_declared_ty, let_stmt.span)
                                .is_none()
                            {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "'lista_criar()' exige anotação de tipo de lista em 'nova'; encontrado '{}'",
                                        resolved_declared_ty.name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            self.declare_var(
                                &let_stmt.name,
                                resolved_declared_ty,
                                let_stmt.is_mut,
                                let_stmt.span,
                            )?;
                            continue;
                        }
                        if Self::expr_is_generic_map_create(&let_stmt.init) {
                            let resolved_declared_ty = self.resolve_type_or_error(declared_ty)?;
                            if !Self::is_map_type(&resolved_declared_ty) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "'mapa_criar()' exige anotação de tipo de mapa em 'nova'; encontrado '{}'",
                                        resolved_declared_ty.name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            self.declare_var(
                                &let_stmt.name,
                                resolved_declared_ty,
                                let_stmt.is_mut,
                                let_stmt.span,
                            )?;
                            continue;
                        }
                    }
                    let init_ty = self.check_value_expr(
                        &let_stmt.init,
                        "resultado de função sem retorno não pode ser usado em inicialização de variável",
                    )?;

                    let ty = match &let_stmt.ty {
                        Some(declared_ty) => {
                            let resolved_declared_ty = self.resolve_type_or_error(declared_ty)?;
                            if !Self::check_expected_type_for_expr(
                                &resolved_declared_ty,
                                &init_ty,
                                &let_stmt.init,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo de inicialização incompatível para '{}': esperado '{}', encontrado '{}'",
                                        let_stmt.name,
                                        resolved_declared_ty.display_name(),
                                        init_ty.display_name()
                                    ),
                                    span: let_stmt.init.span,
                                });
                            }
                            Self::validate_int_literal_range(
                                &resolved_declared_ty,
                                &let_stmt.init,
                            )?;
                            resolved_declared_ty
                        }
                        None => init_ty,
                    };

                    self.declare_var(&let_stmt.name, ty, let_stmt.is_mut, let_stmt.span)?;
                }
                Stmt::Return(return_stmt) => self.check_return_stmt(return_stmt)?,
                Stmt::Assign(assign_stmt) => {
                    let value_ty = self.check_value_expr(
                        &assign_stmt.expr,
                        "resultado de função sem retorno não pode ser usado em atribuição",
                    )?;
                    match &assign_stmt.target {
                        AssignTarget::Ident(name) => {
                            let Some(var_meta) = self.resolve_var(name) else {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "variável '{}' não declarada para atribuição",
                                        name
                                    ),
                                    span: assign_stmt.span,
                                });
                            };

                            if !var_meta.is_mut {
                                return Err(PinkerError::Semantic {
                                    msg: format!("reatribuição inválida: '{}' não é mutável", name),
                                    span: assign_stmt.span,
                                });
                            }

                            if !Self::check_expected_type_for_expr(
                                &var_meta.ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na atribuição para '{}': esperado '{}', encontrado '{}'",
                                        name,
                                        var_meta.ty.name(),
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(&var_meta.ty, &assign_stmt.expr)?;
                        }
                        AssignTarget::Deref(ptr_expr) => {
                            let ptr_ty = self.check_value_expr(
                                ptr_expr,
                                "resultado de função sem retorno não pode ser usado como ponteiro de escrita indireta",
                            )?;
                            let expected_value_ty = match ptr_ty {
                                Type::Pointer { base, .. }
                                    if matches!(
                                        base.as_ref(),
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
                                    ) =>
                                {
                                    base.as_ref().clone()
                                }
                                Type::Pointer { .. } => {
                                    return Err(PinkerError::Semantic {
                                        msg: "escrita indireta aceita ponteiros para escalares públicos de uma palavra".to_string(),
                                        span: ptr_expr.span,
                                    });
                                }
                                _ => {
                                    return Err(PinkerError::Semantic {
                                        msg: "escrita indireta requer operando do tipo 'seta<T>'"
                                            .to_string(),
                                        span: ptr_expr.span,
                                    });
                                }
                            };

                            if !Self::check_expected_type_for_expr(
                                &expected_value_ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na escrita indireta: esperado '{}', encontrado '{}'",
                                        expected_value_ty.name(),
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(
                                &expected_value_ty,
                                &assign_stmt.expr,
                            )?;
                        }
                        AssignTarget::FieldDeref { base, field } => {
                            let base_ty = self.check_value_expr(
                                base,
                                "resultado de função sem retorno não pode ser base de escrita a campo",
                            )?;
                            let field_ty =
                                self.resolve_struct_field_type(&base_ty, field, assign_stmt.span)?;
                            if !Self::check_expected_type_for_expr(
                                &field_ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na escrita de campo '{}': esperado '{}', encontrado '{}'",
                                        field,
                                        field_ty.name(),
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(&field_ty, &assign_stmt.expr)?;
                        }
                        AssignTarget::Index { base, index } => {
                            let base_ty = self.check_value_expr(
                                base,
                                "resultado de função sem retorno não pode ser base de escrita por índice",
                            )?;
                            match &base_ty {
                                Type::FixedArray { element, .. } => {
                                    if !matches!(element.as_ref(), Type::Bombom(_)) {
                                        return Err(PinkerError::Semantic {
                                            msg: "escrita por índice nesta fase aceita apenas '[bombom; N]'".to_string(),
                                            span: assign_stmt.span,
                                        });
                                    }
                                }
                                _ => {
                                    return Err(PinkerError::Semantic {
                                        msg:
                                            "escrita por índice exige base de array fixo nesta fase"
                                                .to_string(),
                                        span: assign_stmt.span,
                                    });
                                }
                            }
                            let index_ty = self.check_value_expr(
                                index,
                                "resultado de função sem retorno não pode ser índice de escrita",
                            )?;
                            if !matches!(index_ty, Type::Bombom(_)) {
                                return Err(PinkerError::Semantic {
                                    msg: "índice de escrita nesta fase deve ser 'bombom'"
                                        .to_string(),
                                    span: index.span,
                                });
                            }
                            let expected_ty = Type::Bombom(assign_stmt.span);
                            if !Self::check_expected_type_for_expr(
                                &expected_ty,
                                &value_ty,
                                &assign_stmt.expr,
                            ) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "tipo incompatível na escrita por índice: esperado 'bombom', encontrado '{}'",
                                        value_ty.name()
                                    ),
                                    span: assign_stmt.expr.span,
                                });
                            }
                            Self::validate_int_literal_range(&expected_ty, &assign_stmt.expr)?;
                        }
                    }
                }
                Stmt::If(if_stmt) => {
                    let cond_ty = self.check_value_expr(
                        &if_stmt.condition,
                        "condição não pode usar resultado de função sem retorno",
                    )?;
                    if !matches!(cond_ty, Type::Logica(_)) {
                        return Err(PinkerError::Semantic {
                            msg: "condição de 'talvez' deve ser 'logica'".to_string(),
                            span: if_stmt.condition.span,
                        });
                    }

                    self.check_block(&if_stmt.then_branch, false)?;

                    if let Some(else_branch) = &if_stmt.else_branch {
                        match else_branch {
                            ElseBlock::Block(block) => self.check_block(block, false)?,
                            ElseBlock::If(if_stmt) => self.check_if_as_nested_branch(if_stmt)?,
                        }
                    }
                }
                Stmt::While(while_stmt) => {
                    let cond_ty = self.check_value_expr(
                        &while_stmt.condition,
                        "condição não pode usar resultado de função sem retorno",
                    )?;
                    if !matches!(cond_ty, Type::Logica(_)) {
                        return Err(PinkerError::Semantic {
                            msg: "condição de 'sempre que' deve ser 'logica'".to_string(),
                            span: while_stmt.condition.span,
                        });
                    }

                    self.loop_depth += 1;
                    let body_result = self.check_block(&while_stmt.body, false);
                    self.loop_depth -= 1;
                    body_result?;
                }
                Stmt::Break(break_stmt) => {
                    if self.loop_depth == 0 {
                        return Err(PinkerError::Semantic {
                            msg: "'quebrar' só pode ser usado dentro de 'sempre que'".to_string(),
                            span: break_stmt.span,
                        });
                    }
                }
                Stmt::Continue(continue_stmt) => {
                    if self.loop_depth == 0 {
                        return Err(PinkerError::Semantic {
                            msg: "'continuar' só pode ser usado dentro de 'sempre que'".to_string(),
                            span: continue_stmt.span,
                        });
                    }
                }
                Stmt::Falar(falar_stmt) => {
                    for arg in &falar_stmt.args {
                        let ty = self.check_value_expr(
                            arg,
                            "'falar' exige expressão com valor (não nulo)",
                        )?;
                        let is_printable = matches!(
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
                                | Type::Verso(_)
                        );
                        if !is_printable {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "'falar' não suporta tipo '{}'; apenas bombom, u8, u16, u32, u64, i8, i16, i32, i64, logica e verso são imprimíveis",
                                    ty.name()
                                ),
                                span: falar_stmt.span,
                            });
                        }
                    }
                }
                Stmt::InlineAsm(inline_asm_stmt) => self.check_inline_asm(inline_asm_stmt)?,
                Stmt::EnumMatch(enum_match) => self.check_enum_match(enum_match)?,
                Stmt::UnionMatch(union_match) => self.check_union_match(union_match)?,
                Stmt::Expr(expr) => {
                    self.check_expr(expr)?;
                }
            }
        }

        if !function_level {
            self.pop_scope();
        }

        Ok(())
    }
    // @pinker-nav:end semantic.comandos.verificacao

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

    // @pinker-nav:start semantic.unioes.encaixe
    // @pinker-nav:domain unioes
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de `encaixe` de união: resolve o tipo do scrutinee e o tipo de cada braço integralmente (apelidos inclusos), deriva a chave canônica compartilhada de `union_canon`, exige que cada braço pertença à união, rejeita duplicata após a resolução (dois apelidos do mesmo tipo canônico são o mesmo membro), exige cobertura exata dos membros canônicos e abre um escopo por braço com o binding declarado no tipo resolvido do membro. Nenhuma tag é calculada ou armazenada aqui — a tag pertence ao registry internado pelo lowering.
    fn check_union_match(&mut self, union_match: &UnionMatchStmt) -> Result<(), PinkerError> {
        let scrutinee_ty = self.check_value_expr(
            &union_match.scrutinee,
            "resultado de função sem retorno não pode ser inspecionado por 'encaixe'",
        )?;
        let scrutinee_ty = self.resolve_type_or_error(&scrutinee_ty)?;
        let Type::Union {
            members: canonical_members,
            ..
        } = &scrutinee_ty
        else {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "'encaixe' de união exige scrutinee de união estrutural; encontrado '{}'",
                    scrutinee_ty.name()
                ),
                span: union_match.scrutinee.span,
            });
        };

        // Cada braço é associado ao membro canônico pelo **tipo resolvido**.
        // O spelling original (o nome do apelido escrito) é preservado apenas
        // para diagnóstico.
        let mut covered = HashSet::<String>::new();
        let mut arm_members = Vec::with_capacity(union_match.arms.len());
        for arm in &union_match.arms {
            let resolved_member = self.resolve_type_or_error(&arm.member_type)?;
            let key = union_canon::member_key(&resolved_member);
            let Some(index) = union_canon::canonical_member_index(canonical_members, &key) else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "braço '{}' de 'encaixe' não é membro da união '{}'",
                        arm.member_type.name(),
                        scrutinee_ty.name()
                    ),
                    span: arm.span,
                });
            };
            if !covered.insert(key.canonical_type_key.clone()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "membro '{}' repetido no 'encaixe' de união após resolução de apelidos",
                        canonical_members[index].name()
                    ),
                    span: arm.span,
                });
            }
            arm_members.push(canonical_members[index].clone());
        }

        if covered.len() != canonical_members.len() {
            let faltantes = canonical_members
                .iter()
                .filter(|member| !covered.contains(union_canon::member_key(member).as_str()))
                .map(|member| member.name().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(PinkerError::Semantic {
                msg: format!(
                    "encaixe de união deve ser exaustivo: os braços devem cobrir exatamente todos os membros canônicos; ausente(s): {faltantes}"
                ),
                span: union_match.span,
            });
        }

        for (arm, member_ty) in union_match.arms.iter().zip(arm_members) {
            self.push_scope();
            let checked = self
                .declare_var(&arm.binding, member_ty.with_span(arm.span), false, arm.span)
                .and_then(|()| self.check_block(&arm.body, true));
            self.pop_scope();
            checked?;
        }

        Ok(())
    }
    // @pinker-nav:end semantic.unioes.encaixe

    // @pinker-nav:start semantic.fluxo.retornos
    // @pinker-nav:domain fluxo
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Fluxo e retornos: verificação de ramo `talvez`/`senão` aninhado com escopo próprio, checagem de `mimo` de retorno contra o tipo declarado (presença/ausência de valor, tipo e faixa) e análise superficial de alcançabilidade — um bloco retorna se contém `mimo` direto ou uma seleção exaustiva (`talvez`/`senão` ou `encaixe`) em que todos os braços retornam.
    fn check_if_as_nested_branch(&mut self, if_stmt: &IfStmt) -> Result<(), PinkerError> {
        self.push_scope();
        let cond_ty = self.check_value_expr(
            &if_stmt.condition,
            "condição não pode usar resultado de função sem retorno",
        )?;
        if !matches!(cond_ty, Type::Logica(_)) {
            self.pop_scope();
            return Err(PinkerError::Semantic {
                msg: "condição de 'talvez' deve ser 'logica'".to_string(),
                span: if_stmt.condition.span,
            });
        }

        self.check_block(&if_stmt.then_branch, false)?;
        if let Some(else_branch) = &if_stmt.else_branch {
            match else_branch {
                ElseBlock::Block(block) => self.check_block(block, false)?,
                ElseBlock::If(inner) => self.check_if_as_nested_branch(inner)?,
            }
        }
        self.pop_scope();
        Ok(())
    }

    fn check_return_stmt(&mut self, return_stmt: &ReturnStmt) -> Result<(), PinkerError> {
        let current_ret = self.current_func_ret.clone();
        match (current_ret, &return_stmt.expr) {
            (None, None) => Ok(()),
            (None, Some(_)) => Err(PinkerError::Semantic {
                msg: "mimo com valor não é permitido em função sem retorno declarado".to_string(),
                span: return_stmt.span,
            }),
            (Some(_), None) => Err(PinkerError::Semantic {
                msg: "mimo sem valor não é permitido em função com retorno declarado".to_string(),
                span: return_stmt.span,
            }),
            (Some(expected), Some(expr)) => {
                let value_ty = self.check_value_expr(
                    expr,
                    "resultado de função sem retorno não pode ser retornado como valor",
                )?;
                if !Self::check_expected_type_for_expr(&expected, &value_ty, expr) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "retorno incompatível em '{}': esperado '{}', encontrado '{}'",
                            self.current_func_name
                                .as_deref()
                                .unwrap_or("<desconhecida>"),
                            expected.name(),
                            value_ty.name()
                        ),
                        span: expr.span,
                    });
                }
                Self::validate_int_literal_range(&expected, expr)?;
                Ok(())
            }
        }
    }

    // Análise de alcançabilidade de retorno superficial: verifica se o bloco
    // contém um `mimo` direto ou uma seleção exaustiva onde todos os ramos retornam.
    // Não analisa fluxo complexo nem condições de laço — suficiente para a v0.
    fn block_returns(&self, block: &Block) -> bool {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Return(_) => return true,
                Stmt::If(if_stmt) if self.if_returns(if_stmt) => return true,
                Stmt::EnumMatch(enum_match) if self.enum_match_returns(enum_match) => return true,
                Stmt::UnionMatch(union_match) if self.union_match_returns(union_match) => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn if_returns(&self, if_stmt: &IfStmt) -> bool {
        let then_returns = self.block_returns(&if_stmt.then_branch);
        let else_returns = match &if_stmt.else_branch {
            Some(ElseBlock::Block(block)) => self.block_returns(block),
            Some(ElseBlock::If(inner)) => self.if_returns(inner),
            None => false,
        };
        then_returns && else_returns
    }

    fn enum_match_returns(&self, enum_match: &EnumMatchStmt) -> bool {
        let arms_return = !enum_match.arms.is_empty()
            && enum_match
                .arms
                .iter()
                .all(|arm| self.block_returns(&arm.body));
        let otherwise_returns = match &enum_match.otherwise {
            Some(otherwise) => self.block_returns(otherwise),
            None => true,
        };
        arms_return && otherwise_returns
    }

    fn union_match_returns(&self, union_match: &UnionMatchStmt) -> bool {
        !union_match.arms.is_empty()
            && union_match
                .arms
                .iter()
                .all(|arm| self.block_returns(&arm.body))
    }
    // @pinker-nav:end semantic.fluxo.retornos

    // @pinker-nav:start semantic.expressoes.verificacao
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Verificação de expressões que produz o tipo de cada nó: exigência de valor não-`Nulo` (`check_value_expr`), tipo de resultado de função, e o despacho central (`check_expr`) sobre literais, identificadores, cursores internos de mapa, acesso a campo/variante de leque, indexação, `virar` (cast), `peso`/`alinhamento`, operações binárias (incluindo aritmética de ponteiro) e unárias (negação, `nao`, bitwise, dereferência).
    fn check_value_expr(&mut self, expr: &Expr, void_message: &str) -> Result<Type, PinkerError> {
        let ty = self.check_expr(expr)?;
        if ty.is_nulo() {
            return Err(PinkerError::Semantic {
                msg: void_message.to_string(),
                span: expr.span,
            });
        }
        Ok(ty)
    }

    // `Nulo` existe só internamente para a semântica da v0: função sem `-> tipo` retorna `Nulo`.
    // Esse tipo nunca pode aparecer em declaração de usuário.
    fn function_result_type(&self, function: &FunctionDecl, span: Span) -> Type {
        let base = function
            .ret_type
            .as_ref()
            .and_then(|ty| self.resolve_type_or_error(ty).ok())
            .unwrap_or(Type::Nulo(span));
        base.with_span(span)
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type, PinkerError> {
        match &expr.kind {
            ExprKind::IntLit(_) => Ok(Type::Bombom(expr.span)),
            ExprKind::BoolLit(_) => Ok(Type::Logica(expr.span)),
            ExprKind::StringLit(_) => Ok(Type::Verso(expr.span)),
            // #532: intrínseca resolvida fora de posição de chamada. A
            // superfície modular não expõe a intrínseca como VALOR, e a recusa
            // é a mesma que a grafia canônica sempre produziu — sem consultar
            // `resolve_var`, que depois desta Issue poderia encontrar uma
            // função do usuário homônima e deixá-la capturar a referência.
            ExprKind::Intrinsic(identity) => Err(PinkerError::Semantic {
                msg: format!(
                    "identificador '{}' não declarado",
                    identity.canonical_public_spelling()
                ),
                span: expr.span,
            }),
            ExprKind::Ident(name) => {
                // Fase 243: nome sintético de literal `carinho` (Fase 225) —
                // resolve como criação de closure (materializa captura por
                // valor e checa o corpo com o ambiente correto), não como
                // resolução genérica de variável/função da Fase 242.
                if name.starts_with("__anon_carinho_") {
                    return self.resolve_closure_value(name, expr.span);
                }
                self.resolve_var(name)
                    .map(|meta| meta.ty)
                    .ok_or_else(|| PinkerError::Semantic {
                        msg: format!("identificador '{}' não declarado", name),
                        span: expr.span,
                    })
            }
            ExprKind::InternalMapIterCreate(map) => {
                let map_ty =
                    self.check_value_expr(map, "cursor interno de mapa requer mapa como valor")?;
                if !matches!(map_ty, Type::MapVersoBombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "cursor interno de mapa exige 'mapa<verso,bombom>'".to_string(),
                        span: map.span,
                    });
                }
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::InternalMapIterNextKey(iterator) => {
                let iterator_ty = self.check_value_expr(
                    iterator,
                    "avanço interno de iteração de mapa requer cursor como valor",
                )?;
                if !matches!(iterator_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "cursor interno de mapa exige handle 'bombom'".to_string(),
                        span: iterator.span,
                    });
                }
                Ok(Type::Verso(expr.span))
            }
            ExprKind::Call(callee, args) => self.check_call_expr(expr.span, callee, args),
            ExprKind::AddressOf(operand) => {
                let ExprKind::Ident(name) = &operand.kind else {
                    return Err(PinkerError::Semantic {
                        msg: "obtenção de endereço cru exige nome de função top-level".to_string(),
                        span: operand.span,
                    });
                };
                if self.resolve_local_var_type(name).is_some() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "obtenção de endereço cru de '{}' rejeita variável, callable ou closure; use uma função top-level",
                            name
                        ),
                        span: operand.span,
                    });
                }
                if name.starts_with("__anon_carinho_")
                    || name.starts_with("__fnref_env_")
                    || method_identity::parse_provisional_function_name(name).is_some()
                {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "obtenção de endereço cru de '{}' rejeita closure, wrapper de callable ou método",
                            name
                        ),
                        span: operand.span,
                    });
                }
                let Some(function) = self.funcs.get(name).cloned() else {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "símbolo de função '{}' não resolvido para obtenção de endereço cru",
                            name
                        ),
                        span: operand.span,
                    });
                };
                if !function.type_params.is_empty() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "função genérica '{}' exige especialização concreta antes da obtenção de endereço cru",
                            name
                        ),
                        span: operand.span,
                    });
                }
                let signature = Type::Function {
                    params: function
                        .params
                        .iter()
                        .map(|param| param.ty.clone())
                        .collect(),
                    ret: Box::new(
                        function
                            .ret_type
                            .clone()
                            .unwrap_or_else(|| Type::Nulo(function.span)),
                    ),
                    span: function.span,
                };
                self.resolve_type_or_error(&Type::Pointer {
                    base: Box::new(signature),
                    is_volatile: false,
                    span: expr.span,
                })
            }
            ExprKind::FieldAccess { base, field } => {
                // `Leque.Variante` — o nome do leque em posição de base tem
                // precedência sobre variáveis homônimas nesta fase.
                if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(enum_name) = self.resolve_enum_base_name(base_name) {
                        let enum_decl = self.enums.get(&enum_name).expect("leque resolvido existe");
                        let Some(variant) = enum_decl
                            .variants
                            .iter()
                            .find(|variant| variant.name == *field)
                        else {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "variante '{}' não existe no leque '{}'",
                                    field, base_name
                                ),
                                span: expr.span,
                            });
                        };
                        if !variant.payloads.is_empty() {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "variante '{}' carrega valor; construa com '{}.{}(valor)'",
                                    field, base_name, field
                                ),
                                span: expr.span,
                            });
                        }
                        return Ok(Type::Enum {
                            name: enum_name,
                            span: expr.span,
                        });
                    }
                }
                // Parte G: `familia.membro` sem o `trazer`. Só chega aqui o
                // que o parser NÃO canonicalizou, e só vira erro de família
                // depois de provado que nada mais reivindica o nome.
                if let ExprKind::Ident(base_name) = &base.kind {
                    if let Some(erro) =
                        self.dica_de_familia_nao_importada(base_name, field, expr.span)
                    {
                        return Err(erro);
                    }
                }
                let base_ty = self.check_value_expr(
                    base,
                    "resultado de função sem retorno não pode ser base de acesso a campo",
                )?;
                self.resolve_struct_field_type(&base_ty, field, expr.span)
            }
            ExprKind::Index { base, index } => {
                let base_ty = self.check_value_expr(
                    base,
                    "resultado de função sem retorno não pode ser base de indexação",
                )?;
                let index_ty = self.check_value_expr(
                    index,
                    "resultado de função sem retorno não pode ser índice",
                )?;
                if !matches!(index_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: "índice nesta fase deve ser 'bombom'".to_string(),
                        span: index.span,
                    });
                }
                match base_ty {
                    Type::FixedArray { element, .. } => Ok(element.as_ref().with_span(expr.span)),
                    _ => Err(PinkerError::Semantic {
                        msg: "indexação exige base de array fixo nesta fase".to_string(),
                        span: expr.span,
                    }),
                }
            }
            ExprKind::Cast {
                expr: source_expr,
                target,
            } => {
                let source_ty = self.check_value_expr(
                    source_expr,
                    "resultado de função sem retorno não pode ser convertido com 'virar'",
                )?;
                let target_ty = self.resolve_type_or_error(target)?.with_span(expr.span);
                if let Type::Union { members, .. } = &target_ty {
                    let matching = members
                        .iter()
                        .filter(|member| Self::check_type_match(member, &source_ty))
                        .count();
                    return match matching {
                        1 => Ok(target_ty),
                        0 => Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo '{}' não pertence à união estrutural alvo",
                                Self::type_key(&source_ty)
                            ),
                            span: source_expr.span,
                        }),
                        _ => Err(PinkerError::Semantic {
                            msg: "injeção em união estrutural é ambígua; aplique 'virar' explicitamente para o membro desejado antes da união"
                                .to_string(),
                            span: source_expr.span,
                        }),
                    };
                }
                if matches!(source_ty, Type::Union { .. }) {
                    return Err(PinkerError::Semantic {
                        msg: "downcast de união fora de 'encaixe' não é permitido".to_string(),
                        span: source_expr.span,
                    });
                }
                if let Some(trait_name) = Self::trait_object_name(&target_ty).map(str::to_string) {
                    let supported_concrete = matches!(
                        &source_ty,
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
                            | Type::Struct { .. }
                    );
                    if !supported_concrete {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "objeto de trato nesta fase aceita tipo concreto escalar ou ninho; encontrado '{}'",
                                Self::type_key(&source_ty)
                            ),
                            span: source_expr.span,
                        });
                    }

                    let source_direct = Self::type_key(&source_ty);
                    let source_identity = self.resolved_type_identity(&source_ty)?;

                    let has_impl = self.impl_methods.iter().any(|meta| {
                        meta.identity.trait_name == trait_name
                            && meta.identity.target == source_identity
                    });

                    if !has_impl {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo '{}' não implementa o trato '{}' e não pode formar '{}'",
                                source_direct,
                                trait_name,
                                Self::type_key(&target_ty)
                            ),
                            span: source_expr.span,
                        });
                    }

                    return Ok(target_ty);
                }

                if let Type::Enum { name, .. } = &source_ty {
                    if self.enum_has_payload(name) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "'virar' não é suportado para leque com carga ('{}'); use 'encaixe'",
                                name
                            ),
                            span: expr.span,
                        });
                    }
                }
                if !Self::is_cast_allowed(&source_ty, &target_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "cast explícito inválido nesta fase: '{}' virar '{}'",
                            source_ty.name(),
                            target_ty.name()
                        ),
                        span: expr.span,
                    });
                }
                Ok(target_ty)
            }
            ExprKind::SizeOfType { target } => {
                let resolved = self.resolve_type_or_error(target)?.with_span(expr.span);
                layout::layout_of_type(&resolved, &self.type_aliases, &self.structs).map_err(
                    |msg| PinkerError::Semantic {
                        msg: format!("consulta de peso inválida: {}", msg),
                        span: expr.span,
                    },
                )?;
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::AlignOfType { target } => {
                let resolved = self.resolve_type_or_error(target)?.with_span(expr.span);
                layout::layout_of_type(&resolved, &self.type_aliases, &self.structs).map_err(
                    |msg| PinkerError::Semantic {
                        msg: format!("consulta de alinhamento inválida: {}", msg),
                        span: expr.span,
                    },
                )?;
                Ok(Type::Bombom(expr.span))
            }
            ExprKind::Binary(lhs, op, rhs) => {
                let lhs_ty = self.check_value_expr(
                    lhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;
                let rhs_ty = self.check_value_expr(
                    rhs,
                    "resultado de função sem retorno não pode ser usado em operação binária",
                )?;

                if matches!(op, BinaryOp::Add | BinaryOp::Sub) {
                    if let Some(pointer_result) =
                        self.check_pointer_arithmetic(expr.span, *op, &lhs_ty, &rhs_ty, rhs)
                    {
                        return pointer_result;
                    }
                }

                let raw_pointer_null_comparison = if matches!(
                    op,
                    BinaryOp::Eq
                        | BinaryOp::Neq
                        | BinaryOp::Lt
                        | BinaryOp::Lte
                        | BinaryOp::Gt
                        | BinaryOp::Gte
                ) {
                    let lhs_resolved = self.resolve_type_or_error(&lhs_ty)?;
                    let rhs_resolved = self.resolve_type_or_error(&rhs_ty)?;
                    let raw_function_pointer = |ty: &Type| {
                        matches!(
                            ty,
                            Type::Pointer { base, .. }
                                if matches!(base.as_ref(), Type::Function { .. })
                        )
                    };
                    if (raw_function_pointer(&lhs_resolved) || raw_function_pointer(&rhs_resolved))
                        && !matches!(op, BinaryOp::Eq | BinaryOp::Neq)
                    {
                        return Err(PinkerError::Semantic {
                            msg: "ponteiro cru de função aceita apenas igualdade '==' e desigualdade '!='; ordem não possui contrato"
                                .to_string(),
                            span: expr.span,
                        });
                    }
                    if Self::trait_object_name(&lhs_resolved).is_some()
                        || Self::trait_object_name(&rhs_resolved).is_some()
                    {
                        return Err(PinkerError::Semantic {
                            msg: "comparação entre objetos de trato não é suportada: igualdade, ordem e identidade observável ainda não possuem contrato"
                                .to_string(),
                            span: expr.span,
                        });
                    }
                    matches!(op, BinaryOp::Eq | BinaryOp::Neq)
                        && ((raw_function_pointer(&lhs_resolved)
                            && Self::expr_is_zero_literal(rhs))
                            || (raw_function_pointer(&rhs_resolved)
                                && Self::expr_is_zero_literal(lhs)))
                } else {
                    false
                };

                let binary_types_compatible = Self::check_type_match(&lhs_ty, &rhs_ty)
                    || (Self::expr_is_int_literal(lhs) && Self::is_integer_type(&rhs_ty))
                    || (Self::expr_is_int_literal(rhs) && Self::is_integer_type(&lhs_ty))
                    || raw_pointer_null_comparison;
                if !binary_types_compatible {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipos incompatíveis em operação binária: '{}' e '{}'",
                            lhs_ty.name(),
                            rhs_ty.name()
                        ),
                        span: expr.span,
                    });
                }

                match op {
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                        if matches!(lhs_ty, Type::Logica(_)) {
                            Ok(Type::Logica(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação lógica requer operandos 'logica'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
                    | BinaryOp::Shl
                    | BinaryOp::Shr => {
                        if Self::is_integer_type(&lhs_ty) {
                            if Self::expr_is_int_literal(lhs)
                                && !Self::expr_is_int_literal(rhs)
                                && Self::is_integer_type(&rhs_ty)
                            {
                                Ok(rhs_ty.with_span(expr.span))
                            } else {
                                Ok(lhs_ty.with_span(expr.span))
                            }
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "operação aritmética/bitwise requer operandos inteiros compatíveis"
                                    .to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    BinaryOp::Eq | BinaryOp::Neq => {
                        if matches!(&lhs_ty, Type::Union { .. }) {
                            return Err(PinkerError::Semantic {
                                msg: "igualdade e desigualdade de união estrutural não são suportadas nesta fase; use 'encaixe'"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                        if let Type::Enum { name, .. } = &lhs_ty {
                            if self.enum_has_payload(name) {
                                return Err(PinkerError::Semantic {
                                    msg: format!(
                                        "igualdade direta não é suportada para leque com carga ('{}'); use 'encaixe'",
                                        name
                                    ),
                                    span: expr.span,
                                });
                            }
                        }
                        Ok(Type::Logica(expr.span))
                    }
                    BinaryOp::Lt | BinaryOp::Lte | BinaryOp::Gt | BinaryOp::Gte => {
                        if matches!(&lhs_ty, Type::Union { .. }) {
                            return Err(PinkerError::Semantic {
                                msg: "comparação de ordem não é suportada para união estrutural; use 'encaixe'"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                        if matches!(lhs_ty, Type::Enum { .. }) {
                            return Err(PinkerError::Semantic {
                                msg: "comparação de ordem não é suportada entre valores de leque; use '==' ou '!='"
                                    .to_string(),
                                span: expr.span,
                            });
                        }
                        Ok(Type::Logica(expr.span))
                    }
                }
            }
            ExprKind::Unary(op, operand) => {
                let inner_ty = self.check_value_expr(
                    operand,
                    "resultado de função sem retorno não pode ser usado em operação unária",
                )?;
                match op {
                    UnaryOp::Neg => {
                        if Self::is_integer_type(&inner_ty) {
                            Ok(inner_ty.with_span(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação aritmética requer operando inteiro".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::Not => {
                        if matches!(inner_ty, Type::Logica(_)) {
                            Ok(Type::Logica(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação lógica requer operando 'logica'".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::BitNot => {
                        if Self::is_integer_type(&inner_ty) {
                            Ok(inner_ty.with_span(expr.span))
                        } else {
                            Err(PinkerError::Semantic {
                                msg: "negação bitwise requer operando inteiro".to_string(),
                                span: expr.span,
                            })
                        }
                    }
                    UnaryOp::Deref => match inner_ty {
                        Type::Pointer { base, .. } => match base.as_ref() {
                            Type::Bombom(_)
                            | Type::U8(_)
                            | Type::U16(_)
                            | Type::U32(_)
                            | Type::U64(_)
                            | Type::I8(_)
                            | Type::I16(_)
                            | Type::I32(_)
                            | Type::I64(_)
                            | Type::Logica(_) => Ok(base.as_ref().clone().with_span(expr.span)),
                            Type::FixedArray { element, size, .. }
                                if matches!(element.as_ref(), Type::Bombom(_)) =>
                            {
                                Ok(Type::FixedArray {
                                    element: Box::new(Type::Bombom(expr.span)),
                                    size: *size,
                                    span: expr.span,
                                })
                            }
                            Type::Struct { name, .. } => Ok(Type::Struct {
                                name: name.clone(),
                                span: expr.span,
                            }),
                            _ => Err(PinkerError::Semantic {
                                msg: "dereferência aceita ponteiro para escalar público, array suportado ou ninho".to_string(),
                                span: expr.span,
                            }),
                        },
                        _ => Err(PinkerError::Semantic {
                            msg: "dereferência requer operando do tipo 'seta<T>'".to_string(),
                            span: expr.span,
                        }),
                    },
                }
            }
        }
    }

    fn check_pointer_arithmetic(
        &self,
        expr_span: Span,
        op: BinaryOp,
        lhs_ty: &Type,
        rhs_ty: &Type,
        rhs_expr: &Expr,
    ) -> Option<Result<Type, PinkerError>> {
        let is_bombom = |ty: &Type| matches!(ty, Type::Bombom(_));

        if let Type::Pointer { base, .. } = lhs_ty {
            if is_bombom(rhs_ty) {
                if op == BinaryOp::Sub {
                    return Some(if matches!(base.as_ref(), Type::Bombom(_)) {
                        Ok(lhs_ty.with_span(expr_span))
                    } else {
                        Err(PinkerError::Semantic {
                            msg: "subtração de ponteiro preserva somente o contrato legado 'seta<bombom> - bombom'; D5 não amplia esta operação".to_string(),
                            span: expr_span,
                        })
                    });
                }

                if matches!(rhs_expr.kind, ExprKind::Unary(UnaryOp::Neg, _)) {
                    return Some(Err(PinkerError::Semantic {
                        msg: "deslocamento de seta<T> deve ser 'bombom' não negativo".to_string(),
                        span: rhs_expr.span,
                    }));
                }

                let supported = matches!(
                    base.as_ref(),
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
                        | Type::Struct { .. }
                ) || matches!(
                    base.as_ref(),
                    Type::FixedArray { element, .. }
                        if matches!(element.as_ref(), Type::Bombom(_))
                );
                if !supported {
                    return Some(Err(PinkerError::Semantic {
                        msg: format!(
                            "aritmética de seta<T> exige elemento com layout e acesso coerentes; '{}' não participa de D5",
                            base.name()
                        ),
                        span: expr_span,
                    }));
                }

                let element_layout =
                    match layout::layout_of_type(base, &self.type_aliases, &self.structs) {
                        Ok(layout) => layout,
                        Err(msg) => {
                            return Some(Err(PinkerError::Semantic {
                                msg: format!(
                                    "aritmética de seta<T> exige layout canônico conhecido: {}",
                                    msg
                                ),
                                span: expr_span,
                            }))
                        }
                    };
                if element_layout.size == 0
                    || element_layout.align == 0
                    || element_layout.size % element_layout.align != 0
                {
                    return Some(Err(PinkerError::Semantic {
                        msg: "layout de elemento inválido para aritmética de seta<T>".to_string(),
                        span: expr_span,
                    }));
                }
                if let ExprKind::IntLit(offset) = rhs_expr.kind {
                    if offset.checked_mul(element_layout.size).is_none() {
                        return Some(Err(PinkerError::Semantic {
                            msg: "E-POINTER-OFFSET-OVERFLOW: overflow ao escalar deslocamento de seta<T>"
                                .to_string(),
                            span: rhs_expr.span,
                        }));
                    }
                }
                return Some(Ok(lhs_ty.with_span(expr_span)));
            }
        }
        if is_bombom(lhs_ty) && matches!(rhs_ty, Type::Pointer { .. }) {
            let msg = match op {
                BinaryOp::Add => "aritmética de ponteiro suporta apenas 'seta<T> + bombom'",
                BinaryOp::Sub => "subtração de ponteiro nesta fase suporta apenas 'ptr - bombom'",
                _ => unreachable!("check_pointer_arithmetic só recebe add/sub"),
            };
            return Some(Err(PinkerError::Semantic {
                msg: msg.to_string(),
                span: expr_span,
            }));
        }
        if matches!(lhs_ty, Type::Pointer { .. }) || matches!(rhs_ty, Type::Pointer { .. }) {
            let msg = match op {
                BinaryOp::Add => "aritmética de ponteiro exige 'seta<T> + bombom'",
                BinaryOp::Sub => "aritmética de ponteiro nesta fase exige 'seta<bombom> - bombom'",
                _ => unreachable!("check_pointer_arithmetic só recebe add/sub"),
            };
            return Some(Err(PinkerError::Semantic {
                msg: msg.to_string(),
                span: expr_span,
            }));
        }
        None
    }
    // @pinker-nav:end semantic.expressoes.verificacao

    // @pinker-nav:start semantic.chamadas.despacho
    // @pinker-nav:domain chamadas
    // @pinker-nav:layer semantic
    // @pinker-nav:summary Despacho de chamadas: resolução de método de impl (direta e qualificada por trato, com detecção de ambiguidade), restringida aos tratos que a unidade-fonte da chamada autorizou — uma chamada de método não nomeia o trato, então sem esse filtro um trato da raiz forneceria método default ao corpo de um módulo que nunca o importou, seleção monomórfica das intrínsecas genéricas de mapa, checagem de chamada nomeada (aridade e tipos de argumento) e o despachante `check_call_expr` — construção de variante de leque, desugaring de `encaixe`, a checagem genérica das grafias históricas de contrato declarado, dirigida por `intrinsics::registry`, e os contratos próprios que sobram (aridade variável, formas genéricas de lista/mapa e restrições que não cabem em `(params, ret)`), caindo para a chamada de função declarada.
    fn check_trait_object_method_call(
        &mut self,
        expr_span: Span,
        callee_span: Span,
        trait_name: &str,
        method_name: &str,
        args: &[Expr],
    ) -> Result<Type, PinkerError> {
        let (method_params, method_ret_type) = {
            let trait_decl = self
                .traits
                .get(trait_name)
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!("trato '{}' não declarado", trait_name),
                    span: callee_span,
                })?;

            let method = trait_decl
                .methods
                .iter()
                .find(|method| method.name == method_name)
                .ok_or_else(|| PinkerError::Semantic {
                    msg: format!(
                        "método '{}' não existe no trato objetificável '{}'",
                        method_name, trait_name
                    ),
                    span: callee_span,
                })?;

            (
                method.params.iter().skip(1).cloned().collect::<Vec<_>>(),
                method.ret_type.clone(),
            )
        };

        if args.len() != method_params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "chamada dinâmica de '{}.{}' com aridade inválida: esperado {}, recebido {}",
                    trait_name,
                    method_name,
                    method_params.len(),
                    args.len()
                ),
                span: expr_span,
            });
        }

        for (index, (arg, expected)) in args.iter().zip(method_params.iter()).enumerate() {
            let arg_ty = self.check_value_expr(
                arg,
                "resultado de função sem retorno não pode ser usado como argumento de método dinâmico",
            )?;
            let expected_ty = self.resolve_type_or_error(&expected.ty)?;

            if !Self::check_expected_type_for_expr(&expected_ty, &arg_ty, arg) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento {} da chamada dinâmica '{}.{}': esperado '{}', encontrado '{}'",
                        index + 1,
                        trait_name,
                        method_name,
                        Self::type_key(&expected_ty),
                        Self::type_key(&arg_ty)
                    ),
                    span: arg.span,
                });
            }

            Self::validate_int_literal_range(&expected_ty, arg)?;
        }

        match method_ret_type {
            Some(ret_type) => self
                .resolve_type_or_error(&ret_type)
                .map(|resolved| resolved.with_span(expr_span)),
            None => Ok(Type::Nulo(expr_span)),
        }
    }

    fn resolve_impl_method(
        &self,
        receiver_ty: &Type,
        method_name: &str,
        span: Span,
    ) -> Result<String, PinkerError> {
        let direct_key = Self::type_key(receiver_ty);
        let resolved_key = self.resolved_type_identity(receiver_ty)?;
        let candidates = self
            .method_index
            .get(&(resolved_key, method_name.to_string()))
            .cloned()
            .unwrap_or_default();
        // MODULE_IMPORTER_NON_INTERFERENCE para despacho de método: só entram
        // os candidatos cujo trato alcança a unidade que escreveu a chamada. Um
        // trato declarado na raiz deixa de fornecer método default ao corpo de
        // um módulo que nunca o importou.
        //
        // #577: alcançar tem dois níveis. O trato que a unidade declarou ou
        // importou é autoridade dela; o que ela só alcança porque importou quem
        // implementa é dependência semântica transportada. Resolver pelo nível
        // mais forte que produzir candidato é o que deixa a raiz consumir a
        // superfície do implementador sem que essa dependência dispute com o
        // que a própria raiz autorizou — inclusive com um homônimo dela.
        let candidates: Vec<(NivelDeDespacho, String)> = candidates
            .into_iter()
            .filter_map(|function_name| {
                match self
                    .impl_methods
                    .iter()
                    .find(|meta| meta.function_name == function_name)
                {
                    Some(meta) => {
                        let fonte = self
                            .fontes_das_relacoes
                            .get(&(
                                meta.identity.trait_name.clone(),
                                meta.identity.target.clone(),
                            ))
                            .copied();
                        self.nivel_de_despacho(span, &meta.identity.trait_name, fonte)
                            .map(|nivel| (nivel, function_name))
                    }
                    None => Some((NivelDeDespacho::Proprio, function_name)),
                }
            })
            .collect();
        let candidates: Vec<String> = match candidates.iter().map(|(nivel, _)| *nivel).min() {
            Some(mais_forte) => candidates
                .into_iter()
                .filter(|(nivel, _)| *nivel == mais_forte)
                .map(|(_, function_name)| function_name)
                .collect(),
            None => Vec::new(),
        };

        match candidates.as_slice() {
            [function_name] => Ok(function_name.clone()),
            [] => Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' não implementado para tipo '{}'",
                    method_name, direct_key
                ),
                span,
            }),
            _ => Err(PinkerError::Semantic {
                msg: format!(
                    "método '{}' para tipo '{}' é ambíguo; use 'Trato.{}(valor, ...)'",
                    method_name, direct_key, method_name
                ),
                span,
            }),
        }
    }

    fn resolve_qualified_impl_method(
        &self,
        trait_name: &str,
        receiver_ty: &Type,
        method_name: &str,
        span: Span,
    ) -> Result<String, PinkerError> {
        let direct_key = Self::type_key(receiver_ty);
        let resolved_key = self.resolved_type_identity(receiver_ty)?;
        if let Some(meta) = self.impl_methods.iter().find(|meta| {
            meta.identity.trait_name == trait_name
                && meta.identity.target == resolved_key
                && meta.identity.method_name == method_name
        }) {
            return Ok(meta.function_name.clone());
        }
        Err(PinkerError::Semantic {
            msg: format!(
                "método '{}.{}' não implementado para tipo '{}'",
                trait_name, method_name, direct_key
            ),
            span,
        })
    }

    fn generic_map_monomorphic_callee(map_ty: &Type, name: &str) -> Option<&'static str> {
        match (map_ty, name) {
            (Type::MapVersoBombom(_), "mapa_definir") => Some("mapa_verso_bombom_definir"),
            (Type::MapVersoBombom(_), "mapa_obter") => Some("mapa_verso_bombom_obter"),
            (Type::MapVersoBombom(_), "mapa_tem") => Some("mapa_verso_bombom_tem"),
            (Type::MapVersoBombom(_), "mapa_tamanho") => Some("mapa_verso_bombom_tamanho"),
            (Type::MapVersoBombom(_), "mapa_remover") => Some("mapa_verso_bombom_remover"),
            (Type::MapVersoVerso(_), "mapa_definir") => Some("mapa_verso_verso_definir"),
            (Type::MapVersoVerso(_), "mapa_obter") => Some("mapa_verso_verso_obter"),
            (Type::MapVersoVerso(_), "mapa_tem") => Some("mapa_verso_verso_tem"),
            (Type::MapVersoVerso(_), "mapa_tamanho") => Some("mapa_verso_verso_tamanho"),
            (Type::MapVersoVerso(_), "mapa_remover") => Some("mapa_verso_verso_remover"),
            (Type::MapBombomBombom(_), "mapa_definir") => Some("mapa_bombom_bombom_definir"),
            (Type::MapBombomBombom(_), "mapa_obter") => Some("mapa_bombom_bombom_obter"),
            (Type::MapBombomBombom(_), "mapa_tem") => Some("mapa_bombom_bombom_tem"),
            (Type::MapBombomBombom(_), "mapa_tamanho") => Some("mapa_bombom_bombom_tamanho"),
            (Type::MapBombomBombom(_), "mapa_remover") => Some("mapa_bombom_bombom_remover"),
            (Type::MapBombomVerso(_), "mapa_definir") => Some("mapa_bombom_verso_definir"),
            (Type::MapBombomVerso(_), "mapa_obter") => Some("mapa_bombom_verso_obter"),
            (Type::MapBombomVerso(_), "mapa_tem") => Some("mapa_bombom_verso_tem"),
            (Type::MapBombomVerso(_), "mapa_tamanho") => Some("mapa_bombom_verso_tamanho"),
            (Type::MapBombomVerso(_), "mapa_remover") => Some("mapa_bombom_verso_remover"),
            _ => None,
        }
    }

    fn check_named_function_call(
        &mut self,
        expr_span: Span,
        callee_span: Span,
        name: &str,
        args: &[&Expr],
    ) -> Result<Type, PinkerError> {
        // MODULE_IMPORTER_NON_INTERFERENCE, última fronteira: a busca por
        // função de usuário acontece depois do despacho de intrínsecas, então
        // chegar aqui com grafia crua vinda de um módulo significa que o
        // builtin não atendeu e a única candidata restante é da raiz.
        if self.grafia_crua_de_modulo(callee_span, name) && self.funcs.contains_key(name) {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "função '{}' não declarada neste ambiente: ela é declarada na raiz, e o módulo não a importou",
                    name
                ),
                span: callee_span,
            });
        }
        let Some(function) = self.funcs.get(name).cloned() else {
            return Err(PinkerError::Semantic {
                msg: format!("função '{}' não declarada", name),
                span: callee_span,
            });
        };

        if args.len() != function.params.len() {
            return Err(PinkerError::Semantic {
                msg: format!(
                    "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                    name,
                    function.params.len(),
                    args.len()
                ),
                span: expr_span,
            });
        }

        for (index, (arg, param)) in args.iter().zip(function.params.iter()).enumerate() {
            let arg_ty = self.check_value_expr(
                arg,
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            let expected_param_ty = self.resolve_type_or_error(&param.ty)?;
            if !Self::check_expected_type_for_expr(&expected_param_ty, &arg_ty, arg) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                        index + 1,
                        name,
                        Self::type_key(&expected_param_ty),
                        Self::type_key(&arg_ty)
                    ),
                    span: arg.span,
                });
            }
            Self::validate_int_literal_range(&expected_param_ty, arg)?;
        }

        Ok(self.function_result_type(&function, expr_span))
    }

    fn check_call_expr(
        &mut self,
        expr_span: Span,
        callee: &Expr,
        args: &[Expr],
    ) -> Result<Type, PinkerError> {
        // Construção de variante de leque: `Leque.Variante(carga)`.
        if let ExprKind::FieldAccess { base, field } = &callee.kind {
            if let ExprKind::Ident(base_name) = &base.kind {
                if let Some(enum_name) = self.resolve_enum_base_name(base_name) {
                    let enum_decl = self.enums.get(&enum_name).expect("leque resolvido existe");
                    let Some(variant) = enum_decl
                        .variants
                        .iter()
                        .find(|variant| variant.name == *field)
                        .cloned()
                    else {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "variante '{}' não existe no leque '{}'",
                                field, base_name
                            ),
                            span: expr_span,
                        });
                    };
                    if variant.payloads.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "variante '{}' não carrega valor; use '{}.{}' sem parênteses",
                                field, enum_name, field
                            ),
                            span: expr_span,
                        });
                    }
                    if args.len() != variant.payloads.len() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "construção de '{}.{}' exige {} argumento(s) de carga, recebido {}",
                                enum_name,
                                field,
                                variant.payloads.len(),
                                args.len()
                            ),
                            span: expr_span,
                        });
                    }
                    for (index, payload_ty) in variant.payloads.iter().enumerate() {
                        let expected = self.resolve_type_or_error(payload_ty)?;
                        let arg_ty = self.check_value_expr(
                            &args[index],
                            "resultado de função sem retorno não pode ser carga de variante",
                        )?;
                        if !Self::check_expected_type_for_expr(&expected, &arg_ty, &args[index]) {
                            return Err(PinkerError::Semantic {
                                msg: format!(
                                    "carga {} inválida para '{}.{}': esperado '{}', encontrado '{}'",
                                    index + 1,
                                    enum_name,
                                    field,
                                    // Nome fiel: a mensagem existe para
                                    // distinguir `lista<Cor>` de `lista<Token>`,
                                    // que compartilham a categoria operacional.
                                    expected.display_name(),
                                    arg_ty.display_name()
                                ),
                                span: args[index].span,
                            });
                        }
                    }
                    return Ok(Type::Enum {
                        name: enum_name,
                        span: expr_span,
                    });
                }
                // Parte G: `familia.membro(...)` sem o `trazer`, em posição de
                // chamada. Mesma regra do acesso a campo — a dica só sai
                // depois de o nome se provar órfão.
                if let Some(erro) = self.dica_de_familia_nao_importada(base_name, field, expr_span)
                {
                    return Err(erro);
                }
                if self.traits.contains_key(base_name) {
                    if args.is_empty() {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "chamada qualificada '{}.{}' exige receiver como primeiro argumento",
                                base_name, field
                            ),
                            span: expr_span,
                        });
                    }
                    let receiver_ty = self.check_value_expr(
                        &args[0],
                        "resultado de função sem retorno não pode ser receiver de método",
                    )?;
                    if Self::trait_object_name(&receiver_ty) == Some(base_name.as_str()) {
                        return self.check_trait_object_method_call(
                            expr_span,
                            callee.span,
                            base_name,
                            field,
                            &args[1..],
                        );
                    }

                    let function_name = self.resolve_qualified_impl_method(
                        base_name,
                        &receiver_ty,
                        field,
                        callee.span,
                    )?;
                    let qualified_args: Vec<&Expr> = args.iter().collect();
                    return self.check_named_function_call(
                        expr_span,
                        callee.span,
                        &function_name,
                        &qualified_args,
                    );
                }
            }
            let receiver_ty = self.check_value_expr(
                base,
                "resultado de função sem retorno não pode ser receiver de método",
            )?;
            if let Some(trait_name) = Self::trait_object_name(&receiver_ty).map(str::to_string) {
                return self.check_trait_object_method_call(
                    expr_span,
                    callee.span,
                    &trait_name,
                    field,
                    args,
                );
            }

            let function_name = match self.resolve_impl_method(&receiver_ty, field, callee.span) {
                Ok(function_name) => function_name,
                Err(_) if self.funcs.contains_key(field) => field.clone(),
                Err(err) => return Err(err),
            };
            let mut method_args = Vec::with_capacity(args.len() + 1);
            method_args.push(base.as_ref());
            method_args.extend(args.iter());
            return self.check_named_function_call(
                expr_span,
                callee.span,
                &function_name,
                &method_args,
            );
        }

        // #532 — CANONICALIZATION_BOUNDARY do lado do consumidor.
        //
        // `nome_fonte` é o texto que o usuário escreveu (ou o nome sintético que
        // o compilador materializou); `identidade_do_callee` é a decisão que a
        // resolução já tomou. As duas coisas eram uma só, e por isso a cadeia
        // de intrínsecas abaixo respondia por uma função do usuário homônima.
        let identidade_do_callee = match &callee.kind {
            ExprKind::Intrinsic(identity) => {
                crate::intrinsics::identity::CalleeIdentity::Intrinsic(*identity)
            }
            ExprKind::Ident(name) => {
                crate::intrinsics::identity::callee_identity_de_ident(name.as_str())
            }
            _ => crate::intrinsics::identity::CalleeIdentity::User,
        };
        let grafia_do_callee = match &callee.kind {
            ExprKind::Intrinsic(identity) => Some(identity.canonical_public_spelling().to_string()),
            ExprKind::Ident(name) => Some(name.clone()),
            _ => None,
        };
        let Some(name) = grafia_do_callee.as_ref() else {
            let callee_ty = self.check_value_expr(
                callee,
                "resultado sem retorno não pode ocupar a posição de chamada",
            )?;
            let Type::Pointer { ref base, .. } = callee_ty else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "expressão em posição de chamada não é chamável (tipo '{}')",
                        Self::type_key(&callee_ty)
                    ),
                    span: callee.span,
                });
            };
            let Type::Function { params, ret, .. } = base.as_ref() else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "expressão em posição de chamada não é ponteiro cru de função (tipo '{}')",
                        Self::type_key(&callee_ty)
                    ),
                    span: callee.span,
                });
            };
            if args.len() != params.len() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada por expressão de ponteiro cru com aridade inválida: esperado {}, recebido {}",
                        params.len(),
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let params = params.clone();
            let ret = ret.as_ref().clone();
            for (index, (arg, expected)) in args.iter().zip(params.iter()).enumerate() {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado sem retorno não pode ser argumento de ponteiro cru",
                )?;
                let expected_resolved = self.resolve_type_or_error(expected)?;
                if !Self::check_expected_type_for_expr(&expected_resolved, &arg_ty, arg) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da chamada por expressão de ponteiro cru: esperado '{}', encontrado '{}'",
                            index + 1,
                            Self::type_key(&expected_resolved),
                            Self::type_key(&arg_ty)
                        ),
                        span: arg.span,
                    });
                }
                Self::validate_int_literal_range(&expected_resolved, arg)?;
            }
            return self.resolve_type_or_error(&ret);
        };

        // Fase 242: variável local (parâmetro ou `nova`) tem precedência
        // sobre função top-level homônima em posição de chamada — chamada
        // indireta real, sem depender de resolução estática do nome
        // concreto no parse (ao contrário da especialização da Fase 239).
        if let Some(local_ty) = self.resolve_local_var_type(name) {
            let (params, ret, raw) = match &local_ty {
                Type::Function { params, ret, .. } => (params, ret.as_ref(), false),
                Type::Pointer { base, .. } => match base.as_ref() {
                    Type::Function { params, ret, .. } => (params, ret.as_ref(), true),
                    _ => {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "'{}' não é ponteiro de função chamável (tipo '{}')",
                                name,
                                Self::type_key(&local_ty)
                            ),
                            span: callee.span,
                        });
                    }
                },
                _ => {
                    return Err(PinkerError::Semantic {
                        msg: format!("'{}' não é chamável (tipo '{}')", name, local_ty.name()),
                        span: callee.span,
                    });
                }
            };
            if args.len() != params.len() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "{} de '{}' com aridade inválida: esperado {}, recebido {}",
                        if raw {
                            "chamada por ponteiro cru"
                        } else {
                            "chamada indireta"
                        },
                        name,
                        params.len(),
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let params = params.clone();
            let ret = ret.clone();
            for (index, (arg, expected)) in args.iter().zip(params.iter()).enumerate() {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                let expected_resolved = self.resolve_type_or_error(expected)?;
                if !Self::check_expected_type_for_expr(&expected_resolved, &arg_ty, arg) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da {} de '{}': esperado '{}', encontrado '{}'",
                            index + 1,
                            if raw {
                                "chamada por ponteiro cru"
                            } else {
                                "chamada indireta"
                            },
                            name,
                            Self::type_key(&expected_resolved),
                            Self::type_key(&arg_ty)
                        ),
                        span: arg.span,
                    });
                }
                Self::validate_int_literal_range(&expected_resolved, arg)?;
            }
            return self.resolve_type_or_error(&ret);
        }

        // #532 — daqui para baixo começa a cadeia de intrínsecas, e ela só é
        // atravessada por um callee cuja IDENTIDADE diz que ele é builtin.
        //
        // ```text
        // CALL_IS_INTRINSIC <- RESOLVED_IDENTITY, NOT SPELLING
        // ```
        //
        // Antes, a cadeia era atravessada por qualquer chamada, e a primeira
        // comparação textual que casasse decidia. Era isso que obrigava a
        // reservar as grafias canônicas contra declaração do usuário: sem a
        // reserva, `carinho tamanho_verso(...)` seria aceito e depois sombreado
        // aqui em silêncio. Com a decisão vindo da identidade, o callee de
        // usuário vai direto para a resolução de função de usuário.
        if identidade_do_callee.is_user() {
            let arg_refs: Vec<&Expr> = args.iter().collect();
            return self.check_named_function_call(expr_span, callee.span, name, &arg_refs);
        }

        // Fase 246: superfície pública de memória explícita. O tamanho é
        // sempre expresso em bytes (`u64`) e o ponteiro devolvido é `seta<u8>`.
        if name == "alocar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'alocar' exige exatamente 1 argumento de tamanho em bytes, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let expected = Type::U64(args[0].span);
            let actual = self.check_value_expr(
                &args[0],
                "resultado sem retorno não pode ser tamanho de alocação",
            )?;
            if !Self::check_expected_type_for_expr(&expected, &actual, &args[0]) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'alocar' exige tamanho 'u64' em bytes; encontrado '{}'",
                        Self::type_key(&actual)
                    ),
                    span: args[0].span,
                });
            }
            Self::validate_int_literal_range(&expected, &args[0])?;
            return Ok(Type::Pointer {
                base: Box::new(Type::U8(expr_span)),
                is_volatile: false,
                span: expr_span,
            });
        }
        if name == "liberar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'liberar' exige exatamente 1 ponteiro-base, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let actual =
                self.check_value_expr(&args[0], "resultado sem retorno não pode ser liberado")?;
            let expected = Type::Pointer {
                base: Box::new(Type::U8(args[0].span)),
                is_volatile: false,
                span: args[0].span,
            };
            if !Self::check_type_match(&expected, &actual) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'liberar' exige ponteiro-base 'seta<u8>'; encontrado '{}'",
                        Self::type_key(&actual)
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Nulo(expr_span));
        }

        if matches!(
            name.as_str(),
            "mapa_definir" | "mapa_obter" | "mapa_tem" | "mapa_tamanho" | "mapa_remover"
        ) {
            let Some(first_arg) = args.first() else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado ao menos 1 argumento",
                        name
                    ),
                    span: expr_span,
                });
            };
            let map_ty = self.check_value_expr(
                first_arg,
                "resultado de função sem retorno não pode ser usado como mapa",
            )?;
            if let Some(mono_name) = Self::generic_map_monomorphic_callee(&map_ty, name) {
                // #532: a monomorfização troca a grafia DENTRO da identidade
                // intrínseca. Reemitir um `Ident` aqui devolveria a chamada ao
                // namespace do usuário, e ela voltaria a poder ser capturada
                // por uma função homônima.
                let mono_callee = Expr {
                    kind: ExprKind::Intrinsic(
                        crate::intrinsics::identity::intrinsic_from_public_spelling(mono_name)
                            .expect("forma monomórfica de mapa é grafia pública registrada"),
                    ),
                    span: callee.span,
                };
                return self.check_call_expr(expr_span, &mono_callee, args);
            }
            if let Type::Map { key, value, .. } = &map_ty {
                let expected_arity = match name.as_str() {
                    "mapa_definir" => 3,
                    "mapa_tamanho" => 1,
                    _ => 2,
                };
                if args.len() != expected_arity {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                            name,
                            expected_arity,
                            args.len()
                        ),
                        span: expr_span,
                    });
                }
                if expected_arity >= 2 {
                    let actual_key = self.check_value_expr(
                        &args[1],
                        "resultado sem retorno não pode ser usado como chave de mapa",
                    )?;
                    if !Self::check_type_match(key, &actual_key) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "chave incompatível em '{}': esperado '{}', encontrado '{}'",
                                name,
                                Self::type_key(key),
                                Self::type_key(&actual_key)
                            ),
                            span: args[1].span,
                        });
                    }
                }
                if name == "mapa_definir" {
                    let actual_value = self.check_value_expr(
                        &args[2],
                        "resultado sem retorno não pode ser armazenado em mapa",
                    )?;
                    if !Self::check_type_match(value, &actual_value) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "valor incompatível em 'mapa_definir': esperado '{}', encontrado '{}'",
                                Self::type_key(value),
                                Self::type_key(&actual_value)
                            ),
                            span: args[2].span,
                        });
                    }
                }
                return Ok(match name.as_str() {
                    "mapa_obter" => value.with_span(expr_span),
                    "mapa_tem" => Type::Logica(expr_span),
                    "mapa_tamanho" => Type::Bombom(expr_span),
                    "mapa_definir" | "mapa_remover" => Type::Nulo(expr_span),
                    _ => unreachable!(),
                });
            }
            return Err(PinkerError::Semantic {
                msg: format!(
                    "operação genérica '{}' exige mapa como primeiro argumento; encontrado '{}'",
                    name,
                    map_ty.name()
                ),
                span: first_arg.span,
            });
        }

        if name == "__pinker_internal_mapa_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado sem retorno não pode ser iterado como mapa",
            )?;
            if !matches!(map_ty, Type::Map { .. }) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "iterador interno exige mapa genérico; encontrado '{}'",
                        Self::type_key(&map_ty)
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }

        if matches!(
            name.as_str(),
            "__pinker_internal_mapa_iterador_proxima_chave_bombom"
                | "__pinker_internal_mapa_iterador_proxima_chave_verso"
        ) {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "avanço de iterador interno de mapa exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let cursor_ty = self.check_value_expr(
                &args[0],
                "resultado sem retorno não pode ser cursor de mapa",
            )?;
            if !matches!(cursor_ty, Type::Bombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: "cursor interno de mapa exige 'bombom'".to_string(),
                    span: args[0].span,
                });
            }
            return Ok(if name.ends_with("_verso") {
                Type::Verso(expr_span)
            } else {
                Type::Bombom(expr_span)
            });
        }

        // As operações de tag e extração de união **não** são chamadas da
        // linguagem: são nós tipados da IR (`ValueIR::UnionTag` e
        // `ValueIR::UnionExtract`) criados pelo lowering a partir de
        // `Stmt::UnionMatch`. Não há intrínseca de união chamável aqui, e o
        // namespace `__pinker_internal_` permanece recusado à fonte.

        // Intrínsecas internas do desugaring de `encaixe` (Fases 209–210).
        if name == "__pinker_internal_leque_tag" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige 1 argumento (valor de leque)",
                        name
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser inspecionado por encaixe",
            )?;
            let Type::Enum {
                name: enum_name, ..
            } = &arg_ty
            else {
                return Err(PinkerError::Semantic {
                    msg: format!("intrínseca interna '{}' exige valor de leque", name),
                    span: args[0].span,
                });
            };
            if !self.enum_has_payload(enum_name) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige leque com carga; '{}' não tem variantes com carga",
                        name, enum_name
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if crate::enum_payload::is_carga_intrinsic(name) {
            if args.len() != 3 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige 3 argumentos (leque, tag, índice)",
                        name
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser inspecionado por encaixe",
            )?;
            let Type::Enum {
                name: enum_name, ..
            } = &arg_ty
            else {
                return Err(PinkerError::Semantic {
                    msg: format!("intrínseca interna '{}' exige valor de leque", name),
                    span: args[0].span,
                });
            };
            let (ExprKind::IntLit(tag), ExprKind::IntLit(index)) = (&args[1].kind, &args[2].kind)
            else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' exige tag e índice literais (uso interno do encaixe)",
                        name
                    ),
                    span: expr_span,
                });
            };
            let enum_name = enum_name.clone();
            let payload_ty = self
                .enums
                .get(&enum_name)
                .and_then(|decl| decl.variants.get(*tag as usize))
                .and_then(|variant| variant.payloads.get(*index as usize))
                .cloned();
            let Some(payload_ty) = payload_ty else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' referencia carga inexistente no leque '{}'",
                        name, enum_name
                    ),
                    span: expr_span,
                });
            };
            // O helper usado tem de ser exatamente o que a autoridade de
            // classificação escolhe para esta carga. Isso impede que uma
            // extração `lista<verso>` seja encaminhada pelo caminho de `verso`
            // — ou o contrário — mesmo que ambos caibam numa palavra.
            let shape = self.classify_enum_payload(&payload_ty).map_err(|rejection| {
                PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' usada com carga sem classificação no leque '{}'; {}",
                        name,
                        enum_name,
                        rejection.message()
                    ),
                    span: expr_span,
                }
            })?;
            if shape.carga_intrinsic() != name {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "intrínseca interna '{}' usada com carga de classe '{}' no leque '{}'; o helper correto é '{}'",
                        name,
                        shape.class.name(),
                        enum_name,
                        shape.carga_intrinsic()
                    ),
                    span: expr_span,
                });
            }
            return Ok(shape.resolved.with_span(expr_span));
        }

        // Intrínsecas genéricas de lista (Fase 211): tipam sobre qualquer
        // lista (`lista<bombom>`, `lista<verso>`, `lista<Leque>`), com o tipo
        // do elemento derivado do primeiro argumento.
        if name == "lista_criar" {
            return Err(PinkerError::Semantic {
                msg: "'lista_criar()' só pode aparecer como inicialização de 'nova' com anotação de tipo de lista nesta fase"
                    .to_string(),
                span: expr_span,
            });
        }
        if matches!(
            name.as_str(),
            "lista_tamanho"
                | "lista_obter"
                | "lista_anexar"
                | "lista_definir"
                | "lista_tirar_ultimo"
                | "lista_inserir"
        ) {
            let expected_arity: usize = match name.as_str() {
                "lista_tamanho" | "lista_tirar_ultimo" => 1,
                "lista_obter" | "lista_anexar" => 2,
                _ => 3,
            };
            if args.len() != expected_arity {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                        name,
                        expected_arity,
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let list_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como lista",
            )?;
            let Some(element_ty) = Self::list_element_type(&list_ty, expr_span) else {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "'{}' exige lista no argumento 1, encontrado '{}'",
                        name,
                        list_ty.name()
                    ),
                    span: args[0].span,
                });
            };
            let check_index = |checker: &mut Self, index_arg: &Expr| -> Result<(), PinkerError> {
                let index_ty = checker.check_value_expr(
                    index_arg,
                    "resultado de função sem retorno não pode ser índice",
                )?;
                if !matches!(index_ty, Type::Bombom(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!("'{}' exige índice 'bombom'", name),
                        span: index_arg.span,
                    });
                }
                Ok(())
            };
            let check_element = |checker: &mut Self, value_arg: &Expr| -> Result<(), PinkerError> {
                let value_ty = checker.check_value_expr(
                    value_arg,
                    "resultado de função sem retorno não pode ser elemento de lista",
                )?;
                if !Self::check_expected_type_for_expr(&element_ty, &value_ty, value_arg) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "'{}' exige elemento '{}', encontrado '{}'",
                            name,
                            element_ty.name(),
                            value_ty.name()
                        ),
                        span: value_arg.span,
                    });
                }
                Ok(())
            };
            return match name.as_str() {
                "lista_tamanho" => Ok(Type::Bombom(expr_span)),
                "lista_tirar_ultimo" => Ok(element_ty),
                "lista_obter" => {
                    check_index(self, &args[1])?;
                    Ok(element_ty)
                }
                "lista_anexar" => {
                    check_element(self, &args[1])?;
                    Ok(Type::Nulo(expr_span))
                }
                "lista_definir" | "lista_inserir" => {
                    check_index(self, &args[1])?;
                    check_element(self, &args[2])?;
                    Ok(Type::Nulo(expr_span))
                }
                _ => unreachable!(),
            };
        }

        // #442/C1 — o contrato declarativo da grafia histórica vem do registry.
        //
        // Estes cento e dez blocos eram cento e dez cópias do mesmo formato:
        // aridade exata, um tipo por argumento, tipo de retorno. O texto dos
        // diagnósticos é idêntico ao que cada bloco montava — a grafia usada é
        // a que o call site escreveu, então alias histórico continua se
        // diagnosticando pelo próprio nome.
        //
        // As grafias com contrato próprio — aridade variável, forma genérica
        // ainda não monomorfizada ou restrição que não cabe em
        // `(params, ret)` — seguem abaixo, marcadas no registry como
        // `SemanticContract::PhaseSpecific`.
        if let Some(contrato) = crate::intrinsics::registry::entrada(name) {
            if contrato.semantic == crate::intrinsics::registry::SemanticContract::Declared {
                let (ret, params) = contrato
                    .assinatura_ir()
                    .expect("contrato declarado tem assinatura no registry");
                if args.len() != params.len() {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                            name,
                            params.len(),
                            args.len()
                        ),
                        span: expr_span,
                    });
                }
                for (index, (arg, esperado)) in args.iter().zip(params).enumerate() {
                    let arg_ty = self.check_value_expr(
                        arg,
                        "resultado de função sem retorno não pode ser usado como argumento",
                    )?;
                    let esperado = Self::tipo_de_intrinseca(*esperado, arg.span);
                    if std::mem::discriminant(&arg_ty) != std::mem::discriminant(&esperado) {
                        return Err(PinkerError::Semantic {
                            msg: format!(
                                "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                                index + 1,
                                name,
                                esperado.name(),
                                arg_ty.name()
                            ),
                            span: arg.span,
                        });
                    }
                }
                return Ok(Self::tipo_de_intrinseca(ret, expr_span));
            }
        }
        if name == "afirmar" {
            if args.is_empty() || args.len() > 2 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'afirmar' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let cond_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(cond_ty, Type::Logica(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'afirmar': esperado 'logica', encontrado '{}'",
                        cond_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let msg_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(msg_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'afirmar': esperado 'verso', encontrado '{}'",
                            msg_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Nulo(expr_span));
        }
        // Superfícies falíveis: assinatura e retorno vêm da autoridade única.
        if let Some(superficie) = crate::falha_operacional::superficie(name) {
            // Parte B1: esta chamada produz uma tag cujo significado vem da
            // taxonomia do leque materializado. Aqui o programa já está
            // completo — imports resolvidos, genéricos monomorfizados —, então é
            // o ponto onde a pergunta pode ser respondida sobre o artefato
            // inteiro, e não sobre uma unidade de compilação. A condição
            // continua sendo a mesma da conjunção do parser: só é verificada
            // porque o programa realmente produz o valor.
            self.check_runtime_result_identity(superficie, expr_span)?;
            if args.len() != superficie.aridade() {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                        name,
                        superficie.aridade(),
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            for (index, (arg, esperado)) in
                args.iter().zip(superficie.argumentos.iter()).enumerate()
            {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !esperado.aceita(&arg_ty) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da chamada '{}': esperado '{}', encontrado '{}'",
                            index + 1,
                            name,
                            esperado.nome_para_diagnostico(),
                            arg_ty.display_name()
                        ),
                        span: arg.span,
                    });
                }
            }
            return Ok(superficie.tipo_de_retorno(expr_span));
        }
        // Parte E1 — acessores da árvore JSON.
        //
        // O primeiro argumento é sempre `ValorJson`; a aridade e o tipo do
        // segundo derivam da intrínseca. `json_lista_obter` e
        // `json_objeto_obter` devolvem `ValorJson`, que é o que torna o nesting
        // atravessável sem superfície nova por formato.
        if crate::valor_json::e_acessor(name) {
            use crate::valor_json::intrinsecas as ji;
            let segundo: Option<Type> = match name.as_str() {
                ji::LISTA_OBTER => Some(Type::Bombom(expr_span)),
                ji::OBJETO_TEM | ji::OBJETO_OBTER => Some(Type::Verso(expr_span)),
                _ => None,
            };
            let esperado = 1 + usize::from(segundo.is_some());
            if args.len() != esperado {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado {}, recebido {}",
                        name,
                        esperado,
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(
                arg_ty,
                Type::OpaqueHandle {
                    name: ref handle_name,
                    ..
                } if handle_name == crate::valor_json::TIPO_VALOR_JSON
            ) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada '{}': esperado '{}', encontrado '{}'",
                        name,
                        crate::valor_json::TIPO_VALOR_JSON,
                        arg_ty.display_name()
                    ),
                    span: args[0].span,
                });
            }
            if let Some(esperado_2) = segundo {
                let arg2_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                let compativel = match esperado_2 {
                    Type::Bombom(_) => matches!(arg2_ty, Type::Bombom(_)),
                    Type::Verso(_) => matches!(arg2_ty, Type::Verso(_)),
                    _ => false,
                };
                if !compativel {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada '{}': esperado '{}', encontrado '{}'",
                            name,
                            esperado_2.name(),
                            arg2_ty.display_name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(match name.as_str() {
                ji::EMITIR | ji::VERSO => Type::Verso(expr_span),
                ji::TIPO => Type::Enum {
                    name: crate::valor_json::LEQUE_TIPO_JSON.to_string(),
                    span: expr_span,
                },
                ji::NUMERO => Type::I64(expr_span),
                ji::LOGICA | ji::OBJETO_TEM => Type::Logica(expr_span),
                ji::LISTA_TAMANHO | ji::OBJETO_TAMANHO => Type::Bombom(expr_span),
                ji::LISTA_OBTER | ji::OBJETO_OBTER => Type::OpaqueHandle {
                    name: crate::valor_json::TIPO_VALOR_JSON.to_string(),
                    span: expr_span,
                },
                ji::OBJETO_CHAVES => Type::ListVerso(expr_span),
                _ => unreachable!("acessor JSON sem tipo de retorno"),
            });
        }
        if crate::saida_processo::e_acessor(name) {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de '{}' com aridade inválida: esperado 1, recebido {}",
                        name,
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(
                arg_ty,
                Type::OpaqueHandle {
                    name: ref handle_name,
                    ..
                } if handle_name == crate::saida_processo::TIPO_SAIDA_PROCESSO
            ) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada '{}': esperado 'SaidaProcesso', encontrado '{}'",
                        name,
                        arg_ty.display_name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(match name.as_str() {
                crate::saida_processo::ACESSOR_CODIGO => Type::Bombom(expr_span),
                crate::saida_processo::ACESSOR_SAIDA | crate::saida_processo::ACESSOR_ERRO => {
                    Type::Verso(expr_span)
                }
                _ => unreachable!(),
            });
        }
        // Parte E2: `sha256_verso(verso) -> verso`. Aridade e tipo errados são
        // INVALID_PROGRAM_USE — erro de compilação, nunca `Resultado`.
        if crate::sha256::e_acessor(name) {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'sha256_verso' com aridade inválida: esperado 1, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let arg_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(arg_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'sha256_verso': esperado 'verso', encontrado '{}'",
                        arg_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            return Ok(Type::Verso(expr_span));
        }
        // Fase 140 — buscar_verso(texto, padrao) -> bombom
        // Fase 137 — dividir_verso_em(texto, sep, indice) -> verso
        // Fase 137 — dividir_verso_contar(texto, sep) -> bombom
        // Fase 138 — substituir_verso(texto, de, para) -> verso

        // Fase 139 — juntar_verso_com(a, sep, b) -> verso

        // Fase 157 — formatar_verso(modelo, a[, b, ...]) -> verso
        if name == "formatar_verso" {
            if args.len() < 2 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'formatar_verso' com aridade inválida: esperado pelo menos 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let modelo_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(modelo_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'formatar_verso': esperado 'verso', encontrado '{}'",
                        modelo_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            for (idx, arg) in args.iter().enumerate().skip(1) {
                let arg_ty = self.check_value_expr(
                    arg,
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(arg_ty, Type::Bombom(_) | Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento {} da chamada 'formatar_verso': esperado 'bombom' ou 'verso', encontrado '{}'",
                            idx + 1,
                            arg_ty.name()
                        ),
                        span: arg.span,
                    });
                }
            }
            return Ok(Type::Verso(expr_span));
        }

        if name == "__ternario" {
            if args.len() != 3 {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "expressão ternária requer exatamente 3 argumentos (condição, valor_verdade, valor_falso), recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let cond_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como condição ternária",
            )?;
            if !matches!(cond_ty, Type::Logica(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "condição da expressão ternária deve ser 'logica', encontrado '{}'",
                        cond_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            let then_ty = self.check_value_expr(
                &args[1],
                "resultado de função sem retorno não pode ser usado em expressão ternária",
            )?;
            let else_ty = self.check_value_expr(
                &args[2],
                "resultado de função sem retorno não pode ser usado em expressão ternária",
            )?;
            if then_ty != else_ty {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "ramos da expressão ternária devem ter o mesmo tipo: '{}' vs '{}'",
                        then_ty.name(),
                        else_ty.name()
                    ),
                    span: expr_span,
                });
            }
            return Ok(then_ty.with_span(expr_span));
        }

        // Fase 158 — ler_linha_csv_bombom(linha, sep) -> lista<bombom>

        // Fase 158 — emitir_linha_csv_bombom(itens, sep) -> verso

        // Fase 159 — ler_json_plano_bombom(json) -> mapa<verso,bombom>

        // Fase 159 — emitir_json_plano_bombom(mapa) -> verso

        // Fase 160 — tempo_unix() -> bombom

        // Fase 160 — formatar_tempo_unix(ts) -> verso

        // Fase 168 — executar_processo(comando[, argv1]) -> bombom
        if name == "executar_processo" {
            if !(1..=2).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'executar_processo' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'executar_processo': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let argv1_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'executar_processo': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Bombom(expr_span));
        }

        // Fase 177 — executar_com_entrada(comando, entrada[, argv1]) -> bombom
        if name == "executar_com_entrada" {
            if !(2..=3).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'executar_com_entrada' com aridade inválida: esperado 2 ou 3, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'executar_com_entrada': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            let input_ty = self.check_value_expr(
                &args[1],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(input_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 2 da chamada 'executar_com_entrada': esperado 'verso', encontrado '{}'",
                        input_ty.name()
                    ),
                    span: args[1].span,
                });
            }
            if args.len() == 3 {
                let argv1_ty = self.check_value_expr(
                    &args[2],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 3 da chamada 'executar_com_entrada': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[2].span,
                    });
                }
            }
            return Ok(Type::Bombom(expr_span));
        }

        // Fase 166 — pipeline_minimo(produtor, consumidor) -> bombom

        // Fase 169 — capturar_stdout(comando[, argv1]) -> verso
        if name == "capturar_stdout" {
            if !(1..=2).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'capturar_stdout' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'capturar_stdout': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let argv1_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'capturar_stdout': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Verso(expr_span));
        }

        // Fase 170 — capturar_stderr(comando[, argv1]) -> verso
        if name == "capturar_stderr" {
            if !(1..=2).contains(&args.len()) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "chamada de 'capturar_stderr' com aridade inválida: esperado 1 ou 2, recebido {}",
                        args.len()
                    ),
                    span: expr_span,
                });
            }
            let command_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(command_ty, Type::Verso(_)) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo inválido no argumento 1 da chamada 'capturar_stderr': esperado 'verso', encontrado '{}'",
                        command_ty.name()
                    ),
                    span: args[0].span,
                });
            }
            if args.len() == 2 {
                let argv1_ty = self.check_value_expr(
                    &args[1],
                    "resultado de função sem retorno não pode ser usado como argumento",
                )?;
                if !matches!(argv1_ty, Type::Verso(_)) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "tipo inválido no argumento 2 da chamada 'capturar_stderr': esperado 'verso', encontrado '{}'",
                            argv1_ty.name()
                        ),
                        span: args[1].span,
                    });
                }
            }
            return Ok(Type::Verso(expr_span));
        }

        if name == "__pinker_internal_mapa_verso_verso_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<verso,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(map_ty, Type::MapVersoVerso(_)) {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<verso,verso> exige mapa<verso,verso>"
                        .to_string(),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "__pinker_internal_mapa_verso_verso_iterador_proxima_chave" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<verso,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            return Ok(Type::Verso(expr_span));
        }

        if name == "__pinker_internal_mapa_bombom_bombom_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,bombom> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(map_ty, Type::MapBombomBombom(_)) {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,bombom> exige mapa<bombom,bombom>"
                        .to_string(),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,bombom> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            return Ok(Type::Bombom(expr_span));
        }

        if name == "__pinker_internal_mapa_bombom_verso_iterador_criar" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            let map_ty = self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            if !matches!(map_ty, Type::MapBombomVerso(_)) {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,verso> exige mapa<bombom,verso>"
                        .to_string(),
                    span: args[0].span,
                });
            }
            return Ok(Type::Bombom(expr_span));
        }
        if name == "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave" {
            if args.len() != 1 {
                return Err(PinkerError::Semantic {
                    msg: "iterador interno de mapa<bombom,verso> exige 1 argumento".to_string(),
                    span: expr_span,
                });
            }
            self.check_value_expr(
                &args[0],
                "resultado de função sem retorno não pode ser usado como argumento",
            )?;
            return Ok(Type::Bombom(expr_span));
        }

        let arg_refs: Vec<&Expr> = args.iter().collect();
        self.check_named_function_call(expr_span, callee.span, name, &arg_refs)
    }
    // @pinker-nav:end semantic.chamadas.despacho
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
