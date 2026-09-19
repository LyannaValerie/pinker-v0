//! Carga e projeção de módulos Pinker (`cli.modules.import`), unidade
//! MAIN-3 da decomposição física #605.
//!
//! Movimento físico: as decisões, o estado e a ordem são os do entrypoint.
//! `main.rs` continua dono da orquestração; aqui mora só a implementação.

// @pinker-nav:start cli.modules.import
// @pinker-nav:domain modules
// @pinker-nav:layer cli
// @pinker-nav:summary parse_program_from_source tokenizes and parses a source string already bound to the unit's SourceId, handing the parser the import context resolved before the parse. base_dir_de returns the resolution directory of the `.pink` files; contexto_de_import answers, BEFORE the parse, the questions the parser cannot answer on its own: which names in `trazer X.y;` are a real Pinker module (via modulo_real_existe), which top-level identities this file's `trazer <modulo>;` bring in, which traits the explicit imports authorize as an `impl` target (#517), which synthetic closures the default bodies of those traits cite (#567) and whether any of those reads failed (import_incompleto). The last three come from a single read per module in contexto_de_import_com_pilha, best-effort, with no diagnostic of its own and DIRECT — only the module's own items enter, so no implicit re-export is born there. Reading each module builds ITS context by the same reckoning, because reading the neighbour with an empty context meant reading a different file from the one the loader reads next; the `visitando` stack is the analogue of `loading` and stops on a cycle without contributing. The absence of `<module>.pink` in the selective form is classified as the loader classifies it: a built-in family does not correspond to a file and its absence is legitimate; a module that does not exist is its own error. When a read fails or the requested module does not exist, the parser does not turn its own blindness into a refusal: import_incompleto suspends the refusal of `impl` over an unseen trait, and the loader produces the module's real error — missing, unreadable or in a cycle — with the right span and source. load_module_program registers the module's source in the SourceMap before parsing it, detects a cycle by comparing against the `loading` stack, recurses into the module's imports — skipping there the same built-in family the root program skips — and only then inserts the unit into the ModuleGraph, so that insertion order is already dependency order. carregar_e_projetar is the entry point: it builds the graph without discarding anything from the unit, validates each root import by the surface the importer comes to see (collision with a local item, collision between imports, duplicate import, nonexistent symbol) WITHOUT materializing any item, runs each unit's local modular validation with the family imports it wrote, resolves the graph into canonical identities and only then projects it into a single Program. A built-in family import still does not become an item and survives into the projection for the semantic authority to validate it. colher_closures_de_default accompanies the imported trait, in the same read that produced it: a default body containing a closure cites a synthetic function that the declaring unit's parser lifted to the top level, and handing over the body without it hands over a reference with no referent. It is a template pool — what materializes is the parser of the unit that did the `impl`, and only what the body reaches enters the program.
use super::*;

fn parse_program_from_source(
    source: &str,
    base_dir: &Path,
    generic_origin: GenericOrigin,
    source_id: SourceId,
) -> Result<ast::Program, PinkerError> {
    let mut lexer = Lexer::com_fonte(source, source_id);
    let tokens = lexer.tokenize()?;
    let contexto = contexto_de_import(&tokens, base_dir);
    let mut parser = Parser::com_contexto_de_import(tokens, generic_origin, contexto);
    // O parser deriva os spans dos tokens, que já vieram vinculados. O carimbo
    // aqui é a rede para span sintético que o parser tenha criado sem token de
    // origem; span já vinculado nunca é reatribuído.
    parser
        .parse()
        .map_err(|err| err.com_fonte_padrao(source_id))
}

