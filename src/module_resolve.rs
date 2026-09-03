//! Ambiente de import explícito e resolução nominal canônica por unidade.
//!
//! Depois que a composição virou um `ModuleGraph`, cada unidade ainda referia
//! entidades de topo pela grafia. Grafia é global; identidade não é. Enquanto a
//! resolução fosse feita por grafia sobre a agregação, o significado do corpo
//! de um módulo dependia do que a raiz — ou um módulo irmão — tivesse
//! materializado por acaso.
//!
//! Esta camada faz duas coisas, nessa ordem:
//!
//! 1. **Ambiente de import explícito.** Para cada unidade, monta o conjunto de
//!    ligações que ela autorizou: as próprias declarações de topo e os imports
//!    que ela escreveu. Nada mais entra. Um módulo irmão não entra porque
//!    existe, e a raiz não entra porque é a raiz.
//!
//! 2. **Resolução nominal canônica.** Reescreve declarações e referências para
//!    o nome canônico da unidade de origem. A raiz preserva a grafia — é o
//!    programa que executa, e nenhum símbolo de runtime muda de nome porque a
//!    identidade de frontend passou a existir. Um módulo qualifica pela própria
//!    chave, que já era a forma dos tipos qualificados.
//!
//! ```text
//! SOURCE SPELLING != SYMBOL IDENTITY
//! IMPORTER SURFACE VISIBILITY != MODULE IMPLEMENTATION DEPENDENCY ENVIRONMENT
//! ```
//!
//! O que NÃO é reescrito, e por quê:
//!
//! - namespaces possuídos pelo compilador (`__anon_carinho_*`, `__gen_*`, ...):
//!   já possuem identidade injetiva derivada de `SourceOrigin`. Requalificá-los
//!   quebraria a autoridade que já os distingue;
//! - intrínsecas públicas e membros de família: são superfície global aprovada,
//!   não entidade de unidade;
//! - locais, parâmetros, bindings de padrão e parâmetros de tipo: têm escopo, e
//!   escopo é decidido no ponto de uso.
//!
//! Uma referência livre que a unidade não autorizou e que existe em OUTRA
//! unidade não é reescrita nem silenciosamente religada: ela é recusada aqui,
//! com o span e a fonte da unidade que a escreveu. É a diferença entre "este
//! nome não existe" e "este nome existe, em outro lugar, e você não o pediu".

use std::collections::{HashMap, HashSet};

use crate::ast::{
    AssignTarget, Block, ConstDecl, ElseBlock, EnumDecl, EnumPattern, Expr, ExprKind, FunctionDecl,
    IfStmt, ImplDecl, Item, Param, Program, Stmt, StructDecl, TraitDecl, TraitMethodSig, Type,
    TypeAliasDecl,
};
use crate::error::PinkerError;
use crate::module_graph::{ModuleGraph, ModuleId, ModuleKey, ModuleUnit};
use crate::source_map::SourceId;
use crate::token::Span;

// @pinker-nav:start modulos.ambiente.import-explicito
// @pinker-nav:domain modulos
// @pinker-nav:layer compilador
// @pinker-nav:summary ModuleEnvironment é o conjunto de ligações que uma unidade autorizou — as próprias declarações de topo mais exclusivamente os imports que ela escreveu — e é a única fonte de resolução do corpo dessa unidade; ambientes_do_grafo monta um ambiente por unidade a partir do grafo, recusando import seletivo de símbolo inexistente e sem jamais herdar as dependências internas do módulo importado para o importador, que é o que separa superfície visível ao importador de ambiente de implementação do módulo.
/// Ligações autorizadas por uma unidade.
#[derive(Debug, Clone, Default)]
pub struct ModuleEnvironment {
    /// grafia -> símbolo canônico.
    bindings: HashMap<String, String>,
    /// Grafias trazidas por import explícito, com o span do import que as
    /// trouxe. Serve ao diagnóstico: um binding declarado é diferente de um
    /// binding que a unidade só possui porque declarou a entidade.
    imported_spans: HashMap<String, Span>,
    /// Módulos trazidos INTEIROS por esta unidade.
    ///
    /// Import inteiro autoriza a superfície do módulo, então a forma
    /// qualificada `<módulo>.<membro>` pode consultar qualquer membro dele — a
    /// existência do membro é pergunta da autoridade semântica, não desta
    /// camada. Import seletivo autoriza um símbolo, e só ele.
    modulos_inteiros: HashSet<String>,
}

impl ModuleEnvironment {
    pub fn lookup(&self, spelling: &str) -> Option<&str> {
        self.bindings.get(spelling).map(String::as_str)
    }

    /// O nome canônico está autorizado por esta unidade?
    ///
    /// A forma qualificada `<módulo>.<Tipo>` é escrita direto no texto e não
    /// passa por grafia: sem esta pergunta, ela alcançaria qualquer unidade
    /// carregada, importada ou não.
    pub fn autoriza_canonico(&self, canonical: &str) -> bool {
        self.bindings.values().any(|valor| valor == canonical)
    }

    pub fn importou_inteiro(&self, module_key: &str) -> bool {
        self.modulos_inteiros.contains(module_key)
    }

    pub fn is_imported(&self, spelling: &str) -> bool {
        self.imported_spans.contains_key(spelling)
    }

    fn declare(&mut self, spelling: &str, canonical: String) {
        self.bindings.insert(spelling.to_string(), canonical);
    }

    fn import(&mut self, spelling: &str, canonical: String, span: Span) {
        self.bindings.insert(spelling.to_string(), canonical);
        self.imported_spans.insert(spelling.to_string(), span);
    }
}

/// Nome de topo importável de um item, se houver.
pub fn importable_item_name(item: &Item) -> Option<&str> {
    match item {
        Item::Function(function) => Some(function.name.as_str()),
        Item::Const(constant) => Some(constant.name.as_str()),
        Item::Struct(struct_decl) => Some(struct_decl.name.as_str()),
        Item::TypeAlias(alias) => Some(alias.name.as_str()),
        Item::Enum(enum_decl) => Some(enum_decl.name.as_str()),
        Item::Trait(trait_decl) => Some(trait_decl.name.as_str()),
    }
}

fn set_item_name(item: &mut Item, name: String) {
    match item {
        Item::Function(function) => function.name = name,
        Item::Const(constant) => constant.name = name,
        Item::Struct(struct_decl) => struct_decl.name = name,
        Item::TypeAlias(alias) => alias.name = name,
        Item::Enum(enum_decl) => enum_decl.name = name,
        Item::Trait(trait_decl) => trait_decl.name = name,
    }
}

/// O nome NÃO é entidade de uma unidade-fonte.
///
/// Duas categorias, cada uma com sua autoridade única:
///
/// - **identidade gerada** (`__anon_carinho_*`, `__gen_*`, `__impl_*`, ...):
///   já carrega a proveniência na própria identidade. Requalificá-la
///   substituiria a autoridade que a distingue por outra, mais fraca. A
///   pergunta é de `native_symbol::is_compiler_generated`, NÃO de
///   `starts_with("__")` — o superprefixo não pertence à Pinker, e `__usuario`
///   é identificador de usuário legal;
/// - **identidade reservada do runtime** (`TipoEntrada`, `LimiteTempo`,
///   `TipoJson`, ...): o parser as materializa como `Item::Enum` comum em
///   qualquer unidade que as mencione. São superfície do runtime, não
///   declaração de quem as mencionou; canonizá-las faria a cópia de um módulo
///   virar `M.LimiteTempo` enquanto a superfície continua devolvendo
///   `LimiteTempo`, e a mesma fonte aceita como raiz seria recusada como
///   módulo.
fn nao_e_entidade_de_unidade(name: &str) -> bool {
    crate::native_symbol::is_compiler_generated(name)
        || crate::runtime_identity::runtime_reserved_identity(name).is_some()
}

/// A identidade gerada é endereçada pelo próprio conteúdo?
///
/// `__gen_*` (especialização genérica) e `__anon_carinho_*` (callable anônimo)
/// codificam integralmente a identidade que os produziu: nomes iguais provam
/// entidades iguais, e materializar as duas cópias duplicaria símbolo de
/// runtime sem criar entidade nova.
///
/// `__trait_default_check_*` pertence, depois que a recomposição o endereça
/// pelo trato canônico. O que prova a igualdade é a origem, não a impressão:
/// o nome é `(trato canônico, alvo canônico, método)`, o corpo default tem uma
/// declaração só — a do trato —, e desde a #517 ele é resolvido no ambiente da
/// unidade que DECLAROU o trato, nunca no de quem hospeda o `impl`. Logo nome
/// igual implica mesma declaração e mesmo corpo, e as duas cópias são a mesma
/// entidade. A conferência estrutural que este caminho aplica em seguida é de
/// assinatura, não de corpo: ela é rede de segurança, não a prova. Sem esta
/// entrada, duas unidades que sobrescrevem a mesma relação emitiriam duas
/// funções homônimas e a duplicata seria recusada por choque de nome de função
/// sintética, no lugar da autoridade de contratos de trato.
///
/// `__impl_*` NÃO pertence a este conjunto. Ele codifica apenas
/// `(trato, alvo, método)`; dois corpos genuinamente distintos da mesma relação
/// produzem o mesmo nome. Deduplicá-lo descartaria uma implementação em
/// silêncio, exatamente onde a autoridade de contratos de trato precisa ver as
/// duas para recusar a duplicata.
fn e_identidade_endereçada_por_conteudo(name: &str) -> bool {
    name.starts_with("__gen_")
        || name.starts_with(crate::anonymous_identity::ANONYMOUS_CALLABLE_PREFIX)
        || name.starts_with(crate::method_identity::TRAIT_DEFAULT_CHECK_PREFIX)
}

