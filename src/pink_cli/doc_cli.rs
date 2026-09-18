//! Comandos `pink doc` (`cli.doc.consulta`, `cli.doc.sincronizacao`,
//! `cli.doc.mudancas`, `cli.doc.verificacao`), unidade MAIN-2 da
//! decomposição física #605.
//!
//! Movimento físico: as decisões, o estado e a ordem são os do entrypoint.
//! `main.rs` continua dono da orquestração; aqui mora só a implementação.

// @pinker-nav:start cli.doc.consulta
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary load_doc_config carrega doc::DocConfig::load (sai com 1 em erro); run_doc despacha DocSub (Marco/Mostrar/Listar/Buscar/Rota/Sincronizar/Verificar) para as funções correspondentes; scan_docs varre docs/ via doc_index::DocIndex::scan; load_doc_catalog lê o catálogo gerado; write_atomic é o único mecanismo desta base que grava atomicamente — escreve um arquivo `.jsonl.tmp` e usa fs::rename por cima do caminho final, usado pelas rotinas de sincronização (não pelas consultas abaixo); run_doc_mostrar/run_doc_listar/run_doc_buscar/run_doc_rota e print_doc_results_json apenas leem o catálogo e imprimem resultados em texto ou JSON, sem escrever em disco.
use super::*;

pub(super) fn load_doc_config(repo_root: &Path) -> doc::DocConfig {
    match doc::DocConfig::load(repo_root) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

pub(super) fn run_doc(config: DocConfigCli) -> i32 {
    let repo_root = Path::new(&config.repo);
    let doc_config = match doc::DocConfig::load(repo_root) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            return EXIT_CATALOG;
        }
    };

    match config.sub {
        DocSub::Marco => {
            let github = &doc_config.github;
            let limite = if github.baseline_inclusive {
                "inclusivo"
            } else {
                "exclusivo"
            };
            println!("Trama Pinker — marco documental");
            println!("  modo:    {}", github.mode);
            println!("  marco:   PR #{}, {}", github.baseline_pr, limite);
            println!("  commit:  {}", github.baseline_commit);
            println!("  docs:    {}", doc_config.generated.docs_index);
            println!("  código:  {}", doc_config.generated.code_index);
            EXIT_OK
        }
        DocSub::Mostrar { id } => run_doc_mostrar(repo_root, &doc_config, &id, config.json),
        DocSub::Listar { territorio } => {
            run_doc_listar(repo_root, &doc_config, &territorio, config.json)
        }
        DocSub::Buscar { consulta } => run_doc_buscar(
            repo_root,
            &doc_config,
            &consulta,
            config.json,
            config.limite,
        ),
        DocSub::Rota { consulta } => run_doc_rota(
            repo_root,
            &doc_config,
            &consulta,
            config.json,
            config.limite,
        ),
        DocSub::Sincronizar => run_doc_sincronizar(repo_root, &doc_config),
        DocSub::Verificar => run_doc_verificar(repo_root, &doc_config),
    }
}

fn scan_docs(repo_root: &Path) -> doc_index::DocIndex {
    let docs_root = repo_root.join("docs");
    match doc_index::DocIndex::scan(&docs_root) {
        Ok(index) => index,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

/// Carrega o catálogo documental versionado (superfície de consulta — §5).
fn load_doc_catalog(
    repo_root: &Path,
    config: &doc::DocConfig,
) -> Result<doc_index::DocCatalog, i32> {
    let path = repo_root.join(&config.generated.docs_index);
    match doc_index::DocCatalog::load(&path) {
        Ok(catalog) => Ok(catalog),
        Err(err) => {
            eprintln!("{err}");
            Err(EXIT_CATALOG)
        }
    }
}

/// Escrita atômica: grava em arquivo temporário e renomeia por cima (§8).
pub(super) fn write_atomic(path: &Path, content: &str) -> Result<(), i32> {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!("Falha ao criar '{}': {}", parent.display(), err);
            return Err(1);
        }
    }
    let tmp = path.with_extension("jsonl.tmp");
    if let Err(err) = fs::write(&tmp, content) {
        eprintln!("Falha ao gravar temporário '{}': {}", tmp.display(), err);
        return Err(1);
    }
    if let Err(err) = fs::rename(&tmp, path) {
        eprintln!(
            "Falha ao substituir '{}' por '{}': {}",
            path.display(),
            tmp.display(),
            err
        );
        let _ = fs::remove_file(&tmp);
        return Err(1);
    }
    Ok(())
}

