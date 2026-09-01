use crate::anonymous_identity;
use crate::ast::*;
use crate::error::PinkerError;
use crate::generic_identity::{self, GenericKind, GenericOrigin};
use crate::lexer::Lexer;
use crate::token::{Span, Token, TokenKind};
use std::collections::{HashMap, HashSet};

/// Tipo de coleção detectado durante o parse de declarações de variáveis e parâmetros.
/// Usado para despachar o construto `para cada` para a desugaring correta.
#[derive(Clone)]
enum CollectionKind {
    ListBombom,
    ListVerso,
    ListEnum(String),
    MapVersoBombom,
    MapVersoVerso,
    MapBombomBombom,
    MapBombomVerso,
    Map { key: Type },
}

impl CollectionKind {
    fn generic_map_callee(&self, name: &str) -> Option<&'static str> {
        match (self, name) {
            (CollectionKind::MapVersoBombom, "mapa_definir") => Some("mapa_verso_bombom_definir"),
            (CollectionKind::MapVersoBombom, "mapa_obter") => Some("mapa_verso_bombom_obter"),
            (CollectionKind::MapVersoBombom, "mapa_tem") => Some("mapa_verso_bombom_tem"),
            (CollectionKind::MapVersoBombom, "mapa_tamanho") => Some("mapa_verso_bombom_tamanho"),
            (CollectionKind::MapVersoBombom, "mapa_remover") => Some("mapa_verso_bombom_remover"),

            (CollectionKind::MapVersoVerso, "mapa_definir") => Some("mapa_verso_verso_definir"),
            (CollectionKind::MapVersoVerso, "mapa_obter") => Some("mapa_verso_verso_obter"),
            (CollectionKind::MapVersoVerso, "mapa_tem") => Some("mapa_verso_verso_tem"),
            (CollectionKind::MapVersoVerso, "mapa_tamanho") => Some("mapa_verso_verso_tamanho"),
            (CollectionKind::MapVersoVerso, "mapa_remover") => Some("mapa_verso_verso_remover"),

            (CollectionKind::MapBombomBombom, "mapa_definir") => Some("mapa_bombom_bombom_definir"),
            (CollectionKind::MapBombomBombom, "mapa_obter") => Some("mapa_bombom_bombom_obter"),
            (CollectionKind::MapBombomBombom, "mapa_tem") => Some("mapa_bombom_bombom_tem"),
            (CollectionKind::MapBombomBombom, "mapa_tamanho") => Some("mapa_bombom_bombom_tamanho"),
            (CollectionKind::MapBombomBombom, "mapa_remover") => Some("mapa_bombom_bombom_remover"),

            (CollectionKind::MapBombomVerso, "mapa_definir") => Some("mapa_bombom_verso_definir"),
            (CollectionKind::MapBombomVerso, "mapa_obter") => Some("mapa_bombom_verso_obter"),
            (CollectionKind::MapBombomVerso, "mapa_tem") => Some("mapa_bombom_verso_tem"),
            (CollectionKind::MapBombomVerso, "mapa_tamanho") => Some("mapa_bombom_verso_tamanho"),
            (CollectionKind::MapBombomVerso, "mapa_remover") => Some("mapa_bombom_verso_remover"),
            _ => None,
        }
    }
}

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    synthetic_counter: usize,
    generic_origin: GenericOrigin,
    /// Parte G: famílias built-in trazidas inteiras (`trazer arquivo;`).
    /// Habilitam a forma qualificada `arquivo.<membro>(...)` neste arquivo, e
    /// só nele — a família não é um nome global e não vaza para outro módulo.
    familias_importadas: HashSet<String>,
    /// Parte G: membros trazidos por import seletivo (`trazer arquivo.<membro>;`),
    /// já resolvidos para a identidade executiva canônica no ponto do import.
    /// O parser guarda a identidade, não o membro: depois deste ponto a grafia
    /// do membro não existe mais em lugar nenhum do pipeline.
    membros_familia_importados: HashMap<String, &'static str>,
    /// Parte G: identidades de **topo** do arquivo, colhidas do fluxo de tokens
    /// em profundidade zero antes de qualquer resolução.
    ///
    /// Só entram aqui as declarações que a Pinker resolve com precedência
    /// independente da ordem textual — `carinho`, `eterno`, `ninho`, `leque`,
    /// `trato`, `apelido` e o símbolo de `trazer modulo.simbolo;`. São as
    /// únicas cujo alcance é o programa inteiro, e por isso as únicas cuja
    /// existência pode desabilitar a família em todo o arquivo.
    ///
    /// `EXISTE_EM_ALGUM_ESCOPO` não é `ESTÁ_VISÍVEL_NESTE_PONTO`: parâmetro,
    /// local, variável de laço, carga de padrão e campo de `ninho` NÃO moram
    /// aqui. Eles têm escopo, e escopo é decidido no ponto de uso por
    /// `escopos_locais`.
    nomes_de_topo: HashSet<String>,
    /// #505: nome cujo INICIALIZADOR está sendo parseado agora.
    ///
    /// `nova f = carinho() { f(...) }` usa o nome no mesmo statement que o
    /// liga, e nesse ponto ele ainda não entrou em `escopos_locais`. Ceder a
    /// esta pilha é o mínimo para não recusar no parser um programa que sempre
    /// pertenceu à semântica — e é MÍNIMO de propósito: um censo do arquivo
    /// inteiro cederia também a ligação de outra função, reabrindo a
    /// superfície global para quem só tem um parâmetro homônimo em algum
    /// canto.
    declarando: Vec<String>,
    /// Parte G: pilha de escopos léxicos reais das ligações locais.
    ///
    /// Empilha e desempilha junto de `value_type_scopes`, e recebe todo nome
    /// que o parser liga de fato: parâmetro (de função, de método e de
    /// `carinho` anônimo), `nova`/`muda`, variável de `para`/`para cada`,
    /// carga de `caso`, ligação de braço de `tentar` e de encaixe de união.
    ///
    /// A pergunta que ela responde é a única correta para um fallback:
    /// «alguma ligação VISÍVEL NESTE PONTO já reivindica este nome?».
    escopos_locais: Vec<HashSet<String>>,
    /// Parte G: o que a autoridade de import já sabe e o parser não pode
    /// descobrir sozinho.
    ///
    /// `NO_PRECANONICALIZATION_BEFORE_IMPORT_KIND_AUTHORITY`. O parser não
    /// sabe — e não pode saber — o que é um módulo no disco nem o que um
    /// `trazer <modulo>;` traz: isso é política do carregador. Ele recebe aqui
    /// o veredito pronto, só para os nomes deste arquivo, e nunca consulta
    /// filesystem. Vazio significa «o chamador não tem autoridade de resolução
    /// de módulo», que é a verdade dos caminhos de biblioteca, REPL, editor e
    /// impressora.
    contexto_de_import: ContextoDeImport,
    /// Mapeamento plano de nomes de variáveis/parâmetros para o tipo de coleção detectado.
    /// Reiniciado no início de cada função para evitar contaminação entre escopos de função.
    collection_types: HashMap<String, CollectionKind>,
    /// Leques declarados até o ponto atual do parse (nome -> variantes com cargas).
    /// Usado pelo desugaring de `encaixe`; exige o leque declarado antes do uso.
    enum_decls: HashMap<String, Vec<(String, Vec<Type>)>>,
    /// Nomes **declarados** de leque (sem os apelidos que apontam para eles).
    /// Separado de `enum_decls` porque a identidade semântica de uma carga é a
    /// do leque de destino, nunca a do apelido que o nomeia.
    enum_names: HashSet<String>,
    /// Alvos de `apelido` vistos até aqui, para que o desugaring de `encaixe`
    /// possa classificar uma carga escrita como apelido (D1) pela mesma
    /// autoridade que a semântica e o lowering usam.
    type_alias_targets: HashMap<String, Type>,
    /// Nomes de `ninho` declarados até aqui, necessários para que a resolução
    /// de apelidos distinga um ninho de um apelido inexistente.
    struct_names: HashSet<String>,
    /// Tratos declarados até o ponto atual do parse.
    /// Usado por `impl`; além da precedência nominal, conserva os corpos
    /// default que serão materializados para o tipo concreto.
    trait_decls: HashMap<String, TraitDecl>,
    /// Funções sintéticas geradas por literais `carinho (...) { ... }` não capturantes.
    pending_functions: Vec<FunctionDecl>,
    /// Templates de funções genéricas de usuário, monomorfizados após o parse.
    generic_templates: HashMap<String, FunctionDecl>,
    generic_instantiations: Vec<GenericInstantiation>,
    /// Templates de leques genéricos, monomorfizados por uso explícito `Nome<T,...>`.
    enum_generic_templates: HashMap<String, EnumDecl>,
    enum_generic_instantiations: Vec<EnumGenericInstantiation>,
    /// Templates de funções que recebem callback estático `carinho(...) -> T`.
    /// A função original não é materializada; cada chamada com callback concreto
    /// gera uma especialização sem parâmetro-função e com chamadas diretas.
    function_param_templates: HashMap<String, FunctionDecl>,
    function_param_instantiations: Vec<FunctionParamInstantiation>,
    /// Aliases locais estáticos de função: `nova f: carinho(...) -> T = carinho(...) -> T { ... };`.
    /// Nesta fase, servem apenas para reescrever `f(...)` como chamada direta.
    function_value_scopes: Vec<HashMap<String, String>>,
    /// Tipos locais já declarados/sintetizáveis, usados exclusivamente como
    /// fonte de evidência para inferência genérica local em chamadas.
    ///
    /// Esta tabela não substitui a checagem semântica: ela só transporta tipos
    /// escritos em parâmetros/`nova` (ou literais localmente evidentes) até o
    /// ponto da chamada. Compatibilidade e coercions continuam no semantic.
    value_type_scopes: Vec<HashMap<String, Type>>,
    /// Nomes de leques genéricos predeclarados pelo parser (Fase 241, ex.: `Resultado`).
    /// Um nome permanece aqui enquanto ainda for o template sintético; qualquer
    /// declaração do usuário com o mesmo nome o remove daqui e suprime/substitui o
    /// template, garantindo que jamais coexistam dois `Resultado` para o mesmo uso.
    predeclared_generic_enums: HashSet<String>,
    /// Leques builtin predeclarados **sem** parâmetros de tipo, disponíveis
    /// para materialização sob demanda (`TipoEntrada` e `LimiteTempo`).
    ///
    /// Separado de `predeclared_generic_enums` porque não há monomorfização
    /// envolvida: o leque já é concreto e só precisa entrar no `Program` quando
    /// alguma superfície que o devolve for realmente chamada. A reserva do nome
    /// não deriva deste recipiente: `runtime_identity` rejeita qualquer declaração
    /// homônima do usuário, independentemente da ordem textual.
    predeclared_plain_enums: HashMap<String, EnumDecl>,
    /// Leques predeclarados simples já materializados por uso, na ordem em que
    /// foram exigidos. Anexados a `Program.items` no fim do parse.
    predeclared_plain_enums_materializados: Vec<EnumDecl>,
    /// Parte B1: declaração do usuário que tomou para si uma identidade que o
    /// runtime produz (`Resultado`), com o span real da declaração.
    ///
    /// Guardado — em vez de decidido no lugar — porque a regra é uma conjunção
    /// sobre o programa inteiro: a outra metade
    /// ([`Parser::identidade_runtime_produzida`]) pode aparecer antes ou depois
    /// no texto. Ver [`Parser::registrar_redeclaracao_de_identidade`].
    identidade_runtime_redeclarada: Option<(String, Span)>,
    /// Parte B1: o programa produz algum valor cuja tag vem da implementação
    /// (chamou alguma superfície falível).
    identidade_runtime_produzida: bool,
    /// Fase 243: nomes sintéticos `__anon_carinho_N` cujo corpo referencia
    /// (pela aproximação sintática conservadora) algum identificador livre
    /// — excluídos dos caminhos rápidos estáticos das Fases 238/239.
    capturing_anon_functions: HashSet<String>,
}

#[derive(Clone)]
struct GenericInstantiation {
    name: String,
    type_args: Vec<Type>,
    span: Span,
}

#[derive(Clone)]
struct EnumGenericInstantiation {
    name: String,
    type_args: Vec<Type>,
    span: Span,
}

#[derive(Clone)]
struct FunctionParamInstantiation {
    name: String,
    bindings: Vec<FunctionParamBinding>,
    span: Span,
}

#[derive(Clone)]
struct FunctionParamBinding {
    index: usize,
    function_name: String,
    span: Span,
}

struct ParsedImplBlock {
    relation: ImplDecl,
    explicit_method_names: HashSet<String>,
    methods: Vec<FunctionDecl>,
}

struct PendingImplRelation {
    relation: ImplDecl,
    explicit_method_names: HashSet<String>,
}

fn merge_span(a: Span, b: Span) -> Span {
    a.merge(b)
}

/// Parte G: o que a autoridade de import entrega pronto ao parser.
///
/// São as duas perguntas cuja resposta o parser não pode produzir — uma pede
/// filesystem, a outra pede o conteúdo de outro arquivo — e cuja resposta
/// precisa chegar **antes** da canonicalização, que é irreversível.
///
/// Nenhuma delas é política: são fatos. A política de módulo continua inteira
/// em `main.rs`, e a de família em `familia_superficie` e `semantic`.
/// #533: uma declaração `trazer` já lida do fluxo de tokens.
///
/// Guarda ÍNDICES, não lexemas: quem lê decide o que extrair, e o leitor não
/// precisa de empréstimo vivo sobre `self.tokens` enquanto o parser segue
/// mutando. `membros` vazio significa import inteiro (`trazer M;`); caso
/// contrário são os membros na ORDEM TEXTUAL, que é a ordem em que a forma
/// separada equivalente teria sido escrita.
#[derive(Debug, Clone)]
pub struct DeclaracaoTrazer {
    pub inicio: usize,
    pub fim: usize,
    pub modulo: usize,
    pub membros: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct ContextoDeImport {
    /// Nomes de `trazer X.y;` que resolvem para um módulo Pinker real ao lado
    /// da fonte. `REAL_MODULE_X > BUILTIN_FAMILY_X` se decide por esta lista.
    pub modulos_reais: HashSet<String>,
    /// Identidades de topo que os `trazer <modulo>;` deste arquivo trazem.
    /// Entram no censo de topo e vencem a família como qualquer outro item.
    pub nomes_importados: HashSet<String>,
    /// #517: tratos que os imports explícitos desta unidade autorizam como
    /// alvo de `impl`, indexados pela grafia que o importador enxerga.
    ///
    /// `IMPORTED_TRAIT_VISIBLE_FOR_REFERENCE` e
    /// `IMPORTED_TRAIT_VALID_IMPL_TARGET` passam a ser a MESMA autoridade: a de
    /// import. O parser recebe o veredito pronto, como já recebe
    /// `modulos_reais` e `nomes_importados`; ele continua sem política de
    /// módulo e sem procurar arquivo. A identidade canônica NÃO é decidida
    /// aqui — a grafia registrada é a que o importador escreveu, e quem a
    /// canoniza é `module_resolve`, pelo ambiente que a unidade autorizou.
    pub tratos_importados: HashMap<String, TraitDecl>,
    /// #517: algum módulo importado por esta unidade não pôde ser lido pelo
    /// prepass — ausente, ilegível, com erro de sintaxe ou em ciclo.
    ///
    /// A superfície acima está INCOMPLETA, e o parser não pode transformar a
    /// própria cegueira em recusa: quem não enxergou não pode dizer "não
    /// existe". O carregador refaz a mesma leitura logo em seguida e produz o
    /// erro real — módulo ausente, falha ao ler o módulo, ciclo — com o span e
    /// a fonte certos. Sem esta flag, um erro de sintaxe dentro do módulo
    /// chegava ao usuário disfarçado de "você esqueceu o import", apontando a
    /// linha do `impl` na raiz.
    pub import_incompleto: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self::with_generic_origin(tokens, GenericOrigin::Root)
    }

    pub fn with_generic_origin(tokens: Vec<Token>, generic_origin: GenericOrigin) -> Self {
        Self::com_contexto_de_import(tokens, generic_origin, ContextoDeImport::default())
    }

    /// Parte G — `REAL_MODULE_CLASSIFICATION MUST_PRECEDE FAMILY_SELECTIVE_CANONICALIZATION`.
    ///
    /// Constrói o parser já sabendo o que só a autoridade de import sabe: quais
    /// nomes de `trazer X.y;` são módulo Pinker real, e que identidades de topo
    /// um `trazer <modulo>;` traz para este arquivo. Sem essas respostas o
    /// parser canonicalizaria **antes** de o carregador descobrir os fatos, e a
    /// identidade original desapareceria da AST sem diagnóstico.
    ///
    /// O parser continua sem qualquer política de módulo: recebe o veredito,
    /// não o calcula. Quem calcula é `main.rs`, dono da resolução de módulos,
    /// pelas mesmas consultas que `load_program_with_imports` faria em seguida.
    pub fn com_contexto_de_import(
        tokens: Vec<Token>,
        generic_origin: GenericOrigin,
        contexto_de_import: ContextoDeImport,
    ) -> Self {
        Self {
            tokens,
            current: 0,
            synthetic_counter: 0,
            generic_origin,
            familias_importadas: HashSet::new(),
            membros_familia_importados: HashMap::new(),
            nomes_de_topo: HashSet::new(),
            declarando: Vec::new(),
            escopos_locais: Vec::new(),
            contexto_de_import,
            collection_types: HashMap::new(),
            enum_decls: HashMap::new(),
            enum_names: HashSet::new(),
            type_alias_targets: HashMap::new(),
            struct_names: HashSet::new(),
            trait_decls: HashMap::new(),
            pending_functions: Vec::new(),
            generic_templates: HashMap::new(),
            generic_instantiations: Vec::new(),
            enum_generic_templates: HashMap::new(),
            enum_generic_instantiations: Vec::new(),
            function_param_templates: HashMap::new(),
            function_param_instantiations: Vec::new(),
            function_value_scopes: Vec::new(),
            value_type_scopes: Vec::new(),
            predeclared_generic_enums: HashSet::new(),
            predeclared_plain_enums: HashMap::new(),
            predeclared_plain_enums_materializados: Vec::new(),
            identidade_runtime_redeclarada: None,
            identidade_runtime_produzida: false,
            capturing_anon_functions: HashSet::new(),
        }
    }

    fn impl_type_key(ty: &Type) -> String {
        match ty {
            Type::Alias { name, .. }
            | Type::Struct { name, .. }
            | Type::OpaqueHandle { name, .. }
            | Type::Enum { name, .. } => name.clone(),
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
            | Type::Nulo(_) => ty.name().to_string(),
            // Tipos estruturais/monomorfizados precisam apenas de transporte
            // injetivo neste estágio. A identidade semântica continua sendo
            // decidida depois da resolução integral, nunca por este texto.
            _ => generic_identity::render_monomorphization_type_identity(ty),
        }
    }

    fn impl_function_name(trait_name: &str, target_ty: &Type, method_name: &str) -> String {
        let target_key = Self::impl_type_key(target_ty);
        crate::method_identity::render_provisional_function_name(
            trait_name,
            &target_key,
            method_name,
        )
    }

    fn trait_default_check_function_name(
        trait_name: &str,
        target_ty: &Type,
        method_name: &str,
    ) -> String {
        let target_key = Self::impl_type_key(target_ty);
        crate::method_identity::render_trait_default_check_function_name(
            trait_name,
            &target_key,
            method_name,
        )
    }