/// Superfície global aprovada: intrínseca pública ou forma qualificada de
/// família. Nenhuma das duas pertence a uma unidade-fonte.
fn e_superficie_global(name: &str) -> bool {
    if crate::intrinsic_authority::e_grafia_builtin_chamavel(name) {
        return true;
    }
    match name.split_once('.') {
        Some((familia, membro)) => {
            crate::familia_superficie::forma_qualificada_valida(familia, membro)
        }
        None => false,
    }
}

/// Import que a superfície de famílias built-in atende e o carregador não
/// resolve como arquivo. A precedência é a mesma que o carregador aplica.
///
/// #532: a forma sintática deixou de participar da resposta. `tem_simbolo`
/// respondia junto com a existência do módulo, e era isso que fazia a forma
/// inteira ignorar um módulo real que a seletiva respeitava.
fn e_import_de_familia(unit_imports_module: &str, existe_modulo: bool) -> bool {
    crate::familia_superficie::familia_governa(unit_imports_module, existe_modulo)
}

/// Monta um ambiente por unidade do grafo.
pub fn ambientes_do_grafo(
    graph: &ModuleGraph,
) -> Result<HashMap<ModuleId, ModuleEnvironment>, PinkerError> {
    let mut ambientes = HashMap::new();
    for unit in graph.units() {
        ambientes.insert(unit.id, ambiente_da_unidade(graph, unit)?);
    }
    Ok(ambientes)
}

/// Uma grafia entra na superfície de uma unidade uma vez só.
///
/// É a mesma regra que a raiz sempre teve; ela apenas nunca havia sido aplicada
/// às demais unidades. As mensagens são as históricas porque a pergunta é a
/// mesma — só o lugar em que ela passou a ser feita é novo.
fn declarar_superficie(
    nome: &str,
    span: Span,
    unidade: &ModuleKey,
    declaracoes_proprias: &HashSet<String>,
    grafias_importadas: &mut HashMap<String, Span>,
) -> Result<(), PinkerError> {
    if declaracoes_proprias.contains(nome) {
        // A raiz conserva a frase histórica. Um módulo não: dizer que a colisão
        // é "no arquivo principal" quando ela é com a declaração do próprio
        // módulo seria mandar o leitor procurar no arquivo errado — e a regra só
        // passou a valer fora da raiz agora.
        let onde = match unidade {
            ModuleKey::Root => "no arquivo principal".to_string(),
            ModuleKey::Module(chave) => format!("no módulo '{}'", chave),
        };
        return Err(PinkerError::Semantic {
            msg: format!("colisão de nome no import: '{}' já existe {}", nome, onde),
            span,
        });
    }
    if let Some(anterior) = grafias_importadas.get(nome) {
        return Err(PinkerError::Semantic {
            msg: format!(
                "colisão de nome no import: '{}' trazido por múltiplos módulos",
                nome
            ),
            span: anterior.merge(span),
        });
    }
    grafias_importadas.insert(nome.to_string(), span);
    Ok(())
}

fn ambiente_da_unidade(
    graph: &ModuleGraph,
    unit: &ModuleUnit,
) -> Result<ModuleEnvironment, PinkerError> {
    let mut env = ModuleEnvironment::default();

    // 1. As próprias declarações de topo.
    for item in &unit.items {
        let Some(name) = importable_item_name(item) else {
            continue;
        };
        if nao_e_entidade_de_unidade(name) {
            continue;
        }
        env.declare(name, unit.canonical(name));
    }

    // 2. Exclusivamente os imports que esta unidade escreveu.
    //
    // NO_IMPLICIT_REEXPORT: só os itens do próprio módulo importado entram.
    // O que ELE importou continua sendo ambiente de implementação dele.
    //
    // A superfície de QUALQUER unidade é validada aqui, não só a da raiz: um
    // módulo que importe duas grafias iguais de módulos distintos, ou que
    // importe por cima da própria declaração, tinha o último import vencendo em
    // silêncio.
    let declaracoes_proprias: HashSet<String> = unit
        .items
        .iter()
        .filter_map(importable_item_name)
        .map(ToOwned::to_owned)
        .collect();
    let mut chaves_de_import: HashSet<String> = HashSet::new();
    let mut grafias_importadas: HashMap<String, Span> = HashMap::new();
    for import in &unit.imports {
        let existe_modulo = graph.module(import.module.as_str()).is_some();
        if e_import_de_familia(import.module.as_str(), existe_modulo) {
            // Import de família não vira ligação de módulo, mas a forma
            // seletiva declara uma grafia na superfície da unidade — e essa
            // grafia disputa espaço com as demais exatamente como qualquer
            // outra. A raiz sempre aplicou esta regra; deixá-la de fora aqui
            // reabriria, na família, a assimetria que o resto desta função
            // acabou de fechar.
            if let (Some(symbol), false) = (&import.symbol, unit.is_root()) {
                declarar_superficie(
                    symbol,
                    import.span,
                    &unit.key,
                    &declaracoes_proprias,
                    &mut grafias_importadas,
                )?;
            }
            continue;
        }
        let Some(origem) = graph.module(import.module.as_str()) else {
            // O carregador já recusou módulo inexistente antes de chegar aqui.
            continue;
        };
        if !unit.is_root() {
            let chave = format!(
                "{}::{}",
                import.module,
                import.symbol.as_deref().unwrap_or("*")
            );
            if !chaves_de_import.insert(chave) {
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "import duplicado para '{}{}'",
                        import.module,
                        import
                            .symbol
                            .as_ref()
                            .map(|symbol| format!(".{}", symbol))
                            .unwrap_or_default()
                    ),
                    span: import.span,
                });
            }
        }
        match &import.symbol {
            Some(symbol) => {
                // SELECTIVE_IMPORT_SURFACE: entra o símbolo pedido, e só ele.
                // As dependências internas dele continuam existindo no módulo
                // de origem — não são apagadas nem promovidas ao importador.
                let existe = origem
                    .items
                    .iter()
                    .any(|item| importable_item_name(item) == Some(symbol.as_str()));
                if !existe {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "símbolo '{}' não encontrado no módulo '{}'",
                            symbol, import.module
                        ),
                        span: import.span,
                    });
                }
                if !unit.is_root() {
                    declarar_superficie(
                        symbol,
                        import.span,
                        &unit.key,
                        &declaracoes_proprias,
                        &mut grafias_importadas,
                    )?;
                }
                env.import(symbol, origem.canonical(symbol), import.span);
            }
            None => {
                env.modulos_inteiros.insert(import.module.clone());
                for item in &origem.items {
                    let Some(name) = importable_item_name(item) else {
                        continue;
                    };
                    if nao_e_entidade_de_unidade(name) {
                        continue;
                    }
                    if !unit.is_root() {
                        declarar_superficie(
                            name,
                            import.span,
                            &unit.key,
                            &declaracoes_proprias,
                            &mut grafias_importadas,
                        )?;
                    }
                    env.import(name, origem.canonical(name), import.span);
                }
            }
        }
    }

    Ok(env)
}
// @pinker-nav:end modulos.ambiente.import-explicito

// @pinker-nav:start modulos.resolucao.nominal-canonica
// @pinker-nav:domain modulos
// @pinker-nav:layer compilador
// @pinker-nav:summary resolver_grafo reescreve declarações e referências de cada unidade para o nome canônico da unidade de origem, usando exclusivamente o ambiente que aquela unidade autorizou: a raiz preserva a grafia e o módulo qualifica pela própria chave, de modo que dois módulos independentes possam declarar o mesmo nome interno sem colidir e nenhuma referência de módulo possa ser satisfeita por disponibilidade acidental na raiz ou em irmão. Nomes possuídos pelo compilador, intrínsecas públicas, formas qualificadas de família, locais, parâmetros, bindings de padrão e parâmetros de tipo não são reescritos. Referência livre não autorizada que exista em outra unidade é recusada com o span e a fonte de quem a escreveu, em vez de religada em silêncio. Desde a #517 o CORPO default de um trato importado é resolvido contra o ambiente da unidade que DECLAROU o trato, e não contra o do importador: o parser copia esse corpo para a unidade que fez o `impl`, e como a raiz preserva grafia, resolvê-lo ali deixaria um homônimo da raiz capturar em silêncio o auxiliar do módulo. Só o corpo troca de ambiente; os tipos, inclusive o alvo do `impl`, continuam sendo da unidade que escreveu o `impl`. Desde a #567 a mesma troca vale para as closures sintéticas que esse corpo cita, que são reconhecidas pelo fato `default_body_trait` e não pelo nome: o nome anônimo é cunhado por quem materializa — tem de ser, porque o índice local do codec só é injetivo ali — e portanto não poderia responder pela origem.
struct Resolvedor<'a> {
    unit_key: ModuleKey,
    env: &'a ModuleEnvironment,
    /// Chaves de todos os módulos carregados, para reconhecer a forma
    /// qualificada `<módulo>.<entidade>` escrita direto no texto.
    modulos_carregados: &'a HashSet<String>,
    /// Grafias de topo declaradas em QUALQUER unidade. Serve só para
    /// distinguir "não existe" de "existe alhures e não foi pedido".
    declaradas_no_grafo: &'a HashMap<String, Vec<Declaracao>>,
    /// Grafias de topo declaradas pela RAIZ.
    ///
    /// Só elas podem capturar. Depois da canonicalização, a entidade de um
    /// módulo se chama `M.x`; nenhuma grafia crua pode ser satisfeita por ela.
    /// A raiz é a única unidade que preserva grafia, e portanto a única cuja
    /// declaração transforma uma grafia builtin em entidade alcançável por
    /// engano.
    declaradas_na_raiz: &'a HashSet<String>,
    /// #517: unidades que declararam cada trato, para que o corpo default
    /// copiado pelo parser continue significando o que significava na origem.
    origem_dos_tratos: OrigemDosTratos<'a>,
    /// Escopos de valor: parâmetros, locais e bindings de padrão.
    bound: Vec<HashSet<String>>,
    /// Escopos de tipo: parâmetros de tipo de função, struct e leque.
    type_bound: Vec<HashSet<String>>,
}