fn run_doc_mostrar(repo_root: &Path, config: &doc::DocConfig, id: &str, json: bool) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if let Some(section) = catalog.section(id) {
        let path = repo_root.join(&section.file);
        let source = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                eprintln!(
                    "E-DOC-SOURCE\nFalha ao ler fonte '{}': {}",
                    path.display(),
                    err
                );
                return EXIT_SOURCE;
            }
        };
        // Valida que a âncora ainda delimita o intervalo registrado (§5).
        if !doc_index::validate_section_anchor(&source, section) {
            eprintln!(
                "E-DOC-SOURCE\nÂncora divergente para '{}' em {}; catálogo desatualizado. Rode `pink doc sincronizar`.",
                section.id, section.file
            );
            return EXIT_SOURCE;
        }
        let lines: Vec<&str> = source.lines().collect();
        let start = section.start.saturating_sub(1);
        let end = section.end.min(lines.len());
        let content: Vec<&str> = lines[start..end].to_vec();
        if json {
            let mut out = String::new();
            out.push_str(&format!("{{\"schema\":{}", doc_index::CATALOG_SCHEMA));
            out.push_str(",\"record\":\"section\"");
            out.push_str(&format!(",\"id\":{}", json_escape(&section.id)));
            out.push_str(&format!(",\"document\":{}", json_escape(&section.document)));
            out.push_str(&format!(",\"file\":{}", json_escape(&section.file)));
            out.push_str(&format!(",\"start\":{}", section.start));
            out.push_str(&format!(",\"end\":{}", section.end));
            out.push_str(&format!(",\"title\":{}", json_escape(&section.title)));
            if !section.summary.is_empty() {
                out.push_str(&format!(",\"summary\":{}", json_escape(&section.summary)));
            }
            out.push_str(&format!(
                ",\"content\":{}",
                json_escape(&content.join("\n"))
            ));
            out.push('}');
            println!("{out}");
        } else {
            println!(
                "# {} — {}:{}-{}",
                section.id, section.file, section.start, section.end
            );
            if !section.summary.is_empty() {
                println!("# {}", section.summary);
            }
            println!();
            for line in &content {
                println!("{line}");
            }
        }
        return EXIT_OK;
    }

    if let Some(doc) = catalog.document(id) {
        let sections = catalog.sections_of(&doc.id);
        if json {
            let mut out = String::new();
            out.push_str(&format!("{{\"schema\":{}", doc_index::CATALOG_SCHEMA));
            out.push_str(",\"record\":\"document\"");
            out.push_str(&format!(",\"id\":{}", json_escape(&doc.id)));
            out.push_str(&format!(",\"domain\":{}", json_escape(&doc.domain)));
            out.push_str(&format!(",\"kind\":{}", json_escape(&doc.kind)));
            out.push_str(&format!(",\"status\":{}", json_escape(&doc.status)));
            out.push_str(&format!(",\"file\":{}", json_escape(&doc.file)));
            if !doc.canonical_for.is_empty() {
                out.push_str(&format!(
                    ",\"canonical_for\":{}",
                    json_string_array(&doc.canonical_for)
                ));
            }
            let ids: Vec<String> = sections.iter().map(|s| s.id.clone()).collect();
            out.push_str(&format!(",\"sections\":{}", json_string_array(&ids)));
            out.push('}');
            println!("{out}");
        } else {
            println!("# documento {} ({})", doc.id, doc.kind);
            println!("  território: {}", doc.domain);
            println!("  arquivo:    {}", doc.file);
            if !doc.canonical_for.is_empty() {
                println!("  autoridade: {}", doc.canonical_for.join(", "));
            }
            if sections.is_empty() {
                println!("  seções:     (nenhuma âncora)");
            } else {
                println!("  seções:");
                for section in sections {
                    println!(
                        "    - {} ({}:{}-{})",
                        section.id, section.file, section.start, section.end
                    );
                }
            }
        }
        return EXIT_OK;
    }

    eprintln!("id documental não encontrado: '{id}'. Tente `pink doc buscar \"{id}\"`.");
    EXIT_NORESULT
}

fn run_doc_listar(repo_root: &Path, config: &doc::DocConfig, territorio: &str, json: bool) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let docs = catalog.documents_in_domain(territorio);
    if docs.is_empty() {
        if json {
            println!(
                "{{\"domain\":{},\"documents\":[]}}",
                json_escape(territorio)
            );
        } else {
            eprintln!("Nenhum documento estrutural no território '{territorio}'.");
        }
        return EXIT_NORESULT;
    }
    if json {
        let ids: Vec<String> = docs.iter().map(|d| d.id.clone()).collect();
        println!(
            "{{\"domain\":{},\"documents\":{}}}",
            json_escape(territorio),
            json_string_array(&ids)
        );
    } else {
        println!("Território '{territorio}':");
        for doc in docs {
            println!("- {} [{}] {}", doc.id, doc.kind, doc.file);
            for section in catalog.sections_of(&doc.id) {
                println!("    · {} — {}", section.id, section.title);
            }
        }
    }
    EXIT_OK
}