/// Diretório a partir do qual os `.pink` importados são resolvidos.
///
/// É a mesma conta que `load_program_with_imports` sempre fez; virou função
/// porque a classificação de import precisa dela **antes** do parse.
pub(super) fn base_dir_de(source_file: &str) -> PathBuf {
    let source_path = PathBuf::from(source_file);
    source_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

/// Parte G — `NO_PRECANONICALIZATION_BEFORE_IMPORT_KIND_AUTHORITY`.
///
/// Responde, ANTES do parse, as duas perguntas que o parser não pode responder
/// sozinho. As perguntas vêm dele, que lê os tokens e não conhece filesystem; a
/// resposta vem daqui, que é o dono da resolução de módulos e usa exatamente as
/// mesmas consultas que `load_program_with_imports` faria em seguida.
///
/// Sem esta ordem, o parser canonicalizaria e o carregador descobriria os fatos
/// depois — e a canonicalização é irreversível. É a única forma de
/// `REAL_MODULE_X > BUILTIN_FAMILY_X` e de `TODA_IDENTIDADE_EXISTENTE > FAMÍLIA`
/// valerem de fato, e não só no carregador.
pub(super) fn contexto_de_import(tokens: &[Token], base_dir: &Path) -> ContextoDeImport {
    contexto_de_import_com_pilha(tokens, base_dir, &mut HashSet::new())
}

/// A mesma resposta, com a pilha de leitura que impede recursão infinita.
///
/// #517 — `MODULE_PREPASS_MUST_SEE_WHAT_THE_LOADER_SEES`. Ler o módulo vizinho
/// com contexto de import VAZIO era ler um arquivo diferente do que o
/// carregador lê logo em seguida: qualquer construção do módulo cujo parse
/// dependa do próprio contexto dele — inclusive, recursivamente, um `impl`
/// sobre trato importado — falhava aqui e só aqui, e o `.ok()?` engolia a
/// falha. O importador recebia então "trato não trazido por import" sobre um
/// módulo perfeitamente válido. O prepass agora monta o contexto de cada módulo
/// pela mesma conta, e a pilha `visitando` é o análogo de `loading` em
/// `load_module_program`: ciclo para aqui sem contribuir nada, e o carregador
/// produz o diagnóstico de ciclo na ordem histórica.
fn contexto_de_import_com_pilha(
    tokens: &[Token],
    base_dir: &Path,
    visitando: &mut HashSet<String>,
) -> ContextoDeImport {
    let mut nomes_importados = HashSet::new();
    let mut tratos_importados: HashMap<String, ast::TraitDecl> = HashMap::new();
    let mut closures_de_default_importadas: HashMap<String, ast::FunctionDecl> = HashMap::new();
    let mut import_incompleto = false;

    // `trazer <modulo>;` traz os itens de topo do próprio módulo. Famílias
    // built-in ficam de fora de propósito: essa forma nunca carregou módulo,
    // nem antes da Parte G.
    for modulo in Parser::modulos_trazidos_inteiros(tokens) {
        // #532: quando a família governa o nome, `trazer <familia>;` não
        // corresponde a arquivo nenhum — G-517-1, ausência legítima. Quando
        // existe `<nome>.pink`, o módulo real vence e a forma inteira traz os
        // itens dele, exatamente como a seletiva já fazia.
        if pinker_v0::intrinsics::public_surface::familia_governa(
            modulo.as_str(),
            modulo_real_existe(base_dir, &modulo),
        ) {
            continue;
        }
        let Some(programa) = ler_modulo_best_effort(base_dir, &modulo, visitando) else {
            import_incompleto = true;
            continue;
        };
        nomes_importados.extend(
            programa
                .items
                .iter()
                .filter_map(importable_item_name)
                .map(ToOwned::to_owned),
        );
        let mut trouxe_trato = false;
        for trait_decl in tratos_do_programa(&programa) {
            tratos_importados.insert(trait_decl.name.clone(), trait_decl.clone());
            trouxe_trato = true;
        }
        // O pool acompanha o trato trazido: sem trato não há default a
        // materializar aqui, e nada a acompanhar.
        if trouxe_trato {
            colher_closures_de_default(&programa, &mut closures_de_default_importadas);
        }
    }

    // `trazer M.a, b;` traz os membros pedidos, e só eles. Consulta o disco
    // apenas quando existe módulo real, pela mesma precedência
    // `REAL_MODULE_X > BUILTIN_FAMILY_X` que decide `modulos_reais`.
    for (modulo, membros) in Parser::membros_trazidos_seletivamente(tokens) {
        if !modulo_real_existe(base_dir, &modulo) {
            // Ausência de `<módulo>.pink` significa duas coisas MUITO
            // diferentes, e a classificação é a mesma que `carregar_e_projetar`
            // aplica logo em seguida — não uma segunda política.
            //
            // Família built-in: a ausência é legítima, a família não
            // corresponde a arquivo nenhum e nunca correspondeu. Nada a ler,
            // nada a marcar.
            //
            // Módulo que simplesmente não existe: quem não leu não pode dizer
            // "não existe". Sem esta marca o prepass entregava ao parser uma
            // superfície vazia com cara de completa, e `impl` sobre um trato
            // desse módulo era recusado ANTES de o carregador dizer "módulo não
            // encontrado" — o erro autoritativo e o span do import sumiam.
            if !pinker_v0::intrinsics::public_surface::familia_conhecida(modulo.as_str()) {
                import_incompleto = true;
            }
            continue;
        }
        let Some(programa) = ler_modulo_best_effort(base_dir, &modulo, visitando) else {
            import_incompleto = true;
            continue;
        };
        let mut pediu_trato = false;
        for trait_decl in tratos_do_programa(&programa) {
            if membros.iter().any(|membro| *membro == trait_decl.name) {
                tratos_importados.insert(trait_decl.name.clone(), trait_decl.clone());
                pediu_trato = true;
            }
        }
        // O pool acompanha o trato pedido, não o módulo inteiro: sem trato
        // importado desta unidade não há default a materializar aqui.
        if pediu_trato {
            colher_closures_de_default(&programa, &mut closures_de_default_importadas);
        }
    }

    ContextoDeImport {
        modulos_reais: Parser::familias_candidatas(tokens)
            .into_iter()
            .filter(|module| modulo_real_existe(base_dir, module))
            .collect(),
        nomes_importados,
        tratos_importados,
        closures_de_default_importadas,
        import_incompleto,
    }
}

fn tratos_do_programa(programa: &ast::Program) -> impl Iterator<Item = &ast::TraitDecl> {
    programa.items.iter().filter_map(|item| match item {
        ast::Item::Trait(trait_decl) => Some(trait_decl),
        _ => None,
    })
}

/// #567 — `MATERIALIZED_DEFAULT MUST_NOT_LOSE_SYNTHETIC_DEPENDENCIES`.
///
/// O corpo default de um trato é entregue ao importador como árvore; quando ele
/// contém uma closure, essa árvore cita uma função sintética que o parser da
/// unidade declarante levantou para o topo. Entregar o trato sem essas funções
/// entrega uma referência sem referente.
///
/// Vem da MESMA leitura que já produziu o trato — a autoridade de import, dona
/// da resolução de módulos —, e não de uma segunda política dentro do parser.
/// As chaves são os nomes anônimos canônicos: eles já codificam integralmente a
/// proveniência da unidade, então dois módulos nunca disputam a mesma entrada e
/// nenhuma grafia participa.
///
/// É um pool, não materialização: quem decide o que entra no programa é a
/// materialização do default, e ela copia apenas o que o corpo alcança.
fn colher_closures_de_default(
    programa: &ast::Program,
    destino: &mut HashMap<String, ast::FunctionDecl>,
) {
    for item in &programa.items {
        let ast::Item::Function(function) = item else {
            continue;
        };
        if !pinker_v0::anonymous_identity::is_anonymous_callable_name(&function.name) {
            continue;
        }
        destino
            .entry(function.name.clone())
            .or_insert_with(|| function.clone());
    }
}

/// Leitura DIRETA de um módulo vizinho, sem diagnóstico próprio.
///
/// Best-effort: módulo ausente, ilegível, com erro de sintaxe ou já na pilha de
/// leitura não contribui nada e não interrompe nada — o carregador refaz a
/// mesma leitura logo em seguida e produz o erro histórico, na ordem histórica.
/// Para que isso seja verdade e não promessa, quem falha aqui marca
/// `import_incompleto`, e o parser deixa de recusar por conta própria uma
/// superfície que ele não pôde enxergar.
///
/// Só a leitura direta importa: `trazer <modulo>;` traz os itens do próprio
/// módulo, nunca os que ele importou.
fn ler_modulo_best_effort(
    base_dir: &Path,
    modulo: &str,
    visitando: &mut HashSet<String>,
) -> Option<ast::Program> {
    if !visitando.insert(modulo.to_string()) {
        return None;
    }
    let programa = ler_modulo_com_contexto(base_dir, modulo, visitando);
    visitando.remove(modulo);
    programa
}

fn ler_modulo_com_contexto(
    base_dir: &Path,
    modulo: &str,
    visitando: &mut HashSet<String>,
) -> Option<ast::Program> {
    let fonte = fs::read_to_string(base_dir.join(format!("{}.pink", modulo))).ok()?;
    let tokens = Lexer::new(&fonte).tokenize().ok()?;
    let contexto = contexto_de_import_com_pilha(&tokens, base_dir, visitando);
    Parser::com_contexto_de_import(tokens, GenericOrigin::module(modulo), contexto)
        .parse()
        .ok()
}

fn importable_item_name(item: &ast::Item) -> Option<&str> {
    match item {
        ast::Item::Function(function) => Some(function.name.as_str()),
        ast::Item::Const(constant) => Some(constant.name.as_str()),
        ast::Item::Struct(struct_decl) => Some(struct_decl.name.as_str()),
        ast::Item::TypeAlias(alias) => Some(alias.name.as_str()),
        ast::Item::Enum(enum_decl) => Some(enum_decl.name.as_str()),
        ast::Item::Trait(trait_decl) => Some(trait_decl.name.as_str()),
    }
}

#[allow(clippy::too_many_arguments)]
fn load_module_program(
    module: &str,
    base_dir: &Path,
    source_path: &Path,
    raiz_fisica: Option<&Path>,
    import_span: Span,
    loading: &mut Vec<String>,
    sources: &mut SourceMap,
    graph: &mut ModuleGraph,
) -> Result<(), PinkerError> {
    if graph.module_id(module).is_some() {
        return Ok(());
    }
    if loading.iter().any(|entry| entry == module) {
        return Err(PinkerError::Semantic {
            msg: format!(
                "ciclo de módulos detectado: {} -> {}",
                loading.join(" -> "),
                module
            ),
            span: import_span,
        });
    }

    let module_path = base_dir.join(format!("{}.pink", module));
    let source = fs::read_to_string(&module_path).map_err(|_| PinkerError::Semantic {
        msg: format!(
            "módulo '{}' não encontrado a partir de '{}'",
            module,
            source_path.display()
        ),
        span: import_span,
    })?;
    // A unidade-fonte é registrada ANTES do parse: é o registro que dá ao
    // léxico do módulo a identidade que todo span dele vai carregar.
    //
    // Um arquivo que importa a si mesmo é o mesmo texto sob duas chaves. Ele
    // continua sendo um ciclo e continua sendo recusado, mas reusar a fonte
    // primária evita que o diagnóstico rotule o arquivo principal como se
    // viesse de outro lugar.
    let module_source_id = match (raiz_fisica, fs::canonicalize(&module_path).ok()) {
        (Some(raiz), Some(fisica)) if raiz == fisica => SourceId::ROOT,
        _ => sources.register_module(module, module_path.display().to_string(), source.clone()),
    };
    let program = parse_program_from_source(
        &source,
        base_dir,
        GenericOrigin::module(module),
        module_source_id,
    )
    .map_err(|err| match err {
        PinkerError::Lexer { msg, span }
        | PinkerError::Parse { msg, span }
        | PinkerError::Expected {
            expected: msg,
            span,
            ..
        }
        | PinkerError::Semantic { msg, span } => PinkerError::Semantic {
            msg: format!("falha ao ler módulo '{}': {}", module, msg),
            span,
        },
        other => other,
    })?;

    loading.push(module.to_string());
    // O módulo é inserido no grafo DEPOIS de recursar nos imports dele, de
    // modo que a ordem de inserção já é ordem de dependência.
    let module_imports = program.imports.clone();
    for import in &module_imports {
        // Parte G: a mesma precedência que o programa raiz aplica vale dentro
        // de um módulo. `trazer arquivo;` é import de família e não procura
        // `arquivo.pink`; a forma seletiva cede a vez a um módulo real que
        // exista de fato. Sem isto, a superfície aprovada existiria só no
        // arquivo raiz e um módulo que a usasse levaria "módulo 'arquivo' não
        // encontrado" — que é o comportamento histórico, mas historicamente
        // `trazer arquivo;` num módulo não tinha o que oferecer.
        if pinker_v0::intrinsics::public_surface::familia_governa(
            import.module.as_str(),
            modulo_real_existe(base_dir, &import.module),
        ) {
            continue;
        }
        load_module_program(
            import.module.as_str(),
            base_dir,
            &module_path,
            raiz_fisica,
            import.span,
            loading,
            sources,
            graph,
        )?;
    }
    loading.pop();
    graph.insert_module(
        module,
        module_source_id,
        module_path.display().to_string(),
        program,
    );
    Ok(())
}

/// Parte G: existe um módulo `.pink` real com este nome ao lado da fonte?
///
/// Só a forma seletiva pergunta. A resposta decide precedência de import, e a
/// pergunta é a mesma que `load_module_program` faria em seguida — não é uma
/// busca nova, é a busca histórica antecipada para poder ceder a vez a ela.
fn modulo_real_existe(base_dir: &Path, module: &str) -> bool {
    base_dir.join(format!("{}.pink", module)).is_file()
}

/// Carrega a composição preservando a unidade modular e devolve, junto do
/// programa projetado, o grafo de onde ele saiu.
///
/// A projeção continua sendo a mesma de sempre — o que mudou é que ela agora é
/// derivada de um grafo que ainda existe depois dela. Antes, a unidade era
/// destruída no ato do carregamento e não havia de onde derivar nada.
pub(super) fn carregar_e_projetar(
    source_file: &str,
    root_program: ast::Program,
    sources: &mut SourceMap,
) -> Result<(ast::Program, ModuleGraph), PinkerError> {
    let mut graph = ModuleGraph::new();
    graph.insert_root(SourceId::ROOT, source_file, root_program.clone());

    if root_program.imports.is_empty() {
        // Sem import não há composição: a unidade É o programa, e nada nele
        // muda de nome. Programa de arquivo único atravessa este caminho
        // exatamente como sempre atravessou.
        return Ok((root_program, graph));
    }

    let source_path = PathBuf::from(source_file);
    let base_dir = base_dir_de(source_file);
    let raiz_fisica = fs::canonicalize(&source_path).ok();

    let mut loading = Vec::new();
    let mut seen_imports = HashSet::new();
    // Superfície visível ao importador. É onde colisão de import é decidida —
    // e agora é SÓ isso que ela decide: duas entidades homônimas em módulos
    // distintos não colidem mais como símbolo, apenas disputam a grafia que o
    // importador pediu para enxergar.
    let mut imported_names = HashMap::<String, Span>::new();
    let local_names: HashSet<String> = root_program
        .items
        .iter()
        .filter_map(importable_item_name)
        .map(ToOwned::to_owned)
        .collect();

    for import in &root_program.imports {
        // Fases 186–188 — famílias built-in importáveis não correspondem a
        // arquivo .pink. As intrínsecas já estão disponíveis globalmente; basta
        // pular a carga de módulo.
        //
        // Parte G — `REAL_MODULE_X > BUILTIN_FAMILY_X`.
        //
        // A família built-in não corresponde a arquivo `.pink`, mas o nome dela
        // não é reservado: um módulo real chamado `texto.pink` existia antes
        // desta Parte e continua vencendo. A precedência NÃO pode ser decidida
        // perguntando "a família exporta este membro?" — isso arrancaria de um
        // módulo histórico qualquer export cujo nome coincidisse com o de um
        // membro aprovado. Pergunta-se primeiro se o módulo resolve.
        //
        // `trazer X;` (família inteira) nunca carregou módulo, nem antes desta
        // Parte; só a forma seletiva tinha semântica de módulo, e é só ela que
        // precisa consultar o disco. Isso mantém intacto o invariante de que a
        // superfície aprovada não procura `<familia>.pink`.
        if pinker_v0::intrinsics::public_surface::familia_governa(
            import.module.as_str(),
            modulo_real_existe(&base_dir, &import.module),
        ) {
            if let Some(symbol) = &import.symbol {
                // Colisão com item de topo é decidida pela autoridade semântica
                // — a mesma que o caminho de biblioteca atravessa. Repetir a
                // regra aqui daria duas políticas para uma pergunta só, que é
                // exatamente o defeito que a Parte G acabou de fechar.
                pinker_v0::semantic::validate_family_import_collision(import, &root_program.items)?;
                if let Some(previous_span) = imported_names.get(symbol) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "colisão de nome no import: '{}' trazido por múltiplos módulos",
                            symbol
                        ),
                        span: previous_span.merge(import.span),
                    });
                }
                imported_names.insert(symbol.clone(), import.span);
            }
            continue;
        }

        let import_key = format!(
            "{}::{}",
            import.module,
            import.symbol.as_deref().unwrap_or("*")
        );
        if !seen_imports.insert(import_key) {
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

        load_module_program(
            import.module.as_str(),
            &base_dir,
            &source_path,
            raiz_fisica.as_deref(),
            import.span,
            &mut loading,
            sources,
            &mut graph,
        )?;
        let module_program = graph
            .module(import.module.as_str())
            .expect("módulo carregado");

        // A partir daqui a checagem é de SUPERFÍCIE, não de materialização.
        // Nenhum item é clonado para dentro da raiz: o que se decide é qual
        // grafia o importador passa a enxergar.
        let declarar_superficie =
            |nome: &str, span: Span, imported_names: &mut HashMap<String, Span>| {
                if local_names.contains(nome) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "colisão de nome no import: '{}' já existe no arquivo principal",
                            nome
                        ),
                        span,
                    });
                }
                if let Some(previous_span) = imported_names.get(nome) {
                    return Err(PinkerError::Semantic {
                        msg: format!(
                            "colisão de nome no import: '{}' trazido por múltiplos módulos",
                            nome
                        ),
                        span: previous_span.merge(span),
                    });
                }
                imported_names.insert(nome.to_string(), span);
                Ok(())
            };

        match &import.symbol {
            Some(symbol) => {
                declarar_superficie(symbol, import.span, &mut imported_names)?;
                let existe = module_program
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
            }
            None => {
                let nomes: Vec<String> = module_program
                    .items
                    .iter()
                    .filter_map(importable_item_name)
                    .map(ToOwned::to_owned)
                    .collect();
                for nome in nomes {
                    declarar_superficie(&nome, import.span, &mut imported_names)?;
                }
            }
        }
    }

    // MODULE_LOCAL_VALIDATION antes de qualquer projeção: as regras cujo
    // gatilho é a própria unidade rodam enquanto a unidade ainda existe.
    //
    // A unidade é apresentada com os imports de família que ela escreveu — os
    // mesmos que sobreviveriam numa raiz, pelo mesmo critério de precedência.
    // Import que resolve para módulo real não é import de família e já foi
    // validado pelo carregador e pelo ambiente.
    for unit in graph.units() {
        if unit.is_root() {
            continue;
        }
        let mut unidade = unit.to_program();
        unidade.imports.retain(|import| {
            pinker_v0::intrinsics::public_surface::familia_governa(
                import.module.as_str(),
                modulo_real_existe(&base_dir, &import.module),
            )
        });
        semantic::check_module_unit(&unidade)?;
    }

    let resolvido = module_resolve::resolver_grafo(&graph)?;
    let program = module_resolve::projetar_programa(&resolvido)?;
    Ok((program, resolvido))
}

// @pinker-nav:end cli.modules.import