/// #517 — `TRAIT_DEFAULT_BODY_BELONGS_TO_THE_DECLARING_UNIT`.
///
/// O parser materializa o corpo default de um trato dentro da unidade que
/// escreveu o `impl`, com as grafias originais. Enquanto trato e `impl` moravam
/// no mesmo arquivo isso era inofensivo. Com `impl` sobre trato importado o
/// corpo passa a ser resolvido contra o ambiente de OUTRA unidade — e a raiz,
/// que preserva grafia, capturaria em silêncio qualquer homônimo do auxiliar
/// que o módulo usa. Estes três índices dizem, para um nome canônico de trato,
/// qual unidade o declarou e qual ambiente o corpo dele autorizou.
#[derive(Clone, Copy)]
struct OrigemDosTratos<'a> {
    tratos: &'a HashMap<String, ModuleId>,
    chaves: &'a HashMap<ModuleId, ModuleKey>,
    ambientes: &'a HashMap<ModuleId, ModuleEnvironment>,
}

/// A função carrega o corpo default de um trato, copiado pelo parser?
///
/// Três formas, um só fato: o default materializado como o próprio método do
/// `impl` (quando nenhum override venceu), o default materializado só para
/// checagem (quando um override venceu) e — #567 — as closures sintéticas que
/// esses corpos citam. O método escrito pelo usuário no `impl` NÃO entra: o
/// corpo dele foi escrito na unidade que fez o `impl`.
///
/// A closure responde pelo fato explícito que a materialização gravou, não pelo
/// nome. O nome anônimo é cunhado pela unidade que materializa — ele tem de
/// ser, porque o índice local do codec só é injetivo ali — e portanto não pode
/// ser a autoridade sobre a origem. O corpo, esse, continua sendo o da unidade
/// declarante, e é contra o ambiente DELA que ele significa: sem isto, um
/// auxiliar homônimo do importador capturaria em silêncio a referência que a
/// origem escreveu.
fn corpo_default_de_trato(function: &FunctionDecl) -> Option<String> {
    if let Some(trait_name) = &function.default_body_trait {
        return Some(trait_name.clone());
    }
    if let Some((trait_name, _, _)) =
        crate::method_identity::parse_trait_default_check_function_name(&function.name)
    {
        return Some(trait_name);
    }
    let facts = function.impl_facts.as_ref()?;
    if !facts.generated_default {
        return None;
    }
    crate::method_identity::parse_provisional_function_name(&function.name)
        .map(|(trait_name, _, _)| trait_name)
}

impl<'a> Resolvedor<'a> {
    fn new(
        unit_key: ModuleKey,
        env: &'a ModuleEnvironment,
        modulos_carregados: &'a HashSet<String>,
        declaradas_no_grafo: &'a HashMap<String, Vec<Declaracao>>,
        declaradas_na_raiz: &'a HashSet<String>,
        origem_dos_tratos: OrigemDosTratos<'a>,
    ) -> Self {
        Self {
            unit_key,
            env,
            modulos_carregados,
            declaradas_no_grafo,
            declaradas_na_raiz,
            origem_dos_tratos,
            bound: Vec::new(),
            type_bound: Vec::new(),
        }
    }

    /// #517: unidade e ambiente em que o corpo default desta função foi escrito,
    /// quando ela veio de um trato de OUTRA unidade.
    ///
    /// A identidade do trato é a canônica, obtida pelo mesmo ambiente que
    /// resolve o `impl`. Trato da própria unidade devolve `None` e o corpo
    /// continua sendo resolvido exatamente como sempre foi.
    fn origem_do_corpo_default(
        &self,
        function: &FunctionDecl,
    ) -> Option<(ModuleKey, &'a ModuleEnvironment)> {
        let trait_spelling = corpo_default_de_trato(function)?;
        let canonical = canonizar_grafia(&trait_spelling, self.env);
        let id = *self.origem_dos_tratos.tratos.get(&canonical)?;
        let chave = self.origem_dos_tratos.chaves.get(&id)?;
        if *chave == self.unit_key {
            return None;
        }
        Some((chave.clone(), self.origem_dos_tratos.ambientes.get(&id)?))
    }

    fn ligado(&self, name: &str) -> bool {
        self.bound.iter().any(|scope| scope.contains(name))
    }

    fn tipo_ligado(&self, name: &str) -> bool {
        self.type_bound.iter().any(|scope| scope.contains(name))
    }