fn run_doc_buscar(
    repo_root: &Path,
    config: &doc::DocConfig,
    consulta: &str,
    json: bool,
    limite: Option<usize>,
) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_BUSCAR);
    let hits = catalog.search(consulta);
    if hits.is_empty() {
        if json {
            print_doc_results_json(consulta, &[], None);
        } else {
            eprintln!("Nenhuma seção encontrada para: {consulta}");
        }
        return EXIT_NORESULT;
    }
    let shown: Vec<&doc_index::SearchHit> = hits.iter().take(limit).collect();
    if json {
        print_doc_results_json(consulta, &shown, None);
    } else {
        for hit in &shown {
            println!("{}", hit.id);
            println!("   {}", hit.summary);
            println!("   {}:{}-{}", hit.file, hit.start, hit.end);
        }
    }
    EXIT_OK
}

fn run_doc_rota(
    repo_root: &Path,
    config: &doc::DocConfig,
    consulta: &str,
    json: bool,
    limite: Option<usize>,
) -> i32 {
    let catalog = match load_doc_catalog(repo_root, config) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_ROTA);
    let hits = catalog.search(consulta);
    if hits.is_empty() {
        if json {
            print_doc_results_json(consulta, &[], None);
        } else {
            println!("Consulta: {consulta}");
            eprintln!("Nenhuma rota encontrada. Tente `pink doc buscar`.");
        }
        return EXIT_NORESULT;
    }
    let shown: Vec<&doc_index::SearchHit> = hits.iter().take(limit).collect();
    let next = format!("pink doc mostrar {}", shown[0].id);
    if json {
        print_doc_results_json(consulta, &shown, Some(&next));
    } else {
        println!("Consulta: {consulta}");
        println!();
        for (i, hit) in shown.iter().enumerate() {
            println!("{}. {}", i + 1, hit.id);
            println!("   {}", hit.summary);
            println!("   {}:{}-{}", hit.file, hit.start, hit.end);
        }
        println!();
        println!("Use:");
        println!("    {next}");
    }
    EXIT_OK
}

/// Saída JSON estável de resultados de `buscar`/`rota` (§7.2).
fn print_doc_results_json(consulta: &str, hits: &[&doc_index::SearchHit], next: Option<&str>) {
    let results: Vec<String> = hits
        .iter()
        .map(|h| {
            let mut o = String::from("{");
            o.push_str(&format!("\"id\":{}", json_escape(&h.id)));
            o.push_str(&format!(",\"score\":{}", h.score));
            o.push_str(&format!(",\"file\":{}", json_escape(&h.file)));
            o.push_str(&format!(",\"start\":{}", h.start));
            o.push_str(&format!(",\"end\":{}", h.end));
            o.push_str(&format!(",\"summary\":{}", json_escape(&h.summary)));
            o.push_str(&format!(
                ",\"next\":{}",
                json_escape(&format!("pink doc mostrar {}", h.id))
            ));
            o.push('}');
            o
        })
        .collect();
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", doc_index::CATALOG_SCHEMA));
    out.push_str(&format!(",\"query\":{}", json_escape(consulta)));
    out.push_str(&format!(
        ",\"normalized\":{}",
        json_escape(&pinker_v0::text_norm::normalize(consulta))
    ));
    out.push_str(&format!(",\"results\":[{}]", results.join(",")));
    if let Some(next) = next {
        out.push_str(&format!(",\"next\":{}", json_escape(next)));
    }
    out.push('}');
    println!("{out}");
}
// @pinker-nav:end cli.doc.consulta