    fn collect_trait_default_closure_names(
        function: &FunctionDecl,
        templates: &HashMap<String, FunctionDecl>,
        ordered: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) -> Result<(), PinkerError> {
        for candidate in crate::ast::capture_candidates_in_function(function) {
            if !candidate.starts_with("__anon_carinho_") || !seen.insert(candidate.clone()) {
                continue;
            }
            let Some(template) = templates.get(&candidate) else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "closure sintética '{}' do default não foi encontrada",
                        candidate
                    ),
                    span: function.span,
                });
            };
            ordered.push(candidate);
            Self::collect_trait_default_closure_names(template, templates, ordered, seen)?;
        }
        Ok(())
    }

    fn clone_trait_default_closures(
        &mut self,
        function: &mut FunctionDecl,
        templates: &HashMap<String, FunctionDecl>,
        consumed_templates: &mut HashSet<String>,
        cloned_functions: &mut Vec<FunctionDecl>,
    ) -> Result<(), PinkerError> {
        let mut ordered = Vec::new();
        Self::collect_trait_default_closure_names(
            function,
            templates,
            &mut ordered,
            &mut HashSet::new(),
        )?;
        if ordered.is_empty() {
            return Ok(());
        }

        let mut replacements = HashMap::new();
        for old_name in &ordered {
            self.synthetic_counter += 1;
            replacements.insert(
                old_name.clone(),
                anonymous_identity::anonymous_callable_name(
                    &self.generic_origin,
                    self.synthetic_counter,
                ),
            );
        }

        function.body = Self::substitute_function_param_block(&function.body, &replacements);
        for old_name in ordered {
            let mut cloned = templates
                .get(&old_name)
                .expect("closure template was validated while collecting")
                .clone();
            let new_name = replacements
                .get(&old_name)
                .expect("every collected closure receives a fresh name")
                .clone();
            cloned.name.clone_from(&new_name);
            cloned.body = Self::substitute_function_param_block(&cloned.body, &replacements);
            if self.capturing_anon_functions.contains(&old_name) {
                self.capturing_anon_functions.insert(new_name);
            }
            consumed_templates.insert(old_name);
            cloned_functions.push(cloned);
        }
        Ok(())
    }

    // @pinker-nav:start parser.fluxo.nucleo
    // @pinker-nav:domain fluxo
    // @pinker-nav:layer parser
    // @pinker-nav:summary Núcleo do parser: cursor sobre a lista de tokens (peek/advance/previous/check/consume) e antecipação com offset, mais o erro `Expected` de sincronização básica; utilitários fundamentais que todas as regras gramaticais consomem.
    fn peek(&self) -> Option<&Token> {
        self.tokens
            .get(self.current)
            .filter(|token| token.kind != TokenKind::Eof)
    }

    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.current)
            .map(|token| token.span)
            .or_else(|| self.tokens.last().map(|token| token.span))
            .unwrap_or_else(|| Span::single(crate::token::Position::new(1, 1)))
    }

    fn advance(&mut self) -> Option<&Token> {
        if self.current >= self.tokens.len() {
            return None;
        }

        let token = &self.tokens[self.current];
        self.current += 1;
        if token.kind == TokenKind::Eof {
            None
        } else {
            Some(token)
        }
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek().map(|token| token.kind == kind).unwrap_or(false)
    }

    fn check_at(&self, offset: usize, kind: TokenKind) -> bool {
        self.tokens
            .get(self.current + offset)
            .map(|token| token.kind == kind)
            .unwrap_or(false)
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: TokenKind, expected: &str) -> Result<&Token, PinkerError> {
        if self.check(kind) {
            Ok(self.advance().unwrap())
        } else {
            let found = self
                .peek()
                .map(|token| token.lexeme.clone())
                .unwrap_or_default();
            Err(PinkerError::Expected {
                expected: expected.to_string(),
                found,
                span: self.peek_span(),
            })
        }
    }

    /// Fecha uma lista de argumentos de tipo aceitando `>` ou a primeira
    /// metade de um `>>`.
    ///
    /// O lexer produz `>>` como um único token de deslocamento, e nada abaixo
    /// dele distingue os dois usos. Sem esta divisão,
    /// `Opcao<lista<bombom>>` seria erro sintático; a alternativa — proibir o
    /// aninhamento — obrigaria a inventar uma segunda sintaxe de tipos de lista
    /// só para cargas, exatamente o que o contrato proíbe. O token restante é
    /// reescrito como um `>` com o span da segunda coluna, para que os
    /// diagnósticos seguintes continuem apontando a posição real.
    fn consume_type_arg_close(&mut self) -> Result<(), PinkerError> {
        if self.check(TokenKind::Greater) {
            self.advance();
            return Ok(());
        }
        if self.check(TokenKind::GreaterGreater) {
            let span = self.tokens[self.current].span;
            let middle = crate::token::Position::new(span.start.line, span.start.col + 1);
            self.tokens[self.current] = Token::new(
                TokenKind::Greater,
                ">".to_string(),
                // Os dois `>` sintetizados herdam a fonte do `>>` original:
                // dividir um token não muda de que arquivo ele veio.
                Span::em(span.source, middle, span.end),
            );
            // O `previous()` dos chamadores precisa enxergar um `>` fechado: o
            // token anterior passa a ser o primeiro `>` sintetizado.
            self.tokens.insert(
                self.current,
                Token::new(
                    TokenKind::Greater,
                    ">".to_string(),
                    Span::em(span.source, span.start, middle),
                ),
            );
            self.advance();
            return Ok(());
        }
        let found = self
            .peek()
            .map(|token| token.lexeme.clone())
            .unwrap_or_default();
        Err(PinkerError::Expected {
            expected: ">".to_string(),
            found,
            span: self.peek_span(),
        })
    }

    // @pinker-nav:end parser.fluxo.nucleo

    // @pinker-nav:start parser.programa.estrutura
    // @pinker-nav:domain programa
    // @pinker-nav:layer parser
    // @pinker-nav:summary Ponto de entrada do parser: constrói o `Program` reconhecendo `pacote`, `trazer` (imports) e a marca freestanding, e despacha os itens de topo via `parse_item`. Desde a #533 a declaração `trazer` aceita vários membros de UM módulo (`trazer M.a, b, c;`) e desaçucara ali mesmo para as unidades `Import` de sempre, na ordem textual, sem que nada a jusante conheça multi-import; a leitura da declaração é `ler_declaracao_trazer`, a mesma que os prepasses de import usam, e o caminho de cursor só produz o diagnóstico das formas recusadas (`M.a, b,;`, `M.a, N.b;`, `M, a;`). Dois prepasses estreitos de autoridade precedem qualquer resolução, ambos sobre tokens e sem resolver tipos: um detecta templates genéricos de usuário antes de registrar o `Resultado<T,E>` builtin, tornando USER_WINS independente da ordem textual; outro colhe as identidades de topo do arquivo (`coletar_nomes_de_topo`), que é o que dá à superfície por família a mesma independência de ordem. A redeclaração continua formando com a produção runtime a conjunção inválida da Parte B1.
    pub fn parse(&mut self) -> Result<Program, PinkerError> {
        // Parte G: o censo de identidades de topo precede toda resolução. A
        // família é fallback, e fallback precisa saber o que já está
        // reivindicado antes de responder pela primeira vez. As ligações com
        // escopo não entram aqui: elas são decididas no ponto de uso, pela
        // pilha `escopos_locais`.
        self.coletar_nomes_de_topo();

        let package = if self.match_token(TokenKind::KwPacote) {
            let start_span = self.previous().span;
            let name = self
                .consume(TokenKind::Ident, "nome do pacote")?
                .lexeme
                .clone();
            self.consume(TokenKind::Semi, ";")?;
            Some(PackageDecl {
                name,
                span: merge_span(start_span, self.previous().span),
            })
        } else {
            None
        };

        let mut freestanding = None;
        if self.match_token(TokenKind::KwLivre) {
            let marker_span = self.previous().span;
            self.consume(TokenKind::Semi, ";")?;
            freestanding = Some(merge_span(marker_span, self.previous().span));
        }

        let mut imports = Vec::new();
        while self.check(TokenKind::KwTrazer) {
            // #533: a lista de membros é lida por `ler_declaracao_trazer`, a
            // MESMA autoridade sintática que os prepasses de import consultam.
            // O caminho de cursor abaixo existe só para produzir o diagnóstico
            // exato quando o leitor recusa a forma; ele nunca aceita nada que o
            // leitor tenha recusado, e é isso que impede duas gramáticas.
            let Some(declaracao) = Self::ler_declaracao_trazer(&self.tokens, self.current) else {
                self.diagnosticar_declaracao_trazer()?;
                unreachable!("diagnosticar_declaracao_trazer sempre falha quando o leitor recusa");
            };
            let start_span = self.tokens[declaracao.inicio].span;
            let end_span = self.tokens[declaracao.fim].span;
            let module = self.tokens[declaracao.modulo].lexeme.clone();
            let membros: Vec<String> = declaracao
                .membros
                .iter()
                .map(|indice| self.tokens[*indice].lexeme.clone())
                .collect();
            self.current = declaracao.fim + 1;

            // #533: `trazer M.a, b, c;` DESAÇUCARA aqui, na fronteira
            // sintática, para as mesmas unidades `Import` que a forma separada
            // sempre produziu — na ordem textual. Nada a jusante conhece
            // "multi-import": `DOWNSTREAM_MULTI_IMPORT_CONCEPT = 0`.
            let span = merge_span(start_span, end_span);
            if membros.is_empty() {
                self.registrar_import_de_familia(&module, None);
                imports.push(ImportDecl {
                    module,
                    symbol: None,
                    span,
                });
            } else {
                for membro in membros {
                    self.registrar_import_de_familia(&module, Some(membro.as_str()));
                    imports.push(ImportDecl {
                        module: module.clone(),
                        symbol: Some(membro),
                        span,
                    });
                }
            }
        }

        // Fase 241: predeclara o leque padrão `Resultado<T, E> { Ok(T), Erro(E) }`
        // como template sintético antes de parsear os itens de topo, para que
        // `apelido X = Resultado<A, B>;` funcione sem declaração manual. O
        // registrador faz antes um prepass estreito de autoridade: se o arquivo
        // contém um template genérico de usuário homônimo, o builtin nem entra,
        // independentemente da ordem entre declaração e uso.
        let user_resultado_span =
            self.source_top_level_generic_enum_span(crate::falha_operacional::LEQUE_RESULTADO);
        self.register_predeclared_generic_enums();
        if let Some(span) = user_resultado_span {
            // A política USER_WINS e a defesa runtime são fatos sobre o
            // programa inteiro. Registrar já o span conhecido pelo prepass faz
            // a conjunção da Parte B1 independer de a produção runtime aparecer
            // antes ou depois da declaração, sem preparsear tipos ou o template.
            self.registrar_redeclaracao_de_identidade(
                crate::falha_operacional::LEQUE_RESULTADO,
                span,
            )?;
        }
        self.register_predeclared_plain_enums();

        let mut items = Vec::new();
        let mut impl_relations: Vec<PendingImplRelation> = Vec::new();
        while self.peek().is_some() {
            if self.match_token(TokenKind::KwImpl) {
                let parsed = self.parse_impl_block()?;
                if let Some(existing) = impl_relations.iter_mut().find(|pending| {
                    pending.relation.trait_name == parsed.relation.trait_name
                        && Self::impl_type_key(&pending.relation.target_ty)
                            == Self::impl_type_key(&parsed.relation.target_ty)
                }) {
                    existing
                        .explicit_method_names
                        .extend(parsed.explicit_method_names.iter().cloned());
                    existing.relation.span = existing.relation.span.merge(parsed.relation.span);
                } else {
                    impl_relations.push(PendingImplRelation {
                        relation: parsed.relation.clone(),
                        explicit_method_names: parsed.explicit_method_names.clone(),
                    });
                }
                for function in parsed.methods {
                    if !function.type_params.is_empty() {
                        return Err(PinkerError::Parse {
                            msg: "método de impl não pode declarar parâmetros genéricos nesta fase"
                                .to_string(),
                            span: function.span,
                        });
                    }
                    items.push(Item::Function(function));
                }
            } else {
                let item = self.parse_item()?;
                // Fase 241: qualquer declaração do usuário chamada como um leque
                // genérico predeclarado (ex.: `Resultado`) suprime o template
                // sintético — o usuário vence. O caso de um leque genérico do
                // usuário de mesmo nome é tratado adiante (substituição sem erro
                // de duplicata); aqui cobrimos as demais formas (leque não-genérico,
                // `ninho`, `apelido`, `carinho`, `eterno`), removendo o predeclarado.
                if let Some(item_name) = Self::item_name(&item).map(str::to_string) {
                    let is_generic_enum =
                        matches!(&item, Item::Enum(enum_decl) if !enum_decl.type_params.is_empty());
                    if !is_generic_enum && self.predeclared_generic_enums.remove(&item_name) {
                        self.enum_generic_templates.remove(&item_name);
                        // Parte B1: a supressão é legítima enquanto o programa não
                        // produzir valores dessa identidade pelo runtime.
                        self.registrar_redeclaracao_de_identidade(&item_name, item.span())?;
                    }
                    // Identidade semântica consumida/produzida pelo runtime é
                    // **reservada**, não substituível.
                    //
                    // A regra "o usuário vence" da Fase 241 vale para uma
                    // predeclaração de biblioteca, que o programa pode
                    // legitimamente trocar. Ela não vale para as autoridades
                    // registradas em `runtime_identity`: `TipoEntrada` e
                    // `LimiteTempo` têm tags interpretadas pelo runtime;
                    // `SaidaProcesso` é um handle builtin nominal. Compartilhar
                    // discriminante ou palavra de máquina não autoriza uma
                    // declaração arbitrária a assumir a mesma identidade.
                    //
                    // A lista canônica dessas identidades não aparece aqui de
                    // propósito: ampliar a autoridade ocorre em
                    // `runtime_identity`, não neste recipiente acidental de
                    // leques predeclarados.
                    //
                    // ```text
                    // BUILTIN_RUNTIME_SEMANTICS
                    // MUST_NOT_BE_REINTERPRETED_BY
                    // ARBITRARY_USER_TYPE_SHADOWING
                    // ```
                    //
                    // A recusa é aqui, na aceitação do item, e não no ponto de
                    // uso: assim vale para qualquer categoria de item, e o
                    // resultado não depende de a declaração vir antes ou depois
                    // do uso. O span é o da declaração do usuário — a rejeição
                    // pelo caminho antigo apontava para o span sintético 0:0 do
                    // predeclarado, que não existe em fonte alguma.
                    if let Some(identity) =
                        crate::runtime_identity::runtime_reserved_identity(&item_name)
                    {
                        return Err(PinkerError::Parse {
                            msg: crate::runtime_identity::conflict_message(identity),
                            span: item.span(),
                        });
                    }
                }
                if let Item::Function(function) = &item {
                    if !function.type_params.is_empty() {
                        if self.generic_templates.contains_key(&function.name) {
                            return Err(PinkerError::Parse {
                                msg: format!("função genérica '{}' já declarada", function.name),
                                span: function.span,
                            });
                        }
                        self.generic_templates
                            .insert(function.name.clone(), function.clone());
                        continue;
                    }
                    if Self::has_function_param(function) {
                        if self.function_param_templates.contains_key(&function.name) {
                            return Err(PinkerError::Parse {
                                msg: format!(
                                    "função com parâmetro função '{}' já declarada",
                                    function.name
                                ),
                                span: function.span,
                            });
                        }
                        self.function_param_templates
                            .insert(function.name.clone(), function.clone());
                        // Fase 242: ao contrário da Fase 239 isolada, a função
                        // também é materializada normalmente (sem `continue`)
                        // — permite chamada indireta geral quando o call site
                        // não resolve um callback estático (parser.rs:4708+
                        // continua cobrindo o caminho estático como otimização).
                    }
                }
                if let Item::Enum(enum_decl) = &item {
                    if !enum_decl.type_params.is_empty() {
                        // Fase 241: se o nome ainda é o template predeclarado, a
                        // declaração genérica do usuário o substitui sem erro de
                        // duplicata. Uma segunda declaração do usuário (nome não
                        // mais predeclarado) continua sendo duplicata inválida.
                        let was_predeclared =
                            self.predeclared_generic_enums.remove(&enum_decl.name);
                        if crate::falha_operacional::identidade_produzida_pelo_runtime(
                            &enum_decl.name,
                        ) {
                            // Parte B1: um template de usuário que toma a
                            // identidade runtime é legítimo enquanto o programa
                            // não produzir valores dessa identidade. O registro
                            // independe de `was_predeclared`: o prepass estreito
                            // pode ter evitado a inserção do builtin.
                            self.registrar_redeclaracao_de_identidade(
                                &enum_decl.name,
                                enum_decl.span,
                            )?;
                        }
                        if self.enum_generic_templates.contains_key(&enum_decl.name)
                            && !was_predeclared
                        {
                            return Err(PinkerError::Parse {
                                msg: format!("leque genérico '{}' já declarado", enum_decl.name),
                                span: enum_decl.span,
                            });
                        }
                        self.enum_generic_templates
                            .insert(enum_decl.name.clone(), enum_decl.clone());
                        continue;
                    }
                }
                items.push(item);
            }
        }
        items.extend(
            self.materialize_trait_defaults(&impl_relations)?
                .into_iter()
                .map(Item::Function),
        );
        // Os leques builtin predeclarados simples entram antes das
        // especializações genéricas — uma especialização pode carregar um deles
        // como argumento de tipo, e a IR classifica cargas só depois de coletar
        // todos os nomes de leque, mas manter a ordem de dependência evita
        // depender desse detalhe.
        items.extend(
            self.predeclared_plain_enums_materializados
                .drain(..)
                .map(Item::Enum),
        );
        let generic_enums = self.instantiate_generic_enums()?;
        items.extend(generic_enums.into_iter().map(Item::Enum));
        let generic_functions = self.instantiate_generic_functions()?;
        items.extend(generic_functions.into_iter().map(Item::Function));
        let function_param_functions = self.instantiate_function_param_functions(&items)?;
        items.extend(function_param_functions.into_iter().map(Item::Function));
        items.extend(self.pending_functions.drain(..).map(Item::Function));

        Ok(Program {
            package,
            freestanding,
            imports,
            impls: impl_relations
                .into_iter()
                .map(|pending| pending.relation)
                .collect(),
            items,
        })
    }

    fn parse_item(&mut self) -> Result<Item, PinkerError> {
        if self.match_token(TokenKind::KwCarinho) {
            Ok(Item::Function(self.parse_function()?))
        } else if self.match_token(TokenKind::KwEterno) {
            Ok(Item::Const(self.parse_const()?))
        } else if self.match_token(TokenKind::KwApelido) {
            Ok(Item::TypeAlias(self.parse_type_alias()?))
        } else if self.match_token(TokenKind::KwNinho) {
            Ok(Item::Struct(self.parse_struct_decl()?))
        } else if self.match_token(TokenKind::KwLeque) {
            Ok(Item::Enum(self.parse_enum_decl()?))
        } else if self.match_token(TokenKind::KwTrato) {
            Ok(Item::Trait(self.parse_trait_decl()?))
        } else if self.match_token(TokenKind::KwLivre) {
            Err(PinkerError::Expected {
                expected: "marcador `livre;` apenas uma vez no topo do programa (após `pacote`, antes dos itens)".to_string(),
                found: "livre".to_string(),
                span: self.previous().span,
            })
        } else if self.match_token(TokenKind::KwTrazer) {
            Err(PinkerError::Expected {
                expected: "declaração `trazer` apenas no topo do programa (após `pacote`/`livre`, antes dos itens)".to_string(),
                found: "trazer".to_string(),
                span: self.previous().span,
            })
        } else {
            Err(PinkerError::Expected {
                expected: "carinho, eterno, apelido, ninho, leque, trato ou impl".to_string(),
                found: self
                    .peek()
                    .map(|token| token.lexeme.clone())
                    .unwrap_or_default(),
                span: self.peek_span(),
            })
        }
    }

    // @pinker-nav:end parser.programa.estrutura

    // @pinker-nav:start parser.tipos.gramatica
    // @pinker-nav:domain tipos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Gramática de tipos: reconhece tipos primitivos, nomeados, listas/mapas, ponteiros e arrays, função e aplicações genéricas na sintaxe, produzindo nós `ast::Type` sem resolver aliases nem significado (isso é da semântica).
    fn parse_type(&mut self) -> Result<Type, PinkerError> {
        let span = self.peek_span();
        if self.match_token(TokenKind::KwFragil) {
            let qualifier_span = self.previous().span;
            let ty = self.parse_type()?;
            return match ty {
                Type::Pointer {
                    base,
                    is_volatile: false,
                    span: pointer_span,
                } => Ok(Type::Pointer {
                    base,
                    is_volatile: true,
                    span: merge_span(qualifier_span, pointer_span),
                }),
                Type::Pointer {
                    is_volatile: true,
                    span: pointer_span,
                    ..
                } => Err(PinkerError::Expected {
                    expected: "tipo seta sem qualificador repetido".to_string(),
                    found: "fragil".to_string(),
                    span: merge_span(qualifier_span, pointer_span),
                }),
                _ => Err(PinkerError::Expected {
                    expected: "'fragil' só pode qualificar tipo seta (ex.: fragil seta<u8>)"
                        .to_string(),
                    found: ty.name().to_string(),
                    span: ty.span(),
                }),
            };
        }
        if self.match_token(TokenKind::LBracket) {
            let start_span = self.previous().span;
            let element = self.parse_type()?;
            self.consume(TokenKind::Semi, ";")?;
            let size_token = self.consume(TokenKind::IntLit, "tamanho inteiro do array fixo")?;
            let size = size_token
                .lexeme
                .parse::<u64>()
                .map_err(|_| PinkerError::Expected {
                    expected: "tamanho inteiro válido do array fixo".to_string(),
                    found: size_token.lexeme.clone(),
                    span: size_token.span,
                })?;
            self.consume(TokenKind::RBracket, "]")?;
            return Ok(Type::FixedArray {
                element: Box::new(element),
                size,
                span: merge_span(start_span, self.previous().span),
            });
        }
        if self.match_token(TokenKind::KwSeta) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Less, "<")?;
            let base = if self.match_token(TokenKind::KwCarinho) {
                self.parse_function_type_after_keyword(self.previous().span, true)?
            } else {
                self.parse_type()?
            };
            self.consume_type_arg_close()?;
            return Ok(Type::Pointer {
                base: Box::new(base),
                is_volatile: false,
                span: merge_span(start_span, self.previous().span),
            });
        }
        if self.match_token(TokenKind::KwCarinho) {
            let start_span = self.previous().span;
            return self.parse_function_type_after_keyword(start_span, false);
        }

        // Fase 244: tipo explícito de objeto de trato.
        //
        // Internamente, a AST reutiliza `Type::Applied` para preservar o
        // nome nominal do trato sem introduzir representação física nesta
        // camada. Semântica, IR e backend definem o significado nas etapas
        // posteriores da fase.
        if self.match_token(TokenKind::KwTrato) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Less, "< em tipo de objeto de trato")?;
            let trait_token = self.consume(TokenKind::Ident, "nome do trato em tipo de objeto")?;
            let trait_name = trait_token.lexeme.clone();
            let trait_span = trait_token.span;
            self.consume(TokenKind::Greater, "> em tipo de objeto de trato")?;

            return Ok(Type::Applied {
                name: "trato".to_string(),
                args: vec![Type::Alias {
                    name: trait_name,
                    span: trait_span,
                }],
                span: merge_span(start_span, self.previous().span),
            });
        }

        if self.match_token(TokenKind::KwBombom) {
            Ok(Type::Bombom(span))
        } else if self.match_token(TokenKind::KwU8) {
            Ok(Type::U8(span))
        } else if self.match_token(TokenKind::KwU16) {
            Ok(Type::U16(span))
        } else if self.match_token(TokenKind::KwU32) {
            Ok(Type::U32(span))
        } else if self.match_token(TokenKind::KwU64) {
            Ok(Type::U64(span))
        } else if self.match_token(TokenKind::KwI8) {
            Ok(Type::I8(span))
        } else if self.match_token(TokenKind::KwI16) {
            Ok(Type::I16(span))
        } else if self.match_token(TokenKind::KwI32) {
            Ok(Type::I32(span))
        } else if self.match_token(TokenKind::KwI64) {
            Ok(Type::I64(span))
        } else if self.match_token(TokenKind::KwLogica) {
            Ok(Type::Logica(span))
        } else if self.match_token(TokenKind::KwVerso) {
            Ok(Type::Verso(span))
        } else if self.match_token(TokenKind::Ident) {
            if self.previous().lexeme == "uniao" && self.match_token(TokenKind::Less) {
                let mut members = Vec::new();
                loop {
                    members.push(self.parse_type()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
                self.consume_type_arg_close()?;
                if members.len() < 2 {
                    return Err(PinkerError::Expected {
                        expected: "ao menos dois membros em uniao<T1, T2, ...>".to_string(),
                        found: format!("{} membro(s)", members.len()),
                        span: merge_span(span, self.previous().span),
                    });
                }
                return Ok(Type::Union {
                    members,
                    span: merge_span(span, self.previous().span),
                });
            }
            if self.previous().lexeme == "lista" && self.match_token(TokenKind::Less) {
                let inner = self.parse_type()?;
                self.consume_type_arg_close()?;
                let outer_span = merge_span(span, self.previous().span);
                if matches!(inner, Type::Bombom(_)) {
                    return Ok(Type::ListBombom(outer_span));
                }
                if matches!(inner, Type::Verso(_)) {
                    return Ok(Type::ListVerso(outer_span));
                }
                // `lista<NomeDeLeque>` — a existência do leque é validada na semântica.
                if let Type::Alias { name, .. } = &inner {
                    return Ok(Type::ListEnum {
                        element: name.clone(),
                        span: outer_span,
                    });
                }
                return Err(PinkerError::Expected {
                    expected: "tipo 'lista<bombom>', 'lista<verso>' ou 'lista<Leque>' nesta fase"
                        .to_string(),
                    found: format!("lista<{}>", inner.name()),
                    span: inner.span(),
                });
            }
            if self.previous().lexeme == "mapa" && self.match_token(TokenKind::Less) {
                let key_ty = self.parse_type()?;
                self.consume(TokenKind::Comma, ",")?;
                let value_ty = self.parse_type()?;
                self.consume_type_arg_close()?;
                let outer_span = merge_span(span, self.previous().span);
                if matches!(key_ty, Type::Verso(_)) && matches!(value_ty, Type::Bombom(_)) {
                    return Ok(Type::MapVersoBombom(outer_span));
                }
                if matches!(key_ty, Type::Verso(_)) && matches!(value_ty, Type::Verso(_)) {
                    return Ok(Type::MapVersoVerso(outer_span));
                }
                if matches!(key_ty, Type::Bombom(_)) && matches!(value_ty, Type::Bombom(_)) {
                    return Ok(Type::MapBombomBombom(outer_span));
                }
                if matches!(key_ty, Type::Bombom(_)) && matches!(value_ty, Type::Verso(_)) {
                    return Ok(Type::MapBombomVerso(outer_span));
                }
                return Ok(Type::Map {
                    key: Box::new(key_ty),
                    value: Box::new(value_ty),
                    span: outer_span,
                });
            }
            let mut name = self.previous().lexeme.clone();
            let mut type_span = self.previous().span;
            if self.match_token(TokenKind::Less) {
                let mut args = Vec::new();
                loop {
                    args.push(self.parse_type()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
                self.consume_type_arg_close()?;
                let applied_span = merge_span(type_span, self.previous().span);
                self.enum_generic_instantiations
                    .push(EnumGenericInstantiation {
                        name: name.clone(),
                        type_args: args.clone(),
                        span: applied_span,
                    });
                if let Some(template) = self.enum_generic_templates.get(&name).cloned() {
                    let concrete =
                        self.instantiate_generic_enum_decl(&template, &args, applied_span)?;
                    self.enum_names.insert(concrete.name.clone());
                    self.enum_decls.insert(
                        concrete.name,
                        concrete
                            .variants
                            .into_iter()
                            .map(|variant| (variant.name, variant.payloads))
                            .collect(),
                    );
                }
                return Ok(Type::Enum {
                    name: self.generic_enum_name(&name, &args),
                    span: applied_span,
                });
            }
            if self.match_token(TokenKind::Dot) {
                let separator_span = self.previous().span;
                let qualified = self
                    .consume(
                        TokenKind::Ident,
                        "nome do tipo após '.' em tipo qualificado",
                    )?
                    .lexeme
                    .clone();
                name = format!("{}.{}", name, qualified);
                type_span = merge_span(type_span, separator_span);
                type_span = merge_span(type_span, self.previous().span);
            }
            if matches!(
                crate::runtime_identity::runtime_reserved_identity(&name),
                Some(crate::runtime_identity::RuntimeReservedIdentity {
                    kind: crate::runtime_identity::RuntimeSemanticKind::OpaqueWordHandle,
                    ..
                })
            ) {
                return Ok(Type::OpaqueHandle {
                    name,
                    span: type_span,
                });
            }
            if self.predeclared_plain_enums.contains_key(&name) {
                self.registrar_leque_predeclarado(&name);
            }
            Ok(Type::Alias {
                name,
                span: type_span,
            })
        } else {
            Err(PinkerError::Expected {
                expected:
                    "tipo válido (ex.: bombom, logica, verso, alias, [tipo; N], seta<tipo> ou fragil seta<tipo>)"
                        .to_string(),
                found: self
                    .peek()
                    .map(|token| token.lexeme.clone())
                    .unwrap_or_default(),
                span,
            })
        }
    }

    fn parse_function_type_after_keyword(
        &mut self,
        start_span: Span,
        allow_implicit_nulo: bool,
    ) -> Result<Type, PinkerError> {
        self.consume(TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                params.push(self.parse_type()?);
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, ")")?;
        let ret = if self.match_token(TokenKind::Arrow) {
            self.parse_type()?
        } else if allow_implicit_nulo {
            Type::Nulo(self.previous().span)
        } else {
            return Err(PinkerError::Expected {
                expected: "-> em tipo função".to_string(),
                found: self
                    .peek()
                    .map(|token| token.lexeme.clone())
                    .unwrap_or_else(|| "fim do arquivo".to_string()),
                span: self.peek_span(),
            });
        };
        Ok(Type::Function {
            params,
            ret: Box::new(ret),
            span: merge_span(start_span, self.previous().span),
        })
    }

    // @pinker-nav:end parser.tipos.gramatica

    // @pinker-nav:start parser.declaracoes.tipos
    // @pinker-nav:domain declaracoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Declarações de tipos e itens de topo: `apelido` (type alias), `ninho` (struct), blocos `impl`, `trato` e `leque` (enum), consumindo cada gramática e produzindo os nós de declaração correspondentes.
    fn parse_type_alias(&mut self) -> Result<TypeAliasDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome do alias de tipo")?
            .lexeme
            .clone();
        self.consume(TokenKind::Eq, "=")?;
        let target = self.parse_type()?;
        self.consume(TokenKind::Semi, ";")?;
        // Um apelido de leque herda as variantes do alvo, inclusive quando o
        // alvo é ele próprio um apelido: a cadeia é transparente e não cria
        // identidade nominal nova.
        if let Type::Enum {
            name: target_name, ..
        }
        | Type::Alias {
            name: target_name, ..
        } = &target
        {
            if let Some(variants) = self.enum_decls.get(target_name).cloned() {
                // Só as variantes são herdadas. O nome do apelido **não** entra
                // em `enum_names`: a identidade semântica continua sendo a do
                // leque de destino.
                self.enum_decls.insert(name.clone(), variants);
            }
        }
        self.type_alias_targets.insert(name.clone(), target.clone());
        Ok(TypeAliasDecl {
            name,
            target,
            span: merge_span(start_span, self.previous().span),
        })
    }

    /// Helper de extração e tipo do binding de uma carga, pela autoridade única
    /// de classificação (D1).
    ///
    /// O desugaring de `encaixe` não decide mais por `match` parcial sobre o
    /// tipo-fonte: pergunta à autoridade, que resolve apelidos em profundidade
    /// e devolve a classe operacional junto do tipo resolvido. O binding recebe
    /// o **tipo resolvido**, de modo que `apelido N = lista<bombom>` e
    /// `lista<bombom>` produzam bindings com a mesma identidade.
    ///
    /// Cargas ainda não classificáveis aqui — apelido inexistente, genérico não
    /// monomorfizado, tipo fora do contrato — conservam o tipo escrito e o
    /// helper imediato: quem emite o diagnóstico fiel é a semântica, com as
    /// tabelas completas, e é lá que a coerência do helper é reconferida.
    /// Registra o tipo de coleção de cada binding de carga de um braço.
    ///
    /// Existe para que o despacho das operações genéricas de lista derive do
    /// **tipo do binding** — `lista<verso>` não é `lista<bombom>` e
    /// `lista<Arvore>` não perde o elemento — em vez do caminho padrão.
    fn register_payload_binding_collections(
        &mut self,
        enum_name: &str,
        variant: &str,
        names: &[String],
    ) {
        let Some(payloads) = self.enum_decls.get(enum_name).and_then(|variants| {
            variants
                .iter()
                .find(|(candidate, _)| candidate == variant)
                .map(|(_, payloads)| payloads.clone())
        }) else {
            return;
        };
        let span = Span::single(crate::token::Position::new(1, 1));
        for (name, payload_ty) in names.iter().zip(payloads) {
            let (_, binding_ty) = self.payload_binding(&payload_ty, span);
            self.register_collection_type(name, &binding_ty);
        }
    }

    fn payload_binding(&self, declared: &Type, span: Span) -> (&'static str, Type) {
        match crate::enum_payload::classify_enum_payload(
            declared,
            &self.type_alias_targets,
            &self.enum_names,
            &self.struct_names,
        ) {
            Ok(shape) => (shape.carga_intrinsic(), shape.resolved.with_span(span)),
            Err(_) => (
                crate::enum_payload::CARGA_IMEDIATO,
                declared.with_span(span),
            ),
        }
    }

    fn parse_struct_decl(&mut self) -> Result<StructDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome da struct")?
            .lexeme
            .clone();
        self.consume(TokenKind::LBrace, "{")?;
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) {
            let field_name = self
                .consume(TokenKind::Ident, "nome do campo da struct")?
                .lexeme
                .clone();
            let field_start = self.previous().span;
            self.consume(TokenKind::Colon, ":")?;
            let ty = self.parse_type()?;
            self.consume(TokenKind::Semi, ";")?;
            fields.push(StructField {
                name: field_name,
                ty,
                span: merge_span(field_start, self.previous().span),
            });
        }
        self.consume(TokenKind::RBrace, "}")?;
        self.struct_names.insert(name.clone());
        Ok(StructDecl {
            name,
            fields,
            span: merge_span(start_span, self.previous().span),
        })
    }

    /// #517: o trato que este `impl` referencia, se esta unidade tem autoridade
    /// sobre ele.
    ///
    /// `TRAIT_SPELLING != TRAIT_IDENTITY`: esta função responde apenas se a
    /// unidade PODE implementar a grafia, e devolve a declaração de onde os
    /// defaults saem. A identidade canônica continua sendo decidida em
    /// `module_resolve`, contra o ambiente de import — nunca por esta grafia.
    ///
    /// A declaração local vem primeiro porque ela é a autoridade da própria
    /// unidade; quando as duas existem, a colisão de import é recusada logo em
    /// seguida pela autoridade que a possui, com a mensagem histórica.
    fn trato_alvo_de_impl(&self, trait_name: &str) -> Option<&TraitDecl> {
        self.trait_decls
            .get(trait_name)
            .or_else(|| self.contexto_de_import.tratos_importados.get(trait_name))
    }

    fn parse_impl_block(&mut self) -> Result<ParsedImplBlock, PinkerError> {
        let start_span = self.previous().span;
        let trait_name = self
            .consume(TokenKind::Ident, "nome do trato em impl")?
            .lexeme
            .clone();
        if self.trato_alvo_de_impl(&trait_name).is_none()
            && !self.contexto_de_import.import_incompleto
        {
            return Err(PinkerError::Parse {
                msg: format!(
                    "impl usa trato '{}' não declarado antes deste ponto nem trazido por import",
                    trait_name
                ),
                span: self.previous().span,
            });
        }
        self.consume(TokenKind::KwPara, "para em impl")?;
        let target_ty = self.parse_type()?;
        self.consume(TokenKind::LBrace, "{")?;
        let mut methods = Vec::new();
        let mut explicit_method_names = HashSet::new();
        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            self.consume(TokenKind::KwCarinho, "carinho dentro de impl")?;
            let mut function = self.parse_function()?;
            if function.params.is_empty() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "impl '{}' para '{}' exige métodos com receiver explícito como primeiro parâmetro",
                        trait_name,
                        target_ty.display_name()
                    ),
                    span: function.span,
                });
            }
            function.impl_facts = Some(ImplFunctionFacts {
                target_ty: target_ty.clone(),
                generated_default: false,
            });
            explicit_method_names.insert(function.name.clone());
            function.name = Self::impl_function_name(&trait_name, &target_ty, &function.name);
            methods.push(function);
        }
        self.consume(TokenKind::RBrace, "}")?;
        Ok(ParsedImplBlock {
            relation: ImplDecl {
                trait_name,
                target_ty,
                span: merge_span(start_span, self.previous().span),
            },
            explicit_method_names,
            methods,
        })
    }

    fn materialize_trait_defaults(
        &mut self,
        impl_relations: &[PendingImplRelation],
    ) -> Result<Vec<FunctionDecl>, PinkerError> {
        let mut defaults = Vec::new();
        let closure_templates: HashMap<String, FunctionDecl> = self
            .pending_functions
            .iter()
            .filter(|function| function.name.starts_with("__anon_carinho_"))
            .map(|function| (function.name.clone(), function.clone()))
            .collect();
        let mut consumed_closure_templates = HashSet::new();
        let mut cloned_closures = Vec::new();
        for pending in impl_relations {
            let trait_name = &pending.relation.trait_name;
            let target_ty = &pending.relation.target_ty;
            // Com a superfície de import incompleta o `impl` foi aceito sem
            // trato conhecido: não há default a materializar, e o carregador
            // recusa o programa em seguida com o erro real do módulo.
            let Some(trait_decl) = self.trato_alvo_de_impl(trait_name) else {
                debug_assert!(self.contexto_de_import.import_incompleto);
                continue;
            };
            let methods = trait_decl.methods.clone();

            for method in &methods {
                let Some(body) = &method.body else {
                    continue;
                };
                let mut params = method.params.clone();
                let Some(receiver) = params.first_mut() else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "default '{}.{}' exige receiver como primeiro parâmetro",
                            trait_name, method.name
                        ),
                        span: method.span,
                    });
                };
                if matches!(&receiver.ty, Type::Alias { name, .. } if name == "si") {
                    receiver.ty = target_ty.clone();
                }

                // O corpo default pertence ao contrato mesmo quando um override
                // vence a seleção. Nesse caso materializamos uma função privada
                // somente para a checagem semântica; ela não usa o prefixo
                // `__impl_`, portanto nunca entra em method_index nem em vtable.
                let is_generated_impl_default =
                    !pending.explicit_method_names.contains(&method.name);
                let name = if is_generated_impl_default {
                    Self::impl_function_name(trait_name, target_ty, &method.name)
                } else {
                    Self::trait_default_check_function_name(trait_name, target_ty, &method.name)
                };

                let mut function = FunctionDecl {
                    name,
                    impl_facts: is_generated_impl_default.then(|| ImplFunctionFacts {
                        target_ty: target_ty.clone(),
                        generated_default: true,
                    }),
                    type_params: Vec::new(),
                    params,
                    ret_type: method.ret_type.clone(),
                    span: method.span,
                    body: body.clone(),
                };
                self.clone_trait_default_closures(
                    &mut function,
                    &closure_templates,
                    &mut consumed_closure_templates,
                    &mut cloned_closures,
                )?;
                defaults.push(function);
            }
        }
        self.pending_functions
            .retain(|function| !consumed_closure_templates.contains(&function.name));
        self.capturing_anon_functions
            .retain(|name| !consumed_closure_templates.contains(name));
        self.pending_functions.extend(cloned_closures);
        Ok(defaults)
    }

    fn parse_trait_decl(&mut self) -> Result<TraitDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome do trato")?
            .lexeme
            .clone();
        self.consume(TokenKind::LBrace, "{")?;
        let mut methods = Vec::new();
        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            let method_start = self
                .consume(TokenKind::KwCarinho, "carinho em assinatura de trato")?
                .span;
            let method_name = self
                .consume(TokenKind::Ident, "nome do método do trato")?
                .lexeme
                .clone();
            self.consume(TokenKind::LParen, "(")?;
            let mut params = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    let param_start = self.peek_span();
                    let param_name = self
                        .consume(TokenKind::Ident, "nome do parâmetro do método")?
                        .lexeme
                        .clone();
                    self.consume(TokenKind::Colon, ":")?;
                    let ty = self.parse_type()?;
                    params.push(Param {
                        name: param_name,
                        ty,
                        span: merge_span(param_start, self.previous().span),
                    });
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, ")")?;
            let ret_type = if self.match_token(TokenKind::Arrow) {
                Some(self.parse_type()?)
            } else {
                None
            };
            let body = if self.match_token(TokenKind::Semi) {
                None
            } else {
                Some(self.parse_callable_body(&params, ret_type.is_some())?)
            };
            methods.push(TraitMethodSig {
                name: method_name,
                params,
                ret_type,
                body,
                span: merge_span(method_start, self.previous().span),
            });
        }
        self.consume(TokenKind::RBrace, "}")?;
        let trait_decl = TraitDecl {
            name,
            methods,
            span: merge_span(start_span, self.previous().span),
        };
        self.trait_decls
            .insert(trait_decl.name.clone(), trait_decl.clone());
        Ok(trait_decl)
    }

    fn parse_enum_decl(&mut self) -> Result<EnumDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome do leque")?
            .lexeme
            .clone();
        let type_params = self.parse_optional_type_params()?;
        self.consume(TokenKind::LBrace, "{")?;
        let mut variants = Vec::new();
        loop {
            let variant_token = self.consume(TokenKind::Ident, "nome da variante do leque")?;
            let variant_name = variant_token.lexeme.clone();
            let variant_start = variant_token.span;
            let mut payloads = Vec::new();
            if self.match_token(TokenKind::LParen) {
                loop {
                    payloads.push(self.parse_type()?);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
                self.consume(TokenKind::RParen, ")")?;
            }
            variants.push(EnumVariant {
                name: variant_name,
                payloads,
                span: merge_span(variant_start, self.previous().span),
            });
            if !self.match_token(TokenKind::Comma) {
                break;
            }
            if self.check(TokenKind::RBrace) {
                break;
            }
        }
        self.consume(TokenKind::RBrace, "}")?;
        self.enum_decls.insert(
            name.clone(),
            variants
                .iter()
                .map(|variant| (variant.name.clone(), variant.payloads.clone()))
                .collect(),
        );
        self.enum_names.insert(name.clone());
        Ok(EnumDecl {
            name,
            type_params,
            variants,
            span: merge_span(start_span, self.previous().span),
        })
    }

    // @pinker-nav:end parser.declaracoes.tipos

    // @pinker-nav:start parser.encaixe.expressao
    // @pinker-nav:domain encaixe
    // @pinker-nav:layer parser
    // @pinker-nav:summary Parser de `encaixe`: preserva leques como `EnumMatchStmt` com `EnumPattern` recursivo (variantes e bindings) e mantém uniões estruturais no `UnionMatchStmt` vigente; tipo, exaustividade, duplicatas e unreachable patterns pertencem à semântica.
    /// Parse de `encaixe` sobre leques e uniões estruturais.
    ///
    /// ```text
    /// encaixe expr {
    ///     caso Leque.ComCarga(nome) { ... }
    ///     caso Leque.SemCarga { ... }
    ///     senao { ... }
    /// }
    /// ```
    ///
    /// Patterns de leque permanecem recursivos na AST; este estágio não calcula
    /// tags, não extrai payloads e não decide exaustividade.
    fn parse_encaixe(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        self.consume(TokenKind::KwEncaixe, "encaixe")?;
        let start_span = self.previous().span;
        let scrutinee = self.parse_expr()?;
        self.consume(TokenKind::LBrace, "{")?;
        if self.check(TokenKind::KwCaso) && !self.check_at(2, TokenKind::Dot) {
            return self.parse_union_encaixe_after_header(start_span, scrutinee);
        }

        let mut arms = Vec::new();
        let mut otherwise = None;
        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            if self.match_token(TokenKind::KwCaso) {
                let arm_span = self.previous().span;
                // Parte G: a carga do padrão liga nome, e liga só neste braço.
                self.abrir_escopo_local();
                let arm_result = self.parse_braco_de_caso();
                self.fechar_escopo_local();
                let (pattern, body) = arm_result?;
                arms.push(EnumMatchArm {
                    pattern,
                    body,
                    span: arm_span,
                });
            } else if self.match_token(TokenKind::KwSenao) {
                otherwise = Some(self.parse_block()?);
                break;
            } else {
                return Err(PinkerError::Parse {
                    msg: "esperado 'caso' ou 'senao' dentro de 'encaixe'".to_string(),
                    span: self.peek_span(),
                });
            }
        }
        self.consume(TokenKind::RBrace, "}")?;
        let match_span = merge_span(start_span, self.previous().span);
        if arms.is_empty() {
            return Err(PinkerError::Parse {
                msg: "encaixe exige ao menos um 'caso Leque.Variante'".to_string(),
                span: match_span,
            });
        }

        Ok(vec![Stmt::EnumMatch(EnumMatchStmt {
            scrutinee,
            arms,
            otherwise,
            span: match_span,
        })])
    }

    /// Padrão e corpo de um braço `caso`, lidos como uma unidade.
    ///
    /// Existe para que `parse_encaixe` possa abrir e fechar o escopo léxico do
    /// braço em volta dos dois — a carga do padrão liga nome e o corpo o usa.
    fn parse_braco_de_caso(&mut self) -> Result<(EnumPattern, Block), PinkerError> {
        let pattern = self.parse_enum_pattern()?;
        self.register_enum_pattern_collections(&pattern);
        let body = self.parse_block()?;
        Ok((pattern, body))
    }

    fn parse_enum_pattern(&mut self) -> Result<EnumPattern, PinkerError> {
        let enum_token = self.consume(TokenKind::Ident, "nome do leque no padrão do caso")?;
        let enum_name = enum_token.lexeme.clone();
        let start_span = enum_token.span;
        self.consume(TokenKind::Dot, ".")?;
        let variant_token = self.consume(TokenKind::Ident, "nome da variante no padrão do caso")?;
        let variant = variant_token.lexeme.clone();
        let mut end_span = variant_token.span;
        let mut payloads = Vec::new();

        if self.match_token(TokenKind::LParen) {
            if !self.check(TokenKind::RParen) {
                loop {
                    let payload =
                        if self.check(TokenKind::Ident) && self.check_at(1, TokenKind::Dot) {
                            self.parse_enum_pattern()?
                        } else {
                            let binding = self
                                .consume(
                                    TokenKind::Ident,
                                    "binding ou padrão aninhado da carga do caso",
                                )?
                                .clone();
                            // Parte G: a carga de variante liga nome sem
                            // palavra-chave e sem `:`. É registrada aqui, no
                            // escopo que `parse_encaixe` abriu para este braço,
                            // porque é aqui que o parser acaba de decidir que
                            // aquilo é uma ligação — e ela vale só neste braço.
                            self.registrar_ligacao_local(&binding.lexeme);
                            EnumPattern::Binding {
                                name: binding.lexeme.clone(),
                                span: binding.span,
                            }
                        };
                    payloads.push(payload);
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, ")")?;
            end_span = self.previous().span;
        }

        Ok(EnumPattern::Variant {
            enum_name,
            variant,
            payloads,
            span: merge_span(start_span, end_span),
        })
    }

    fn register_enum_pattern_collections(&mut self, pattern: &EnumPattern) {
        let EnumPattern::Variant {
            enum_name,
            variant,
            payloads,
            ..
        } = pattern
        else {
            return;
        };
        if payloads
            .iter()
            .all(|payload| matches!(payload, EnumPattern::Binding { .. }))
        {
            let names = payloads
                .iter()
                .filter_map(|payload| match payload {
                    EnumPattern::Binding { name, .. } => Some(name.clone()),
                    EnumPattern::Variant { .. } => None,
                })
                .collect::<Vec<_>>();
            self.register_payload_binding_collections(enum_name, variant, &names);
            return;
        }
        let Some(payload_types) = self.enum_decls.get(enum_name).and_then(|variants| {
            variants
                .iter()
                .find(|(name, _)| name == variant)
                .map(|(_, payloads)| payloads.clone())
        }) else {
            return;
        };

        for (payload, payload_ty) in payloads.iter().zip(payload_types) {
            match payload {
                EnumPattern::Binding { name, span } => {
                    let (_, binding_ty) = self.payload_binding(&payload_ty, *span);
                    self.register_collection_type(name, &binding_ty);
                }
                EnumPattern::Variant { .. } => self.register_enum_pattern_collections(payload),
            }
        }
    }
    fn parse_union_encaixe_after_header(
        &mut self,
        start_span: Span,
        scrutinee: Expr,
    ) -> Result<Vec<Stmt>, PinkerError> {
        let mut arms: Vec<UnionMatchArm> = Vec::new();
        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            if self.match_token(TokenKind::KwSenao) {
                return Err(PinkerError::Parse {
                    msg: "'senao' não substitui cobertura exaustiva de união nesta fase"
                        .to_string(),
                    span: self.previous().span,
                });
            }
            self.consume(TokenKind::KwCaso, "caso")?;
            let arm_span = self.previous().span;
            let member_type = self.parse_type()?;
            self.consume(TokenKind::LParen, "(")?;
            let binding = self
                .consume(TokenKind::Ident, "binding do membro da união")?
                .lexeme
                .clone();
            self.consume(TokenKind::RParen, ")")?;
            // Parte G: a ligação do membro da união vale só neste braço.
            self.abrir_escopo_local();
            self.registrar_ligacao_local(&binding);
            let body_result = self.parse_block();
            self.fechar_escopo_local();
            let body = body_result?;
            arms.push(UnionMatchArm {
                member_type,
                binding,
                body,
                span: arm_span,
            });
        }
        self.consume(TokenKind::RBrace, "}")?;
        let match_span = merge_span(start_span, self.previous().span);
        if arms.len() < 2 {
            return Err(PinkerError::Parse {
                msg: "encaixe de união exige ao menos dois membros".to_string(),
                span: match_span,
            });
        }

        Ok(vec![Stmt::UnionMatch(UnionMatchStmt {
            scrutinee,
            arms,
            span: match_span,
        })])
    }

    // @pinker-nav:end parser.encaixe.expressao

    // @pinker-nav:start parser.resultado.tentar-propagar
    // @pinker-nav:domain resultado
    // @pinker-nav:layer parser
    // @pinker-nav:summary Desugaring de `tentar` e `propagar`/`propagar?` sobre leques de resultado: reconhece os braços `sucesso`/`falha` (ou a forma curta de propagação) e abaixa para a mesma representação de `encaixe`, produzindo `ast::Stmt` sem caminho especial de runtime.
    /// Desugaring de `tentar` (Fase 223): tratamento estruturado sobre um
    /// leque de resultado declarado pelo usuário.
    ///
    /// ```text
    /// tentar expr {
    ///     sucesso Resultado.Ok(valor) { ... }
    ///     falha Resultado.Erro(erro) { ... }
    /// }
    /// ```
    ///
    /// O construto exige exatamente um braço `sucesso` e um braço `falha`, ambos
    /// apontando para variantes do mesmo leque. A execução abaixa para a mesma
    /// representação de `encaixe`, logo funciona no interpretador e no backend
    /// nativo sem caminho especial interpreter-only.
    fn parse_tentar_desugared(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        self.consume(TokenKind::KwTentar, "tentar")?;
        let start_span = self.previous().span;
        let scrutinee = self.parse_expr()?;
        self.consume(TokenKind::LBrace, "{")?;

        struct TentarArm {
            variant: String,
            bindings: Vec<String>,
            body: Block,
            span: Span,
        }

        let mut enum_name: Option<String> = None;
        let mut arms: Vec<TentarArm> = Vec::new();
        let mut saw_success = false;
        let mut saw_failure = false;

        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            let label = self
                .consume(TokenKind::Ident, "'sucesso' ou 'falha' dentro de 'tentar'")?
                .clone();
            let label_span = label.span;
            match label.lexeme.as_str() {
                "sucesso" => {
                    if saw_success {
                        return Err(PinkerError::Parse {
                            msg: "tentar aceita apenas um braço 'sucesso'".to_string(),
                            span: label_span,
                        });
                    }
                    saw_success = true;
                }
                "falha" => {
                    if saw_failure {
                        return Err(PinkerError::Parse {
                            msg: "tentar aceita apenas um braço 'falha'".to_string(),
                            span: label_span,
                        });
                    }
                    saw_failure = true;
                }
                other => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "esperado 'sucesso' ou 'falha' dentro de 'tentar', encontrado '{}'",
                            other
                        ),
                        span: label_span,
                    });
                }
            }

            let base = self
                .consume(TokenKind::Ident, "nome do leque no braço de tentar")?
                .lexeme
                .clone();
            self.consume(TokenKind::Dot, ".")?;
            let variant = self
                .consume(TokenKind::Ident, "nome da variante no braço de tentar")?
                .lexeme
                .clone();
            self.consume(TokenKind::LParen, "(")?;
            let mut bindings = Vec::new();
            if !self.check(TokenKind::RParen) {
                loop {
                    bindings.push(
                        self.consume(TokenKind::Ident, "nome da variável ligada pelo braço")?
                            .lexeme
                            .clone(),
                    );
                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, ")")?;

            match &enum_name {
                None => enum_name = Some(base),
                Some(existing) if *existing == base => {}
                Some(existing) => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "tentar mistura leques diferentes: '{}' e '{}'",
                            existing, base
                        ),
                        span: label_span,
                    });
                }
            }

            // Parte C: a categoria de coleção da variável ligada precisa existir
            // **antes** de o corpo do braço ser lido. O desugaring abaixo também
            // registra, mas registrar só lá é tarde demais: uma chamada como
            // `lista_tamanho(nomes)` dentro do corpo já teria sido resolvida
            // como `lista<bombom>` por falta de categoria.
            //
            // Latente até aqui porque as cargas de sucesso da Parte B eram
            // `verso` e `bombom` — nenhuma coleção passava por este caminho.
            if let Some(enum_ref) = enum_name.as_ref() {
                if let Some(variants) = self.enum_decls.get(enum_ref).cloned() {
                    if let Some((_, payloads)) = variants.iter().find(|(nome, _)| *nome == variant)
                    {
                        for (bind_name, payload_ty) in bindings.iter().zip(payloads.iter()) {
                            let (_, binding_ty) = self.payload_binding(payload_ty, label_span);
                            self.register_collection_type(bind_name, &binding_ty);
                        }
                    }
                }
            }

            // Parte G: as ligações do braço valem só dentro do corpo dele — é
            // lá que o desugaring emite os `Stmt::Let`. Mesmo motivo pelo qual
            // a categoria de coleção acima também precisa existir antes.
            self.abrir_escopo_local();
            for bind_name in &bindings {
                self.registrar_ligacao_local(bind_name);
            }
            let body_result = self.parse_block();
            self.fechar_escopo_local();
            let body = body_result?;
            arms.push(TentarArm {
                variant,
                bindings,
                body,
                span: label_span,
            });
        }
        self.consume(TokenKind::RBrace, "}")?;
        let end_span = self.previous().span;
        let helper_span = merge_span(start_span, end_span);

        if !saw_success || !saw_failure {
            return Err(PinkerError::Parse {
                msg: "tentar exige exatamente um braço 'sucesso' e um braço 'falha'".to_string(),
                span: helper_span,
            });
        }
        let Some(enum_name) = enum_name else {
            return Err(PinkerError::Parse {
                msg: "tentar exige braços com padrão Leque.Variante(...)".to_string(),
                span: helper_span,
            });
        };
        let Some(declared_variants) = self.enum_decls.get(&enum_name).cloned() else {
            return Err(PinkerError::Parse {
                msg: format!(
                    "tentar usa leque '{}' não declarado antes deste ponto",
                    enum_name
                ),
                span: helper_span,
            });
        };
        if !declared_variants
            .iter()
            .any(|(_, payloads)| !payloads.is_empty())
        {
            return Err(PinkerError::Parse {
                msg: "tentar exige leque com variantes de carga para transportar sucesso/falha"
                    .to_string(),
                span: helper_span,
            });
        }

        let mut seen_variants: Vec<&str> = Vec::new();
        for arm in &arms {
            let Some((_, payloads)) = declared_variants
                .iter()
                .find(|(name, _)| *name == arm.variant)
            else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "variante '{}' não existe no leque '{}'",
                        arm.variant, enum_name
                    ),
                    span: arm.span,
                });
            };
            if seen_variants.contains(&arm.variant.as_str()) {
                return Err(PinkerError::Parse {
                    msg: format!("variante '{}' repetida no tentar", arm.variant),
                    span: arm.span,
                });
            }
            seen_variants.push(arm.variant.as_str());
            if payloads.is_empty() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "variante '{}' não carrega valor; tentar exige carga explícita",
                        arm.variant
                    ),
                    span: arm.span,
                });
            }
            if payloads.len() != arm.bindings.len() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "variante '{}' carrega {} valor(es), mas o braço liga {} nome(s)",
                        arm.variant,
                        payloads.len(),
                        arm.bindings.len()
                    ),
                    span: arm.span,
                });
            }
        }

        self.synthetic_counter += 1;
        let target_name = format!("__tentar_alvo_{}", self.synthetic_counter);
        let target_stmt = Stmt::Let(LetStmt {
            name: target_name.clone(),
            is_mut: false,
            ty: Some(Type::Alias {
                name: enum_name.clone(),
                span: helper_span,
            }),
            init: scrutinee,
            span: helper_span,
        });
        let target_ident = |span: Span| Expr {
            kind: ExprKind::Ident(target_name.clone()),
            span,
        };

        let mut else_branch: Option<ElseBlock> = None;
        for arm in arms.into_iter().rev() {
            let tag = declared_variants
                .iter()
                .position(|(name, _)| *name == arm.variant)
                .expect("variante validada acima") as u64;
            let condition = Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Call(
                            Box::new(Expr {
                                kind: ExprKind::Ident("__pinker_internal_leque_tag".to_string()),
                                span: arm.span,
                            }),
                            vec![target_ident(arm.span)],
                        ),
                        span: arm.span,
                    }),
                    BinaryOp::Eq,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(tag),
                        span: arm.span,
                    }),
                ),
                span: arm.span,
            };

            let payload_types = declared_variants
                .iter()
                .find(|(name, _)| *name == arm.variant)
                .map(|(_, payloads)| payloads.clone())
                .expect("variante validada acima");
            let mut body_stmts = Vec::new();
            for (index, (bind_name, payload_ty)) in
                arm.bindings.into_iter().zip(payload_types).enumerate()
            {
                let (carga_fn, binding_ty) = self.payload_binding(&payload_ty, arm.span);
                self.register_collection_type(&bind_name, &binding_ty);
                body_stmts.push(Stmt::Let(LetStmt {
                    name: bind_name,
                    is_mut: false,
                    ty: Some(binding_ty),
                    init: Expr {
                        kind: ExprKind::Call(
                            Box::new(Expr {
                                kind: ExprKind::Ident(carga_fn.to_string()),
                                span: arm.span,
                            }),
                            vec![
                                target_ident(arm.span),
                                Expr {
                                    kind: ExprKind::IntLit(tag),
                                    span: arm.span,
                                },
                                Expr {
                                    kind: ExprKind::IntLit(index as u64),
                                    span: arm.span,
                                },
                            ],
                        ),
                        span: arm.span,
                    },
                    span: arm.span,
                }));
            }
            body_stmts.extend(arm.body.stmts);
            let then_branch = Block {
                stmts: body_stmts,
                span: arm.body.span,
            };
            let if_stmt = IfStmt {
                condition,
                then_branch,
                else_branch,
                span: helper_span,
            };
            else_branch = Some(ElseBlock::If(Box::new(if_stmt)));
        }

        let Some(ElseBlock::If(root_if)) = else_branch else {
            unreachable!("tentar tem dois braços validados acima");
        };
        Ok(vec![target_stmt, Stmt::If(*root_if)])
    }

    /// Desugaring de `propagar` (Fases 224 e 237): retorno antecipado explícito
    /// para resultados baseados em leques.
    ///
    /// ```text
    /// propagar expr como Resultado.Ok(valor) senao Resultado.Erro(erro);
    /// propagar? expr como Resultado.Ok(valor);
    /// ```
    ///
    /// A variante de sucesso é validada e ignorada; a variante de falha tem sua
    /// carga extraída e retornada como `Resultado.Erro(carga)`. A sintaxe mantém
    /// o leque e as variantes explícitos para evitar inferência global prematura.
    /// A forma curta com `?` infere a falha apenas quando há exatamente uma
    /// outra variante com uma carga no mesmo leque.
    fn parse_propagar_desugared(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        self.consume(TokenKind::KwPropagar, "propagar")?;
        let start_span = self.previous().span;
        let short_form = self.match_token(TokenKind::Question);
        let scrutinee = self.parse_expr()?;
        let como = self.consume(TokenKind::Ident, "'como' após expressão de propagar")?;
        if como.lexeme != "como" {
            return Err(PinkerError::Parse {
                msg: format!(
                    "esperado 'como' após expressão de propagar, encontrado '{}'",
                    como.lexeme
                ),
                span: como.span,
            });
        }
        let success_base = self
            .consume(TokenKind::Ident, "nome do leque no sucesso de propagar")?
            .lexeme
            .clone();
        self.consume(TokenKind::Dot, ".")?;
        let success_variant = self
            .consume(TokenKind::Ident, "nome da variante de sucesso em propagar")?
            .lexeme
            .clone();
        self.consume(TokenKind::LParen, "(")?;
        let success_binding = self
            .consume(
                TokenKind::Ident,
                "nome simbólico da carga de sucesso em propagar",
            )?
            .lexeme
            .clone();
        self.consume(TokenKind::RParen, ")")?;

        let (failure_base, failure_variant, failure_binding) = if short_form {
            self.consume(TokenKind::Semi, ";")?;
            self.synthetic_counter += 1;
            (
                success_base.clone(),
                String::new(),
                format!("__propagar_falha_{}", self.synthetic_counter),
            )
        } else {
            self.consume(TokenKind::KwSenao, "senao")?;
            let failure_base = self
                .consume(TokenKind::Ident, "nome do leque na falha de propagar")?
                .lexeme
                .clone();
            self.consume(TokenKind::Dot, ".")?;
            let failure_variant = self
                .consume(TokenKind::Ident, "nome da variante de falha em propagar")?
                .lexeme
                .clone();
            self.consume(TokenKind::LParen, "(")?;
            let failure_binding = self
                .consume(TokenKind::Ident, "nome da carga de falha em propagar")?
                .lexeme
                .clone();
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            (failure_base, failure_variant, failure_binding)
        };
        let helper_span = merge_span(start_span, self.previous().span);

        if success_base != failure_base {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar mistura leques diferentes: '{}' e '{}'",
                    success_base, failure_base
                ),
                span: helper_span,
            });
        }
        let Some(declared_variants) = self.enum_decls.get(&success_base).cloned() else {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar usa leque '{}' não declarado antes deste ponto",
                    success_base
                ),
                span: helper_span,
            });
        };
        let failure_variant = if short_form {
            let candidates: Vec<String> = declared_variants
                .iter()
                .filter(|(name, payloads)| *name != success_variant && payloads.len() == 1)
                .map(|(name, _)| name.clone())
                .collect();
            match candidates.as_slice() {
                [only] => only.clone(),
                [] => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "propagar? não encontrou variante de falha única com 1 carga no leque '{}'",
                            success_base
                        ),
                        span: helper_span,
                    });
                }
                many => {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "propagar? é ambíguo no leque '{}'; variantes candidatas: {}",
                            success_base,
                            many.join(", ")
                        ),
                        span: helper_span,
                    });
                }
            }
        } else {
            failure_variant
        };
        if success_variant == failure_variant {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar exige variantes distintas para sucesso e falha; '{}' foi repetida",
                    success_variant
                ),
                span: helper_span,
            });
        }
        let success_payloads = declared_variants
            .iter()
            .find(|(name, _)| *name == success_variant)
            .map(|(_, payloads)| payloads.clone())
            .ok_or_else(|| PinkerError::Parse {
                msg: format!(
                    "variante '{}' não existe no leque '{}'",
                    success_variant, success_base
                ),
                span: helper_span,
            })?;
        if success_payloads.len() != 1 {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar exige sucesso com exatamente 1 carga; variante '{}' tem {}",
                    success_variant,
                    success_payloads.len()
                ),
                span: helper_span,
            });
        }
        let failure_payloads = declared_variants
            .iter()
            .find(|(name, _)| *name == failure_variant)
            .map(|(_, payloads)| payloads.clone())
            .ok_or_else(|| PinkerError::Parse {
                msg: format!(
                    "variante '{}' não existe no leque '{}'",
                    failure_variant, success_base
                ),
                span: helper_span,
            })?;
        if failure_payloads.len() != 1 {
            return Err(PinkerError::Parse {
                msg: format!(
                    "propagar exige falha com exatamente 1 carga; variante '{}' tem {}",
                    failure_variant,
                    failure_payloads.len()
                ),
                span: helper_span,
            });
        }

        self.synthetic_counter += 1;
        let target_name = format!("__propagar_alvo_{}", self.synthetic_counter);
        let target_stmt = Stmt::Let(LetStmt {
            name: target_name.clone(),
            is_mut: false,
            ty: Some(Type::Alias {
                name: success_base.clone(),
                span: helper_span,
            }),
            init: scrutinee,
            span: helper_span,
        });
        let failure_tag = declared_variants
            .iter()
            .position(|(name, _)| *name == failure_variant)
            .expect("variante validada acima") as u64;
        let success_tag = declared_variants
            .iter()
            .position(|(name, _)| *name == success_variant)
            .expect("variante validada acima") as u64;
        let target_ident = || Expr {
            kind: ExprKind::Ident(target_name.clone()),
            span: helper_span,
        };
        let success_declared_ty = success_payloads
            .into_iter()
            .next()
            .expect("validado exatamente uma carga");
        let failure_declared_ty = failure_payloads
            .into_iter()
            .next()
            .expect("validado exatamente uma carga");
        let (success_carga_fn, success_payload_ty) =
            self.payload_binding(&success_declared_ty, helper_span);
        let (failure_carga_fn, failure_payload_ty) =
            self.payload_binding(&failure_declared_ty, helper_span);
        self.register_collection_type(&success_binding, &success_payload_ty);
        self.register_collection_type(&failure_binding, &failure_payload_ty);
        let success_binding_stmt = Stmt::Let(LetStmt {
            name: success_binding,
            is_mut: false,
            ty: Some(success_payload_ty),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(success_carga_fn.to_string()),
                        span: helper_span,
                    }),
                    vec![
                        target_ident(),
                        Expr {
                            kind: ExprKind::IntLit(success_tag),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::IntLit(0),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let failure_binding_stmt = Stmt::Let(LetStmt {
            name: failure_binding.clone(),
            is_mut: false,
            ty: Some(failure_payload_ty),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(failure_carga_fn.to_string()),
                        span: helper_span,
                    }),
                    vec![
                        target_ident(),
                        Expr {
                            kind: ExprKind::IntLit(failure_tag),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::IntLit(0),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let return_stmt = Stmt::Return(ReturnStmt {
            expr: Some(Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::FieldAccess {
                            base: Box::new(Expr {
                                kind: ExprKind::Ident(success_base),
                                span: helper_span,
                            }),
                            field: failure_variant,
                        },
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(failure_binding),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            }),
            span: helper_span,
        });
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Ident("__pinker_internal_leque_tag".to_string()),
                            span: helper_span,
                        }),
                        vec![target_ident()],
                    ),
                    span: helper_span,
                }),
                BinaryOp::Eq,
                Box::new(Expr {
                    kind: ExprKind::IntLit(failure_tag),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };
        Ok(vec![
            target_stmt,
            Stmt::If(IfStmt {
                condition,
                then_branch: Block {
                    stmts: vec![failure_binding_stmt, return_stmt],
                    span: helper_span,
                },
                else_branch: None,
                span: helper_span,
            }),
            success_binding_stmt,
        ])
    }

    // @pinker-nav:end parser.resultado.tentar-propagar

    fn register_collection_type(&mut self, name: &str, ty: &Type) {
        match ty {
            Type::ListBombom(_) => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::ListBombom);
            }
            Type::ListVerso(_) => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::ListVerso);
            }
            Type::ListEnum { element, .. } => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::ListEnum(element.clone()));
            }
            Type::MapVersoBombom(_) => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::MapVersoBombom);
            }
            Type::MapVersoVerso(_) => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::MapVersoVerso);
            }
            Type::MapBombomBombom(_) => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::MapBombomBombom);
            }
            Type::MapBombomVerso(_) => {
                self.collection_types
                    .insert(name.to_string(), CollectionKind::MapBombomVerso);
            }
            Type::Map { key, .. } => {
                self.collection_types.insert(
                    name.to_string(),
                    CollectionKind::Map {
                        key: key.as_ref().clone(),
                    },
                );
            }
            _ => {}
        }
    }

    // @pinker-nav:start parser.closures.expressao
    // @pinker-nav:domain closures
    // @pinker-nav:layer parser
    // @pinker-nav:summary Funções anônimas e vínculos de valor-função: reconhece a closure `(params) -> tipo { corpo }`, materializa sua identidade sintética com proveniência canônica da fonte mais índice local e reconhece os `nova f = ...` que ligam nomes a funções, mantendo o escopo de aliases de valor-função e produzindo `ast::Expr`/vínculos locais.
    fn parse_anonymous_function_expr(&mut self, start_span: Span) -> Result<Expr, PinkerError> {
        self.consume(TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                let param_start = self.peek_span();
                let name = self
                    .consume(TokenKind::Ident, "nome do parâmetro")?
                    .lexeme
                    .clone();
                self.consume(TokenKind::Colon, ":")?;
                let ty = self.parse_type()?;
                params.push(Param {
                    name,
                    ty,
                    span: merge_span(param_start, self.previous().span),
                });
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, ")")?;
        let ret_type = if self.match_token(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.synthetic_counter += 1;
        let name = anonymous_identity::anonymous_callable_name(
            &self.generic_origin,
            self.synthetic_counter,
        );
        let saved_collection_types = self.collection_types.clone();
        self.collection_types.clear();
        for param in &params {
            self.register_collection_type(&param.name, &param.ty);
        }
        self.push_callable_param_scope(&params);
        // Parte G: o `carinho` anônimo não passa por `parse_callable_body`, e
        // os parâmetros dele ligam nome exatamente como os de uma função de
        // topo. Sem este escopo, a família capturaria um parâmetro de closure.
        self.abrir_escopo_local();
        for param in &params {
            self.registrar_ligacao_local(&param.name);
        }
        let body_result = self.parse_block();
        self.fechar_escopo_local();
        self.function_value_scopes.pop();
        let body = body_result?;
        self.collection_types = saved_collection_types;
        let span = merge_span(start_span, body.span);
        let function = FunctionDecl {
            name: name.clone(),
            impl_facts: None,
            type_params: Vec::new(),
            params,
            ret_type,
            body,
            span,
        };
        // Fase 243: aproximação conservadora e sintática (sem resolução real
        // de escopo) — qualquer identificador livre no corpo marca o literal
        // como "potencialmente capturante", excluindo-o dos caminhos rápidos
        // estáticos das Fases 238/239 (alias de parser / especialização por
        // callback), que assumem corpo sem referência a escopo externo. A
        // resolução real (quais nomes são de fato captura) acontece no
        // semantic, em `resolve_var`/`check_call_expr`.
        let has_free_value = !crate::ast::free_identifiers_in_function(&function).is_empty();
        let captures_runtime_callable = crate::ast::capture_candidates_in_function(&function)
            .iter()
            .any(|candidate| {
                self.resolve_function_value_alias(candidate)
                    .is_some_and(|resolved| resolved == *candidate)
            });
        if has_free_value || captures_runtime_callable {
            self.capturing_anon_functions.insert(name.clone());
        }
        self.pending_functions.push(function);
        Ok(Expr {
            kind: ExprKind::Ident(name),
            span,
        })
    }

    fn starts_function_value_let(&self) -> bool {
        if !self.check(TokenKind::KwNova) {
            return false;
        }
        let mut offset = 1;
        if self.check_at(offset, TokenKind::KwMuda) {
            offset += 1;
        }
        self.check_at(offset, TokenKind::Ident)
            && self.check_at(offset + 1, TokenKind::Colon)
            && self.check_at(offset + 2, TokenKind::KwCarinho)
    }

    fn current_function_value_scope_mut(&mut self) -> &mut HashMap<String, String> {
        if self.function_value_scopes.is_empty() {
            self.function_value_scopes.push(HashMap::new());
        }
        self.function_value_scopes
            .last_mut()
            .expect("escopo presente")
    }

    fn resolve_function_value_alias(&self, name: &str) -> Option<String> {
        self.function_value_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn push_callable_param_scope(&mut self, params: &[Param]) {
        self.function_value_scopes.push(
            params
                .iter()
                .filter(|param| matches!(param.ty, Type::Function { .. }))
                .map(|param| (param.name.clone(), param.name.clone()))
                .collect(),
        );
    }

    // Decide entre dois caminhos para `nova [muda] nome: carinho(...) -> T = <expr>;`:
    //
    // - Caminho rápido (Fase 238, otimização): inicialização é exatamente o
    //   literal `carinho` anônimo recém-parseado (`__anon_carinho_N`), sem
    //   `muda`. Vira um alias de parser (`function_value_scopes`) — toda
    //   chamada `nome(...)` é reescrita em tempo de parse para chamada
    //   direta à função sintética; nenhum `Stmt::Let` é emitido (não existe
    //   slot em runtime). Comportamento idêntico ao de antes da Fase 242.
    //
    // - Caminho geral (Fase 242): qualquer outra inicialização (nome de
    //   função top-level, retorno de chamada, outra variável, `muda`
    //   callable) vira uma declaração `nova` real — o valor é materializado
    //   (handle callable) e a chamada correspondente, quando existir, é
    //   resolvida como chamada indireta pelo semantic/IR. Sem isso, a
    //   Fase 242 não conseguiria expressar `nova op: carinho(...) -> T =
    //   escolher(verdade);` (Ret expr não é Ident).
    fn parse_function_value_let(&mut self) -> Result<Option<Stmt>, PinkerError> {
        let start_span = self.consume(TokenKind::KwNova, "nova")?.span;
        let is_mut = self.match_token(TokenKind::KwMuda);
        let local_name = self
            .consume(TokenKind::Ident, "nome da função local")?
            .lexeme
            .clone();
        self.consume(TokenKind::Colon, ":")?;
        let declared_ty = self.parse_type()?;
        if !matches!(declared_ty, Type::Function { .. }) {
            return Err(PinkerError::Parse {
                msg: "função local exige anotação de tipo função".to_string(),
                span: declared_ty.span(),
            });
        }
        self.consume(TokenKind::Eq, "=")?;
        // #505: o corpo da closure pode citar o próprio nome que este `nova`
        // liga, e nesse ponto ele ainda não está em `escopos_locais`.
        self.declarando.push(local_name.clone());
        let init = self.parse_expr();
        self.declarando.pop();
        let init = init?;
        self.consume(TokenKind::Semi, ";")?;
        let end_span = merge_span(start_span, self.previous().span);

        if !is_mut {
            if let ExprKind::Ident(function_name) = &init.kind {
                if function_name.starts_with("__anon_carinho_")
                    && !self.capturing_anon_functions.contains(function_name)
                {
                    let function_name = function_name.clone();
                    let Some(function) = self
                        .pending_functions
                        .iter()
                        .find(|function| function.name == function_name)
                    else {
                        return Err(PinkerError::Parse {
                            msg: "função sintética não encontrada para função local".to_string(),
                            span: end_span,
                        });
                    };
                    let actual_ty = Type::Function {
                        params: function
                            .params
                            .iter()
                            .map(|param| param.ty.clone())
                            .collect(),
                        ret: Box::new(function.ret_type.clone().ok_or_else(|| {
                            PinkerError::Parse {
                                msg: "função local exige retorno declarado nesta fase".to_string(),
                                span: function.span,
                            }
                        })?),
                        span: init.span,
                    };
                    if declared_ty != actual_ty {
                        return Err(PinkerError::Parse {
                            msg: "tipo da função local é incompatível com o literal informado"
                                .to_string(),
                            span: init.span,
                        });
                    }
                    // Parte G: este ramo NÃO emite `Stmt::Let` — o alias vai
                    // direto para `function_value_scopes` e o bloco não tem o
                    // que ler. Sem registrar aqui, o nome ficava invisível para
                    // `escopos_locais` e a família capturava, em silêncio, uma
                    // ligação que o programador acabou de criar.
                    self.registrar_ligacao_local(&local_name);
                    self.current_function_value_scope_mut()
                        .insert(local_name, function_name);
                    return Ok(None);
                }
            }
        }

        self.current_function_value_scope_mut()
            .insert(local_name.clone(), local_name.clone());

        Ok(Some(Stmt::Let(LetStmt {
            name: local_name,
            is_mut,
            ty: Some(declared_ty),
            init,
            span: end_span,
        })))
    }

    // @pinker-nav:end parser.closures.expressao

    // @pinker-nav:start parser.funcoes.declaracao
    // @pinker-nav:domain funcoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Declaração de função: reconhece `carinho nome<params-de-tipo>(params) -> tipo { corpo }`, incluindo os parâmetros de tipo genéricos opcionais, e produz o nó `ast::FunctionDecl`.
    fn parse_function(&mut self) -> Result<FunctionDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome da função")?
            .lexeme
            .clone();
        let type_params = self.parse_optional_type_params()?;

        self.consume(TokenKind::LParen, "(")?;
        let mut params = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                let name = self
                    .consume(TokenKind::Ident, "nome do parâmetro")?
                    .lexeme
                    .clone();
                let param_start = self.previous().span;
                self.consume(TokenKind::Colon, ":")?;
                let ty = self.parse_type()?;
                params.push(Param {
                    name,
                    ty,
                    span: merge_span(param_start, self.previous().span),
                });
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, ")")?;

        let ret_type = if self.match_token(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_callable_body(&params, ret_type.is_some())?;

        Ok(FunctionDecl {
            name,
            impl_facts: None,
            type_params,
            params,
            ret_type,
            span: merge_span(start_span, body.span),
            body,
        })
    }

    fn parse_callable_body(
        &mut self,
        params: &[Param],
        has_return: bool,
    ) -> Result<Block, PinkerError> {
        // Reinicia rastreamento de coleções para este escopo executável e
        // registra parâmetros. O mesmo caminho serve funções comuns e corpos
        // default de trato, evitando duas gramáticas de corpo.
        self.collection_types.clear();
        for param in params {
            match &param.ty {
                Type::ListBombom(_) => {
                    self.collection_types
                        .insert(param.name.clone(), CollectionKind::ListBombom);
                }
                Type::ListVerso(_) => {
                    self.collection_types
                        .insert(param.name.clone(), CollectionKind::ListVerso);
                }
                Type::ListEnum { element, .. } => {
                    self.collection_types.insert(
                        param.name.clone(),
                        CollectionKind::ListEnum(element.clone()),
                    );
                }
                Type::MapVersoBombom(_) => {
                    self.collection_types
                        .insert(param.name.clone(), CollectionKind::MapVersoBombom);
                }
                Type::MapVersoVerso(_) => {
                    self.collection_types
                        .insert(param.name.clone(), CollectionKind::MapVersoVerso);
                }
                Type::MapBombomBombom(_) => {
                    self.collection_types
                        .insert(param.name.clone(), CollectionKind::MapBombomBombom);
                }
                Type::MapBombomVerso(_) => {
                    self.collection_types
                        .insert(param.name.clone(), CollectionKind::MapBombomVerso);
                }
                Type::Map { key, .. } => {
                    self.collection_types.insert(
                        param.name.clone(),
                        CollectionKind::Map {
                            key: key.as_ref().clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        self.push_callable_param_scope(params);
        self.push_value_param_scope(params);
        // Parte G: parâmetro é ligação local com escopo — o corpo, e só ele.
        self.abrir_escopo_local();
        for param in params {
            self.registrar_ligacao_local(&param.name);
        }
        let body_result = self.parse_block();
        self.fechar_escopo_local();
        self.value_type_scopes.pop();
        self.function_value_scopes.pop();
        let mut body = body_result?;

        if has_return {
            if let Some(Stmt::Expr(expr)) = body.stmts.last() {
                let span = expr.span;
                let expr_clone = expr.clone();
                let len = body.stmts.len();
                body.stmts[len - 1] = Stmt::Return(ReturnStmt {
                    expr: Some(expr_clone),
                    span,
                });
            }
        }

        Ok(body)
    }

    fn parse_optional_type_params(&mut self) -> Result<Vec<String>, PinkerError> {
        if !self.match_token(TokenKind::Less) {
            return Ok(Vec::new());
        }
        let mut params = Vec::new();
        loop {
            let token = self
                .consume(TokenKind::Ident, "nome do parâmetro de tipo")?
                .clone();
            if params.iter().any(|param| param == &token.lexeme) {
                return Err(PinkerError::Parse {
                    msg: format!("parâmetro de tipo '{}' repetido", token.lexeme),
                    span: token.span,
                });
            }
            params.push(token.lexeme.clone());
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }
        self.consume(TokenKind::Greater, ">")?;
        Ok(params)
    }

    // @pinker-nav:end parser.funcoes.declaracao

    // @pinker-nav:start parser.genericos.identidade-especializacao
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary O parser não codifica identidade: entrega kind, proveniência do template no estágio atual, nome local e argumentos ordenados à autoridade compartilhada `generic_identity`. A presença em `predeclared_generic_enums`, definida pelo prepass de autoridade, distingue o `Resultado` builtin global do template homônimo de fonte raiz ou modular; módulos recebem a chave transportada pelo loader.
    fn generic_function_name(&self, name: &str, type_args: &[Type]) -> String {
        generic_identity::specialization_name(
            GenericKind::Function,
            &self.generic_origin,
            name,
            type_args,
        )
    }

    fn generic_enum_name(&self, name: &str, type_args: &[Type]) -> String {
        let origin = if self.predeclared_generic_enums.contains(name) {
            &GenericOrigin::Builtin
        } else {
            &self.generic_origin
        };
        generic_identity::specialization_name(GenericKind::Enum, origin, name, type_args)
    }

    // @pinker-nav:end parser.genericos.identidade-especializacao

    // @pinker-nav:start parser.genericos.inferencia-local
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Inferência genérica local e determinística para chamadas sem argumentos de tipo explícitos: sintetiza somente tipos locais de argumentos, unifica recursivamente posições formais com parâmetros de tipo, exige substituição única, diagnostica conflito/ausência de fonte e registra a mesma instanciação monomórfica usada pelo caminho explícito. Não usa tipo de retorno esperado, não executa coercion e não contém dispatch nominal por função.
    fn push_value_param_scope(&mut self, params: &[Param]) {
        self.value_type_scopes.push(
            params
                .iter()
                .map(|param| (param.name.clone(), param.ty.clone()))
                .collect(),
        );
    }

    fn register_value_type(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.value_type_scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn resolve_value_type(&self, name: &str) -> Option<Type> {
        self.value_type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn inference_type_eq(lhs: &Type, rhs: &Type) -> bool {
        match (lhs, rhs) {
            (Type::Bombom(_), Type::Bombom(_))
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
            (
                Type::Alias { name: lhs, .. }
                | Type::Struct { name: lhs, .. }
                | Type::Enum { name: lhs, .. },
                Type::Alias { name: rhs, .. }
                | Type::Struct { name: rhs, .. }
                | Type::Enum { name: rhs, .. },
            ) => lhs == rhs,
            (Type::ListEnum { element: lhs, .. }, Type::ListEnum { element: rhs, .. }) => {
                lhs == rhs
            }
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
                Self::inference_type_eq(lhs_key, rhs_key)
                    && Self::inference_type_eq(lhs_value, rhs_value)
            }
            (
                Type::FixedArray {
                    element: lhs,
                    size: lhs_size,
                    ..
                },
                Type::FixedArray {
                    element: rhs,
                    size: rhs_size,
                    ..
                },
            ) => lhs_size == rhs_size && Self::inference_type_eq(lhs, rhs),
            (
                Type::Pointer {
                    base: lhs,
                    is_volatile: lhs_volatile,
                    ..
                },
                Type::Pointer {
                    base: rhs,
                    is_volatile: rhs_volatile,
                    ..
                },
            ) => lhs_volatile == rhs_volatile && Self::inference_type_eq(lhs, rhs),
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
                        .zip(rhs_params)
                        .all(|(lhs, rhs)| Self::inference_type_eq(lhs, rhs))
                    && Self::inference_type_eq(lhs_ret, rhs_ret)
            }
            (
                Type::Applied {
                    name: lhs_name,
                    args: lhs_args,
                    ..
                },
                Type::Applied {
                    name: rhs_name,
                    args: rhs_args,
                    ..
                },
            ) => {
                lhs_name == rhs_name
                    && lhs_args.len() == rhs_args.len()
                    && lhs_args
                        .iter()
                        .zip(rhs_args)
                        .all(|(lhs, rhs)| Self::inference_type_eq(lhs, rhs))
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
                        .all(|(lhs, rhs)| Self::inference_type_eq(lhs, rhs))
            }
            _ => false,
        }
    }

    fn formal_contains_type_param(formal: &Type, type_params: &HashSet<String>) -> bool {
        match formal {
            Type::Alias { name, .. } => type_params.contains(name),
            Type::ListEnum { element, .. } => type_params.contains(element),
            Type::FixedArray { element, .. } => {
                Self::formal_contains_type_param(element, type_params)
            }
            Type::Pointer { base, .. } => Self::formal_contains_type_param(base, type_params),
            Type::Function { params, ret, .. } => {
                params
                    .iter()
                    .any(|param| Self::formal_contains_type_param(param, type_params))
                    || Self::formal_contains_type_param(ret, type_params)
            }
            Type::Applied { args, .. } => args
                .iter()
                .any(|arg| Self::formal_contains_type_param(arg, type_params)),
            Type::Map { key, value, .. } => {
                Self::formal_contains_type_param(key, type_params)
                    || Self::formal_contains_type_param(value, type_params)
            }
            Type::Union { members, .. } => members
                .iter()
                .any(|member| Self::formal_contains_type_param(member, type_params)),
            _ => false,
        }
    }

    fn bind_inferred_type(
        name: &str,
        actual: &Type,
        substitutions: &mut HashMap<String, Type>,
        span: Span,
    ) -> Result<(), PinkerError> {
        if let Some(previous) = substitutions.get(name) {
            if !Self::inference_type_eq(previous, actual) {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "E-GENERIC-CONFLICTING-INFERENCE: parâmetro '{}' recebeu evidências incompatíveis '{}' e '{}'",
                        name,
                        previous.display_name(),
                        actual.display_name()
                    ),
                    span,
                });
            }
        } else {
            substitutions.insert(name.to_string(), actual.clone());
        }
        Ok(())
    }

    fn infer_substitutions_from_types(
        formal: &Type,
        actual: &Type,
        type_params: &HashSet<String>,
        substitutions: &mut HashMap<String, Type>,
        span: Span,
    ) -> Result<(), PinkerError> {
        if let Type::Alias { name, .. } = formal {
            if type_params.contains(name) {
                return Self::bind_inferred_type(name, actual, substitutions, span);
            }
        }

        match (formal, actual) {
            (
                Type::ListEnum { element, .. },
                Type::ListBombom(actual_span),
            ) if type_params.contains(element) => Self::bind_inferred_type(
                element,
                &Type::Bombom(*actual_span),
                substitutions,
                span,
            ),
            (
                Type::ListEnum { element, .. },
                Type::ListVerso(actual_span),
            ) if type_params.contains(element) => Self::bind_inferred_type(
                element,
                &Type::Verso(*actual_span),
                substitutions,
                span,
            ),
            (
                Type::ListEnum { element, .. },
                Type::ListEnum {
                    element: actual_element,
                    span: actual_span,
                },
            ) if type_params.contains(element) => Self::bind_inferred_type(
                element,
                &Type::Alias {
                    name: actual_element.clone(),
                    span: *actual_span,
                },
                substitutions,
                span,
            ),
            (
                Type::FixedArray {
                    element: formal_element,
                    size: formal_size,
                    ..
                },
                Type::FixedArray {
                    element: actual_element,
                    size: actual_size,
                    ..
                },
            ) if formal_size == actual_size => Self::infer_substitutions_from_types(
                formal_element,
                actual_element,
                type_params,
                substitutions,
                span,
            ),
            (
                Type::Pointer {
                    base: formal_base,
                    is_volatile: formal_volatile,
                    ..
                },
                Type::Pointer {
                    base: actual_base,
                    is_volatile: actual_volatile,
                    ..
                },
            ) if formal_volatile == actual_volatile => Self::infer_substitutions_from_types(
                formal_base,
                actual_base,
                type_params,
                substitutions,
                span,
            ),
            (
                Type::Function {
                    params: formal_params,
                    ret: formal_ret,
                    ..
                },
                Type::Function {
                    params: actual_params,
                    ret: actual_ret,
                    ..
                },
            ) if formal_params.len() == actual_params.len() => {
                for (formal, actual) in formal_params.iter().zip(actual_params) {
                    Self::infer_substitutions_from_types(
                        formal,
                        actual,
                        type_params,
                        substitutions,
                        span,
                    )?;
                }
                Self::infer_substitutions_from_types(
                    formal_ret,
                    actual_ret,
                    type_params,
                    substitutions,
                    span,
                )
            }
            (
                Type::Applied {
                    name: formal_name,
                    args: formal_args,
                    ..
                },
                Type::Applied {
                    name: actual_name,
                    args: actual_args,
                    ..
                },
            ) if formal_name == actual_name && formal_args.len() == actual_args.len() => {
                for (formal, actual) in formal_args.iter().zip(actual_args) {
                    Self::infer_substitutions_from_types(
                        formal,
                        actual,
                        type_params,
                        substitutions,
                        span,
                    )?;
                }
                Ok(())
            }
            (
                Type::Map {
                    key: formal_key,
                    value: formal_value,
                    ..
                },
                Type::Map {
                    key: actual_key,
                    value: actual_value,
                    ..
                },
            ) => {
                Self::infer_substitutions_from_types(
                    formal_key,
                    actual_key,
                    type_params,
                    substitutions,
                    span,
                )?;
                Self::infer_substitutions_from_types(
                    formal_value,
                    actual_value,
                    type_params,
                    substitutions,
                    span,
                )
            }
            (
                Type::Union {
                    members: formal_members,
                    ..
                },
                Type::Union {
                    members: actual_members,
                    ..
                },
            ) if formal_members.len() == actual_members.len() => {
                for (formal, actual) in formal_members.iter().zip(actual_members) {
                    Self::infer_substitutions_from_types(
                        formal,
                        actual,
                        type_params,
                        substitutions,
                        span,
                    )?;
                }
                Ok(())
            }
            _ if !Self::formal_contains_type_param(formal, type_params) => Ok(()),
            _ => Err(PinkerError::Parse {
                msg: format!(
                    "E-GENERIC-NO-INFERENCE-SOURCE: tipo real '{}' não corresponde à estrutura inferível '{}'",
                    actual.display_name(),
                    formal.display_name()
                ),
                span,
            }),
        }
    }

    fn inferred_generic_result_type(&self, name: &str, span: Span) -> Option<Type> {
        self.generic_instantiations
            .iter()
            .rev()
            .find_map(|instantiation| {
                (self.generic_function_name(&instantiation.name, &instantiation.type_args) == name)
                    .then(|| {
                        let template = self.generic_templates.get(&instantiation.name)?;
                        let substitutions = template
                            .type_params
                            .iter()
                            .cloned()
                            .zip(instantiation.type_args.iter().cloned())
                            .collect::<HashMap<_, _>>();
                        template
                            .ret_type
                            .as_ref()
                            .map(|ret| Self::substitute_type(ret, &substitutions).with_span(span))
                    })
                    .flatten()
            })
    }

    fn infer_local_expr_type(&self, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::IntLit(_) => Some(Type::Bombom(expr.span)),
            ExprKind::BoolLit(_) => Some(Type::Logica(expr.span)),
            ExprKind::StringLit(_) => Some(Type::Verso(expr.span)),
            ExprKind::Ident(name) => self.resolve_value_type(name),
            ExprKind::Unary(UnaryOp::Not, _) => Some(Type::Logica(expr.span)),
            ExprKind::Unary(_, inner) => self
                .infer_local_expr_type(inner)
                .map(|ty| ty.with_span(expr.span)),
            ExprKind::AddressOf(inner) => {
                self.infer_local_expr_type(inner).map(|base| Type::Pointer {
                    base: Box::new(base),
                    is_volatile: false,
                    span: expr.span,
                })
            }
            ExprKind::Cast { target, .. } => Some(target.with_span(expr.span)),
            ExprKind::SizeOfType { .. } | ExprKind::AlignOfType { .. } => {
                Some(Type::Bombom(expr.span))
            }
            ExprKind::Binary(
                _,
                BinaryOp::LogicalAnd
                | BinaryOp::LogicalOr
                | BinaryOp::Eq
                | BinaryOp::Neq
                | BinaryOp::Lt
                | BinaryOp::Lte
                | BinaryOp::Gt
                | BinaryOp::Gte,
                _,
            ) => Some(Type::Logica(expr.span)),
            ExprKind::Binary(lhs, _, _) => self
                .infer_local_expr_type(lhs)
                .map(|ty| ty.with_span(expr.span)),
            ExprKind::Call(callee, _) => match &callee.kind {
                ExprKind::Ident(name) => self.inferred_generic_result_type(name, expr.span),
                _ => None,
            },
            _ => None,
        }
    }

    fn infer_generic_call_type_args(
        &self,
        template: &FunctionDecl,
        args: &[Expr],
        span: Span,
    ) -> Result<Vec<Type>, PinkerError> {
        if args.len() != template.params.len() {
            return Err(PinkerError::Parse {
                msg: format!(
                    "E-GENERIC-CALL-ARITY: função genérica '{}' exige {} argumento(s), recebido {}",
                    template.name,
                    template.params.len(),
                    args.len()
                ),
                span,
            });
        }

        let type_params = template.type_params.iter().cloned().collect::<HashSet<_>>();
        let mut substitutions = HashMap::new();
        for (index, (param, arg)) in template.params.iter().zip(args).enumerate() {
            if !Self::formal_contains_type_param(&param.ty, &type_params) {
                continue;
            }
            let Some(actual) = self.infer_local_expr_type(arg) else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "E-GENERIC-NO-INFERENCE-SOURCE: argumento {} da chamada '{}' não possui tipo local sintetizável",
                        index + 1,
                        template.name
                    ),
                    span: arg.span,
                });
            };
            Self::infer_substitutions_from_types(
                &param.ty,
                &actual,
                &type_params,
                &mut substitutions,
                arg.span,
            )?;
        }

        let missing = template
            .type_params
            .iter()
            .filter(|param| !substitutions.contains_key(*param))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(PinkerError::Parse {
                msg: format!(
                    "E-GENERIC-NO-INFERENCE-SOURCE: chamada '{}' não fornece evidência para {}",
                    template.name,
                    missing.join(", ")
                ),
                span,
            });
        }

        Ok(template
            .type_params
            .iter()
            .filter_map(|param| substitutions.remove(param))
            .collect())
    }

    // @pinker-nav:end parser.genericos.inferencia-local

    // @pinker-nav:start parser.importacoes.superficie-familia
    // @pinker-nav:domain importacoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Resolução da superfície modular dentro do parser, e as autoridades de precedência que a governam: `nomes_de_topo`, censo de tokens em profundidade zero com as identidades que a Pinker resolve independentemente da ordem textual, e `escopos_locais`, pilha de escopos léxicos reais. O módulo é FALLBACK e o último a responder: cede a identidade de topo em todo o arquivo e cede a ligação local onde ela está visível. Depois da #505 o que ele NÃO faz mais é ceder ao global: `recusar_intrinseca_sem_import` recusa, no próprio CANONICALIZATION_BOUNDARY, qualquer grafia pública chamada sem import — canônica ou de membro —, e é isso que torna `GLOBAL_PUBLIC_INTRINSIC = 0` uma propriedade do parser em vez de uma lista. A recusa cede a `identidade_lexical_existente`, então declaração do próprio arquivo continua vencendo. A ligação `(módulo, membro) -> identidade` não mora aqui; vem inteira de `familia_superficie`. A #533 acrescentou `ler_declaracao_trazer` como autoridade sintática ÚNICA da declaração: os quatro varredores de token desta região (`familias_seletivas_candidatas`, `modulos_trazidos_inteiros`, `membros_trazidos_seletivamente`, `coletar_nomes_de_topo`) e o laço de import do parser leem por ela, de modo que `ALL_IMPORT_PREPASSES_SEE_THE_SAME_MEMBERS` seja construção e não coincidência — o censo de identidades de topo passou a registrar TODOS os membros da lista, não só o primeiro. `membros_trazidos_seletivamente` é da #517 e devolve `(módulo, membros)` cru, sem decidir o que é família nem o que é módulo real: quem decide continua sendo a autoridade de import.

    /// #533: autoridade sintática ÚNICA da declaração `trazer`.
    ///
    /// Antes desta Issue havia três varreduras de token independentes
    /// (`familias_seletivas_candidatas`, `modulos_trazidos_inteiros`,
    /// `coletar_nomes_de_topo`) mais o laço do parser, cada uma reconhecendo a
    /// forma `trazer Ident . Ident ;` por conta própria. Quatro cópias de uma
    /// gramática são quatro chances de divergir, e a lista de membros é
    /// exatamente o eixo em que elas divergiriam: um prepass que enxerga só o
    /// primeiro membro deixa `b` e `c` de `trazer M.a, b, c;` fora do censo de
    /// identidades de topo, e o import passa a sombrear a si mesmo.
    ///
    /// Por isso a leitura passou a ser uma só. O que continua NÃO morando aqui
    /// é política de resolução: esta função não sabe o que é família, o que é
    /// módulo real, nem o que um membro significa. Ela responde uma pergunta
    /// puramente sintática — «que módulo e que membros esta declaração
    /// escreveu?» — e devolve `None` para toda forma que a gramática não
    /// autoriza, incluindo `trazer M.a, b,;`, `trazer M.a, N.b;` e
    /// `trazer M, a;`.
    pub fn ler_declaracao_trazer(tokens: &[Token], indice: usize) -> Option<DeclaracaoTrazer> {
        if tokens.get(indice)?.kind != TokenKind::KwTrazer {
            return None;
        }
        let modulo = indice + 1;
        if tokens.get(modulo)?.kind != TokenKind::Ident {
            return None;
        }
        let mut cursor = modulo + 1;
        let mut membros = Vec::new();
        if tokens.get(cursor)?.kind == TokenKind::Dot {
            loop {
                cursor += 1;
                if tokens.get(cursor)?.kind != TokenKind::Ident {
                    // Cobre `trazer M.;` e a vírgula final `trazer M.a, b,;`:
                    // item vazio não é membro, e a gramática não o autoriza.
                    return None;
                }
                membros.push(cursor);
                cursor += 1;
                if tokens.get(cursor)?.kind != TokenKind::Comma {
                    break;
                }
            }
        }
        if tokens.get(cursor)?.kind != TokenKind::Semi {
            // `trazer M, a;` para aqui com a vírgula, e `trazer M.a, N.b;` com
            // o ponto do segundo módulo. Nenhuma das duas vira gramática nova.
            return None;
        }
        Some(DeclaracaoTrazer {
            inicio: indice,
            fim: cursor,
            modulo,
            membros,
        })
    }

    /// #533: produz o diagnóstico da declaração `trazer` que o leitor recusou.
    ///
    /// Reconsome a mesma declaração pelo cursor, com as mensagens de sempre.
    /// Só é chamada quando `ler_declaracao_trazer` já devolveu `None`, então
    /// ela SEMPRE termina em erro — nunca é um segundo caminho de aceitação.
    fn diagnosticar_declaracao_trazer(&mut self) -> Result<(), PinkerError> {
        self.consume(TokenKind::KwTrazer, "trazer")?;
        self.consume(TokenKind::Ident, "nome do módulo em trazer")?;
        if self.match_token(TokenKind::Dot) {
            loop {
                self.consume(
                    TokenKind::Ident,
                    "símbolo após '.' em trazer módulo.símbolo",
                )?;
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
                if self.check(TokenKind::Semi) {
                    return Err(PinkerError::Expected {
                        expected:
                            "membro após ',' em trazer módulo.a, b (vírgula final não é permitida)"
                                .to_string(),
                        found: ";".to_string(),
                        span: self.peek_span(),
                    });
                }
            }
            if self.check(TokenKind::Dot) {
                return Err(PinkerError::Expected {
                    expected: "';' — a lista de `trazer` seleciona membros de UM módulo (use `trazer M.a, b;`, não `trazer M.a, N.b;`)"
                        .to_string(),
                    found: ".".to_string(),
                    span: self.peek_span(),
                });
            }
        } else if self.check(TokenKind::Comma) {
            return Err(PinkerError::Expected {
                expected: "';' após `trazer <módulo>` — a lista de membros exige `.` antes do primeiro (use `trazer M.a, b;`)"
                    .to_string(),
                found: ",".to_string(),
                span: self.peek_span(),
            });
        }
        self.consume(TokenKind::Semi, ";")?;
        Err(PinkerError::Expected {
            expected: "declaração `trazer` bem formada".to_string(),
            found: self.previous().lexeme.clone(),
            span: self.previous().span,
        })
    }

    /// Parte G: nomes de módulo em `trazer <nome>.<simbolo>;` cuja classificação
    /// o parser **não pode** fazer sozinho.
    ///
    /// `<nome>` pode ser uma família built-in ou um módulo Pinker real ao lado
    /// da fonte, e as duas leituras dão programas diferentes. Quem sabe o que é
    /// módulo é o carregador; esta função só devolve a pergunta, lida do fluxo
    /// de tokens, para que a resposta volte em `Parser::com_contexto_de_import`.
    ///
    /// A forma inteira (`trazer <nome>;`) não aparece aqui de propósito: ela
    /// nunca carregou módulo, nem antes desta Parte, e continuar sem consultar
    /// o disco é preservar o comportamento histórico, não uma omissão.
    pub fn familias_seletivas_candidatas(tokens: &[Token]) -> Vec<String> {
        let mut candidatos: Vec<String> = Vec::new();
        for indice in 0..tokens.len() {
            // #533: a leitura da declaração é a compartilhada. A pergunta desta
            // função é sobre o MÓDULO, então a lista de membros não muda a
            // resposta — mas ler pelo leitor é o que garante que ela enxergue
            // exatamente as mesmas declarações que o parser aceita, em vez de
            // casar um prefixo de tokens por conta própria.
            let Some(declaracao) = Self::ler_declaracao_trazer(tokens, indice) else {
                continue;
            };
            if declaracao.membros.is_empty() {
                continue;
            }
            let modulo = &tokens[declaracao.modulo].lexeme;
            if !crate::familia_superficie::familia_conhecida(modulo.as_str()) {
                continue;
            }
            if !candidatos.contains(modulo) {
                candidatos.push(modulo.clone());
            }
        }
        candidatos
    }

    /// Parte G: `trazer X.y;` é import seletivo de FAMÍLIA, e não de módulo?
    ///
    /// Autoridade única da classificação, consultada tanto pelo censo de
    /// identidades de topo quanto pelo registro do import. Duas cópias desta
    /// pergunta faziam uma cobrir a outra: desligar qualquer uma das duas
    /// deixava o comportamento intacto, e um guarda que não pode ser derrubado
    /// não guarda nada.
    ///
    /// `REAL_MODULE_X > BUILTIN_FAMILY_X`: a precedência é decidida pela
    /// EXISTÊNCIA do módulo real — veredito que a autoridade de import entregou
    /// em `modulos_reais` — e nunca perguntando o que a família contém, que
    /// arrancaria de um módulo histórico todo export homônimo de membro
    /// aprovado.
    fn seletivo_de_familia(&self, modulo: &str, membro: &str) -> bool {
        !self.contexto_de_import.modulos_reais.contains(modulo)
            && crate::familia_superficie::resolver(modulo, membro).is_some()
    }

    /// Parte G: nomes de `trazer <nome>;` que **podem** ser módulo Pinker.
    ///
    /// Famílias built-in ficam de fora de propósito: `trazer arquivo;` nunca
    /// carregou módulo, nem antes desta Parte, e continuar sem consultar o
    /// disco é preservar o comportamento histórico. O que sobra é a lista de
    /// módulos cujos itens de topo entram neste arquivo — e cujos nomes o
    /// parser precisa conhecer para não capturá-los.
    pub fn modulos_trazidos_inteiros(tokens: &[Token]) -> Vec<String> {
        let mut modulos: Vec<String> = Vec::new();
        for indice in 0..tokens.len() {
            // #533: import inteiro é exatamente a declaração SEM membros. A
            // lista `trazer M.a, b;` continua fora daqui pela mesma razão de
            // sempre — ela não traz os itens de topo do módulo —, agora dita
            // pelo leitor único em vez de por um `seguinte.kind == Semi` que
            // acertava a exclusão por acidente do formato antigo.
            let Some(declaracao) = Self::ler_declaracao_trazer(tokens, indice) else {
                continue;
            };
            if !declaracao.membros.is_empty() {
                continue;
            }
            let modulo = &tokens[declaracao.modulo].lexeme;
            if crate::familia_superficie::familia_conhecida(modulo.as_str()) {
                continue;
            }
            if !modulos.contains(modulo) {
                modulos.push(modulo.clone());
            }
        }
        modulos
    }

    /// #517: declarações `trazer M.a, b;` deste arquivo, na forma sintática.
    ///
    /// Quarta leitora — e não quarta gramática — de `ler_declaracao_trazer`:
    /// devolve `(módulo, membros)` sem decidir o que é família, o que é módulo
    /// real nem o que um membro significa. A forma inteira fica de fora porque
    /// `modulos_trazidos_inteiros` já a responde, com a exclusão histórica das
    /// famílias que esta pergunta não pode reproduzir sozinha.
    pub fn membros_trazidos_seletivamente(tokens: &[Token]) -> Vec<(String, Vec<String>)> {
        let mut declaracoes = Vec::new();
        for indice in 0..tokens.len() {
            let Some(declaracao) = Self::ler_declaracao_trazer(tokens, indice) else {
                continue;
            };
            if declaracao.membros.is_empty() {
                continue;
            }
            declaracoes.push((
                tokens[declaracao.modulo].lexeme.clone(),
                declaracao
                    .membros
                    .iter()
                    .map(|membro| tokens[*membro].lexeme.clone())
                    .collect(),
            ));
        }
        declaracoes
    }

    /// Parte G: registra o efeito de um `trazer` de família built-in.
    ///
    /// `trazer familia;` habilita a forma qualificada; `trazer familia.membro;`
    /// habilita a forma bare **daquele membro**, já canonicalizada. Nenhuma das
    /// duas injeta nomes globais: o legado continua disponível sem import, e o
    /// que o import controla é apenas a superfície nova.
    ///
    /// Aqui não se diagnostica. Membro inexistente e família desconhecida
    /// continuam sendo recusa da autoridade semântica de import, que enxerga o
    /// programa inteiro e produz a mensagem única.
    fn registrar_import_de_familia(&mut self, module: &str, symbol: Option<&str>) {
        if !crate::familia_superficie::familia_conhecida(module) {
            return;
        }
        match symbol {
            None => {
                // Uma identidade de TOPO homônima vence a família mesmo quando
                // é declarada depois do uso: o censo é colhido do arquivo
                // inteiro justamente para que a precedência não dependa da
                // ordem do texto.
                //
                // Ligação local homônima NÃO entra nesta decisão. Ela tem
                // escopo, e desabilitar a família no arquivo inteiro por causa
                // de um parâmetro de uma função era confundir
                // `EXISTE_EM_ALGUM_ESCOPO` com `ESTÁ_VISÍVEL_NESTE_PONTO`.
                // Onde a ligação estiver visível, quem cede é
                // `resolver_membro_de_familia`, no ponto de uso.
                if self.nomes_de_topo.contains(module) {
                    return;
                }
                self.familias_importadas.insert(module.to_string());
            }
            Some(membro) => {
                // A classificação `MODULE vs FAMILY` precede a canonicalização,
                // que é irreversível. Se `module` é módulo Pinker real, este
                // `trazer` é import de módulo e o parser não tem nada a
                // reescrever: a semântica histórica do módulo vence inteira.
                //
                // Colisão do membro com item de topo NÃO é decidida aqui: é
                // recusa da autoridade semântica de import, que atravessa CLI e
                // biblioteca e produz a mensagem única.
                if !self.seletivo_de_familia(module, membro) {
                    return;
                }
                if let Some(canonica) = crate::familia_superficie::resolver(module, membro) {
                    self.membros_familia_importados
                        .insert(membro.to_string(), canonica);
                }
            }
        }
    }

    /// Parte G: colhe as identidades de **topo** do arquivo.
    ///
    /// Uma passada única sobre os tokens, antes de qualquer decisão de
    /// resolução, contando apenas o que está em profundidade zero de chaves:
    ///
    /// * o `Ident` que segue `carinho`, `eterno`, `ninho`, `leque`, `trato` ou
    ///   `apelido` — exatamente as seis categorias de `ast::Item`;
    /// * o símbolo de um `trazer modulo.simbolo;` que vira item de topo.
    ///
    /// `TOP_LEVEL_PREPASS = ONLY_IDENTITIES_WITH_REAL_PROGRAM_SCOPE_FORWARD_PRECEDENCE`.
    /// Estas são as identidades que a semântica coleta na passagem 1, antes de
    /// verificar qualquer corpo, e por isso vencem a família mesmo declaradas
    /// depois do uso. O que tem escopo — parâmetro, local, variável de laço,
    /// carga de padrão — não entra aqui: cabe a `escopos_locais` dizer onde
    /// está visível. Campo de `ninho` não entra por não estar em profundidade
    /// zero, que é a verdade sobre ele: campo não é nome no espaço de valores.
    fn coletar_nomes_de_topo(&mut self) {
        const DECLARAM_ITEM_DE_TOPO: &[TokenKind] = &[
            TokenKind::KwCarinho,
            TokenKind::KwEterno,
            TokenKind::KwNinho,
            TokenKind::KwLeque,
            TokenKind::KwTrato,
            TokenKind::KwApelido,
        ];

        // As identidades que `trazer <modulo>;` traz não passam pelo fluxo de
        // tokens deste arquivo — o parser não tem como enxergá-las. Elas entram
        // aqui pelo veredito da autoridade de import, e a partir daí são
        // identidade de topo como qualquer outra: a família cede a elas em
        // silêncio, exatamente como cede a um `carinho` de topo homônimo.
        let mut nomes: HashSet<String> = self.contexto_de_import.nomes_importados.clone();
        let mut profundidade: i64 = 0;
        for (indice, token) in self.tokens.iter().enumerate() {
            match token.kind {
                TokenKind::LBrace => {
                    profundidade += 1;
                    continue;
                }
                TokenKind::RBrace => {
                    profundidade -= 1;
                    continue;
                }
                _ => {}
            }
            // Profundidade zero é o que separa item de topo de tudo o mais.
            // Método de `trato` e campo de `ninho` vivem em profundidade 1 e não
            // são nome no espaço de valores do arquivo: nenhum deles pode ser
            // chamado como `arquivo(...)` nem sombrear a família.
            if profundidade != 0 {
                continue;
            }

            if DECLARAM_ITEM_DE_TOPO.contains(&token.kind) {
                if let Some(declarado) = self.tokens.get(indice + 1) {
                    if declarado.kind == TokenKind::Ident {
                        nomes.insert(declarado.lexeme.clone());
                    }
                }
                continue;
            }

            // `trazer modulo.simbolo;` liga `simbolo` no topo do arquivo —
            // exceto quando o par é a superfície aprovada de uma família E
            // nenhum módulo real reivindicou o nome. Ali o `simbolo` não é um
            // concorrente: é o próprio nome que o import acaba de criar, e
            // contá-lo faria o import seletivo sombrear a si mesmo.
            if token.kind == TokenKind::KwTrazer {
                // #533: TODOS os membros da declaração entram no censo, não só
                // o primeiro. Este era o ponto em que a lista podia divergir do
                // parser: `trazer M.a, b, c;` ligava `a` no topo e deixava `b`
                // e `c` invisíveis para a precedência, tornando a forma
                // agrupada semanticamente diferente da separada.
                let Some(declaracao) = Self::ler_declaracao_trazer(&self.tokens, indice) else {
                    continue;
                };
                let modulo = self.tokens[declaracao.modulo].lexeme.clone();
                let membros: Vec<String> = declaracao
                    .membros
                    .iter()
                    .map(|posicao| self.tokens[*posicao].lexeme.clone())
                    .collect();
                for membro in membros {
                    if !self.seletivo_de_familia(modulo.as_str(), membro.as_str()) {
                        nomes.insert(membro);
                    }
                }
            }
        }
        self.nomes_de_topo = nomes;
    }

    /// Parte G: abre um escopo léxico para ligações locais.
    ///
    /// Anda junto de `value_type_scopes`: todo lugar que empilha um escopo de
    /// tipo empilha um escopo de nome, e a simetria é o que garante que a
    /// pilha não vaze de um corpo para outro.
    fn abrir_escopo_local(&mut self) {
        self.escopos_locais.push(HashSet::new());
    }

    /// Parte G: fecha o escopo léxico mais interno.
    fn fechar_escopo_local(&mut self) {
        self.escopos_locais.pop();
    }

    /// Parte G: registra uma ligação no escopo mais interno aberto.
    fn registrar_ligacao_local(&mut self, nome: &str) {
        if let Some(escopo) = self.escopos_locais.last_mut() {
            escopo.insert(nome.to_string());
        }
    }

    /// Parte G: registra as ligações que um trecho de bloco acabou de produzir.
    ///
    /// Todo local da Pinker chega ao bloco como `Stmt::Let` — inclusive os que
    /// nascem de desugaring (`para`, `tentar`, `propagar`, encaixe). Ler o que
    /// o parser produziu, em vez de repetir a lista de construtos que ligam
    /// nome, é o que mantém uma autoridade só: se um construto novo abaixar
    /// para `Stmt::Let`, ele já está coberto.
    ///
    /// Uso ANTES da ligação continua resolvendo pela família, e é o correto:
    /// local da Pinker não é hoisted, vale do ponto de declaração em diante.
    fn registrar_ligacoes_de_stmts(&mut self, stmts: &[Stmt]) {
        let ligados: Vec<String> = stmts
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Let(let_stmt) => Some(let_stmt.name.clone()),
                _ => None,
            })
            .collect();
        for nome in ligados {
            self.registrar_ligacao_local(&nome);
        }
    }

    /// Parte G: existe ligação local VISÍVEL NESTE PONTO para este nome?
    fn ligacao_local_visivel(&self, nome: &str) -> bool {
        self.escopos_locais
            .iter()
            .any(|escopo| escopo.contains(nome))
    }

    /// Parte G: alguma identidade léxica já existente responde por este nome,
    /// aqui?
    ///
    /// A política aprovada é `TODA IDENTIDADE JÁ EXISTENTE VENCE A FAMÍLIA`, e
    /// ela tem duas metades com autoridades distintas, porque as duas perguntas
    /// são distintas:
    ///
    /// * `nomes_de_topo` responde «o programa inteiro reivindica este nome?» —
    ///   `carinho`, `eterno`, `ninho`, `leque`, `trato`, `apelido` e símbolo
    ///   importado, com precedência independente da ordem textual;
    /// * `escopos_locais` responde «alguma ligação visível AQUI reivindica este
    ///   nome?» — parâmetro, `nova`/`muda`, variável de laço, carga de padrão.
    ///
    /// Não há terceira lista. Um saco de nomes do arquivo inteiro respondia às
    /// duas perguntas ao mesmo tempo e errava a segunda: um parâmetro de uma
    /// função desabilitava a família em todas as outras.
    fn identidade_lexical_existente(&self, nome: &str) -> bool {
        self.nomes_de_topo.contains(nome) || self.ligacao_local_visivel(nome)
    }

    /// Parte G — CANONICALIZATION_BOUNDARY, forma qualificada.
    ///
    /// `familia.membro` vira a identidade executiva antes de existir como
    /// `FieldAccess`. Devolve `None` quando a forma não é superfície de família:
    /// aí o chamador constrói o `FieldAccess` normal e todo o comportamento
    /// histórico (`Leque.Variante`, campo de ninho, método de trato, tipo
    /// qualificado por módulo) fica preservado por construção.
    fn resolver_membro_de_familia(
        &self,
        base: &Expr,
        field: &str,
    ) -> Result<Option<&'static str>, PinkerError> {
        let ExprKind::Ident(familia) = &base.kind else {
            return Ok(None);
        };
        if !crate::familia_superficie::familia_conhecida(familia.as_str()) {
            return Ok(None);
        }
        // Qualquer identidade já existente com o mesmo nome vence a família,
        // inclusive quando a família foi importada.
        if self.identidade_lexical_existente(familia.as_str()) {
            return Ok(None);
        }
        if !self.familias_importadas.contains(familia.as_str()) {
            // `FAMILY_RESOLUTION_IS_FALLBACK`. Sem o import, esta camada não
            // resolve e não opina: devolve `None` e o `FieldAccess` histórico é
            // construído exatamente como antes da Parte G existir.
            //
            // Aqui já se sabe que o parser não conhece ligação para o nome, mas
            // NÃO se sabe que o programa não conhece: quem enxerga todas as
            // identidades é a autoridade semântica. Emitir daqui a dica de
            // "família não importada" é o que quebrava programa legado sem um
            // único `trazer` no arquivo. A dica passou para `semantic`, que a
            // produz só depois de provar que o identificador não resolve.
            return Ok(None);
        }
        match crate::familia_superficie::resolver(familia.as_str(), field) {
            Some(canonica) => Ok(Some(canonica)),
            // Família importada e não sombreada: membro inexistente é erro
            // determinístico aqui, e nunca queda silenciosa para um global
            // homônimo.
            None => Err(PinkerError::Parse {
                msg: crate::familia_superficie::membro_inexistente(familia.as_str(), field),
                span: base.span,
            }),
        }
    }

    /// #505 — a superfície intrínseca global deixou de existir.
    ///
    /// ```text
    /// PUBLIC_INTRINSIC -> IMPORTABLE_MODULE_SURFACE
    /// GLOBAL_PUBLIC_INTRINSIC = 0
    /// ```
    ///
    /// Toda intrínseca pública é membro de exatamente um módulo importável, e
    /// só é chamável por uma das duas formas que o import habilita: bare, via
    /// `trazer modulo.membro;`, ou qualificada, via `trazer modulo;`. Ambas
    /// passam pelo CANONICALIZATION_BOUNDARY acima e chegam aqui já
    /// canonicalizadas — por isso o chamador só consulta esta recusa quando a
    /// grafia veio do texto do usuário, e não da canonicalização.
    ///
    /// O que se recusa é a chamada, não a declaração: depois desta Issue o
    /// usuário pode declarar `carinho ler_arquivo(...)` num arquivo que não
    /// importa `arquivo.ler_bombom`, e a recusa cede antes disso por
    /// `identidade_lexical_existente`. A pressão global que a #507 exercia
    /// sobre o namespace inteiro morre junto com a superfície que a
    /// justificava.
    fn recusar_intrinseca_sem_import(&self, name: &str, span: Span) -> Result<(), PinkerError> {
        // Identidade já existente vence, exatamente como vence a família: quem
        // declarou o nome está chamando o que declarou. `escopos_locais` responde
        // pela visibilidade real no ponto; `declarando` cobre só a janela em que
        // o nome já foi lido mas ainda não foi ligado.
        if self.identidade_lexical_existente(name)
            || self.declarando.iter().any(|pendente| pendente == name)
        {
            return Ok(());
        }
        let canonica = crate::intrinsic_authority::canonical_public_intrinsic_spelling(name);
        let modulos = crate::familia_superficie::modulos_que_exportam(name);
        if canonica.is_none() && modulos.is_empty() {
            return Ok(());
        }
        // A grafia pode ser as duas coisas ao mesmo tempo: `lista_tamanho` é a
        // grafia canônica do tamanho de lista E o membro `json.lista_tamanho`.
        // A dica precisa oferecer os dois caminhos, ou manda o leitor para o
        // módulo errado.
        let mut caminhos: Vec<String> = modulos
            .iter()
            .map(|modulo| format!("'trazer {modulo}.{name};'"))
            .collect();
        if canonica.is_some() {
            if let Some((modulo, membro)) = crate::familia_superficie::par_da_grafia_canonica(name)
                .filter(|(_, membro)| *membro != name)
            {
                caminhos.push(format!(
                    "'{name}' é a grafia canônica de '{modulo}.{membro}': escreva 'trazer {modulo}.{membro};' e chame '{membro}(...)', ou 'trazer {modulo};' e chame '{modulo}.{membro}(...)'"
                ));
            }
        }
        let como_importar = if caminhos.is_empty() {
            format!("'{name}' não pertence a nenhum módulo importável")
        } else {
            caminhos.join("; ou ")
        };
        Err(PinkerError::Parse {
            msg: format!(
                "intrínseca '{name}' não está no escopo: a superfície intrínseca global não existe mais; {como_importar}"
            ),
            span,
        })
    }

    /// Parte G — CANONICALIZATION_BOUNDARY, forma seletiva.
    ///
    /// Membro trazido por `trazer familia.membro;` em posição de chamada. Uma
    /// ligação local visível aqui vence, pela mesma precedência da forma
    /// qualificada; a colisão com item de topo é recusada antes, pela política
    /// de import que já existe.
    fn resolver_membro_seletivo(&self, name: &str) -> Option<&'static str> {
        if self.identidade_lexical_existente(name) {
            return None;
        }
        self.membros_familia_importados.get(name).copied()
    }

    // @pinker-nav:end parser.importacoes.superficie-familia

    // @pinker-nav:start parser.genericos.leques-template
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Materializa um `EnumDecl` concreto a partir de um template de leque genérico e seus argumentos de tipo: confere a aridade (erro `Parse` se divergir), monta a tabela parâmetro-de-tipo → tipo concreto, substitui as cargas das variantes, remove os parâmetros de tipo e nomeia a instância pelo nome monomórfico. Não valida os tipos resultantes nem os anexa ao `Program`. Inclui também a predeclaração da biblioteca padrão (Fase 241): `register_predeclared_generic_enums`/`predeclared_generic_enum_templates` constroem o template sintético `Resultado<T,E> { Ok(T), Erro(E) }` e `item_name` apoia a supressão por declaração do usuário. `registrar_resultado_falivel` materializa a especialização devolvida por uma superfície falível e, com `registrar_redeclaracao_de_identidade`/`registrar_producao_de_identidade`/`verificar_identidade_runtime`, fecha a conjunção da Parte B1: produzir a identidade pelo runtime e redeclarar o nome é inválido, independentemente da ordem no texto.
    fn instantiate_generic_enum_decl(
        &self,
        template: &EnumDecl,
        type_args: &[Type],
        span: Span,
    ) -> Result<EnumDecl, PinkerError> {
        if template.type_params.len() != type_args.len() {
            return Err(PinkerError::Parse {
                msg: format!(
                    "leque genérico '{}' exige {} argumento(s) de tipo, recebido {}",
                    template.name,
                    template.type_params.len(),
                    type_args.len()
                ),
                span,
            });
        }
        let substitutions = template
            .type_params
            .iter()
            .cloned()
            .zip(type_args.iter().cloned())
            .collect::<HashMap<_, _>>();
        Ok(EnumDecl {
            name: self.generic_enum_name(&template.name, type_args),
            type_params: Vec::new(),
            variants: template
                .variants
                .iter()
                .map(|variant| EnumVariant {
                    name: variant.name.clone(),
                    payloads: variant
                        .payloads
                        .iter()
                        .map(|ty| Self::substitute_type(ty, &substitutions))
                        .collect(),
                    span: variant.span,
                })
                .collect(),
            span: template.span,
        })
    }

    /// Parte B: registra a especialização de `Resultado<T,E>` devolvida por uma
    /// superfície falível.
    ///
    /// Faz exatamente o que a análise do tipo `Resultado<T, E>` escrito na fonte
    /// já faz: enfileira a instanciação para a passagem de monomorfização e
    /// publica o leque concreto nas tabelas locais do parser. A duplicação é
    /// deduplicada por nome monomórfico em `instantiate_generic_enums`, então
    /// chamar a intrínseca e também escrever o tipo não gera dois leques.
    fn registrar_resultado_falivel(
        &mut self,
        superficie: &crate::falha_operacional::SuperficieFalivel,
        span: Span,
    ) -> Result<(), PinkerError> {
        // Parte B1: a partir daqui o programa passa a conter valores cuja tag é
        // produzida pela implementação. A identidade que dá significado a essa
        // tag deixa de ser substituível — em qualquer posição do texto. Vem
        // antes de qualquer materialização: a porta fecha antes de a Parte C
        // publicar o leque da carga.
        self.registrar_producao_de_identidade(superficie.identidade())?;

        // Parte C: quando a carga de sucesso é um leque nomeado, ele precisa
        // existir antes de virar argumento de tipo da especialização.
        for leque in superficie.leques_exigidos() {
            self.registrar_leque_predeclarado(leque);
        }

        let name = crate::falha_operacional::LEQUE_RESULTADO.to_string();
        let args = superficie.argumentos_de_tipo(span);
        self.enum_generic_instantiations
            .push(EnumGenericInstantiation {
                name: name.clone(),
                type_args: args.clone(),
                span,
            });
        if let Some(template) = self.enum_generic_templates.get(&name).cloned() {
            let concrete = self.instantiate_generic_enum_decl(&template, &args, span)?;
            self.enum_names.insert(concrete.name.clone());
            self.enum_decls.insert(
                concrete.name,
                concrete
                    .variants
                    .into_iter()
                    .map(|variant| (variant.name, variant.payloads))
                    .collect(),
            );
        }
        Ok(())
    }

    /// Prepass estreito de autoridade para templates genéricos de topo.
    ///
    /// Não interpreta tipos nem declarações: reconhece apenas a forma lexical
    /// `leque Nome<...>` fora de corpos delimitados por chaves. Isso basta para
    /// decidir se o template builtin homônimo pode ser registrado antes de a
    /// geração de identidades observar um uso anterior à declaração.
    fn source_top_level_generic_enum_span(&self, expected_name: &str) -> Option<Span> {
        let mut brace_depth = 0_usize;
        for (index, token) in self.tokens.iter().enumerate() {
            if token.kind == TokenKind::LBrace {
                brace_depth += 1;
                continue;
            }
            if token.kind == TokenKind::RBrace {
                brace_depth = brace_depth.saturating_sub(1);
                continue;
            }
            if brace_depth != 0 || token.kind != TokenKind::KwLeque {
                continue;
            }
            let Some(name) = self.tokens.get(index + 1) else {
                continue;
            };
            let Some(after_name) = self.tokens.get(index + 2) else {
                continue;
            };
            if name.kind == TokenKind::Ident
                && name.lexeme == expected_name
                && after_name.kind == TokenKind::Less
            {
                let Some(body_start) = self.tokens[index + 3..]
                    .iter()
                    .position(|candidate| candidate.kind == TokenKind::LBrace)
                    .map(|offset| index + 3 + offset)
                else {
                    return Some(merge_span(token.span, name.span));
                };
                let mut body_depth = 0_usize;
                for body_token in &self.tokens[body_start..] {
                    if body_token.kind == TokenKind::LBrace {
                        body_depth += 1;
                    } else if body_token.kind == TokenKind::RBrace {
                        body_depth = body_depth.saturating_sub(1);
                        if body_depth == 0 {
                            return Some(merge_span(token.span, body_token.span));
                        }
                    }
                }
                return Some(merge_span(token.span, name.span));
            }
        }
        None
    }

    /// Fase 241: registra os leques genéricos predeclarados pela biblioteca padrão.
    /// Hoje: `Resultado<T, E> { Ok(T), Erro(E) }`. Construído diretamente como
    /// `EnumDecl` sintético (sem parsing de string, sem I/O, determinístico) e
    /// inserido em `enum_generic_templates`, participando da mesma tabela e do
    /// mesmo caminho de monomorfização dos leques genéricos do usuário (Fase 240).
    /// Um template de usuário detectado pelo prepass mantém a autoridade e
    /// impede a inserção do builtin desde o início.
    fn register_predeclared_generic_enums(&mut self) {
        for template in Self::predeclared_generic_enum_templates() {
            if self
                .source_top_level_generic_enum_span(&template.name)
                .is_some()
            {
                continue;
            }
            self.predeclared_generic_enums.insert(template.name.clone());
            self.enum_generic_templates
                .insert(template.name.clone(), template);
        }
    }

    /// Registra as formas de leque dentre as identidades runtime-reserved.
    ///
    /// `TipoEntrada` e `LimiteTempo` precisam de `EnumDecl` sintético;
    /// `SaidaProcesso` é um handle opaco e, portanto, deliberadamente não entra
    /// neste recipiente. A reserva semântica dos três nomes vem de
    /// `runtime_identity`, não da presença neste mapa.
    fn register_predeclared_plain_enums(&mut self) {
        for template in Self::predeclared_plain_enum_templates() {
            self.predeclared_plain_enums
                .insert(template.name.clone(), template);
        }
    }

    /// Constrói os leques concretos predeclarados. Span sintético 0:0 pela mesma
    /// convenção do template genérico: nunca fingir uma posição de fonte real.
    fn predeclared_plain_enum_templates() -> Vec<EnumDecl> {
        let synthetic = Span::single(crate::token::Position::new(0, 0));
        vec![
            EnumDecl {
                name: crate::tipo_entrada::LEQUE_TIPO_ENTRADA.to_string(),
                type_params: Vec::new(),
                variants: crate::tipo_entrada::VARIANTES
                    .iter()
                    .map(|(nome, _)| EnumVariant {
                        name: (*nome).to_string(),
                        payloads: Vec::new(),
                        span: synthetic,
                    })
                    .collect(),
                span: synthetic,
            },
            EnumDecl {
                name: crate::valor_json::LEQUE_TIPO_JSON.to_string(),
                type_params: Vec::new(),
                variants: crate::valor_json::VARIANTES
                    .iter()
                    .map(|nome| EnumVariant {
                        name: (*nome).to_string(),
                        payloads: Vec::new(),
                        span: synthetic,
                    })
                    .collect(),
                span: synthetic,
            },
            EnumDecl {
                name: crate::limite_tempo::LEQUE_LIMITE_TEMPO.to_string(),
                type_params: Vec::new(),
                variants: vec![
                    EnumVariant {
                        name: crate::limite_tempo::VARIANTE_SEM_LIMITE.to_string(),
                        payloads: Vec::new(),
                        span: synthetic,
                    },
                    EnumVariant {
                        name: crate::limite_tempo::VARIANTE_ATE.to_string(),
                        payloads: vec![Type::Bombom(synthetic)],
                        span: synthetic,
                    },
                ],
                span: synthetic,
            },
        ]
    }

    /// Materializa sob demanda um leque predeclarado simples exigido por uma
    /// superfície builtin.
    ///
    /// Só entra no `Program` quando a superfície que o devolve é realmente
    /// chamada: um programa que não usa a taxonomia não ganha o leque.
    fn registrar_leque_predeclarado(&mut self, nome: &str) {
        let Some(template) = self.predeclared_plain_enums.get(nome).cloned() else {
            return;
        };
        if self.enum_names.contains(nome) {
            return;
        }
        self.enum_names.insert(template.name.clone());
        self.enum_decls.insert(
            template.name.clone(),
            template
                .variants
                .iter()
                .map(|variant| (variant.name.clone(), variant.payloads.clone()))
                .collect(),
        );
        self.predeclared_plain_enums_materializados.push(template);
    }

    /// Constrói os templates sintéticos predeclarados. O span usa a posição 0:0
    /// (inexistente em fonte de usuário) para nunca fingir uma localização real —
    /// diagnósticos de uso apontam sempre para a fonte do usuário.
    fn predeclared_generic_enum_templates() -> Vec<EnumDecl> {
        let synthetic = Span::single(crate::token::Position::new(0, 0));
        let param = |name: &str| Type::Alias {
            name: name.to_string(),
            span: synthetic,
        };
        // Nomes vindos da autoridade: a ordem declarada aqui é o que define
        // TAG_OK/TAG_ERRO, então os dois fatos não podem ser escritos duas vezes.
        vec![EnumDecl {
            name: crate::falha_operacional::LEQUE_RESULTADO.to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariant {
                    name: crate::falha_operacional::VARIANTE_OK.to_string(),
                    payloads: vec![param("T")],
                    span: synthetic,
                },
                EnumVariant {
                    name: crate::falha_operacional::VARIANTE_ERRO.to_string(),
                    payloads: vec![param("E")],
                    span: synthetic,
                },
            ],
            span: synthetic,
        }]
    }

    /// Parte B1: registra que o usuário tomou para si o nome `nome`.
    ///
    /// Não decide nada sozinho — completa metade de uma conjunção. Só levanta
    /// erro se a outra metade já estiver satisfeita, isto é, se o programa já
    /// produziu valores dessa identidade pelo runtime.
    fn registrar_redeclaracao_de_identidade(
        &mut self,
        nome: &str,
        span: Span,
    ) -> Result<(), PinkerError> {
        if !crate::falha_operacional::identidade_produzida_pelo_runtime(nome) {
            return Ok(());
        }
        if self.identidade_runtime_redeclarada.is_none() {
            self.identidade_runtime_redeclarada = Some((nome.to_string(), span));
        }
        self.verificar_identidade_runtime()
    }

    /// Parte B1: registra que o programa produz valores de `nome` pelo runtime.
    ///
    /// A outra metade da conjunção. Chamada na materialização da especialização
    /// devolvida por uma superfície falível.
    fn registrar_producao_de_identidade(&mut self, nome: &str) -> Result<(), PinkerError> {
        debug_assert!(crate::falha_operacional::identidade_produzida_pelo_runtime(
            nome
        ));
        self.identidade_runtime_produzida = true;
        self.verificar_identidade_runtime()
    }

    /// Parte B1: fecha a conjunção.
    ///
    /// ```text
    /// PRODUZ_PELO_RUNTIME ∧ REDECLARADA → INVÁLIDO
    /// ```
    ///
    /// Chamada pelos dois registradores, então quem completa a conjunção por
    /// último levanta o erro — a mensagem e o span não dependem de qual dos dois
    /// fatos apareceu primeiro no texto. O span é sempre o da declaração do
    /// usuário, nunca o `0:0` sintético do predeclarado.
    fn verificar_identidade_runtime(&self) -> Result<(), PinkerError> {
        let Some((nome, span)) = self.identidade_runtime_redeclarada.as_ref() else {
            return Ok(());
        };
        if !self.identidade_runtime_produzida {
            return Ok(());
        }
        Err(PinkerError::Parse {
            msg: crate::falha_operacional::conflito_de_identidade(nome),
            span: *span,
        })
    }

    /// Nome de topo de um item, para a supressão de predeclarados da Fase 241.
    fn item_name(item: &Item) -> Option<&str> {
        match item {
            Item::Function(function) => Some(&function.name),
            Item::Const(constant) => Some(&constant.name),
            Item::TypeAlias(alias) => Some(&alias.name),
            Item::Struct(struct_decl) => Some(&struct_decl.name),
            Item::Enum(enum_decl) => Some(&enum_decl.name),
            Item::Trait(trait_decl) => Some(&trait_decl.name),
        }
    }
    // @pinker-nav:end parser.genericos.leques-template

    fn has_function_param(function: &FunctionDecl) -> bool {
        function
            .params
            .iter()
            .any(|param| matches!(param.ty, Type::Function { .. }))
    }

    fn function_type_for_decl(function: &FunctionDecl, span: Span) -> Result<Type, PinkerError> {
        Ok(Type::Function {
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            ret: Box::new(
                function
                    .ret_type
                    .clone()
                    .ok_or_else(|| PinkerError::Parse {
                        msg: "callback estático exige retorno declarado nesta fase".to_string(),
                        span: function.span,
                    })?,
            ),
            span,
        })
    }

    fn function_param_specialization_name(name: &str, bindings: &[FunctionParamBinding]) -> String {
        let suffix = bindings
            .iter()
            .map(|binding| format!("p{}_{}", binding.index, binding.function_name))
            .collect::<Vec<_>>()
            .join("_");
        format!("__fnparam_{}_{}", name, suffix)
    }

    // @pinker-nav:start parser.genericos.substituicao-ast
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Substituição recursiva de parâmetros de tipo numa AST-template: aplica a tabela parâmetro-de-tipo → tipo concreto percorrendo `Type` (inclusive `ListEnum` que colapsa para lista concreta), `Expr`, `AssignTarget`, `Block`, `ElseBlock`, `IfStmt` e `Stmt`, produzindo uma cópia concreta com os spans preservados. É uma única operação recursiva distribuída por vários helpers `substitute_*`; não executa checagem semântica nem lowering para IR.
    fn substitute_type(ty: &Type, substitutions: &HashMap<String, Type>) -> Type {
        match ty {
            Type::Alias { name, span } => substitutions
                .get(name)
                .map(|ty| ty.with_span(*span))
                .unwrap_or_else(|| ty.clone()),
            Type::FixedArray {
                element,
                size,
                span,
            } => Type::FixedArray {
                element: Box::new(Self::substitute_type(element, substitutions)),
                size: *size,
                span: *span,
            },
            Type::Pointer {
                base,
                is_volatile,
                span,
            } => Type::Pointer {
                base: Box::new(Self::substitute_type(base, substitutions)),
                is_volatile: *is_volatile,
                span: *span,
            },
            Type::Function { params, ret, span } => Type::Function {
                params: params
                    .iter()
                    .map(|param| Self::substitute_type(param, substitutions))
                    .collect(),
                ret: Box::new(Self::substitute_type(ret, substitutions)),
                span: *span,
            },
            Type::Applied { name, args, span } => Type::Applied {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| Self::substitute_type(arg, substitutions))
                    .collect(),
                span: *span,
            },
            Type::ListEnum { element, span } => {
                if let Some(ty) = substitutions.get(element) {
                    match ty {
                        Type::Bombom(_) => Type::ListBombom(*span),
                        Type::Verso(_) => Type::ListVerso(*span),
                        Type::Alias { name, .. }
                        | Type::Enum { name, .. }
                        | Type::Struct { name, .. } => Type::ListEnum {
                            element: name.clone(),
                            span: *span,
                        },
                        _ => ty.clone(),
                    }
                } else {
                    ty.clone()
                }
            }
            Type::Map { key, value, span } => Type::Map {
                key: Box::new(Self::substitute_type(key, substitutions)),
                value: Box::new(Self::substitute_type(value, substitutions)),
                span: *span,
            },
            _ => ty.clone(),
        }
    }

    fn substitute_expr(expr: &Expr, substitutions: &HashMap<String, Type>) -> Expr {
        let kind = match &expr.kind {
            ExprKind::Binary(lhs, op, rhs) => ExprKind::Binary(
                Box::new(Self::substitute_expr(lhs, substitutions)),
                *op,
                Box::new(Self::substitute_expr(rhs, substitutions)),
            ),
            ExprKind::Unary(op, operand) => {
                ExprKind::Unary(*op, Box::new(Self::substitute_expr(operand, substitutions)))
            }
            ExprKind::AddressOf(operand) => {
                ExprKind::AddressOf(Box::new(Self::substitute_expr(operand, substitutions)))
            }
            ExprKind::Call(callee, args) => ExprKind::Call(
                Box::new(Self::substitute_expr(callee, substitutions)),
                args.iter()
                    .map(|arg| Self::substitute_expr(arg, substitutions))
                    .collect(),
            ),
            ExprKind::InternalMapIterCreate(map) => {
                ExprKind::InternalMapIterCreate(Box::new(Self::substitute_expr(map, substitutions)))
            }
            ExprKind::InternalMapIterNextKey(iterator) => ExprKind::InternalMapIterNextKey(
                Box::new(Self::substitute_expr(iterator, substitutions)),
            ),
            ExprKind::FieldAccess { base, field } => ExprKind::FieldAccess {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                field: field.clone(),
            },
            ExprKind::Index { base, index } => ExprKind::Index {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                index: Box::new(Self::substitute_expr(index, substitutions)),
            },
            ExprKind::Cast { expr, target } => ExprKind::Cast {
                expr: Box::new(Self::substitute_expr(expr, substitutions)),
                target: Self::substitute_type(target, substitutions),
            },
            ExprKind::SizeOfType { target } => ExprKind::SizeOfType {
                target: Self::substitute_type(target, substitutions),
            },
            ExprKind::AlignOfType { target } => ExprKind::AlignOfType {
                target: Self::substitute_type(target, substitutions),
            },
            ExprKind::Ident(_)
            | ExprKind::IntLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::StringLit(_) => expr.kind.clone(),
        };
        Expr {
            kind,
            span: expr.span,
        }
    }

    fn substitute_assign_target(
        target: &AssignTarget,
        substitutions: &HashMap<String, Type>,
    ) -> AssignTarget {
        match target {
            AssignTarget::Ident(name) => AssignTarget::Ident(name.clone()),
            AssignTarget::Deref(expr) => {
                AssignTarget::Deref(Box::new(Self::substitute_expr(expr, substitutions)))
            }
            AssignTarget::FieldDeref { base, field } => AssignTarget::FieldDeref {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                field: field.clone(),
            },
            AssignTarget::Index { base, index } => AssignTarget::Index {
                base: Box::new(Self::substitute_expr(base, substitutions)),
                index: Box::new(Self::substitute_expr(index, substitutions)),
            },
        }
    }

    fn substitute_block(block: &Block, substitutions: &HashMap<String, Type>) -> Block {
        Block {
            stmts: block
                .stmts
                .iter()
                .map(|stmt| Self::substitute_stmt(stmt, substitutions))
                .collect(),
            span: block.span,
        }
    }

    fn substitute_else_block(
        else_block: &ElseBlock,
        substitutions: &HashMap<String, Type>,
    ) -> ElseBlock {
        match else_block {
            ElseBlock::Block(block) => {
                ElseBlock::Block(Self::substitute_block(block, substitutions))
            }
            ElseBlock::If(if_stmt) => {
                ElseBlock::If(Box::new(Self::substitute_if_stmt(if_stmt, substitutions)))
            }
        }
    }

    fn substitute_if_stmt(if_stmt: &IfStmt, substitutions: &HashMap<String, Type>) -> IfStmt {
        IfStmt {
            condition: Self::substitute_expr(&if_stmt.condition, substitutions),
            then_branch: Self::substitute_block(&if_stmt.then_branch, substitutions),
            else_branch: if_stmt
                .else_branch
                .as_ref()
                .map(|else_branch| Self::substitute_else_block(else_branch, substitutions)),
            span: if_stmt.span,
        }
    }

    fn substitute_stmt(stmt: &Stmt, substitutions: &HashMap<String, Type>) -> Stmt {
        match stmt {
            Stmt::Let(let_stmt) => Stmt::Let(LetStmt {
                name: let_stmt.name.clone(),
                is_mut: let_stmt.is_mut,
                ty: let_stmt
                    .ty
                    .as_ref()
                    .map(|ty| Self::substitute_type(ty, substitutions)),
                init: Self::substitute_expr(&let_stmt.init, substitutions),
                span: let_stmt.span,
            }),
            Stmt::Return(return_stmt) => Stmt::Return(ReturnStmt {
                expr: return_stmt
                    .expr
                    .as_ref()
                    .map(|expr| Self::substitute_expr(expr, substitutions)),
                span: return_stmt.span,
            }),
            Stmt::Assign(assign_stmt) => Stmt::Assign(AssignStmt {
                target: Self::substitute_assign_target(&assign_stmt.target, substitutions),
                expr: Self::substitute_expr(&assign_stmt.expr, substitutions),
                span: assign_stmt.span,
            }),
            Stmt::If(if_stmt) => Stmt::If(Self::substitute_if_stmt(if_stmt, substitutions)),
            Stmt::While(while_stmt) => Stmt::While(WhileStmt {
                condition: Self::substitute_expr(&while_stmt.condition, substitutions),
                body: Self::substitute_block(&while_stmt.body, substitutions),
                span: while_stmt.span,
            }),
            Stmt::Break(stmt) => Stmt::Break(stmt.clone()),
            Stmt::Continue(stmt) => Stmt::Continue(stmt.clone()),
            Stmt::Falar(falar) => Stmt::Falar(FalarStmt {
                args: falar
                    .args
                    .iter()
                    .map(|arg| Self::substitute_expr(arg, substitutions))
                    .collect(),
                span: falar.span,
            }),
            Stmt::InlineAsm(stmt) => Stmt::InlineAsm(InlineAsmStmt {
                chunks: stmt.chunks.clone(),
                operands: stmt
                    .operands
                    .iter()
                    .map(|operand| InlineAsmOperand {
                        name: operand.name.clone(),
                        direction: operand.direction.clone(),
                        constraint: operand.constraint.clone(),
                        value: Self::substitute_expr(&operand.value, substitutions),
                        span: operand.span,
                    })
                    .collect(),
                clobbers: stmt.clobbers.clone(),
                span: stmt.span,
            }),
            Stmt::EnumMatch(enum_match) => Stmt::EnumMatch(EnumMatchStmt {
                scrutinee: Self::substitute_expr(&enum_match.scrutinee, substitutions),
                arms: enum_match
                    .arms
                    .iter()
                    .map(|arm| EnumMatchArm {
                        pattern: arm.pattern.clone(),
                        body: Self::substitute_block(&arm.body, substitutions),
                        span: arm.span,
                    })
                    .collect(),
                otherwise: enum_match
                    .otherwise
                    .as_ref()
                    .map(|block| Self::substitute_block(block, substitutions)),
                span: enum_match.span,
            }),
            Stmt::UnionMatch(union_match) => Stmt::UnionMatch(UnionMatchStmt {
                scrutinee: Self::substitute_expr(&union_match.scrutinee, substitutions),
                arms: union_match
                    .arms
                    .iter()
                    .map(|arm| UnionMatchArm {
                        member_type: Self::substitute_type(&arm.member_type, substitutions),
                        binding: arm.binding.clone(),
                        body: Self::substitute_block(&arm.body, substitutions),
                        span: arm.span,
                    })
                    .collect(),
                span: union_match.span,
            }),
            Stmt::Expr(expr) => Stmt::Expr(Self::substitute_expr(expr, substitutions)),
        }
    }
    // @pinker-nav:end parser.genericos.substituicao-ast

    // @pinker-nav:start parser.callbacks.substituicao-estatica
    // @pinker-nav:domain callbacks
    // @pinker-nav:layer parser
    // @pinker-nav:summary Reescrita de chamadas a parâmetros-função por chamadas diretas: percorre recursivamente `Expr`, `AssignTarget`, `Block`, `ElseBlock`, `IfStmt` e `Stmt` de um corpo-template e, quando o callee de uma chamada é um identificador ligado a um callback (tabela nome-do-parâmetro → função concreta), troca-o pelo nome da função concreta, preservando as demais expressões e spans. É especialização de callbacks estáticos, não substituição de tipos genéricos.
    fn substitute_function_param_expr(expr: &Expr, replacements: &HashMap<String, String>) -> Expr {
        let kind = match &expr.kind {
            ExprKind::Call(callee, args) => {
                let substituted_callee = Self::substitute_function_param_expr(callee, replacements);
                let callee = if let ExprKind::Ident(name) = &substituted_callee.kind {
                    if let Some(function_name) = replacements.get(name) {
                        Expr {
                            kind: ExprKind::Ident(function_name.clone()),
                            span: substituted_callee.span,
                        }
                    } else {
                        substituted_callee
                    }
                } else {
                    substituted_callee
                };
                ExprKind::Call(
                    Box::new(callee),
                    args.iter()
                        .map(|arg| Self::substitute_function_param_expr(arg, replacements))
                        .collect(),
                )
            }
            ExprKind::Binary(lhs, op, rhs) => ExprKind::Binary(
                Box::new(Self::substitute_function_param_expr(lhs, replacements)),
                *op,
                Box::new(Self::substitute_function_param_expr(rhs, replacements)),
            ),
            ExprKind::Unary(op, operand) => ExprKind::Unary(
                *op,
                Box::new(Self::substitute_function_param_expr(operand, replacements)),
            ),
            ExprKind::AddressOf(operand) => ExprKind::AddressOf(Box::new(
                Self::substitute_function_param_expr(operand, replacements),
            )),
            ExprKind::FieldAccess { base, field } => ExprKind::FieldAccess {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                field: field.clone(),
            },
            ExprKind::Index { base, index } => ExprKind::Index {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                index: Box::new(Self::substitute_function_param_expr(index, replacements)),
            },
            ExprKind::Cast { expr, target } => ExprKind::Cast {
                expr: Box::new(Self::substitute_function_param_expr(expr, replacements)),
                target: target.clone(),
            },
            ExprKind::InternalMapIterCreate(map) => ExprKind::InternalMapIterCreate(Box::new(
                Self::substitute_function_param_expr(map, replacements),
            )),
            ExprKind::InternalMapIterNextKey(iterator) => ExprKind::InternalMapIterNextKey(
                Box::new(Self::substitute_function_param_expr(iterator, replacements)),
            ),
            ExprKind::SizeOfType { target } => ExprKind::SizeOfType {
                target: target.clone(),
            },
            ExprKind::AlignOfType { target } => ExprKind::AlignOfType {
                target: target.clone(),
            },
            ExprKind::Ident(name) => ExprKind::Ident(
                replacements
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone()),
            ),
            ExprKind::IntLit(_) | ExprKind::BoolLit(_) | ExprKind::StringLit(_) => {
                expr.kind.clone()
            }
        };
        Expr {
            kind,
            span: expr.span,
        }
    }

    fn substitute_function_param_assign_target(
        target: &AssignTarget,
        replacements: &HashMap<String, String>,
    ) -> AssignTarget {
        match target {
            AssignTarget::Ident(name) => AssignTarget::Ident(name.clone()),
            AssignTarget::Deref(expr) => AssignTarget::Deref(Box::new(
                Self::substitute_function_param_expr(expr, replacements),
            )),
            AssignTarget::FieldDeref { base, field } => AssignTarget::FieldDeref {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                field: field.clone(),
            },
            AssignTarget::Index { base, index } => AssignTarget::Index {
                base: Box::new(Self::substitute_function_param_expr(base, replacements)),
                index: Box::new(Self::substitute_function_param_expr(index, replacements)),
            },
        }
    }

    fn substitute_function_param_block(
        block: &Block,
        replacements: &HashMap<String, String>,
    ) -> Block {
        Block {
            stmts: block
                .stmts
                .iter()
                .map(|stmt| Self::substitute_function_param_stmt(stmt, replacements))
                .collect(),
            span: block.span,
        }
    }

    fn substitute_function_param_else_block(
        else_block: &ElseBlock,
        replacements: &HashMap<String, String>,
    ) -> ElseBlock {
        match else_block {
            ElseBlock::Block(block) => {
                ElseBlock::Block(Self::substitute_function_param_block(block, replacements))
            }
            ElseBlock::If(if_stmt) => ElseBlock::If(Box::new(
                Self::substitute_function_param_if_stmt(if_stmt, replacements),
            )),
        }
    }

    fn substitute_function_param_if_stmt(
        if_stmt: &IfStmt,
        replacements: &HashMap<String, String>,
    ) -> IfStmt {
        IfStmt {
            condition: Self::substitute_function_param_expr(&if_stmt.condition, replacements),
            then_branch: Self::substitute_function_param_block(&if_stmt.then_branch, replacements),
            else_branch: if_stmt.else_branch.as_ref().map(|else_branch| {
                Self::substitute_function_param_else_block(else_branch, replacements)
            }),
            span: if_stmt.span,
        }
    }

    fn substitute_function_param_stmt(stmt: &Stmt, replacements: &HashMap<String, String>) -> Stmt {
        match stmt {
            Stmt::Let(let_stmt) => Stmt::Let(LetStmt {
                name: let_stmt.name.clone(),
                is_mut: let_stmt.is_mut,
                ty: let_stmt.ty.clone(),
                init: Self::substitute_function_param_expr(&let_stmt.init, replacements),
                span: let_stmt.span,
            }),
            Stmt::Return(return_stmt) => Stmt::Return(ReturnStmt {
                expr: return_stmt
                    .expr
                    .as_ref()
                    .map(|expr| Self::substitute_function_param_expr(expr, replacements)),
                span: return_stmt.span,
            }),
            Stmt::Assign(assign_stmt) => Stmt::Assign(AssignStmt {
                target: Self::substitute_function_param_assign_target(
                    &assign_stmt.target,
                    replacements,
                ),
                expr: Self::substitute_function_param_expr(&assign_stmt.expr, replacements),
                span: assign_stmt.span,
            }),
            Stmt::If(if_stmt) => Stmt::If(Self::substitute_function_param_if_stmt(
                if_stmt,
                replacements,
            )),
            Stmt::While(while_stmt) => Stmt::While(WhileStmt {
                condition: Self::substitute_function_param_expr(
                    &while_stmt.condition,
                    replacements,
                ),
                body: Self::substitute_function_param_block(&while_stmt.body, replacements),
                span: while_stmt.span,
            }),
            Stmt::Break(stmt) => Stmt::Break(stmt.clone()),
            Stmt::Continue(stmt) => Stmt::Continue(stmt.clone()),
            Stmt::Falar(falar) => Stmt::Falar(FalarStmt {
                args: falar
                    .args
                    .iter()
                    .map(|arg| Self::substitute_function_param_expr(arg, replacements))
                    .collect(),
                span: falar.span,
            }),
            Stmt::InlineAsm(stmt) => Stmt::InlineAsm(InlineAsmStmt {
                chunks: stmt.chunks.clone(),
                operands: stmt
                    .operands
                    .iter()
                    .map(|operand| InlineAsmOperand {
                        name: operand.name.clone(),
                        direction: operand.direction.clone(),
                        constraint: operand.constraint.clone(),
                        value: Self::substitute_function_param_expr(&operand.value, replacements),
                        span: operand.span,
                    })
                    .collect(),
                clobbers: stmt.clobbers.clone(),
                span: stmt.span,
            }),
            Stmt::EnumMatch(enum_match) => Stmt::EnumMatch(EnumMatchStmt {
                scrutinee: Self::substitute_function_param_expr(
                    &enum_match.scrutinee,
                    replacements,
                ),
                arms: enum_match
                    .arms
                    .iter()
                    .map(|arm| EnumMatchArm {
                        pattern: arm.pattern.clone(),
                        body: Self::substitute_function_param_block(&arm.body, replacements),
                        span: arm.span,
                    })
                    .collect(),
                otherwise: enum_match
                    .otherwise
                    .as_ref()
                    .map(|block| Self::substitute_function_param_block(block, replacements)),
                span: enum_match.span,
            }),
            Stmt::UnionMatch(union_match) => Stmt::UnionMatch(UnionMatchStmt {
                scrutinee: Self::substitute_function_param_expr(
                    &union_match.scrutinee,
                    replacements,
                ),
                arms: union_match
                    .arms
                    .iter()
                    .map(|arm| UnionMatchArm {
                        member_type: arm.member_type.clone(),
                        binding: arm.binding.clone(),
                        body: Self::substitute_function_param_block(&arm.body, replacements),
                        span: arm.span,
                    })
                    .collect(),
                span: union_match.span,
            }),
            Stmt::Expr(expr) => {
                Stmt::Expr(Self::substitute_function_param_expr(expr, replacements))
            }
        }
    }
    // @pinker-nav:end parser.callbacks.substituicao-estatica

    // @pinker-nav:start parser.callbacks.instanciacao-estatica
    // @pinker-nav:domain callbacks
    // @pinker-nav:layer parser
    // @pinker-nav:summary Materializa as especializações de callback estático solicitadas: localiza a função concreta (entre itens e funções pendentes via `function_decl_by_name`), exige que todo parâmetro-função receba um callback, valida posição e compatibilidade de assinatura de cada vínculo (erros `Parse` locais), gera o nome monomórfico (`__fnparam_*`), remove os parâmetros-função da assinatura, reescreve as chamadas no corpo e deduplica pelas instâncias já emitidas. Produz `FunctionDecl` concretos; não faz checagem semântica.
    fn function_decl_by_name<'a>(
        name: &str,
        items: &'a [Item],
        pending_functions: &'a [FunctionDecl],
    ) -> Option<&'a FunctionDecl> {
        items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .or_else(|| {
                pending_functions
                    .iter()
                    .find(|function| function.name == name)
            })
    }

    fn instantiate_function_param_functions(
        &self,
        items: &[Item],
    ) -> Result<Vec<FunctionDecl>, PinkerError> {
        let mut out = Vec::new();
        let mut emitted = HashSet::new();
        for instantiation in &self.function_param_instantiations {
            let Some(template) = self.function_param_templates.get(&instantiation.name) else {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "função '{}' não aceita callback estático nesta fase",
                        instantiation.name
                    ),
                    span: instantiation.span,
                });
            };
            let mono_name = Self::function_param_specialization_name(
                &instantiation.name,
                &instantiation.bindings,
            );
            if !emitted.insert(mono_name.clone()) {
                continue;
            }
            let binding_indices = instantiation
                .bindings
                .iter()
                .map(|binding| binding.index)
                .collect::<HashSet<_>>();
            for (index, param) in template.params.iter().enumerate() {
                if matches!(param.ty, Type::Function { .. }) && !binding_indices.contains(&index) {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "parâmetro função '{}' de '{}' exige callback estático na chamada",
                            param.name, template.name
                        ),
                        span: param.span,
                    });
                }
            }
            let mut replacements = HashMap::new();
            for binding in &instantiation.bindings {
                let Some(param) = template.params.get(binding.index) else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "callback estático em posição inválida na chamada '{}'",
                            instantiation.name
                        ),
                        span: binding.span,
                    });
                };
                let Type::Function { .. } = &param.ty else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "argumento {} da chamada '{}' não é parâmetro função",
                            binding.index + 1,
                            instantiation.name
                        ),
                        span: binding.span,
                    });
                };
                let Some(function) = Self::function_decl_by_name(
                    &binding.function_name,
                    items,
                    &self.pending_functions,
                ) else {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "callback '{}' não encontrado para especialização de '{}'",
                            binding.function_name, instantiation.name
                        ),
                        span: binding.span,
                    });
                };
                let actual_ty = Self::function_type_for_decl(function, binding.span)?;
                if param.ty != actual_ty {
                    return Err(PinkerError::Parse {
                        msg: format!(
                            "callback '{}' é incompatível com parâmetro '{}'",
                            binding.function_name, param.name
                        ),
                        span: binding.span,
                    });
                }
                replacements.insert(param.name.clone(), binding.function_name.clone());
            }
            out.push(FunctionDecl {
                name: mono_name,
                impl_facts: None,
                type_params: Vec::new(),
                params: template
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !binding_indices.contains(index))
                    .map(|(_, param)| param.clone())
                    .collect(),
                ret_type: template.ret_type.clone(),
                body: Self::substitute_function_param_block(&template.body, &replacements),
                span: template.span,
            });
        }
        Ok(out)
    }
    // @pinker-nav:end parser.callbacks.instanciacao-estatica

    // @pinker-nav:start parser.genericos.funcoes-instanciacao
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Materializa as funções genéricas solicitadas durante o parsing: localiza o template (erro `Parse` se ausente), confere a aridade dos argumentos de tipo, gera o nome monomórfico e deduplica por ele, monta a tabela de substituições e substitui tipos de parâmetros, tipo de retorno e corpo, produzindo `FunctionDecl` concretos sem parâmetros de tipo. Não valida semanticamente nem anexa ao `Program`.
    fn instantiate_generic_functions(&self) -> Result<Vec<FunctionDecl>, PinkerError> {
        let mut out = Vec::new();
        let mut emitted = HashSet::new();
        for instantiation in &self.generic_instantiations {
            let Some(template) = self.generic_templates.get(&instantiation.name) else {
                return Err(PinkerError::Parse {
                    msg: format!("função genérica '{}' não declarada", instantiation.name),
                    span: instantiation.span,
                });
            };
            if template.type_params.len() != instantiation.type_args.len() {
                return Err(PinkerError::Parse {
                    msg: format!(
                        "função genérica '{}' exige {} argumento(s) de tipo, recebido {}",
                        instantiation.name,
                        template.type_params.len(),
                        instantiation.type_args.len()
                    ),
                    span: instantiation.span,
                });
            }
            let mono_name =
                self.generic_function_name(&instantiation.name, &instantiation.type_args);
            if !emitted.insert(mono_name.clone()) {
                continue;
            }
            let substitutions = template
                .type_params
                .iter()
                .cloned()
                .zip(instantiation.type_args.iter().cloned())
                .collect::<HashMap<_, _>>();
            out.push(FunctionDecl {
                name: mono_name,
                impl_facts: None,
                type_params: Vec::new(),
                params: template
                    .params
                    .iter()
                    .map(|param| Param {
                        name: param.name.clone(),
                        ty: Self::substitute_type(&param.ty, &substitutions),
                        span: param.span,
                    })
                    .collect(),
                ret_type: template
                    .ret_type
                    .as_ref()
                    .map(|ty| Self::substitute_type(ty, &substitutions)),
                body: Self::substitute_block(&template.body, &substitutions),
                span: template.span,
            });
        }
        Ok(out)
    }
    // @pinker-nav:end parser.genericos.funcoes-instanciacao

    // @pinker-nav:start parser.genericos.leques-instanciacao
    // @pinker-nav:domain genericos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Percorre as solicitações de leque genérico registradas, localiza cada template (erro `Parse` se ausente), gera o nome monomórfico e deduplica por ele, e delega a `instantiate_generic_enum_decl` a construção da declaração especializada, produzindo os `EnumDecl` concretos que a passagem de entrada anexa ao `Program`. Não valida semanticamente.
    fn instantiate_generic_enums(&self) -> Result<Vec<EnumDecl>, PinkerError> {
        let mut out = Vec::new();
        let mut emitted = HashSet::new();
        for instantiation in &self.enum_generic_instantiations {
            let Some(template) = self.enum_generic_templates.get(&instantiation.name) else {
                return Err(PinkerError::Parse {
                    msg: format!("leque genérico '{}' não declarado", instantiation.name),
                    span: instantiation.span,
                });
            };
            let mono_name = self.generic_enum_name(&instantiation.name, &instantiation.type_args);
            if !emitted.insert(mono_name) {
                continue;
            }
            out.push(self.instantiate_generic_enum_decl(
                template,
                &instantiation.type_args,
                instantiation.span,
            )?);
        }
        Ok(out)
    }
    // @pinker-nav:end parser.genericos.leques-instanciacao

    // @pinker-nav:start parser.constantes.declaracao
    // @pinker-nav:domain constantes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Declaração de constante global: reconhece `eterno nome: tipo = expr;` e produz o nó `ast::ConstDecl`, consumindo tipo e expressão inicial pela gramática comum.
    fn parse_const(&mut self) -> Result<ConstDecl, PinkerError> {
        let start_span = self.previous().span;
        let name = self
            .consume(TokenKind::Ident, "nome da constante")?
            .lexeme
            .clone();
        self.consume(TokenKind::Colon, ":")?;
        let ty = self.parse_type()?;
        self.consume(TokenKind::Eq, "=")?;
        let init = self.parse_expr()?;
        self.consume(TokenKind::Semi, ";")?;

        Ok(ConstDecl {
            name,
            ty,
            init,
            span: merge_span(start_span, self.previous().span),
        })
    }

    // @pinker-nav:end parser.constantes.declaracao

    // @pinker-nav:start parser.comandos.bloco
    // @pinker-nav:domain comandos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Blocos e comandos: reconhece `{ ... }`, declarações locais (`nova`/`muda`), atribuições, `mimo` (retorno), `talvez`/`senao`, laços (`sempre`/`repetir`), `quebrar`/`continuar`, `falar` e asm inline, produzindo `ast::Block`/`ast::Stmt`.
    fn parse_block(&mut self) -> Result<Block, PinkerError> {
        let start_span = self.consume(TokenKind::LBrace, "{")?.span;
        self.function_value_scopes.push(HashMap::new());
        self.value_type_scopes.push(HashMap::new());
        // Parte G: o bloco é a unidade de escopo léxico das ligações locais, e
        // o corpo vive numa função à parte para que a pilha desempilhe também
        // no caminho de erro — simetria que `value_type_scopes` já tinha.
        self.abrir_escopo_local();
        let corpo = self.parse_block_stmts();
        self.fechar_escopo_local();
        self.value_type_scopes.pop();
        self.function_value_scopes.pop();
        let stmts = corpo?;
        Ok(Block {
            stmts,
            span: merge_span(start_span, self.previous().span),
        })
    }

    /// Comandos de um bloco, até o `}` que o fecha.
    ///
    /// Separado de `parse_block` só para que o escopo léxico da Parte G tenha
    /// um ponto único de fechamento.
    fn parse_block_stmts(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        let mut stmts = Vec::new();
        while !self.check(TokenKind::RBrace) && self.peek().is_some() {
            let ja_produzidos = stmts.len();
            if self.check(TokenKind::KwPara) {
                stmts.extend(self.parse_for_stmt_desugared()?);
            } else if self.check(TokenKind::KwEncaixe) {
                stmts.extend(self.parse_encaixe()?);
            } else if self.check(TokenKind::KwTentar) {
                stmts.extend(self.parse_tentar_desugared()?);
            } else if self.check(TokenKind::KwPropagar) {
                stmts.extend(self.parse_propagar_desugared()?);
            } else if self.starts_function_value_let() {
                if let Some(stmt) = self.parse_function_value_let()? {
                    stmts.push(stmt);
                }
            } else {
                let stmt = self.parse_stmt()?;
                if let Stmt::Let(let_stmt) = &stmt {
                    self.current_function_value_scope_mut()
                        .remove(&let_stmt.name);
                }
                stmts.push(stmt);
            }
            // Parte G: o que este comando acabou de ligar passa a ser visível
            // do ponto seguinte em diante, e só dentro deste bloco. Ler o
            // resultado do parser cobre `nova`/`muda`, alias de função e todo
            // local nascido de desugaring sem repetir a lista de construtos.
            self.registrar_ligacoes_de_stmts(&stmts[ja_produzidos..]);
        }
        self.consume(TokenKind::RBrace, "}")?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, PinkerError> {
        if self.match_token(TokenKind::KwNova) {
            let start_span = self.previous().span;
            let is_mut = self.match_token(TokenKind::KwMuda);
            let name = self
                .consume(TokenKind::Ident, "nome da variável")?
                .lexeme
                .clone();
            let ty = if self.match_token(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };
            // Registra tipo de coleção para dispatch em `para cada`.
            if let Some(declared_ty) = &ty {
                match declared_ty {
                    Type::ListBombom(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::ListBombom);
                    }
                    Type::ListVerso(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::ListVerso);
                    }
                    Type::ListEnum { element, .. } => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::ListEnum(element.clone()));
                    }
                    Type::MapVersoBombom(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapVersoBombom);
                    }
                    Type::MapVersoVerso(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapVersoVerso);
                    }
                    Type::MapBombomBombom(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapBombomBombom);
                    }
                    Type::MapBombomVerso(_) => {
                        self.collection_types
                            .insert(name.clone(), CollectionKind::MapBombomVerso);
                    }
                    Type::Map { key, .. } => {
                        self.collection_types.insert(
                            name.clone(),
                            CollectionKind::Map {
                                key: key.as_ref().clone(),
                            },
                        );
                    }
                    _ => {}
                }
            }
            self.consume(TokenKind::Eq, "=")?;
            self.declarando.push(name.clone());
            let init = self.parse_expr();
            self.declarando.pop();
            let init = init?;
            self.consume(TokenKind::Semi, ";")?;
            if let Some(value_ty) = ty.clone().or_else(|| self.infer_local_expr_type(&init)) {
                self.register_value_type(&name, value_ty);
            }
            return Ok(Stmt::Let(LetStmt {
                name,
                is_mut,
                ty,
                init,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwMimo) {
            let start_span = self.previous().span;
            let expr = if self.check(TokenKind::Semi) {
                None
            } else {
                Some(self.parse_expr()?)
            };
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Return(ReturnStmt {
                expr,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwQuebrar) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Break(BreakStmt {
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwContinuar) {
            let start_span = self.previous().span;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Continue(ContinueStmt {
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwFalar) {
            let start_span = self.previous().span;
            self.consume(TokenKind::LParen, "(")?;
            let mut args = vec![self.parse_expr()?];
            while self.match_token(TokenKind::Comma) {
                args.push(self.parse_expr()?);
            }
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Falar(FalarStmt {
                args,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwSussurro) {
            let start_span = self.previous().span;
            self.consume(TokenKind::LParen, "(")?;
            let mut chunks = Vec::new();
            let first_chunk = self.consume(
                TokenKind::StringLit,
                "string literal em sussurro (ex.: \"mov rax, 60\")",
            )?;
            chunks.push(first_chunk.lexeme.clone());
            while self.check(TokenKind::Comma) && self.check_at(1, TokenKind::StringLit) {
                self.advance();
                let chunk = self
                    .consume(TokenKind::StringLit, "string literal em sussurro")?
                    .clone();
                chunks.push(chunk.lexeme);
            }

            let mut operands = Vec::new();
            let mut clobbers = Vec::new();
            if self.match_token(TokenKind::Semi) {
                while !self.check(TokenKind::RParen) {
                    let clause = self
                        .consume(
                            TokenKind::Ident,
                            "'entrada', 'saida' ou 'destroi' em sussurro",
                        )?
                        .clone();
                    if clause.lexeme == "destroi" {
                        self.consume(TokenKind::LParen, "(")?;
                        if !self.check(TokenKind::RParen) {
                            loop {
                                let clobber = self
                                    .consume(TokenKind::Ident, "efeito destruído por sussurro")?
                                    .clone();
                                clobbers.push(InlineAsmClobber {
                                    name: clobber.lexeme,
                                    span: clobber.span,
                                });
                                if !self.match_token(TokenKind::Comma) {
                                    break;
                                }
                            }
                        }
                        self.consume(TokenKind::RParen, ")")?;
                    } else {
                        let direction = match clause.lexeme.as_str() {
                            "entrada" => InlineAsmDirection::Input,
                            "saida" => InlineAsmDirection::Output,
                            _ => InlineAsmDirection::Unknown(clause.lexeme.clone()),
                        };
                        let name = self
                            .consume(TokenKind::Ident, "nome do operando de sussurro")?
                            .clone();
                        self.consume(TokenKind::Colon, ":")?;
                        let constraint = self
                            .consume(TokenKind::Ident, "constraint do operando de sussurro")?
                            .clone();
                        self.consume(TokenKind::Eq, "=")?;
                        let value = self.parse_expr()?;
                        operands.push(InlineAsmOperand {
                            name: name.lexeme,
                            direction,
                            constraint: constraint.lexeme,
                            span: merge_span(clause.span, value.span),
                            value,
                        });
                    }
                    if !self.match_token(TokenKind::Semi) {
                        break;
                    }
                }
            }
            self.consume(TokenKind::RParen, ")")?;
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::InlineAsm(InlineAsmStmt {
                chunks,
                operands,
                clobbers,
                span: merge_span(start_span, self.previous().span),
            }));
        }

        if self.match_token(TokenKind::KwSempre) {
            let start_span = self.previous().span;
            self.consume(TokenKind::KwQue, "que")?;
            let condition = self.parse_expr()?;
            let body = self.parse_block()?;
            let span = merge_span(start_span, body.span);
            return Ok(Stmt::While(WhileStmt {
                condition,
                body,
                span,
            }));
        }

        if self.match_token(TokenKind::KwRepetir) {
            let start_span = self.previous().span;
            let body = self.parse_block()?;
            self.consume(TokenKind::KwAte, "ate")?;
            let condition = self.parse_expr()?;
            self.consume(TokenKind::Semi, ";")?;
            let loop_span = merge_span(start_span, self.previous().span);
            let break_stmt = Stmt::If(IfStmt {
                condition,
                then_branch: Block {
                    stmts: vec![Stmt::Break(BreakStmt { span: loop_span })],
                    span: loop_span,
                },
                else_branch: None,
                span: loop_span,
            });
            let mut while_body = body.stmts;
            while_body.push(break_stmt);
            return Ok(Stmt::While(WhileStmt {
                condition: Expr {
                    kind: ExprKind::BoolLit(true),
                    span: loop_span,
                },
                body: Block {
                    stmts: while_body,
                    span: loop_span,
                },
                span: loop_span,
            }));
        }

        if self.match_token(TokenKind::KwEscolha) {
            let start_span = self.previous().span;
            let scrutinee = self.parse_expr()?;
            self.consume(TokenKind::LBrace, "{")?;

            let mut cases: Vec<(Expr, Block)> = Vec::new();
            let mut default_block: Option<Block> = None;

            while !self.check(TokenKind::RBrace) && self.peek().is_some() {
                if self.match_token(TokenKind::KwCaso) {
                    let pattern = self.parse_expr()?;
                    let body = self.parse_block()?;
                    cases.push((pattern, body));
                } else if self.match_token(TokenKind::KwSenao) {
                    default_block = Some(self.parse_block()?);
                    break;
                } else {
                    return Err(PinkerError::Parse {
                        msg: "esperado 'caso' ou 'senao' dentro de 'escolha'".to_string(),
                        span: self.peek_span(),
                    });
                }
            }
            self.consume(TokenKind::RBrace, "}")?;
            let end_span = self.previous().span;

            let mut result: Option<Stmt> = default_block.map(|blk| {
                Stmt::If(IfStmt {
                    condition: Expr {
                        kind: ExprKind::BoolLit(true),
                        span: blk.span,
                    },
                    then_branch: blk.clone(),
                    else_branch: None,
                    span: blk.span,
                })
            });

            for (pattern, body) in cases.into_iter().rev() {
                let cond = Expr {
                    kind: ExprKind::Binary(
                        Box::new(scrutinee.clone()),
                        BinaryOp::Eq,
                        Box::new(pattern),
                    ),
                    span: body.span,
                };
                let else_branch = result.map(|stmt| match stmt {
                    Stmt::If(if_stmt) => ElseBlock::If(Box::new(if_stmt)),
                    _ => unreachable!(),
                });
                result = Some(Stmt::If(IfStmt {
                    condition: cond,
                    then_branch: body,
                    else_branch,
                    span: merge_span(start_span, end_span),
                }));
            }

            return Ok(result.unwrap_or_else(|| {
                Stmt::Expr(Expr {
                    kind: ExprKind::IntLit(0),
                    span: merge_span(start_span, end_span),
                })
            }));
        }

        if self.match_token(TokenKind::KwTalvez) {
            let start_span = self.previous().span;
            let condition = self.parse_expr()?;
            let then_branch = self.parse_block()?;
            let else_branch = if self.match_token(TokenKind::KwSenao) {
                if self.check(TokenKind::KwTalvez) {
                    let nested = self.parse_stmt()?;
                    match nested {
                        Stmt::If(if_stmt) => Some(ElseBlock::If(Box::new(if_stmt))),
                        _ => unreachable!("parse_stmt após 'senao talvez' deve retornar If"),
                    }
                } else {
                    Some(ElseBlock::Block(self.parse_block()?))
                }
            } else {
                None
            };

            let end_span = else_branch
                .as_ref()
                .map(ElseBlock::span)
                .unwrap_or(then_branch.span);

            return Ok(Stmt::If(IfStmt {
                condition,
                then_branch,
                else_branch,
                span: merge_span(start_span, end_span),
            }));
        }

        let expr = self.parse_expr()?;

        let compound_op = if self.match_token(TokenKind::PlusEq) {
            Some(BinaryOp::Add)
        } else if self.match_token(TokenKind::MinusEq) {
            Some(BinaryOp::Sub)
        } else if self.match_token(TokenKind::StarEq) {
            Some(BinaryOp::Mul)
        } else if self.match_token(TokenKind::SlashEq) {
            Some(BinaryOp::Div)
        } else if self.match_token(TokenKind::PercentEq) {
            Some(BinaryOp::Mod)
        } else {
            None
        };

        if compound_op.is_some() || self.match_token(TokenKind::Eq) {
            let target = match &expr.kind {
                ExprKind::Ident(name) => AssignTarget::Ident(name.clone()),
                ExprKind::Unary(UnaryOp::Deref, ptr_expr) => {
                    AssignTarget::Deref(Box::new((**ptr_expr).clone()))
                }
                ExprKind::FieldAccess { base, field } => AssignTarget::FieldDeref {
                    base: base.clone(),
                    field: field.clone(),
                },
                ExprKind::Index { base, index } => AssignTarget::Index {
                    base: base.clone(),
                    index: index.clone(),
                },
                _ => {
                    return Err(PinkerError::Parse {
                        msg: "atribuição inválida: o lado esquerdo deve ser um identificador, dereferência '*expr', acesso a campo '(*ptr).campo' ou indexação 'base[índice]'".to_string(),
                        span: expr.span,
                    });
                }
            };
            let rhs = self.parse_expr()?;
            let final_rhs = if let Some(op) = compound_op {
                Expr {
                    kind: ExprKind::Binary(Box::new(expr.clone()), op, Box::new(rhs)),
                    span: expr.span,
                }
            } else {
                rhs
            };
            self.consume(TokenKind::Semi, ";")?;
            return Ok(Stmt::Assign(AssignStmt {
                target,
                expr: final_rhs,
                span: merge_span(expr.span, self.previous().span),
            }));
        }

        self.consume(TokenKind::Semi, ";")?;
        Ok(Stmt::Expr(Expr {
            kind: expr.kind,
            span: merge_span(expr.span, self.previous().span),
        }))
    }

    // @pinker-nav:end parser.comandos.bloco

    // @pinker-nav:start parser.lacos.for-each
    // @pinker-nav:domain lacos
    // @pinker-nav:layer parser
    // @pinker-nav:summary Desugaring de `para cada X em COL { ... }`: reconhece a forma for-each e a reescreve em laço explícito com cursor/índice e chamadas de iteração conforme o tipo da coleção (listas e mapas, por chave/valor), produzindo `ast::Stmt`.
    fn parse_for_stmt_desugared(&mut self) -> Result<Vec<Stmt>, PinkerError> {
        let start_span = self.consume(TokenKind::KwPara, "para")?.span;
        if self.match_token(TokenKind::KwCada) {
            return self.parse_for_each_after_cada(start_span);
        }
        let var_name = self
            .consume(TokenKind::Ident, "variável do iterador em 'para'")?
            .lexeme
            .clone();
        // Parte G: a variável do laço precisa estar visível ANTES de o corpo
        // ser lido. O escopo é o bloco envolvente porque é exatamente lá que o
        // desugaring emite o `Stmt::Let` dela.
        self.registrar_ligacao_local(&var_name);
        self.consume(TokenKind::KwEm, "em")?;
        let start_expr = self.parse_expr()?;
        self.consume(TokenKind::DotDot, "..")?;
        let end_expr = self.parse_expr()?;
        let body = self.parse_block()?;
        let loop_span = merge_span(start_span, body.span);
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let limit_name = format!("__range_limite_{suffix}");

        let var_binding = Stmt::Let(LetStmt {
            name: var_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(loop_span)),
            init: start_expr,
            span: loop_span,
        });
        let limit_binding = Stmt::Let(LetStmt {
            name: limit_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(loop_span)),
            init: end_expr,
            span: loop_span,
        });
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(var_name.clone()),
                    span: loop_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(limit_name),
                    span: loop_span,
                }),
            ),
            span: loop_span,
        };
        let increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(var_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(var_name),
                        span: loop_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: loop_span,
                    }),
                ),
                span: loop_span,
            },
            span: loop_span,
        });
        let mut while_body = body.stmts;
        while_body.push(increment);
        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body,
                span: loop_span,
            },
            span: loop_span,
        });
        Ok(vec![var_binding, limit_binding, while_stmt])
    }

    fn parse_for_each_after_cada(&mut self, start_span: Span) -> Result<Vec<Stmt>, PinkerError> {
        let item_name = self
            .consume(TokenKind::Ident, "variável do item em 'para cada'")?
            .lexeme
            .clone();
        self.consume(TokenKind::KwEm, "em")?;
        let collection_expr = self.parse_expr()?;
        // Parte G: o item do laço vale só dentro do corpo — é lá que o
        // desugaring emite o `Stmt::Let` dele. O escopo extra existe para que a
        // ligação não escape para o resto do bloco envolvente.
        self.abrir_escopo_local();
        self.registrar_ligacao_local(&item_name);
        let body_result = self.parse_block();
        self.fechar_escopo_local();
        let body = body_result?;
        let loop_span = merge_span(start_span, body.span);

        let collection_kind = match &collection_expr.kind {
            ExprKind::Ident(name) => self.collection_types.get(name.as_str()).cloned(),
            _ => None,
        };

        match collection_kind {
            Some(CollectionKind::MapVersoBombom) => {
                self.desugar_for_each_map(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::MapVersoVerso) => {
                self.desugar_for_each_map_verso_verso(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::MapBombomBombom) => {
                self.desugar_for_each_map_bombom_bombom(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::MapBombomVerso) => {
                self.desugar_for_each_map_bombom_verso(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::Map { key }) => {
                self.desugar_for_each_map_generic(item_name, key, collection_expr, body, loop_span)
            }
            Some(CollectionKind::ListVerso) => {
                self.desugar_for_each_list_verso(item_name, collection_expr, body, loop_span)
            }
            Some(CollectionKind::ListEnum(element)) => self.desugar_for_each_list_enum(
                item_name,
                element,
                collection_expr,
                body,
                loop_span,
            ),
            _ => self.desugar_for_each_list(item_name, collection_expr, body, loop_span),
        }
    }

    /// Desugaring de `para cada item em lista<Leque>` — usa as intrínsecas
    /// genéricas de lista (Fase 211) e liga o item com o tipo do leque.
    fn desugar_for_each_list_enum(
        &mut self,
        item_name: String,
        element: String,
        list_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let list_slot_name = format!("__iter_lista_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let list_binding_stmt = Stmt::Let(LetStmt {
            name: list_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: list_expr,
            span: helper_span,
        });
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Ident("lista_tamanho".to_string()),
                            span: helper_span,
                        }),
                        vec![Expr {
                            kind: ExprKind::Ident(list_slot_name.clone()),
                            span: helper_span,
                        }],
                    ),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let item_binding = Stmt::Let(LetStmt {
            name: item_name,
            is_mut: false,
            ty: Some(Type::Alias {
                name: element,
                span: helper_span,
            }),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("lista_obter".to_string()),
                        span: helper_span,
                    }),
                    vec![
                        Expr {
                            kind: ExprKind::Ident(list_slot_name),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::Ident(index_slot_name.clone()),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(item_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![list_binding_stmt, index_binding_stmt, while_stmt])
    }

    /// Desugaring de `para cada item em lista<bombom>` — reutilizado da Fase 153.
    fn desugar_for_each_list(
        &mut self,
        item_name: String,
        list_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let list_slot_name = format!("__iter_lista_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let list_binding_stmt = Stmt::Let(LetStmt {
            name: list_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: list_expr,
            span: helper_span,
        });
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Ident("lista_bombom_tamanho".to_string()),
                            span: helper_span,
                        }),
                        vec![Expr {
                            kind: ExprKind::Ident(list_slot_name.clone()),
                            span: helper_span,
                        }],
                    ),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let item_binding = Stmt::Let(LetStmt {
            name: item_name,
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("lista_bombom_obter".to_string()),
                        span: helper_span,
                    }),
                    vec![
                        Expr {
                            kind: ExprKind::Ident(list_slot_name),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::Ident(index_slot_name.clone()),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(item_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![list_binding_stmt, index_binding_stmt, while_stmt])
    }

    fn desugar_for_each_list_verso(
        &mut self,
        item_name: String,
        list_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let list_slot_name = format!("__iter_lista_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let list_binding_stmt = Stmt::Let(LetStmt {
            name: list_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: list_expr,
            span: helper_span,
        });
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Call(
                        Box::new(Expr {
                            kind: ExprKind::Ident("lista_verso_tamanho".to_string()),
                            span: helper_span,
                        }),
                        vec![Expr {
                            kind: ExprKind::Ident(list_slot_name.clone()),
                            span: helper_span,
                        }],
                    ),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let item_binding = Stmt::Let(LetStmt {
            name: item_name,
            is_mut: false,
            ty: Some(Type::Verso(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("lista_verso_obter".to_string()),
                        span: helper_span,
                    }),
                    vec![
                        Expr {
                            kind: ExprKind::Ident(list_slot_name),
                            span: helper_span,
                        },
                        Expr {
                            kind: ExprKind::Ident(index_slot_name.clone()),
                            span: helper_span,
                        },
                    ],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(item_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![list_binding_stmt, index_binding_stmt, while_stmt])
    }

    /// Desugaring de `para cada chave em mapa<verso,bombom>` — Fase 155.
    ///
    /// Lowering auditável:
    /// ```text
    /// nova __iter_mapa_N    = mapa_expr;
    /// nova __iter_tamanho_N = mapa_verso_bombom_tamanho(__iter_mapa_N);
    /// nova __iter_cursor_N  = <cursor interno sobre snapshot de chaves>;
    /// nova muda __iter_indice_N: bombom = 0;
    /// enquanto __iter_indice_N < __iter_tamanho_N {
    ///     nova chave: verso = <próxima chave do cursor interno>;
    ///     __iter_indice_N = __iter_indice_N + 1;
    ///     <corpo>
    /// }
    /// ```
    fn desugar_for_each_map(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        // nova __iter_mapa_N = map_expr;
        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        // nova __iter_tamanho_N: bombom = mapa_verso_bombom_tamanho(__iter_mapa_N);
        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("mapa_verso_bombom_tamanho".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        // nova __iter_cursor_N: bombom = <cursor interno sobre snapshot de chaves>;
        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::InternalMapIterCreate(Box::new(Expr {
                    kind: ExprKind::Ident(map_slot_name.clone()),
                    span: helper_span,
                })),
                span: helper_span,
            },
            span: helper_span,
        });

        // nova muda __iter_indice_N: bombom = 0;
        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        // condição: __iter_indice_N < __iter_tamanho_N
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        // nova key_name: verso = <próxima chave do cursor interno>;
        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Verso(helper_span)),
            init: Expr {
                kind: ExprKind::InternalMapIterNextKey(Box::new(Expr {
                    kind: ExprKind::Ident(cursor_slot_name),
                    span: helper_span,
                })),
                span: helper_span,
            },
            span: helper_span,
        });

        // __iter_indice_N = __iter_indice_N + 1;
        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    /// Desugaring único para a autoridade adulta `mapa<K,V>`. O cursor e a
    /// operação de tamanho são independentes de V; apenas o tipo devolvido da
    /// chave acompanha K.
    fn desugar_for_each_map_generic(
        &mut self,
        key_name: String,
        key_ty: Type,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;
        let next_callee = if matches!(key_ty, Type::Verso(_)) {
            "__pinker_internal_mapa_iterador_proxima_chave_verso"
        } else {
            "__pinker_internal_mapa_iterador_proxima_chave_bombom"
        };

        let map_binding = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });
        let size_binding = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("mapa_tamanho".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let cursor_binding = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("__pinker_internal_mapa_iterador_criar".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let index_binding = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });
        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };
        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(key_ty),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(next_callee.to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });
        let mut while_body = Vec::with_capacity(2 + body.stmts.len());
        while_body.push(key_binding);
        while_body.push(increment);
        while_body.extend(body.stmts);
        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body,
                span: helper_span,
            },
            span: loop_span,
        });
        Ok(vec![
            map_binding,
            size_binding,
            cursor_binding,
            index_binding,
            while_stmt,
        ])
    }

    fn desugar_for_each_map_verso_verso(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("mapa_verso_verso_tamanho".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_verso_verso_iterador_criar".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Verso(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_verso_verso_iterador_proxima_chave".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    fn desugar_for_each_map_bombom_bombom(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("mapa_bombom_bombom_tamanho".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_bombom_iterador_criar".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_bombom_iterador_proxima_chave"
                                .to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    fn desugar_for_each_map_bombom_verso(
        &mut self,
        key_name: String,
        map_expr: Expr,
        body: Block,
        loop_span: Span,
    ) -> Result<Vec<Stmt>, PinkerError> {
        self.synthetic_counter += 1;
        let suffix = self.synthetic_counter;
        let map_slot_name = format!("__iter_mapa_{suffix}");
        let size_slot_name = format!("__iter_tamanho_{suffix}");
        let cursor_slot_name = format!("__iter_cursor_{suffix}");
        let index_slot_name = format!("__iter_indice_{suffix}");
        let helper_span = loop_span;

        let map_binding_stmt = Stmt::Let(LetStmt {
            name: map_slot_name.clone(),
            is_mut: false,
            ty: None,
            init: map_expr,
            span: helper_span,
        });

        let size_binding_stmt = Stmt::Let(LetStmt {
            name: size_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("mapa_bombom_verso_tamanho".to_string()),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let cursor_binding_stmt = Stmt::Let(LetStmt {
            name: cursor_slot_name.clone(),
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_verso_iterador_criar".to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(map_slot_name.clone()),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_binding_stmt = Stmt::Let(LetStmt {
            name: index_slot_name.clone(),
            is_mut: true,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::IntLit(0),
                span: helper_span,
            },
            span: helper_span,
        });

        let condition = Expr {
            kind: ExprKind::Binary(
                Box::new(Expr {
                    kind: ExprKind::Ident(index_slot_name.clone()),
                    span: helper_span,
                }),
                BinaryOp::Lt,
                Box::new(Expr {
                    kind: ExprKind::Ident(size_slot_name),
                    span: helper_span,
                }),
            ),
            span: helper_span,
        };

        let key_binding = Stmt::Let(LetStmt {
            name: key_name,
            is_mut: false,
            ty: Some(Type::Bombom(helper_span)),
            init: Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident(
                            "__pinker_internal_mapa_bombom_verso_iterador_proxima_chave"
                                .to_string(),
                        ),
                        span: helper_span,
                    }),
                    vec![Expr {
                        kind: ExprKind::Ident(cursor_slot_name),
                        span: helper_span,
                    }],
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let index_increment = Stmt::Assign(AssignStmt {
            target: AssignTarget::Ident(index_slot_name.clone()),
            expr: Expr {
                kind: ExprKind::Binary(
                    Box::new(Expr {
                        kind: ExprKind::Ident(index_slot_name),
                        span: helper_span,
                    }),
                    BinaryOp::Add,
                    Box::new(Expr {
                        kind: ExprKind::IntLit(1),
                        span: helper_span,
                    }),
                ),
                span: helper_span,
            },
            span: helper_span,
        });

        let mut while_body_stmts = Vec::with_capacity(2 + body.stmts.len());
        while_body_stmts.push(key_binding);
        while_body_stmts.push(index_increment);
        while_body_stmts.extend(body.stmts);

        let while_stmt = Stmt::While(WhileStmt {
            condition,
            body: Block {
                stmts: while_body_stmts,
                span: helper_span,
            },
            span: loop_span,
        });

        Ok(vec![
            map_binding_stmt,
            size_binding_stmt,
            cursor_binding_stmt,
            index_binding_stmt,
            while_stmt,
        ])
    }

    // @pinker-nav:end parser.lacos.for-each

    // @pinker-nav:start parser.expressoes.precedencia
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Escada de precedência e operadores: `parse_expr`/`parse_expr_binary` com climbing por precedência e associatividade, e `parse_expr_unary`, produzindo `ast::Expr` com `BinaryOp`/`UnaryOp`.
    fn parse_expr(&mut self) -> Result<Expr, PinkerError> {
        let expr = self.parse_expr_binary(0)?;
        if self.match_token(TokenKind::Question) {
            let then_expr = self.parse_expr()?;
            self.consume(TokenKind::Colon, ":")?;
            let else_expr = self.parse_expr()?;
            let span = merge_span(expr.span, else_expr.span);
            return Ok(Expr {
                kind: ExprKind::Call(
                    Box::new(Expr {
                        kind: ExprKind::Ident("__ternario".to_string()),
                        span,
                    }),
                    vec![expr, then_expr, else_expr],
                ),
                span,
            });
        }
        Ok(expr)
    }

    fn parse_expr_binary(&mut self, min_prec: u8) -> Result<Expr, PinkerError> {
        let mut lhs = self.parse_expr_unary()?;

        while let Some(token) = self.peek() {
            let op = match BinaryOp::from_token(token.kind) {
                Some(op) => op,
                None => break,
            };

            let prec = Self::precedence(op);
            if prec < min_prec {
                break;
            }

            self.advance();
            let rhs = self.parse_expr_binary(prec + 1)?;
            lhs = Expr {
                span: merge_span(lhs.span, rhs.span),
                kind: ExprKind::Binary(Box::new(lhs), op, Box::new(rhs)),
            };
        }

        Ok(lhs)
    }

    fn precedence(op: BinaryOp) -> u8 {
        match op {
            BinaryOp::LogicalOr => 1,
            BinaryOp::LogicalAnd => 2,
            BinaryOp::BitOr => 3,
            BinaryOp::BitXor => 4,
            BinaryOp::BitAnd => 5,
            BinaryOp::Eq
            | BinaryOp::Neq
            | BinaryOp::Lt
            | BinaryOp::Lte
            | BinaryOp::Gt
            | BinaryOp::Gte => 6,
            BinaryOp::Shl | BinaryOp::Shr => 7,
            BinaryOp::Add | BinaryOp::Sub => 8,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => 9,
        }
    }

    fn parse_expr_unary(&mut self) -> Result<Expr, PinkerError> {
        if let Some(token) = self.peek() {
            if token.kind == TokenKind::Minus
                || token.kind == TokenKind::Bang
                || token.kind == TokenKind::Star
                || token.kind == TokenKind::Amp
                || token.kind == TokenKind::Tilde
                || token.kind == TokenKind::KwNope
            {
                let op_span = token.span;
                let token_kind = token.kind;
                self.advance();
                let operand = if token_kind == TokenKind::Amp && self.check(TokenKind::Ident) {
                    let ident = self.advance().expect("identificador verificado").clone();
                    let mut name = ident.lexeme.clone();
                    let mut span = ident.span;
                    if self.match_token(TokenKind::Less) {
                        let mut type_args = Vec::new();
                        loop {
                            type_args.push(self.parse_type()?);
                            if !self.match_token(TokenKind::Comma) {
                                break;
                            }
                        }
                        self.consume(
                            TokenKind::Greater,
                            "> após argumentos de tipo no endereço de função",
                        )?;
                        let original_name = name.clone();
                        name = self.generic_function_name(&original_name, &type_args);
                        self.generic_instantiations.push(GenericInstantiation {
                            name: original_name,
                            type_args,
                            span: ident.span,
                        });
                        span = merge_span(span, self.previous().span);
                    }
                    Expr {
                        kind: ExprKind::Ident(name),
                        span,
                    }
                } else {
                    self.parse_expr_unary()?
                };
                if token_kind == TokenKind::Amp {
                    return Ok(Expr {
                        span: merge_span(op_span, operand.span),
                        kind: ExprKind::AddressOf(Box::new(operand)),
                    });
                }
                let unary_expr = Expr {
                    span: merge_span(op_span, operand.span),
                    kind: ExprKind::Unary(
                        if token_kind == TokenKind::Minus {
                            UnaryOp::Neg
                        } else if token_kind == TokenKind::Bang {
                            UnaryOp::Not
                        } else if token_kind == TokenKind::Tilde || token_kind == TokenKind::KwNope
                        {
                            UnaryOp::BitNot
                        } else {
                            UnaryOp::Deref
                        },
                        Box::new(operand),
                    ),
                };
                return self.parse_cast_suffix(unary_expr);
            }
        }

        let expr = self.parse_expr_primary()?;
        self.parse_cast_suffix(expr)
    }

    // @pinker-nav:end parser.expressoes.precedencia

    // @pinker-nav:start parser.expressoes.primarias
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Expressões primárias: literais, identificadores, agrupamento, listas/mapas e construção de struct/leque, produzindo o nó base `ast::Expr` antes da cadeia postfix.
    fn parse_expr_primary(&mut self) -> Result<Expr, PinkerError> {
        let eof_span = self.peek_span();
        let token = self
            .advance()
            .ok_or(PinkerError::Parse {
                msg: "fim inesperado da expressão".to_string(),
                span: eof_span,
            })?
            .clone();

        let base = match token.kind {
            TokenKind::IntLit => {
                let value = token
                    .lexeme
                    .parse::<u64>()
                    .map_err(|_| PinkerError::Parse {
                        msg: "literal inteiro fora da faixa de bombom/u64".to_string(),
                        span: token.span,
                    })?;
                Ok(Expr {
                    kind: ExprKind::IntLit(value),
                    span: token.span,
                })
            }
            TokenKind::KwVerdade => Ok(Expr {
                kind: ExprKind::BoolLit(true),
                span: token.span,
            }),
            TokenKind::KwFalso => Ok(Expr {
                kind: ExprKind::BoolLit(false),
                span: token.span,
            }),
            TokenKind::StringLit => Ok(Expr {
                kind: ExprKind::StringLit(token.lexeme.clone()),
                span: token.span,
            }),
            TokenKind::FStringLit => {
                let raw = token.lexeme.clone();
                let span = token.span;
                return self.desugar_fstring(&raw, span);
            }
            TokenKind::Ident => Ok(Expr {
                kind: ExprKind::Ident(token.lexeme.clone()),
                span: token.span,
            }),
            TokenKind::KwCarinho => self.parse_anonymous_function_expr(token.span),
            TokenKind::LParen => {
                let lparen_span = token.span;
                let expr = self.parse_expr()?;
                self.consume(TokenKind::RParen, ")")?;
                Ok(Expr {
                    kind: expr.kind,
                    span: merge_span(lparen_span, self.previous().span),
                })
            }
            TokenKind::KwPeso => {
                let start_span = token.span;
                self.consume(TokenKind::LParen, "(")?;
                let target = self.parse_type()?;
                self.consume(TokenKind::RParen, ")")?;
                Ok(Expr {
                    kind: ExprKind::SizeOfType { target },
                    span: merge_span(start_span, self.previous().span),
                })
            }
            TokenKind::KwAlinhamento => {
                let start_span = token.span;
                self.consume(TokenKind::LParen, "(")?;
                let target = self.parse_type()?;
                self.consume(TokenKind::RParen, ")")?;
                Ok(Expr {
                    kind: ExprKind::AlignOfType { target },
                    span: merge_span(start_span, self.previous().span),
                })
            }
            _ => Err(PinkerError::Parse {
                msg: format!("expressão inválida: '{}'", token.lexeme),
                span: token.span,
            }),
        }?;

        self.parse_postfix_suffix(base)
    }

    // @pinker-nav:end parser.expressoes.primarias

    // @pinker-nav:start parser.expressoes.postfix
    // @pinker-nav:domain expressoes
    // @pinker-nav:layer parser
    // @pinker-nav:summary Cadeia postfix de expressão: chamadas, acesso a campo, índice, chamada genérica explícita e sufixo de cast (`virar`), aplicados sobre a expressão base para produzir o `ast::Expr` final.
    fn parse_postfix_suffix(&mut self, mut expr: Expr) -> Result<Expr, PinkerError> {
        // #505: a grafia corrente veio da canonicalização de um membro de
        // módulo, e não do texto do usuário? É a única coisa que distingue
        // `arquivo.ler_bombom(...)` de alguém escrevendo `ler_arquivo(...)` a
        // seco — depois do CANONICALIZATION_BOUNDARY as duas são o mesmo
        // `Ident`.
        let mut canonicalizado = false;
        loop {
            if let Some(generic_call) = self.try_parse_explicit_generic_call(&expr)? {
                expr = generic_call;
                continue;
            }
            if self.match_token(TokenKind::LParen) {
                let mut args = Vec::new();
                if !self.check(TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenKind::RParen, ")")?;
                // Parte G — CANONICALIZATION_BOUNDARY (forma seletiva).
                // Vem antes da monomorfização genérica e antes de
                // `falha_operacional::superficie`, de modo que a materialização
                // de `Resultado<T,E>` já enxergue a identidade canônica.
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(canonica) = self.resolver_membro_seletivo(name) {
                        canonicalizado = true;
                        expr = Expr {
                            kind: ExprKind::Ident(canonica.to_string()),
                            span: expr.span,
                        };
                    }
                }
                // #505 — remoção da superfície global. Vem depois das duas
                // formas de import e antes de qualquer outra reescrita, de
                // modo que só chegue aqui grafia que o usuário escreveu sem
                // trazer nada.
                if !canonicalizado {
                    if let ExprKind::Ident(name) = &expr.kind {
                        self.recusar_intrinseca_sem_import(name, expr.span)?;
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(template) = self.generic_templates.get(name).cloned() {
                        let type_args =
                            self.infer_generic_call_type_args(&template, &args, expr.span)?;
                        let mono_name = self.generic_function_name(name, &type_args);
                        self.generic_instantiations.push(GenericInstantiation {
                            name: name.clone(),
                            type_args,
                            span: expr.span,
                        });
                        expr = Expr {
                            kind: ExprKind::Ident(mono_name),
                            span: expr.span,
                        };
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    // Parte B: uma chamada a superfície falível materializa a
                    // especialização de `Resultado<T,E>` que ela devolve, pelo
                    // mesmo caminho de monomorfização que o usuário dispararia
                    // ao escrever o tipo. Materialização de tipo — não
                    // semântica de propagação, que continua cega à origem.
                    if let Some(superficie) = crate::falha_operacional::superficie(name) {
                        let span = expr.span;
                        self.registrar_resultado_falivel(superficie, span)?;
                    }
                    // Parte E1: `json_tipo` devolve o leque de classificação.
                    // Mesma regra das cargas da Parte C — o leque entra no
                    // programa quando a superfície que o produz é realmente
                    // chamada, e não porque existe um template predeclarado.
                    if name == crate::valor_json::intrinsecas::TIPO {
                        self.registrar_leque_predeclarado(crate::valor_json::LEQUE_TIPO_JSON);
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    let mut bindings = Vec::new();
                    let mut runtime_args = Vec::new();
                    for (index, arg) in args.into_iter().enumerate() {
                        let static_function = match &arg.kind {
                            ExprKind::Ident(arg_name) => self
                                .resolve_function_value_alias(arg_name)
                                .filter(|resolved| resolved != arg_name)
                                .or_else(|| {
                                    (arg_name.starts_with("__anon_carinho_")
                                        && !self.capturing_anon_functions.contains(arg_name))
                                    .then(|| arg_name.clone())
                                }),
                            _ => None,
                        };
                        if let Some(function_name) = static_function {
                            bindings.push(FunctionParamBinding {
                                index,
                                function_name,
                                span: arg.span,
                            });
                        } else {
                            runtime_args.push(arg);
                        }
                    }
                    if !bindings.is_empty() {
                        let mono_name = Self::function_param_specialization_name(name, &bindings);
                        self.function_param_instantiations
                            .push(FunctionParamInstantiation {
                                name: name.clone(),
                                bindings,
                                span: expr.span,
                            });
                        expr = Expr {
                            kind: ExprKind::Ident(mono_name),
                            span: expr.span,
                        };
                        args = runtime_args;
                    } else {
                        args = runtime_args;
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(function_name) = self.resolve_function_value_alias(name) {
                        expr = Expr {
                            kind: ExprKind::Ident(function_name),
                            span: expr.span,
                        };
                    }
                }
                if let ExprKind::Ident(name) = &expr.kind {
                    if let Some(first_arg) = args.first() {
                        if let ExprKind::Ident(map_name) = &first_arg.kind {
                            if let Some(kind) = self.collection_types.get(map_name.as_str()) {
                                if let Some(mono_name) = kind.generic_map_callee(name) {
                                    expr = Expr {
                                        kind: ExprKind::Ident(mono_name.to_string()),
                                        span: expr.span,
                                    };
                                }
                            }
                        }
                    }
                }
                expr = Expr {
                    span: merge_span(expr.span, self.previous().span),
                    kind: ExprKind::Call(Box::new(expr), args),
                };
                continue;
            }
            if self.match_token(TokenKind::Dot) {
                let base_expr = expr;
                let field_token = self
                    .consume(TokenKind::Ident, "nome do campo após '.'")?
                    .clone();
                let field = field_token.lexeme.clone();
                // Parte G — CANONICALIZATION_BOUNDARY (forma qualificada).
                // `familia.membro` vira a identidade executiva ANTES de existir
                // como `FieldAccess`. Nada a jusante — nem o resto deste laço —
                // vê família ou membro.
                if let Some(canonica) = self.resolver_membro_de_familia(&base_expr, &field)? {
                    canonicalizado = true;
                    expr = Expr {
                        kind: ExprKind::Ident(canonica.to_string()),
                        span: merge_span(base_expr.span, field_token.span),
                    };
                    continue;
                }
                canonicalizado = false;
                expr = Expr {
                    span: merge_span(base_expr.span, field_token.span),
                    kind: ExprKind::FieldAccess {
                        base: Box::new(base_expr),
                        field,
                    },
                };
                continue;
            }
            if self.match_token(TokenKind::LBracket) {
                let index = self.parse_expr()?;
                self.consume(TokenKind::RBracket, "]")?;
                expr = Expr {
                    span: merge_span(expr.span, self.previous().span),
                    kind: ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    },
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn try_parse_explicit_generic_call(
        &mut self,
        expr: &Expr,
    ) -> Result<Option<Expr>, PinkerError> {
        let ExprKind::Ident(name) = &expr.kind else {
            return Ok(None);
        };
        if !self.check(TokenKind::Less) {
            return Ok(None);
        }
        let saved = self.current;
        self.advance();
        let mut type_args = Vec::new();
        loop {
            match self.parse_type() {
                Ok(ty) => type_args.push(ty),
                Err(_) => {
                    self.current = saved;
                    return Ok(None);
                }
            }
            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }
        if !self.match_token(TokenKind::Greater) || !self.match_token(TokenKind::LParen) {
            self.current = saved;
            return Ok(None);
        }

        let mut args = Vec::new();
        if !self.check(TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if !self.match_token(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, ")")?;
        let mono_name = self.generic_function_name(name, &type_args);
        self.generic_instantiations.push(GenericInstantiation {
            name: name.clone(),
            type_args,
            span: expr.span,
        });
        let callee = Expr {
            kind: ExprKind::Ident(mono_name),
            span: expr.span,
        };
        Ok(Some(Expr {
            span: merge_span(expr.span, self.previous().span),
            kind: ExprKind::Call(Box::new(callee), args),
        }))
    }

    fn parse_cast_suffix(&mut self, mut expr: Expr) -> Result<Expr, PinkerError> {
        while self.match_token(TokenKind::KwVirar) {
            let target = self.parse_type()?;
            expr = Expr {
                span: merge_span(expr.span, target.span()),
                kind: ExprKind::Cast {
                    expr: Box::new(expr),
                    target,
                },
            };
        }
        Ok(expr)
    }

    // @pinker-nav:end parser.expressoes.postfix

    // @pinker-nav:start parser.texto.interpolacao
    // @pinker-nav:domain texto
    // @pinker-nav:layer parser
    // @pinker-nav:summary Desugaring de strings interpoladas `$"..."`: reconhece os segmentos de texto e `{expr}`, parseia cada expressão embutida e produz uma chamada a `formatar_verso` — um `ast::Expr`.
    fn desugar_fstring(&mut self, raw: &str, span: Span) -> Result<Expr, PinkerError> {
        let mut template = String::new();
        let mut expr_sources: Vec<String> = Vec::new();
        let mut chars = raw.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' {
                let mut depth = 1u32;
                let mut expr_str = String::new();
                for inner in chars.by_ref() {
                    if inner == '{' {
                        depth += 1;
                    } else if inner == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    expr_str.push(inner);
                }
                if depth != 0 {
                    return Err(PinkerError::Parse {
                        msg: "'}' não encontrado em string interpolada".to_string(),
                        span,
                    });
                }
                template.push_str("{}");
                expr_sources.push(expr_str);
            } else {
                template.push(c);
            }
        }

        if expr_sources.is_empty() {
            return Ok(Expr {
                kind: ExprKind::StringLit(template),
                span,
            });
        }

        let mut call_args = vec![Expr {
            kind: ExprKind::StringLit(template),
            span,
        }];

        for src in &expr_sources {
            let mut lexer = Lexer::new(src);
            let tokens = lexer.tokenize().map_err(|e| PinkerError::Parse {
                msg: format!("erro ao lexar expressão em string interpolada: {}", e),
                span,
            })?;
            let mut sub_parser = Parser::new(tokens);
            let expr = sub_parser.parse_expr().map_err(|e| PinkerError::Parse {
                msg: format!("erro ao parsear expressão em string interpolada: {}", e),
                span,
            })?;
            call_args.push(expr);
        }

        Ok(Expr {
            kind: ExprKind::Call(
                Box::new(Expr {
                    kind: ExprKind::Ident("formatar_verso".to_string()),
                    span,
                }),
                call_args,
            ),
            span,
        })
    }
    // @pinker-nav:end parser.texto.interpolacao
}