    /// Resolve uma grafia de topo pelo ambiente da unidade.
    fn resolver_nome(&self, name: &str, span: Span) -> Result<Option<String>, PinkerError> {
        if nao_e_entidade_de_unidade(name) {
            return Ok(None);
        }
        // `TODA_IDENTIDADE_EXISTENTE > FAMÍLIA` e `REAL_MODULE_X >
        // BUILTIN_FAMILY_X`: o ambiente é consultado ANTES da superfície
        // global. Um módulo real chamado `arquivo` que exporte `criar` vence a
        // família homônima — é a mesma precedência que o carregador e o parser
        // já aplicam, e inverter a ordem aqui a desfaria em silêncio.
        if let Some(canonical) = self.env.lookup(name) {
            if canonical == name {
                return Ok(None);
            }
            return Ok(Some(canonical.to_string()));
        }
        // Forma qualificada `<módulo>.<entidade>`, escrita direto no texto.
        //
        // Esta pergunta vem ANTES da superfície global porque
        // `REAL_MODULE_X > BUILTIN_FAMILY_X`: um módulo real chamado `arquivo`
        // vence a família homônima, e perguntar à família primeiro deixaria
        // `arquivo.<membro>` de um módulo real passar sem autorização sempre
        // que a grafia coincidisse com a de um membro aprovado.
        // Ela já É um nome canônico, então nenhuma reescrita se aplica — mas
        // por isso mesmo ela contorna a grafia, e sem esta pergunta alcançaria
        // qualquer unidade carregada. Autorização continua sendo do ambiente.
        if let Some((prefixo, sufixo)) = name.split_once('.') {
            if self.modulos_carregados.contains(prefixo) {
                let e_propria = self.unit_key.module_key() == Some(prefixo);
                // Import inteiro autoriza a superfície: se o membro existe é
                // pergunta da autoridade semântica, com a redação histórica.
                if e_propria
                    || self.env.importou_inteiro(prefixo)
                    || self.env.autoriza_canonico(name)
                {
                    return Ok(None);
                }
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "tipo '{}' não existe neste ambiente: {} não importou '{}' de '{}'",
                        name,
                        descricao_unidade(&self.unit_key),
                        sufixo,
                        prefixo,
                    ),
                    span,
                });
            }
        }
        // A grafia é superfície global — MAS só enquanto nenhuma unidade a
        // reivindicar.
        //
        // Curto-circuitar aqui por grafia, sem olhar quem a declara, deixava a
        // referência crua sobreviver à canonicalização; como a raiz preserva
        // grafia, ela aterrissava na entidade homônima da raiz. A #507 protege
        // `Item::Function`, então a raiz pode declarar legalmente `eterno`,
        // `ninho`, `apelido` ou `leque` com grafia de intrínseca, e cada um
        // desses era um caminho de captura distinto.
        //
        // Perguntar "é builtin E a raiz não a declarou" resolve a classe
        // inteira de uma vez, em vez de interceptar cada caminho consumidor.
        //
        // A pergunta é sobre a RAIZ, não sobre o grafo. Só a raiz preserva
        // grafia; a entidade de um módulo se chama `M.x` e não pode ser
        // satisfeita por grafia crua, então a declaração de um irmão não
        // captura ninguém. Perguntar pelo grafo inteiro inverteria a
        // não-interferência: um módulo que este aqui nunca consultou passaria a
        // lhe tirar o builtin, e o diagnóstico ainda apontaria o remédio errado.
        if e_superficie_global(name) && !self.declaradas_na_raiz.contains(name) {
            return Ok(None);
        }
        // Não autorizado por esta unidade. Se a grafia existe em outra
        // unidade, deixá-la passar seria exatamente a captura ambiental que
        // esta camada existe para impedir.
        //
        // A raiz nunca é capturável: depois da canonicalização, item de módulo
        // se chama `M.x` e nenhuma grafia crua da raiz pode ser satisfeita por
        // ele. Recusar aqui só produziria falso positivo — inclusive sobre
        // grafia builtin que uma unidade qualquer resolva declarar.
        if matches!(self.unit_key, ModuleKey::Root) {
            return Ok(None);
        }
        if let Some(declaracoes) = self.declaradas_no_grafo.get(name) {
            let propria = self.unit_key.to_string();
            let alheias: Vec<&Declaracao> = declaracoes
                .iter()
                .filter(|declaracao| declaracao.unidade != propria)
                .collect();
            if let Some(primeira) = alheias.first() {
                // O diagnóstico começa pela frase histórica de "não declarado"
                // para a espécie da entidade, e só então acrescenta o que antes
                // não podia ser dito: ela existe, existe em outro lugar, e esta
                // unidade não a pediu. "não existe" e "existe alhures e não foi
                // importado" são fatos diferentes.
                let (especie, sujeito, participio, objeto) = primeira.especie.frase();
                let onde = alheias
                    .iter()
                    .map(|declaracao| format!("'{}'", declaracao.unidade))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(PinkerError::Semantic {
                    msg: format!(
                        "{} '{}' não {} neste ambiente — {} é {} em {}, e {} não {} importou",
                        especie,
                        name,
                        participio,
                        sujeito,
                        participio,
                        onde,
                        descricao_unidade(&self.unit_key),
                        objeto,
                    ),
                    span,
                });
            }
        }
        // Desconhecido em todo o grafo: pode ser builtin não catalogado aqui ou
        // simplesmente inexistente. Quem decide é a autoridade semântica, com a
        // mensagem histórica.
        Ok(None)
    }

    fn resolver_item(&mut self, item: &mut Item) -> Result<(), PinkerError> {
        match item {
            Item::Function(function) => self.resolver_funcao(function),
            Item::Const(constant) => self.resolver_const(constant),
            Item::Struct(struct_decl) => self.resolver_struct(struct_decl),
            Item::TypeAlias(alias) => self.resolver_alias(alias),
            Item::Enum(enum_decl) => self.resolver_enum(enum_decl),
            Item::Trait(trait_decl) => self.resolver_trait(trait_decl),
        }
    }

    fn resolver_funcao(&mut self, function: &mut FunctionDecl) -> Result<(), PinkerError> {
        self.type_bound
            .push(function.type_params.iter().cloned().collect());
        if let Some(facts) = &mut function.impl_facts {
            self.resolver_tipo(&mut facts.target_ty)?;
        }
        for param in &mut function.params {
            self.resolver_tipo(&mut param.ty)?;
        }
        if let Some(ret) = &mut function.ret_type {
            self.resolver_tipo(ret)?;
        }
        self.bound
            .push(function.params.iter().map(|p| p.name.clone()).collect());
        // Os TIPOS já foram resolvidos acima, contra o ambiente desta unidade:
        // o alvo do `impl` é um tipo desta unidade e continua sendo. O que muda
        // de ambiente é só o CORPO, e só quando ele é o default copiado de um
        // trato de outra unidade.
        let resultado = match self.origem_do_corpo_default(function) {
            Some((chave, env)) => {
                let chave_anterior = std::mem::replace(&mut self.unit_key, chave);
                let env_anterior = std::mem::replace(&mut self.env, env);
                let resultado = self.resolver_bloco(&mut function.body);
                self.unit_key = chave_anterior;
                self.env = env_anterior;
                resultado
            }
            None => self.resolver_bloco(&mut function.body),
        };
        self.bound.pop();
        self.type_bound.pop();
        resultado
    }

    fn resolver_const(&mut self, constant: &mut ConstDecl) -> Result<(), PinkerError> {
        self.resolver_tipo(&mut constant.ty)?;
        self.bound.push(HashSet::new());
        self.resolver_expr(&mut constant.init)?;
        self.bound.pop();
        Ok(())
    }

    fn resolver_struct(&mut self, struct_decl: &mut StructDecl) -> Result<(), PinkerError> {
        for field in &mut struct_decl.fields {
            self.resolver_tipo(&mut field.ty)?;
        }
        Ok(())
    }

    fn resolver_alias(&mut self, alias: &mut TypeAliasDecl) -> Result<(), PinkerError> {
        self.resolver_tipo(&mut alias.target)
    }

    fn resolver_enum(&mut self, enum_decl: &mut EnumDecl) -> Result<(), PinkerError> {
        self.type_bound
            .push(enum_decl.type_params.iter().cloned().collect());
        for variant in &mut enum_decl.variants {
            for payload in &mut variant.payloads {
                self.resolver_tipo(payload)?;
            }
        }
        self.type_bound.pop();
        Ok(())
    }

    fn resolver_trait(&mut self, trait_decl: &mut TraitDecl) -> Result<(), PinkerError> {
        for method in &mut trait_decl.methods {
            self.resolver_assinatura(method)?;
        }
        Ok(())
    }

    fn resolver_assinatura(&mut self, method: &mut TraitMethodSig) -> Result<(), PinkerError> {
        for param in &mut method.params {
            self.resolver_tipo(&mut param.ty)?;
        }
        if let Some(ret) = &mut method.ret_type {
            self.resolver_tipo(ret)?;
        }
        if let Some(body) = &mut method.body {
            self.bound.push(
                method
                    .params
                    .iter()
                    .map(|p: &Param| p.name.clone())
                    .collect(),
            );
            self.resolver_bloco(body)?;
            self.bound.pop();
        }
        Ok(())
    }

    fn resolver_impl(&mut self, impl_decl: &mut ImplDecl) -> Result<(), PinkerError> {
        if let Some(canonical) = self.resolver_nome(&impl_decl.trait_name, impl_decl.span)? {
            impl_decl.trait_name = canonical;
        }
        self.resolver_tipo(&mut impl_decl.target_ty)
    }

    fn resolver_tipo(&mut self, ty: &mut Type) -> Result<(), PinkerError> {
        match ty {
            Type::Alias { name, span }
            | Type::Struct { name, span }
            | Type::Enum { name, span } => {
                if self.tipo_ligado(name) {
                    return Ok(());
                }
                if let Some(canonical) = self.resolver_nome(name, *span)? {
                    *name = canonical;
                }
            }
            Type::ListEnum { element, span } => {
                if !self.tipo_ligado(element) {
                    if let Some(canonical) = self.resolver_nome(element, *span)? {
                        *element = canonical;
                    }
                }
            }
            Type::Applied { name, args, span } => {
                if !self.tipo_ligado(name) {
                    if let Some(canonical) = self.resolver_nome(name, *span)? {
                        *name = canonical;
                    }
                }
                for arg in args {
                    self.resolver_tipo(arg)?;
                }
            }
            Type::Map { key, value, .. } => {
                self.resolver_tipo(key)?;
                self.resolver_tipo(value)?;
            }
            Type::FixedArray { element, .. } => self.resolver_tipo(element)?,
            Type::Pointer { base, .. } => self.resolver_tipo(base)?,
            Type::Function { params, ret, .. } => {
                for param in params {
                    self.resolver_tipo(param)?;
                }
                self.resolver_tipo(ret)?;
            }
            Type::Union { members, .. } => {
                for member in members {
                    self.resolver_tipo(member)?;
                }
            }
            // Tipos builtin e handles opacos são superfície global.
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
            | Type::MapVersoBombom(_)
            | Type::MapVersoVerso(_)
            | Type::MapBombomBombom(_)
            | Type::MapBombomVerso(_)
            | Type::OpaqueHandle { .. }
            | Type::Nulo(_) => {}
        }
        Ok(())
    }

    fn resolver_bloco(&mut self, block: &mut Block) -> Result<(), PinkerError> {
        self.bound.push(HashSet::new());
        for stmt in &mut block.stmts {
            self.resolver_stmt(stmt)?;
        }
        self.bound.pop();
        Ok(())
    }

    fn resolver_if(&mut self, if_stmt: &mut IfStmt) -> Result<(), PinkerError> {
        self.resolver_expr(&mut if_stmt.condition)?;
        self.resolver_bloco(&mut if_stmt.then_branch)?;
        match &mut if_stmt.else_branch {
            Some(ElseBlock::Block(block)) => self.resolver_bloco(block)?,
            Some(ElseBlock::If(inner)) => self.resolver_if(inner)?,
            None => {}
        }
        Ok(())
    }

    fn resolver_stmt(&mut self, stmt: &mut Stmt) -> Result<(), PinkerError> {
        match stmt {
            Stmt::Let(let_stmt) => {
                if let Some(ty) = &mut let_stmt.ty {
                    self.resolver_tipo(ty)?;
                }
                self.resolver_expr(&mut let_stmt.init)?;
                self.bound
                    .last_mut()
                    .expect("escopo ativo")
                    .insert(let_stmt.name.clone());
            }
            Stmt::Return(return_stmt) => {
                if let Some(expr) = &mut return_stmt.expr {
                    self.resolver_expr(expr)?;
                }
            }
            Stmt::Assign(assign_stmt) => {
                let span = assign_stmt.span;
                match &mut assign_stmt.target {
                    AssignTarget::Ident(name) => {
                        if !self.ligado(name) {
                            if let Some(canonical) = self.resolver_nome(name, span)? {
                                *name = canonical;
                            }
                        }
                    }
                    AssignTarget::Deref(ptr) => self.resolver_expr(ptr)?,
                    AssignTarget::FieldDeref { base, .. } => self.resolver_expr(base)?,
                    AssignTarget::Index { base, index } => {
                        self.resolver_expr(base)?;
                        self.resolver_expr(index)?;
                    }
                }
                self.resolver_expr(&mut assign_stmt.expr)?;
            }
            Stmt::If(if_stmt) => self.resolver_if(if_stmt)?,
            Stmt::While(while_stmt) => {
                self.resolver_expr(&mut while_stmt.condition)?;
                self.resolver_bloco(&mut while_stmt.body)?;
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
            Stmt::InlineAsm(asm) => {
                for operand in &mut asm.operands {
                    self.resolver_expr(&mut operand.value)?;
                }
            }
            Stmt::EnumMatch(enum_match) => {
                self.resolver_expr(&mut enum_match.scrutinee)?;
                for arm in &mut enum_match.arms {
                    let mut bindings = HashSet::new();
                    Self::coletar_bindings(&arm.pattern, &mut bindings);
                    self.resolver_padrao(&mut arm.pattern)?;
                    self.bound.push(bindings);
                    self.resolver_bloco(&mut arm.body)?;
                    self.bound.pop();
                }
                if let Some(otherwise) = &mut enum_match.otherwise {
                    self.resolver_bloco(otherwise)?;
                }
            }
            Stmt::UnionMatch(union_match) => {
                self.resolver_expr(&mut union_match.scrutinee)?;
                for arm in &mut union_match.arms {
                    self.resolver_tipo(&mut arm.member_type)?;
                    self.bound.push(HashSet::from([arm.binding.clone()]));
                    self.resolver_bloco(&mut arm.body)?;
                    self.bound.pop();
                }
            }
            Stmt::Falar(falar) => {
                for arg in &mut falar.args {
                    self.resolver_expr(arg)?;
                }
            }
            Stmt::Expr(expr) => self.resolver_expr(expr)?,
        }
        Ok(())
    }

    fn coletar_bindings(pattern: &EnumPattern, bindings: &mut HashSet<String>) {
        match pattern {
            EnumPattern::Binding { name, .. } => {
                bindings.insert(name.clone());
            }
            EnumPattern::Variant { payloads, .. } => {
                for payload in payloads {
                    Self::coletar_bindings(payload, bindings);
                }
            }
        }
    }

    fn resolver_padrao(&mut self, pattern: &mut EnumPattern) -> Result<(), PinkerError> {
        match pattern {
            EnumPattern::Binding { .. } => Ok(()),
            EnumPattern::Variant {
                enum_name,
                payloads,
                span,
                ..
            } => {
                if !self.tipo_ligado(enum_name) {
                    if let Some(canonical) = self.resolver_nome(enum_name, *span)? {
                        *enum_name = canonical;
                    }
                }
                for payload in payloads {
                    self.resolver_padrao(payload)?;
                }
                Ok(())
            }
        }
    }

    fn resolver_expr(&mut self, expr: &mut Expr) -> Result<(), PinkerError> {
        let span = expr.span;
        match &mut expr.kind {
            ExprKind::Ident(name) => {
                if !self.ligado(name) {
                    if let Some(canonical) = self.resolver_nome(name, span)? {
                        *name = canonical;
                    }
                }
            }
            // #532: intrínseca já resolvida não é nome de unidade nenhuma, e
            // por isso não é canonicalizada nem capturável por declaração
            // homônima de qualquer unidade do grafo.
            ExprKind::Intrinsic(_) => {}
            ExprKind::Binary(lhs, _, rhs) => {
                self.resolver_expr(lhs)?;
                self.resolver_expr(rhs)?;
            }
            ExprKind::Unary(_, operand) | ExprKind::AddressOf(operand) => {
                self.resolver_expr(operand)?
            }
            ExprKind::Call(callee, args) => {
                self.resolver_expr(callee)?;
                for arg in args {
                    self.resolver_expr(arg)?;
                }
            }
            ExprKind::InternalMapIterCreate(inner) | ExprKind::InternalMapIterNextKey(inner) => {
                self.resolver_expr(inner)?
            }
            ExprKind::FieldAccess { base, .. } => self.resolver_expr(base)?,
            ExprKind::Index { base, index } => {
                self.resolver_expr(base)?;
                self.resolver_expr(index)?;
            }
            ExprKind::Cast { expr, target } => {
                self.resolver_expr(expr)?;
                self.resolver_tipo(target)?;
            }
            ExprKind::SizeOfType { target } | ExprKind::AlignOfType { target } => {
                self.resolver_tipo(target)?
            }
            ExprKind::IntLit(_) | ExprKind::BoolLit(_) | ExprKind::StringLit(_) => {}
        }
        Ok(())
    }
}