// @pinker-nav:start cli.doc.sincronizacao
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary run_doc_sincronizar reescaneia docs/ e manifestos de mudança, roda verify() em ambos e só prossegue se não houver divergência; calcula o plano de projeções (projection::plan), grava o catálogo via write_atomic, grava o histórico mecânico via write_ledger e aplica as escritas do plano (fs::write por projeção) — é a rotina que efetivamente altera arquivos em disco nesta região documental.
fn run_doc_sincronizar(repo_root: &Path, config: &doc::DocConfig) -> i32 {
    let index = scan_docs(repo_root);
    // Validação completa antes de qualquer escrita (§8): uma árvore inválida
    // nunca sobrescreve o último catálogo válido.
    let problems = index.verify();
    if !problems.is_empty() {
        eprintln!(
            "E-DOC-SYNC: {} divergência(s); catálogo e projeções NÃO alterados.",
            problems.len()
        );
        for problem in &problems {
            eprintln!("  - {problem}");
        }
        return EXIT_SOURCE;
    }
    let manifests = change::Manifests::load(&repo_root.join(".pinker/changes"));
    if !manifests.problems.is_empty() {
        eprintln!(
            "E-DOC-SYNC: {} problema(s) em manifestos; nada alterado.",
            manifests.problems.len()
        );
        for problem in &manifests.problems {
            eprintln!("  - {problem}");
        }
        return EXIT_SOURCE;
    }

    // Renderiza tudo em memória antes de tocar o disco.
    let rendered = index.render_jsonl();
    let catalog_path = repo_root.join(&config.generated.docs_index);

    // Projeções documentais (§12): calculadas em memória e validadas.
    let plan = match projection::plan(repo_root, config, &manifests) {
        Ok(plan) => plan,
        Err(err) => {
            eprintln!("{err}");
            return EXIT_SOURCE;
        }
    };

    // Escrita atômica do catálogo.
    if let Err(code) = write_atomic(&catalog_path, &rendered) {
        return code;
    }
    if let Err(code) = write_ledger(repo_root, &manifests) {
        return code;
    }
    // Aplica as projeções (regiões geradas) idempotentemente.
    for change in &plan.writes {
        if let Err(err) = fs::write(&change.path, &change.content) {
            eprintln!(
                "Falha ao gravar projeção '{}': {}",
                change.path.display(),
                err
            );
            return 1;
        }
    }

    println!(
        "Catálogo documental sincronizado: {} ({} documentos, {} seções).",
        config.generated.docs_index,
        index.documents.len(),
        index.sections.len()
    );
    println!(
        "Histórico mecânico sincronizado: {} ({} manifesto(s)).",
        doc::CHANGE_LEDGER_RELATIVE_PATH,
        manifests.changes.len()
    );
    if !plan.writes.is_empty() {
        println!("Projeções aplicadas: {}.", plan.summary());
    }
    EXIT_OK
}
// @pinker-nav:end cli.doc.sincronizacao

// @pinker-nav:start cli.doc.mudancas
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary CHANGE_LEDGER_RELATIVE_PATH é o caminho canônico do histórico mecânico; write_ledger renderiza os manifestos já aceitos e grava via write_atomic, ou remove o arquivo quando não há manifestos. Não existe caminho de autoria: nenhum manifesto novo é criado a partir de corpo de PR.
fn write_ledger(repo_root: &Path, manifests: &change::Manifests) -> Result<(), i32> {
    let rendered = manifests.render_ledger();
    let path = repo_root.join(doc::CHANGE_LEDGER_RELATIVE_PATH);
    if rendered.is_empty() {
        // Zero manifestos: não materializa arquivo (mantém a árvore limpa).
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    write_atomic(&path, &rendered)
}

// @pinker-nav:end cli.doc.mudancas

// @pinker-nav:start cli.doc.verificacao
// @pinker-nav:domain doc
// @pinker-nav:layer cli
// @pinker-nav:summary run_doc_verificar renderiza o modelo somente leitura de doc::verify_repository, preservando diagnósticos estruturais, drift de catálogo, ledger e projeções e os mesmos códigos da CLI sem duplicar a autoridade observacional.
fn run_doc_verificar(repo_root: &Path, config: &doc::DocConfig) -> i32 {
    let verification = match doc::verify_repository(repo_root, config) {
        Ok(verification) => verification,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_FAILURE;
        }
    };
    if verification.is_ok() {
        println!("Documentação, catálogo, manifestos e projeções verificados: ok.");
        return EXIT_OK;
    }
    eprintln!(
        "E-DOC-VERIFY: {} divergência(s) encontrada(s):",
        verification.total_errors()
    );
    for error in &verification.source_errors {
        eprintln!("  - {error}");
    }
    if verification.catalog_out_of_date {
        eprintln!(
            "  - {}",
            doc_index::DocVerifyError::CatalogOutOfDate {
                path: config.generated.docs_index.clone()
            }
        );
    }
    for error in &verification.manifest_errors {
        eprintln!("  - {error}");
    }
    if verification.ledger_out_of_date {
        eprintln!(
            "  - histórico mecânico '{}' dessincronizado; rode `pink doc sincronizar`",
            doc::CHANGE_LEDGER_RELATIVE_PATH
        );
    }
    for drift in &verification.projection_drifts {
        eprintln!("  - {drift}");
    }
    if let Some(error) = &verification.projection_error {
        eprintln!("  - {error}");
    }
    EXIT_SOURCE
}
// @pinker-nav:end cli.doc.verificacao