fn descricao_unidade(key: &ModuleKey) -> String {
    match key {
        ModuleKey::Root => "raiz".to_string(),
        ModuleKey::Module(module) => format!("módulo '{}'", module),
    }
}

/// Espécie declarada de uma entidade de topo.
///
/// Serve só ao diagnóstico: a frase histórica de "não declarado" muda com a
/// espécie, e um erro de composição não deveria inventar uma redação nova para
/// dizer o que a linguagem já dizia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Especie {
    Funcao,
    Constante,
    Struct,
    Alias,
    Leque,
    Trato,
}

impl Especie {
    fn de(item: &Item) -> Self {
        match item {
            Item::Function(_) => Especie::Funcao,
            Item::Const(_) => Especie::Constante,
            Item::Struct(_) => Especie::Struct,
            Item::TypeAlias(_) => Especie::Alias,
            Item::Enum(_) => Especie::Leque,
            Item::Trait(_) => Especie::Trato,
        }
    }

    /// (espécie, sujeito, particípio, objeto) já concordados.
    fn frase(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Especie::Funcao => ("função", "ela", "declarada", "a"),
            Especie::Constante => ("constante", "ela", "declarada", "a"),
            Especie::Struct => ("struct", "ela", "declarada", "a"),
            Especie::Alias => ("alias de tipo", "ele", "declarado", "o"),
            Especie::Leque => ("leque", "ele", "declarado", "o"),
            Especie::Trato => ("trato", "ele", "declarado", "o"),
        }
    }
}

#[derive(Debug, Clone)]
struct Declaracao {
    unidade: String,
    especie: Especie,
}

/// Grafias de topo declaradas por unidade, para o diagnóstico de captura.
fn declaracoes_do_grafo(graph: &ModuleGraph) -> HashMap<String, Vec<Declaracao>> {
    let mut mapa: HashMap<String, Vec<Declaracao>> = HashMap::new();
    for unit in graph.units() {
        for item in &unit.items {
            let Some(name) = importable_item_name(item) else {
                continue;
            };
            if nao_e_entidade_de_unidade(name) {
                continue;
            }
            let unidade = unit.key.to_string();
            let entrada = mapa.entry(name.to_string()).or_default();
            if !entrada
                .iter()
                .any(|declaracao| declaracao.unidade == unidade)
            {
                entrada.push(Declaracao {
                    unidade,
                    especie: Especie::de(item),
                });
            }
        }
    }
    mapa
}

/// Renomeia as declarações de topo de uma unidade para o nome canônico dela.
fn canonizar_declaracoes(unit: &mut ModuleUnit) {
    if matches!(unit.key, ModuleKey::Root) {
        // A raiz preserva a grafia: `principal` continua `principal` e nenhum
        // símbolo de runtime muda de nome.
        return;
    }
    let key = unit.key.clone();
    for item in &mut unit.items {
        let Some(name) = importable_item_name(item) else {
            continue;
        };
        if nao_e_entidade_de_unidade(name) {
            continue;
        }
        // Método de `impl` transporta as grafias no próprio nome provisional.
        // Ele é renomeado pela recomposição do codec, não por prefixo.
        if crate::method_identity::parse_provisional_function_name(name).is_some() {
            continue;
        }
        let canonical = key.canonical(name);
        set_item_name(item, canonical);
    }
}

/// Recompõe o nome provisional de um corpo sintético de `trato` — `__impl_*` e
/// `__trait_default_check_*` — a partir das identidades já canônicas do trato e
/// do alvo.
///
/// O codec é injetivo por comprimento, então dois módulos que implementam o
/// mesmo trato para o mesmo nome de tipo deixam de produzir o mesmo nome.
///
/// A checagem do default entra pela mesma porta que o método: ela prova o corpo
/// de UM contrato, e o contrato é `(trato canônico, alvo canônico, método)`.
/// Recompor só o método deixava a checagem endereçada pela grafia textual, e
/// dois tratos homônimos de unidades distintas voltavam a compartilhar uma
/// identidade que não é a mesma.
fn recompor_nomes_de_impl(unit: &mut ModuleUnit, env: &ModuleEnvironment) {
    for item in &mut unit.items {
        let Item::Function(function) = item else {
            continue;
        };
        let Some((prefixo, trait_name, target_spelling, method_name)) =
            crate::method_identity::parse_synthetic_trait_body_name(&function.name)
        else {
            continue;
        };
        let canonical_trait = canonizar_grafia(&trait_name, env);
        let canonical_target = canonizar_grafia(&target_spelling, env);
        function.name = crate::method_identity::render_synthetic_trait_body_name(
            prefixo,
            &canonical_trait,
            &canonical_target,
            &method_name,
        );
    }
}

fn canonizar_grafia(name: &str, env: &ModuleEnvironment) -> String {
    if nao_e_entidade_de_unidade(name) {
        return name.to_string();
    }
    // Mesma precedência de `resolver_nome`: ambiente antes de superfície
    // global.
    match env.lookup(name) {
        Some(canonical) => canonical.to_string(),
        None => name.to_string(),
    }
}

/// Resolve o grafo inteiro para identidades canônicas.
pub fn resolver_grafo(graph: &ModuleGraph) -> Result<ModuleGraph, PinkerError> {
    let ambientes = ambientes_do_grafo(graph)?;
    let declaradas = declaracoes_do_grafo(graph);
    let modulos_carregados: HashSet<String> = graph
        .units()
        .iter()
        .filter_map(|unit| unit.key.module_key().map(ToOwned::to_owned))
        .collect();
    let declaradas_na_raiz: HashSet<String> = graph
        .root()
        .items
        .iter()
        .filter_map(importable_item_name)
        .filter(|name| !nao_e_entidade_de_unidade(name))
        .map(ToOwned::to_owned)
        .collect();
    // #517: quem declarou cada trato, pelo nome canônico que ele terá depois da
    // resolução. Montado ANTES do laço, sobre o grafo intacto, porque
    // `canonizar_declaracoes` reescreve os nomes unidade a unidade.
    let tratos_por_unidade: HashMap<String, ModuleId> = graph
        .units()
        .iter()
        .flat_map(|unit| {
            unit.items.iter().filter_map(move |item| match item {
                Item::Trait(trait_decl) => Some((unit.canonical(&trait_decl.name), unit.id)),
                _ => None,
            })
        })
        .collect();
    let chaves_de_unidade: HashMap<ModuleId, ModuleKey> = graph
        .units()
        .iter()
        .map(|unit| (unit.id, unit.key.clone()))
        .collect();
    let origem_dos_tratos = OrigemDosTratos {
        tratos: &tratos_por_unidade,
        chaves: &chaves_de_unidade,
        ambientes: &ambientes,
    };
    let mut resolvido = graph.clone();

    for id in graph.dependency_order() {
        let key = resolvido.unit(id).key.clone();
        let env = ambientes.get(&id).expect("ambiente da unidade");

        // A ordem importa: as referências são resolvidas com as grafias
        // originais, contra o ambiente; só depois as declarações passam a se
        // chamar pelo nome canônico.
        let unit = resolvido.unit_mut(id);
        let mut resolvedor = Resolvedor::new(
            key.clone(),
            env,
            &modulos_carregados,
            &declaradas,
            &declaradas_na_raiz,
            origem_dos_tratos,
        );
        for item in &mut unit.items {
            resolvedor.resolver_item(item)?;
        }
        for impl_decl in &mut unit.impls {
            resolvedor.resolver_impl(impl_decl)?;
        }

        recompor_nomes_de_impl(unit, env);
        canonizar_declaracoes(unit);
    }

    Ok(resolvido)
}
// @pinker-nav:end modulos.resolucao.nominal-canonica

// @pinker-nav:start modulos.projecao.execucao
// @pinker-nav:domain modulos
// @pinker-nav:layer compilador
// @pinker-nav:summary projetar_programa achata o grafo já resolvido num Program único para lowering: a raiz inteira e, de cada módulo, o fecho alcançável a partir da superfície que o importador pediu, mais as relações de `impl` e os corpos sintéticos de `trato` — o método e a checagem do corpo default vencido por override — de toda unidade carregada. Materializar o fecho — e não só o símbolo pedido — é o que preserva as dependências internas da entidade importada sem torná-las visíveis ao importador, porque visibilidade já foi decidida pelo ambiente. Manter as relações de `impl` de toda unidade é o que impede que uma obrigação induzida pela unidade desapareça na projeção. Identidades geradas endereçadas por conteúdo entram uma vez só, então símbolo de runtime não é duplicado; a projeção acontece depois da resolução, então nada volta a depender de grafia. O fecho segue também as closures sintéticas que um corpo sintético de trato cita (#567): o que a origem escreveu dentro delas só é alcançado uma indireção abaixo do método, e sem esse degrau o corpo materializado chegaria íntegro com a dependência que ele usa faltando. Pela mesma razão a DECLARAÇÃO do trato não alcança essas closures: ela é contrato, não código, e o molde que ninguém chama levaria ao programa os identificadores livres do corpo default ainda soltos.
/// Achata o grafo resolvido num `Program` para lowering.
///
/// Duas decisões separadas, que a composição anterior confundia:
///
/// - **visibilidade** é do `ModuleEnvironment` e já foi decidida;
/// - **materialização** é daqui, e segue o alcance real.
///
/// O import seletivo continua materializando o fecho da entidade pedida — as
/// dependências internas dela existem, sob o nome canônico do módulo de origem,
/// sem entrar na superfície de quem importou. O import inteiro semeia com todos
/// os itens importáveis do módulo, como sempre fez.
///
/// As relações de `impl` de toda unidade carregada entram sempre. Um `impl` não
/// tem forma de import: ele é relação, não nome. Deixá-lo de fora era o que
/// fazia uma fonte recusada como raiz passar a ser aceita ao virar módulo.
pub fn projetar_programa(graph: &ModuleGraph) -> Result<Program, PinkerError> {
    let root = graph.root();
    let alcancaveis = fecho_alcancavel(graph);

    let mut items: Vec<Item> = Vec::new();
    let mut impls: Vec<ImplDecl> = Vec::new();
    // Identidades possuídas pelo compilador que já são endereçadas pelo próprio
    // conteúdo: duas unidades que especializam `Resultado<verso, verso>`
    // produzem o MESMO nome porque produzem a mesma entidade. Materializar as
    // duas cópias duplicaria símbolo de runtime sem criar entidade nova. Isto
    // não engole `impl` repetido: `impls` não é deduplicado, e é ele que a
    // autoridade de contratos de trato usa para recusar a relação duplicada.
    // nome canônico -> (unidade que o emitiu, impressão estrutural)
    let mut gerados_emitidos: HashMap<String, (String, String)> = HashMap::new();
    let mut reservadas_emitidas: HashSet<String> = HashSet::new();
    let apelidos = apelidos_do_grafo(graph);

    // Módulos primeiro, em ordem de dependência, e a raiz por último — a mesma
    // ordem relativa que a materialização anterior produzia ao inserir os itens
    // importados no início de `items`.
    for id in graph.dependency_order() {
        let unit = graph.unit(id);
        if unit.is_root() {
            continue;
        }
        for item in &unit.items {
            let Some(nome) = importable_item_name(item) else {
                continue;
            };
            // Corpo sintético de `trato` — o método do `impl` e a checagem do
            // corpo default vencido por override — não é alcançado por
            // referência: ninguém o nomeia. Ele existe porque a relação de
            // `impl` existe, e a relação entra sempre. Perguntar só por
            // `__impl_*` aqui era o que fazia a unidade física que hospeda o
            // `impl` decidir se o corpo default recebia validação semântica.
            let e_corpo_sintetico_de_trato =
                crate::method_identity::parse_synthetic_trait_body_name(nome).is_some();
            if !e_corpo_sintetico_de_trato && !alcancaveis.contains(nome) {
                continue;
            }
            // Identidade reservada do runtime é materializada pelo parser em
            // CADA unidade que a mencione — inclusive implicitamente, por uma
            // chamada a builtin falível cuja carga é o leque. Ela vem de uma
            // autoridade única, então nome igual prova entidade igual sem
            // conferência estrutural: entra uma vez e pronto. Sem isto, duas
            // unidades que toquem `TipoEntrada` produzem duas declarações do
            // mesmo leque e a projeção é recusada — a mesma fonte aceita como
            // raiz volta a ser recusada como módulo, invertida.
            if crate::runtime_identity::runtime_reserved_identity(nome).is_some() {
                if reservadas_emitidas.insert(nome.to_string()) {
                    items.push(item.clone());
                }
                continue;
            }
            if e_identidade_endereçada_por_conteudo(nome) {
                let impressao = impressao_estrutural(item, &apelidos);
                match gerados_emitidos.get(nome) {
                    Some((primeira, anterior)) => {
                        if anterior != &impressao {
                            return Err(colisao_de_identidade_gerada(
                                nome,
                                primeira,
                                &unit.key.to_string(),
                                item.span(),
                            ));
                        }
                        continue;
                    }
                    None => {
                        gerados_emitidos
                            .insert(nome.to_string(), (unit.key.to_string(), impressao));
                    }
                }
            }
            items.push(item.clone());
        }
        impls.extend(unit.impls.iter().cloned());
    }

    for item in &root.items {
        if let Some(nome) = importable_item_name(item) {
            if crate::runtime_identity::runtime_reserved_identity(nome).is_some() {
                if reservadas_emitidas.insert(nome.to_string()) {
                    items.push(item.clone());
                }
                continue;
            }
            if e_identidade_endereçada_por_conteudo(nome) {
                let impressao = impressao_estrutural(item, &apelidos);
                match gerados_emitidos.get(nome) {
                    Some((primeira, anterior)) => {
                        if anterior != &impressao {
                            return Err(colisao_de_identidade_gerada(
                                nome,
                                primeira,
                                "raiz",
                                item.span(),
                            ));
                        }
                        continue;
                    }
                    None => {
                        gerados_emitidos.insert(nome.to_string(), ("raiz".to_string(), impressao));
                    }
                }
            }
        }
        items.push(item.clone());
    }
    impls.extend(root.impls.iter().cloned());

    Ok(Program {
        package: root.package.clone(),
        freestanding: root.freestanding,
        // Imports de módulo já foram consumidos pela resolução; sobreviveram
        // como ligações canônicas nos corpos. Os de família sobrevivem na
        // projeção porque quem os valida é a autoridade semântica.
        imports: root
            .imports
            .iter()
            .filter(|import| {
                e_import_de_familia(
                    import.module.as_str(),
                    graph.module(import.module.as_str()).is_some(),
                )
            })
            .cloned()
            .collect(),
        impls,
        items,
    })
}

/// Apelidos de tipo declarados no grafo, já canônicos, para a impressão
/// estrutural.
///
/// Apelido é transparente: `apelido Cor = bombom` não cria entidade, dá nome a
/// uma. Duas unidades com um apelido privado homônimo denotam a MESMA
/// especialização, e compará-las pela grafia canonizada (`fa.Cor` vs `fb.Cor`)
/// as declararia diferentes — recusando programa correto.
///
/// Guarda o `Type` alvo, não a lista de nomes que ele referencia: a lista
/// descarta a estrutura e, com ela, a diferença entre `bombom` e `verso`.
fn apelidos_do_grafo(graph: &ModuleGraph) -> HashMap<String, Type> {
    let mut mapa = HashMap::new();
    for unit in graph.units() {
        for item in &unit.items {
            if let Item::TypeAlias(alias) = item {
                mapa.insert(alias.name.clone(), alias.target.clone());
            }
        }
    }
    mapa
}

/// Chave estrutural exata de um tipo, com apelidos expandidos.
///
/// A autoridade vive em `union_canon`: ela interna um DAG, aplica a mesma
/// canonicalização normativa de uniões e serializa todos os bytes. Assim o
/// custo não explode em grafos de apelidos e nenhuma colisão probabilística é
/// promovida a igualdade de entidade.
fn chave_exata_de_tipo(ty: &Type, apelidos: &HashMap<String, Type>) -> String {
    crate::union_canon::canonical_type_graph_key(ty, apelidos)
}

/// Impressão estrutural de um item, para conferir a premissa da deduplicação.
///
/// Ignora span de propósito: duas unidades materializam a MESMA entidade em
/// posições diferentes, e posição não é identidade. Apelidos são expandidos
/// pela mesma razão: eles também não são entidade.
fn impressao_estrutural(item: &Item, apelidos: &HashMap<String, Type>) -> String {
    match item {
        Item::Enum(enum_decl) => {
            let variantes: Vec<String> = enum_decl
                .variants
                .iter()
                .map(|variante| {
                    format!(
                        "{}({})",
                        variante.name,
                        variante
                            .payloads
                            .iter()
                            .map(|carga| chave_exata_de_tipo(carga, apelidos))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .collect();
            format!("leque[{}]", variantes.join(";"))
        }
        Item::Function(function) => format!(
            "carinho[{}->{}]",
            function
                .params
                .iter()
                .map(|param| chave_exata_de_tipo(&param.ty, apelidos))
                .collect::<Vec<_>>()
                .join(","),
            function
                .ret_type
                .as_ref()
                .map(|ret| chave_exata_de_tipo(ret, apelidos))
                .unwrap_or_else(|| "nulo".to_string())
        ),
        Item::Struct(struct_decl) => format!(
            "ninho[{}]",
            struct_decl
                .fields
                .iter()
                .map(|campo| {
                    format!(
                        "{}:{}",
                        campo.name,
                        chave_exata_de_tipo(&campo.ty, apelidos)
                    )
                })
                .collect::<Vec<_>>()
                .join(";")
        ),
        Item::TypeAlias(alias) => {
            format!("apelido[{}]", chave_exata_de_tipo(&alias.target, apelidos))
        }
        Item::Const(constant) => {
            format!("eterno[{}]", chave_exata_de_tipo(&constant.ty, apelidos))
        }
        Item::Trait(trait_decl) => format!(
            "trato[{}]",
            trait_decl
                .methods
                .iter()
                .map(|metodo| metodo.name.clone())
                .collect::<Vec<_>>()
                .join(";")
        ),
    }
}

/// Duas unidades produziram o MESMO nome endereçado por conteúdo para
/// entidades estruturalmente distintas.
///
/// A premissa da deduplicação é "nome igual prova entidade igual". Quando ela
/// falha, descartar uma das cópias em silêncio faria a outra unidade ser
/// verificada contra a entidade errada. O nome de uma especialização de origem
/// builtin é cunhado no parse, a partir da GRAFIA do argumento de tipo, e a
/// canonicalização acontece depois — então duas unidades com um `Cor` local
/// cada produzem o mesmo nome para leques diferentes.
fn colisao_de_identidade_gerada(
    nome: &str,
    primeira: &str,
    segunda: &str,
    span: Span,
) -> PinkerError {
    PinkerError::Semantic {
        msg: format!(
            "identidade gerada '{}' foi produzida por '{}' e por '{}' para entidades diferentes; \
             a especialização de origem builtin é cunhada pela grafia do argumento de tipo, \
             antes da canonicalização, então unidades distintas com um tipo local homônimo \
             colidem",
            nome, primeira, segunda
        ),
        span,
    }
}

/// Fecho dos símbolos canônicos de módulo que a composição de fato alcança.
///
/// A semente é a superfície que cada importador pediu, não o corpo que a usa:
/// import inteiro materializa os itens importáveis do módulo, import seletivo
/// materializa o símbolo pedido. O fecho acrescenta tudo o que os itens já
/// incluídos referenciam — inclusive as dependências internas do módulo de
/// origem, que é justamente o que o import seletivo apagava.
fn fecho_alcancavel(graph: &ModuleGraph) -> HashSet<String> {
    let mut por_nome: HashMap<&str, (&Item, ModuleId)> = HashMap::new();
    for unit in graph.units() {
        if unit.is_root() {
            continue;
        }
        for item in &unit.items {
            if let Some(nome) = importable_item_name(item) {
                // PRIMEIRA ocorrência vence, como na projeção.
                //
                // Duas unidades podem materializar o mesmo nome endereçado por
                // conteúdo. Se este índice guardasse a última e a projeção
                // guardasse a primeira, o fecho seria calculado sobre uma cópia
                // e a outra seria emitida: a sobrevivente referenciaria um
                // apelido que ninguém materializou. Quem decide tem de ser um
                // só.
                por_nome.entry(nome).or_insert((item, unit.id));
            }
        }
    }

    let mut alcancaveis: HashSet<String> = HashSet::new();
    let mut pendentes: Vec<String> = Vec::new();
    let semear = |nome: String, alcancaveis: &mut HashSet<String>, pendentes: &mut Vec<String>| {
        if por_nome.contains_key(nome.as_str()) && alcancaveis.insert(nome.clone()) {
            pendentes.push(nome);
        }
    };

    for unit in graph.units() {
        // O que cada unidade importou. A raiz semeia a composição; um módulo
        // semeia o que ele mesmo pediu, e é assim que a cadeia transitiva
        // sobrevive sem virar reexport.
        for import in &unit.imports {
            let Some(origem) = graph.module(import.module.as_str()) else {
                continue;
            };
            match &import.symbol {
                Some(symbol) => semear(origem.canonical(symbol), &mut alcancaveis, &mut pendentes),
                None => {
                    for item in &origem.items {
                        if let Some(nome) = importable_item_name(item) {
                            if !nao_e_entidade_de_unidade(nome) {
                                semear(nome.to_string(), &mut alcancaveis, &mut pendentes);
                            }
                        }
                    }
                }
            }
        }
        // Relações de `impl` entram sempre; o alvo e o trato delas precisam
        // existir para que a validação de contrato possa rodar.
        for impl_decl in &unit.impls {
            semear(
                impl_decl.trait_name.clone(),
                &mut alcancaveis,
                &mut pendentes,
            );
            for referenciado in referencias_de_tipo(&impl_decl.target_ty) {
                semear(referenciado, &mut alcancaveis, &mut pendentes);
            }
        }
        // Corpos sintéticos de `trato` sempre materializados: o que eles usam
        // precisa vir junto. Vale para a checagem do default exatamente como
        // vale para o método — os dois são corpo copiado pelo parser, e um
        // corpo que sobrevive sem suas dependências não é validável.
        //
        // #567: a closure que um desses corpos cita é a MESMA coisa, um degrau
        // adiante. O corpo default a levantou para o topo na unidade de origem,
        // então o que a origem escreveu dentro dela — o auxiliar do módulo, a
        // constante, o tipo — só é alcançado por aqui. Sem este degrau o corpo
        // materializado chega íntegro e a dependência que ele realmente usa
        // fica para trás: `mo.ajudante` deixa de existir no programa por ter
        // sido citado uma indireção abaixo do método.
        for item in &unit.items {
            let Item::Function(function) = item else {
                continue;
            };
            let e_corpo_sintetico =
                crate::method_identity::parse_synthetic_trait_body_name(&function.name).is_some();
            if !e_corpo_sintetico && function.default_body_trait.is_none() {
                continue;
            }
            for referenciado in referencias_do_item(item) {
                semear(referenciado, &mut alcancaveis, &mut pendentes);
            }
        }
    }

    // A raiz NÃO semeia por referência crua. Semear pelo que ela escreveu faria
    // a forma qualificada `<módulo>.<entidade>` materializar qualquer unidade
    // carregada, autorizada ou não — a materialização passaria a decidir
    // visibilidade, que é exatamente a confusão que esta camada desfaz. O que a
    // raiz autorizou já entrou pelos imports dela, acima.

    while let Some(nome) = pendentes.pop() {
        let Some((item, unidade)) = por_nome.get(nome.as_str()).copied() else {
            continue;
        };
        for referenciado in referencias_do_item(item) {
            semear(referenciado, &mut alcancaveis, &mut pendentes);
        }
        // Método de `impl` do módulo cujo alvo é este tipo acompanha o tipo: o
        // despacho o encontra por identidade, nunca por nome, então nenhuma
        // referência textual o traria.
        let unit = graph.unit(unidade);
        for outro in &unit.items {
            let Item::Function(function) = outro else {
                continue;
            };
            let Some(facts) = &function.impl_facts else {
                continue;
            };
            if referencias_de_tipo(&facts.target_ty).contains(&nome) {
                semear(function.name.clone(), &mut alcancaveis, &mut pendentes);
            }
        }
    }

    alcancaveis
}

fn referencias_do_item(item: &Item) -> Vec<String> {
    let mut out = Vec::new();
    match item {
        Item::Function(function) => {
            if let Some(facts) = &function.impl_facts {
                out.extend(referencias_de_tipo(&facts.target_ty));
            }
            for param in &function.params {
                out.extend(referencias_de_tipo(&param.ty));
            }
            if let Some(ret) = &function.ret_type {
                out.extend(referencias_de_tipo(ret));
            }
            referencias_de_bloco(&function.body, &mut out);
        }
        Item::Const(constant) => {
            out.extend(referencias_de_tipo(&constant.ty));
            referencias_de_expr(&constant.init, &mut out);
        }
        Item::Struct(struct_decl) => {
            for field in &struct_decl.fields {
                out.extend(referencias_de_tipo(&field.ty));
            }
        }
        Item::TypeAlias(alias) => out.extend(referencias_de_tipo(&alias.target)),
        Item::Enum(enum_decl) => {
            for variant in &enum_decl.variants {
                for payload in &variant.payloads {
                    out.extend(referencias_de_tipo(payload));
                }
            }
        }
        Item::Trait(trait_decl) => {
            for method in &trait_decl.methods {
                for param in &method.params {
                    out.extend(referencias_de_tipo(&param.ty));
                }
                if let Some(ret) = &method.ret_type {
                    out.extend(referencias_de_tipo(ret));
                }
                let Some(body) = &method.body else {
                    continue;
                };
                // #567: a declaração do trato é contrato, não código. O corpo
                // default dela nunca é baixado — o que é baixado é a cópia que
                // cada `impl` materializa, e essa cópia traz a SUA closure.
                //
                // `SYNTHETIC_DEPENDENCIES ACCOMPANY ONLY THE MATERIALIZED
                // DEFAULT`: alcançar a closure pela declaração materializaria
                // um template que ninguém chama, com os identificadores livres
                // do corpo default ainda soltos — e o programa seria recusado
                // por um nome não declarado dentro de uma função que só existe
                // como molde. Os demais nomes do corpo continuam alcançando,
                // porque a cópia materializada os cita igual.
                let mut do_corpo = Vec::new();
                referencias_de_bloco(body, &mut do_corpo);
                do_corpo.retain(|nome| {
                    !nome.starts_with(crate::anonymous_identity::ANONYMOUS_CALLABLE_PREFIX)
                });
                out.extend(do_corpo);
            }
        }
    }
    out
}

fn referencias_de_tipo(ty: &Type) -> Vec<String> {
    let mut out = Vec::new();
    coletar_tipo(ty, &mut out);
    out
}

fn coletar_tipo(ty: &Type, out: &mut Vec<String>) {
    match ty {
        Type::Alias { name, .. } | Type::Struct { name, .. } | Type::Enum { name, .. } => {
            out.push(name.clone())
        }
        Type::ListEnum { element, .. } => out.push(element.clone()),
        Type::Applied { name, args, .. } => {
            out.push(name.clone());
            for arg in args {
                coletar_tipo(arg, out);
            }
        }
        Type::Map { key, value, .. } => {
            coletar_tipo(key, out);
            coletar_tipo(value, out);
        }
        Type::FixedArray { element, .. } => coletar_tipo(element, out),
        Type::Pointer { base, .. } => coletar_tipo(base, out),
        Type::Function { params, ret, .. } => {
            for param in params {
                coletar_tipo(param, out);
            }
            coletar_tipo(ret, out);
        }
        Type::Union { members, .. } => {
            for member in members {
                coletar_tipo(member, out);
            }
        }
        _ => {}
    }
}

fn referencias_de_bloco(block: &Block, out: &mut Vec<String>) {
    for stmt in &block.stmts {
        referencias_de_stmt(stmt, out);
    }
}

fn referencias_de_stmt(stmt: &Stmt, out: &mut Vec<String>) {
    match stmt {
        Stmt::Let(let_stmt) => {
            if let Some(ty) = &let_stmt.ty {
                out.extend(referencias_de_tipo(ty));
            }
            referencias_de_expr(&let_stmt.init, out);
        }
        Stmt::Return(return_stmt) => {
            if let Some(expr) = &return_stmt.expr {
                referencias_de_expr(expr, out);
            }
        }
        Stmt::Assign(assign_stmt) => {
            match &assign_stmt.target {
                AssignTarget::Ident(name) => out.push(name.clone()),
                AssignTarget::Deref(ptr) => referencias_de_expr(ptr, out),
                AssignTarget::FieldDeref { base, .. } => referencias_de_expr(base, out),
                AssignTarget::Index { base, index } => {
                    referencias_de_expr(base, out);
                    referencias_de_expr(index, out);
                }
            }
            referencias_de_expr(&assign_stmt.expr, out);
        }
        Stmt::If(if_stmt) => referencias_de_if(if_stmt, out),
        Stmt::While(while_stmt) => {
            referencias_de_expr(&while_stmt.condition, out);
            referencias_de_bloco(&while_stmt.body, out);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
        Stmt::InlineAsm(asm) => {
            for operand in &asm.operands {
                referencias_de_expr(&operand.value, out);
            }
        }
        Stmt::EnumMatch(enum_match) => {
            referencias_de_expr(&enum_match.scrutinee, out);
            for arm in &enum_match.arms {
                referencias_de_padrao(&arm.pattern, out);
                referencias_de_bloco(&arm.body, out);
            }
            if let Some(otherwise) = &enum_match.otherwise {
                referencias_de_bloco(otherwise, out);
            }
        }
        Stmt::UnionMatch(union_match) => {
            referencias_de_expr(&union_match.scrutinee, out);
            for arm in &union_match.arms {
                out.extend(referencias_de_tipo(&arm.member_type));
                referencias_de_bloco(&arm.body, out);
            }
        }
        Stmt::Falar(falar) => {
            for arg in &falar.args {
                referencias_de_expr(arg, out);
            }
        }
        Stmt::Expr(expr) => referencias_de_expr(expr, out),
    }
}

fn referencias_de_if(if_stmt: &IfStmt, out: &mut Vec<String>) {
    referencias_de_expr(&if_stmt.condition, out);
    referencias_de_bloco(&if_stmt.then_branch, out);
    match &if_stmt.else_branch {
        Some(ElseBlock::Block(block)) => referencias_de_bloco(block, out),
        Some(ElseBlock::If(inner)) => referencias_de_if(inner, out),
        None => {}
    }
}

fn referencias_de_padrao(pattern: &EnumPattern, out: &mut Vec<String>) {
    if let EnumPattern::Variant {
        enum_name,
        payloads,
        ..
    } = pattern
    {
        out.push(enum_name.clone());
        for payload in payloads {
            referencias_de_padrao(payload, out);
        }
    }
}

fn referencias_de_expr(expr: &Expr, out: &mut Vec<String>) {
    match &expr.kind {
        ExprKind::Ident(name) => out.push(name.clone()),
        // #532: identidade intrínseca não é referência a nome.
        ExprKind::Intrinsic(_) => {}
        ExprKind::Binary(lhs, _, rhs) => {
            referencias_de_expr(lhs, out);
            referencias_de_expr(rhs, out);
        }
        ExprKind::Unary(_, operand) | ExprKind::AddressOf(operand) => {
            referencias_de_expr(operand, out)
        }
        ExprKind::Call(callee, args) => {
            referencias_de_expr(callee, out);
            for arg in args {
                referencias_de_expr(arg, out);
            }
        }
        ExprKind::InternalMapIterCreate(inner) | ExprKind::InternalMapIterNextKey(inner) => {
            referencias_de_expr(inner, out)
        }
        ExprKind::FieldAccess { base, .. } => referencias_de_expr(base, out),
        ExprKind::Index { base, index } => {
            referencias_de_expr(base, out);
            referencias_de_expr(index, out);
        }
        ExprKind::Cast { expr, target } => {
            referencias_de_expr(expr, out);
            out.extend(referencias_de_tipo(target));
        }
        ExprKind::SizeOfType { target } | ExprKind::AlignOfType { target } => {
            out.extend(referencias_de_tipo(target))
        }
        ExprKind::IntLit(_) | ExprKind::BoolLit(_) | ExprKind::StringLit(_) => {}
    }
}
// @pinker-nav:end modulos.projecao.execucao

// @pinker-nav:start modulos.visibilidade.tratos
// @pinker-nav:domain modulos
// @pinker-nav:layer compilador
// @pinker-nav:summary tratos_visiveis_por_fonte deriva, do grafo já resolvido, o conjunto de tratos que cada unidade-fonte pode enxergar — os que ela declara mais os que seus imports autorizam — indexado por SourceId. Despacho de método não passa por identificador livre e portanto não é alcançado pela resolução nominal; sem esta visão, um trato declarado na raiz continuaria fornecendo método default ao corpo de um módulo que nunca o importou. O índice é vazio quando não há composição, e nesse caso nada é filtrado.
/// Tratos visíveis a cada unidade-fonte, por `SourceId`.
///
/// Um `x.metodo()` não menciona o trato: o despacho é por (tipo do receiver,
/// nome do método), e essa tabela é global sobre a agregação. A resolução
/// nominal canônica não a alcança, porque não há identificador livre para
/// reescrever. Sem restringir o despacho ao ambiente de quem escreveu a
/// chamada, `MODULE_IMPORTER_NON_INTERFERENCE` valeria para função, constante,
/// alias, ninho e leque, e falharia exatamente em trato.
pub fn tratos_visiveis_por_fonte(graph: &ModuleGraph) -> HashMap<SourceId, HashSet<String>> {
    let mut por_fonte: HashMap<SourceId, HashSet<String>> = HashMap::new();
    if !graph.has_modules() {
        // Sem composição não há a quem restringir, e um índice vazio é o sinal
        // de que o despacho segue exatamente como sempre seguiu.
        return por_fonte;
    }

    let tratos_da_unidade = |unit: &ModuleUnit| -> HashSet<String> {
        unit.items
            .iter()
            .filter_map(|item| match item {
                Item::Trait(trait_decl) => Some(trait_decl.name.clone()),
                _ => None,
            })
            .collect()
    };

    for unit in graph.units() {
        let mut visiveis = tratos_da_unidade(unit);
        for import in &unit.imports {
            let Some(origem) = graph.module(import.module.as_str()) else {
                continue;
            };
            let da_origem = tratos_da_unidade(origem);
            match &import.symbol {
                // NO_IMPLICIT_REEXPORT vale aqui também: o que entra é o trato
                // pedido, e nunca os tratos que a origem por sua vez importou.
                Some(symbol) => {
                    let canonical = origem.canonical(symbol);
                    if da_origem.contains(&canonical) {
                        visiveis.insert(canonical);
                    }
                }
                None => visiveis.extend(da_origem),
            }
        }
        por_fonte.insert(unit.source_id, visiveis);
    }

    por_fonte
}
// @pinker-nav:end modulos.visibilidade.tratos

// @pinker-nav:start modulos.visibilidade.fontes
// @pinker-nav:domain modulos
// @pinker-nav:layer compilador
// @pinker-nav:summary fontes_de_modulo devolve os SourceId das unidades que são módulo. Depois da resolução nominal canônica toda referência legítima de um módulo a entidade de usuário está qualificada, então grafia crua vinda de um módulo é builtin ou tentativa de alcançar a raiz; o índice permite à autoridade semântica recusar a segunda sem impedir a primeira. Vazio quando não há composição.
/// `SourceId` das unidades que são módulo.
pub fn fontes_de_modulo(graph: &ModuleGraph) -> HashSet<SourceId> {
    if !graph.has_modules() {
        return HashSet::new();
    }
    graph
        .units()
        .iter()
        .filter(|unit| !unit.is_root())
        .map(|unit| unit.source_id)
        .filter(|id| *id != SourceId::ROOT)
        .collect()
}
// @pinker-nav:end modulos.visibilidade.fontes
