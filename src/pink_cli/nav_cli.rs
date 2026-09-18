//! Comandos `pink nav` (`cli.nav.projecao`, `cli.nav.consulta`,
//! `cli.nav.sincronizacao-verificacao`), unidade MAIN-1 da decomposição
//! física #601 — a última do inventário.
//!
//! Movimento físico: as decisões, o estado e a ordem são os do entrypoint.
//! `main.rs` continua dono da orquestração; aqui mora só a implementação.
//!
//! Autoridade de navegação e de projeção não se move nem se duplica: quem
//! decide qual `NavSub` roda continua sendo `run_nav` em `src/main.rs`, e a
//! autoridade real das consultas e dos snapshots continua sendo a biblioteca
//! (`pinker_v0::nav`, `pinker_v0::symbol_index`, `pinker_v0::diff_coverage`,
//! `pinker_v0::nav_projection_archive`). Este arquivo é o adaptador de CLI
//! dessas autoridades, exatamente como era dentro do pai.

// @pinker-nav:start cli.nav.projecao
// @pinker-nav:domain archive
// @pinker-nav:layer cli
// @pinker-nav:summary Final `pink nav projecao` adapter over the materialized historical archive: discovers the root through the automation core, reads the archive index once, and derives listing, single-entry inspection and integrity verification from that one model in deterministic text and JSON — no current navigation catalog, key, summary, hash, path, rename map or recipe is consulted, and a malformed index, an unknown id, an altered payload and a missing payload keep distinct exits.
use super::*;
use pinker_v0::automation as core;
use pinker_v0::nav_projection_archive as archive;

pub(super) fn run_nav_projecao(repo: &Path, json: bool, command: ProjectionSub) -> i32 {
    let root = match core::RepoRoot::discover(repo) {
        Ok(root) => root,
        Err(error) => {
            if json {
                println!(
                    "{}",
                    archive::render_failure_json(
                        "projecao",
                        &archive::ArchiveFailure::Unreadable {
                            path: archive::ARCHIVE_DIR.to_string(),
                            msg: error.to_string(),
                        }
                    )
                );
            } else {
                eprintln!("{error}");
            }
            return EXIT_CATALOG;
        }
    };
    let command_name = match &command {
        ProjectionSub::Listar => "listar",
        ProjectionSub::Mostrar { .. } => "mostrar",
        ProjectionSub::Verificar { .. } => "verificar",
    };
    let index = match archive::load(root.path()) {
        Ok(index) => index,
        Err(failure) => return print_archive_failure(command_name, json, &failure),
    };
    match command {
        ProjectionSub::Listar => {
            if json {
                println!("{}", archive::render_inventory_json(&index));
            } else {
                print!("{}", archive::render_inventory_human(&index));
            }
            EXIT_OK
        }
        ProjectionSub::Mostrar { id } => run_archive_show(&root, json, &index, &id),
        ProjectionSub::Verificar { id } => run_archive_verify(&root, json, &index, id.as_deref()),
    }
}

fn run_archive_show(
    root: &core::RepoRoot,
    json: bool,
    index: &archive::ArchiveIndex,
    id: &str,
) -> i32 {
    let Some(entry) = index.entries.iter().find(|entry| entry.id == id) else {
        if json {
            println!(
                "{}",
                archive::render_failure_json(
                    "mostrar",
                    &archive::ArchiveFailure::InvalidValue {
                        scope: String::new(),
                        field: "id".to_string(),
                        msg: format!("'{id}' não está no arquivo histórico"),
                    }
                )
            );
        } else {
            eprintln!("E-ARCHIVE-IDENTITY\n'{id}' não está no arquivo histórico");
        }
        return EXIT_NORESULT;
    };
    let report = archive::verify_entry(root.path(), entry);
    if json {
        println!("{}", archive::render_entry_json(index, &report));
    } else {
        print!("{}", archive::render_entry_human(index, &report));
    }
    archive_exit(report.outcome.as_str())
}

fn run_archive_verify(
    root: &core::RepoRoot,
    json: bool,
    index: &archive::ArchiveIndex,
    id: Option<&str>,
) -> i32 {
    let selected = match id {
        None => index.entries.clone(),
        Some(id) => match index.entries.iter().find(|entry| entry.id == id) {
            Some(entry) => vec![entry.clone()],
            None => {
                if json {
                    println!(
                        "{}",
                        archive::render_failure_json(
                            "verificar",
                            &archive::ArchiveFailure::InvalidValue {
                                scope: String::new(),
                                field: "id".to_string(),
                                msg: format!("'{id}' não está no arquivo histórico"),
                            }
                        )
                    );
                } else {
                    eprintln!("E-ARCHIVE-IDENTITY\n'{id}' não está no arquivo histórico");
                }
                return EXIT_NORESULT;
            }
        },
    };
    let scoped = archive::ArchiveIndex {
        entries: selected,
        ..index.clone()
    };
    let verification = archive::verify(root.path(), &scoped);
    if json {
        println!("{}", archive::render_verification_json(&verification));
    } else {
        print!("{}", archive::render_verification_human(&verification));
    }
    archive_exit(verification.outcome())
}

/// POT/LPT: INVARIANT archive integrity failure != current catalog drift
fn archive_exit(outcome: &str) -> i32 {
    match outcome {
        "INTACT" => EXIT_OK,
        "MISSING" => EXIT_CATALOG,
        _ => EXIT_SOURCE,
    }
}

fn print_archive_failure(command: &str, json: bool, failure: &archive::ArchiveFailure) -> i32 {
    if json {
        println!("{}", archive::render_failure_json(command, failure));
    } else {
        eprintln!("{failure}");
    }
    match failure {
        archive::ArchiveFailure::Unreadable { .. } => EXIT_CATALOG,
        _ => EXIT_HARNESS,
    }
}
// @pinker-nav:end cli.nav.projecao

// @pinker-nav:start cli.nav.consulta
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:related-symbol pinker_v0::symbol_index::locate
// @pinker-nav:related-symbol pinker_v0::diff_coverage::analyze
// @pinker-nav:summary Consultas nav read-only carregam catálogo e símbolos; cobertura-diff analisa stdin e impacto compõe git diff limitado com as autoridades correntes sem mutar o repositório.
use pinker_v0::symbol_extraction;

/// Carrega o catálogo de código versionado (superfície de consulta — §5).
fn load_code_catalog(repo_root: &Path) -> Result<nav::CodeCatalog, i32> {
    let doc_config = load_doc_config(repo_root);
    let path = repo_root.join(doc_config.generated.code_index.clone());
    match nav::CodeCatalog::load(&path) {
        Ok(catalog) => Ok(catalog),
        Err(err) => {
            eprintln!("{err}");
            Err(EXIT_CATALOG)
        }
    }
}

/// Orçamento de saída de `nav mostrar` (#672 T0-B). A unidade é a LINHA do
/// corpo da região; `desde` é o deslocamento 1-based da primeira linha pedida.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct BodyBudget {
    pub(super) resumo: bool,
    pub(super) linhas: Option<usize>,
    pub(super) desde: Option<usize>,
}

pub(super) fn run_nav_mostrar(repo_root: &Path, key: &str, json: bool, budget: BodyBudget) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let Some(region) = catalog.region(key) else {
        eprintln!("chave de código não encontrada: '{key}'. Tente `pink nav buscar \"{key}\"`.");
        return EXIT_NORESULT;
    };
    let path = repo_root.join(&region.file);
    let source = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!(
                "E-NAV-SOURCE\nFalha ao ler fonte '{}': {}",
                path.display(),
                err
            );
            return EXIT_SOURCE;
        }
    };
    match nav::validate_region(&source, region) {
        nav::RegionCheck::Ok => {}
        nav::RegionCheck::AnchorDrift => {
            eprintln!(
                "E-NAV-SOURCE\nMarcador divergente para '{}' em {}; catálogo desatualizado. Rode `pink nav sincronizar`.",
                region.key, region.file
            );
            return EXIT_SOURCE;
        }
        nav::RegionCheck::HashMismatch { expected, found } => {
            eprintln!(
                "E-NAV-SOURCE\nHash divergente para '{}' em {} (esperado {}, obtido {}); catálogo desatualizado. Rode `pink nav sincronizar`.",
                region.key, region.file, expected, found
            );
            return EXIT_SOURCE;
        }
    }
    // A validação de fonte/âncora/hash acima é a MESMA em qualquer modo: o
    // corpo pode ser recortado, a verificação nunca.
    let content = nav::extract_region_content(&source, region);
    let total_lines = content.len();
    let from = budget.desde.unwrap_or(1).max(1);
    let skipped = (from - 1).min(total_lines);
    let available = total_lines - skipped;
    let shown_lines: &[String] = if budget.resumo {
        &[]
    } else {
        let take = budget.linhas.unwrap_or(available).min(available);
        &content[skipped..skipped + take]
    };
    // `resumo` não é truncamento: é uma resposta que declara não conter corpo.
    // Truncamento é sobrar linha DEPOIS da janela devolvida; o prefixo pulado
    // por `--desde` é declarado em `returned_from`, não escondido.
    let truncated = !budget.resumo && skipped + shown_lines.len() < total_lines;
    let next_line = if truncated {
        Some(skipped + shown_lines.len() + 1)
    } else {
        None
    };

    if json {
        let mut out = String::new();
        out.push_str("{\"schema\":1");
        out.push_str(&format!(",\"key\":{}", json_escape(&region.key)));
        out.push_str(&format!(",\"kind\":{}", json_escape(&region.kind)));
        if let Some(domain) = &region.domain {
            out.push_str(&format!(",\"domain\":{}", json_escape(domain)));
        }
        if let Some(layer) = &region.layer {
            out.push_str(&format!(",\"layer\":{}", json_escape(layer)));
        }
        if let Some(phase) = region.phase {
            out.push_str(&format!(",\"phase\":{}", phase));
        }
        out.push_str(&format!(",\"file\":{}", json_escape(&region.file)));
        out.push_str(&format!(",\"content_start\":{}", region.content_start));
        out.push_str(&format!(",\"content_end\":{}", region.content_end));
        out.push_str(&format!(",\"hash\":{}", json_escape(&region.hash)));
        if !region.summary.is_empty() {
            out.push_str(&format!(",\"summary\":{}", json_escape(&region.summary)));
        }
        out.push_str(&format!(",\"verified\":{}", true));
        out.push_str(&format!(",\"total_lines\":{}", total_lines));
        out.push_str(&format!(",\"returned_from\":{}", skipped + 1));
        out.push_str(&format!(",\"returned_lines\":{}", shown_lines.len()));
        out.push_str(&format!(",\"body_included\":{}", !budget.resumo));
        out.push_str(&format!(",\"truncated\":{}", truncated));
        if let Some(next) = next_line {
            out.push_str(&format!(",\"continuation_desde\":{}", next));
        }
        if !budget.resumo {
            out.push_str(&format!(
                ",\"content\":{}",
                json_escape(&shown_lines.join("\n"))
            ));
        }
        out.push('}');
        println!("{out}");
    } else {
        println!(
            "// {} — {}:{}-{}",
            region.key, region.file, region.content_start, region.content_end
        );
        if !region.summary.is_empty() {
            println!("// {}", region.summary);
        }
        if budget.resumo {
            println!(
                "// verificado: hash {} · {} linhas de corpo não incluídas (use `pink nav mostrar {}` para o corpo inteiro)",
                region.hash, total_lines, region.key
            );
            return EXIT_OK;
        }
        println!();
        for line in shown_lines {
            println!("{line}");
        }
        if truncated {
            println!(
                "// truncado: {} de {} linhas (continue com --desde {})",
                shown_lines.len(),
                total_lines,
                next_line.unwrap_or(total_lines)
            );
        }
    }
    EXIT_OK
}

pub(super) fn run_nav_buscar(
    repo_root: &Path,
    consulta: &str,
    json: bool,
    limite: Option<usize>,
    desde: Option<usize>,
) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_BUSCAR);
    let hits = catalog.search_ranked(consulta);
    let total = hits.len();
    let offset = desde.unwrap_or(0).min(total);
    let shown: Vec<&nav::RegionMatch> = hits.iter().skip(offset).take(limit).collect();
    if shown.is_empty() {
        if json {
            println!(
                "{{\"schema\":1,\"query\":{},\"normalized\":{},\"total_results\":{},\"returned_results\":0,\"offset\":{},\"limit\":{},\"truncated\":false,\"results\":[]}}",
                json_escape(consulta),
                json_escape(&pinker_v0::text_norm::normalize(consulta)),
                total,
                offset,
                limit
            );
        } else {
            eprintln!("Nenhuma região encontrada para: {consulta}");
        }
        return EXIT_NORESULT;
    }
    // O truncamento é declarado sempre que sobrou resultado depois da janela.
    let truncated = offset + shown.len() < total;
    let next_offset = offset + shown.len();
    if json {
        let results: Vec<String> = shown
            .iter()
            .map(|hit| {
                let r = hit.region;
                let mut o = String::from("{");
                o.push_str(&format!("\"key\":{}", json_escape(&r.key)));
                if let Some(domain) = &r.domain {
                    o.push_str(&format!(",\"domain\":{}", json_escape(domain)));
                }
                if let Some(layer) = &r.layer {
                    o.push_str(&format!(",\"layer\":{}", json_escape(layer)));
                }
                o.push_str(&format!(",\"file\":{}", json_escape(&r.file)));
                o.push_str(&format!(",\"content_start\":{}", r.content_start));
                o.push_str(&format!(",\"content_end\":{}", r.content_end));
                if !r.summary.is_empty() {
                    o.push_str(&format!(",\"summary\":{}", json_escape(&r.summary)));
                }
                o.push_str(&format!(",\"score\":{}", hit.score));
                o.push_str(&format!(",\"coverage\":{}", hit.coverage));
                o.push_str(&format!(",\"terms_considered\":{}", hit.terms_considered));
                let terms: Vec<String> = hit.matched_terms.iter().map(|t| json_escape(t)).collect();
                o.push_str(&format!(",\"matched_terms\":[{}]", terms.join(",")));
                o.push('}');
                o
            })
            .collect();
        let mut tail = String::new();
        if truncated {
            tail.push_str(&format!(",\"continuation_desde\":{}", next_offset));
        }
        println!(
            "{{\"schema\":1,\"query\":{},\"normalized\":{},\"total_results\":{},\"returned_results\":{},\"offset\":{},\"limit\":{},\"truncated\":{}{},\"results\":[{}]}}",
            json_escape(consulta),
            json_escape(&pinker_v0::text_norm::normalize(consulta)),
            total,
            shown.len(),
            offset,
            limit,
            truncated,
            tail,
            results.join(",")
        );
    } else {
        for hit in &shown {
            let region = hit.region;
            println!("{}", region.key);
            if !region.summary.is_empty() {
                println!("   {}", region.summary);
            }
            println!(
                "   {}:{}-{}",
                region.file, region.content_start, region.content_end
            );
        }
        if truncated {
            println!(
                "// {} de {} resultados (continue com --desde {})",
                shown.len(),
                total,
                next_offset
            );
        } else {
            println!("// {} de {} resultados", shown.len(), total);
        }
    }
    EXIT_OK
}

pub(super) fn run_nav_localizar(
    repo_root: &Path,
    symbol: &str,
    json: bool,
    offset: Option<usize>,
) -> i32 {
    let code = match load_code_catalog(repo_root) {
        Ok(catalog) => catalog,
        Err(code) => return code,
    };
    let doc_config = load_doc_config(repo_root);
    let doc_path = repo_root.join(doc_config.generated.docs_index);
    let docs = match doc_index::DocCatalog::load(&doc_path) {
        Ok(catalog) => Some(catalog),
        Err(doc_index::CatalogError::Missing { .. }) => None,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let mut report = match symbol_index::locate(&code, docs.as_ref(), symbol) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    // A extensão lexical lê a fonte corrente do worktree, então edição não
    // commitada e arquivo untracked entram no universo observado. O derivador
    // explícito permanece sem E/S: a leitura vive aqui, no adaptador.
    if let Err(error) = symbol_extraction::extend(repo_root, &mut report, offset.unwrap_or(0)) {
        eprintln!("{error}");
        return EXIT_SOURCE;
    }
    // Um arquivo que mudou durante a leitura foi *não observado*, não
    // observado como ausente. O resultado sai, e o exit recusa o sucesso.
    let unstable = !report.unstable_sources.is_empty();
    if json {
        println!("{}", symbol_index::render_json(&report));
    } else if report.found() || unstable {
        print!("{}", symbol_index::render_human(&report));
    } else {
        eprint!("{}", symbol_index::render_human(&report));
    }
    if unstable {
        EXIT_SOURCE
    } else if report.found() {
        EXIT_OK
    } else {
        EXIT_NORESULT
    }
}

pub(super) fn run_nav_cobertura_diff(repo_root: &Path, base: Option<&str>, json: bool) -> i32 {
    let root = match core::RepoRoot::discover(repo_root) {
        Ok(root) => root,
        Err(error) => {
            eprintln!("E-DIFF-ROOT\n{error}");
            return EXIT_HARNESS;
        }
    };
    let mut bytes = Vec::new();
    if let Err(error) = io::stdin()
        .take((diff_coverage::MAX_DIFF_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
    {
        eprintln!("E-DIFF-IO\nFalha ao ler stdin: {error}");
        return EXIT_HARNESS;
    }
    if bytes.len() > diff_coverage::MAX_DIFF_BYTES {
        eprintln!(
            "{}",
            diff_coverage::CoverageError::TooLarge {
                bytes: bytes.len(),
                limit: diff_coverage::MAX_DIFF_BYTES,
            }
        );
        return EXIT_HARNESS;
    }
    let input = match std::str::from_utf8(&bytes) {
        Ok(input) => input,
        Err(_) => {
            eprintln!("{}", diff_coverage::CoverageError::InvalidUtf8);
            return EXIT_HARNESS;
        }
    };
    let config = match doc::DocConfig::load(root.path()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let code_path = root.path().join(&config.generated.code_index);
    let code = match nav::CodeCatalog::load(&code_path) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let docs_path = root.path().join(&config.generated.docs_index);
    let docs = match doc_index::DocCatalog::load(&docs_path) {
        Ok(docs) => Some(docs),
        Err(doc_index::CatalogError::Missing { .. }) => None,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let manifests = change::Manifests::load(&root.path().join(".pinker/changes"));
    let base_code = match base {
        Some(reference) => {
            match tooling::load_base_code_catalog(
                root.path(),
                reference,
                &config.generated.code_index,
            ) {
                Ok(catalog) => Some(catalog),
                Err(error) => {
                    eprintln!("{error}");
                    return EXIT_CATALOG;
                }
            }
        }
        None => None,
    };
    let policy = nav_coverage::CoveragePolicy::load(root.path()).ok();
    let report = match diff_coverage::analyze(
        input,
        diff_coverage::CoverageAuthorities {
            code: &code,
            base_code: base_code.as_ref(),
            policy: policy.as_ref(),
            docs: docs.as_ref(),
            doc_config: Some(&config),
            manifests: Some(&manifests),
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_HARNESS;
        }
    };
    if json {
        println!("{}", diff_coverage::render_json(&report));
    } else {
        print!("{}", diff_coverage::render_human(&report));
    }
    EXIT_OK
}

pub(super) fn run_nav_impacto(repo_root: &Path, diff: &str, base: Option<&str>, json: bool) -> i32 {
    match tooling::collect_impact(repo_root, diff, base) {
        Ok(report) => {
            if json {
                println!("{}", tooling::render_impact_json(&report));
            } else {
                println!("pink nav impacto");
                println!("  diff: {}", report.diff);
                println!("  changed_files: {}", report.changed_files.len());
                println!(
                    "  changed_regions: {}",
                    report.changed_regions.status.as_str()
                );
                println!(
                    "  projections_affected: {}",
                    report.projections_affected.status.as_str()
                );
                println!("  catalog_status: {}", report.catalog_status);
            }
            EXIT_OK
        }
        Err(error) => {
            eprintln!("{error}");
            EXIT_HARNESS
        }
    }
}

pub(super) fn run_nav_listar(repo_root: &Path, seletor: &str, json: bool) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let regions = catalog.list(seletor);
    if regions.is_empty() {
        if json {
            println!("{{\"selector\":{},\"results\":[]}}", json_escape(seletor));
        } else {
            eprintln!("Nenhuma região na camada/domínio '{seletor}'.");
        }
        return EXIT_NORESULT;
    }
    if json {
        let results: Vec<String> = regions.iter().map(|r| json_escape(&r.key)).collect();
        println!(
            "{{\"selector\":{},\"results\":[{}]}}",
            json_escape(seletor),
            results.join(",")
        );
    } else {
        println!("Regiões em '{seletor}':");
        for region in regions {
            println!(
                "- {} [{}/{}] {}:{}-{}",
                region.key,
                region.domain.as_deref().unwrap_or("-"),
                region.layer.as_deref().unwrap_or("-"),
                region.file,
                region.content_start,
                region.content_end
            );
        }
    }
    EXIT_OK
}

pub(super) fn run_nav_mapa(repo_root: &Path, filtro: Option<&str>, json: bool) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let selected = catalog.map_regions(filtro);
    if selected.is_empty() {
        if json {
            println!(
                "{{\"schema\":1,\"filter\":{},\"files\":[]}}",
                filtro
                    .map(json_escape)
                    .unwrap_or_else(|| "null".to_string())
            );
        } else if let Some(filtro) = filtro {
            eprintln!("Nenhuma região encontrada para o mapa: {filtro}");
        } else {
            eprintln!("Nenhuma região disponível para o mapa.");
        }
        return EXIT_NORESULT;
    }

    let mut files: BTreeMap<&str, Vec<&nav::CodeRegion>> = BTreeMap::new();
    for region in selected {
        files.entry(&region.file).or_default().push(region);
    }
    for sections in files.values_mut() {
        sections.sort_by(|a, b| {
            a.content_start
                .cmp(&b.content_start)
                .then(a.content_end.cmp(&b.content_end))
                .then(a.key.cmp(&b.key))
        });
    }

    if json {
        let rendered_files: Vec<String> = files
            .iter()
            .map(|(path, sections)| {
                let domains: BTreeSet<&str> = sections
                    .iter()
                    .filter_map(|region| region.domain.as_deref())
                    .collect();
                let layers: BTreeSet<&str> = sections
                    .iter()
                    .filter_map(|region| region.layer.as_deref())
                    .collect();
                let start = sections
                    .iter()
                    .map(|region| region.content_start)
                    .min()
                    .unwrap_or(0);
                let end = sections
                    .iter()
                    .map(|region| region.content_end)
                    .max()
                    .unwrap_or(0);
                let rendered_sections: Vec<String> = sections
                    .iter()
                    .map(|region| {
                        format!(
                            "{{\"key\":{},\"summary\":{},\"domain\":{},\"layer\":{},\"range\":{{\"start\":{},\"end\":{}}}}}",
                            json_escape(&region.key),
                            if region.summary.is_empty() {
                                "null".to_string()
                            } else {
                                json_escape(&region.summary)
                            },
                            region
                                .domain
                                .as_deref()
                                .map(json_escape)
                                .unwrap_or_else(|| "null".to_string()),
                            region
                                .layer
                                .as_deref()
                                .map(json_escape)
                                .unwrap_or_else(|| "null".to_string()),
                            region.content_start,
                            region.content_end
                        )
                    })
                    .collect();
                let domain_values: Vec<String> = domains.iter().map(|v| json_escape(v)).collect();
                let layer_values: Vec<String> = layers.iter().map(|v| json_escape(v)).collect();
                format!(
                    "{{\"path\":{},\"region_count\":{},\"domains\":[{}],\"layers\":[{}],\"range\":{{\"start\":{},\"end\":{}}},\"sections\":[{}]}}",
                    json_escape(path),
                    sections.len(),
                    domain_values.join(","),
                    layer_values.join(","),
                    start,
                    end,
                    rendered_sections.join(",")
                )
            })
            .collect();
        println!(
            "{{\"schema\":1,\"filter\":{},\"files\":[{}]}}",
            filtro
                .map(json_escape)
                .unwrap_or_else(|| "null".to_string()),
            rendered_files.join(",")
        );
    } else {
        let absolute_root = repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf());
        for (file_index, (path, sections)) in files.iter().enumerate() {
            if file_index > 0 {
                println!();
            }
            let domains: BTreeSet<&str> = sections
                .iter()
                .filter_map(|region| region.domain.as_deref())
                .collect();
            let layers: BTreeSet<&str> = sections
                .iter()
                .filter_map(|region| region.layer.as_deref())
                .collect();
            let start = sections
                .iter()
                .map(|region| region.content_start)
                .min()
                .unwrap_or(0);
            let end = sections
                .iter()
                .map(|region| region.content_end)
                .max()
                .unwrap_or(0);
            let domain_text = if domains.is_empty() {
                "-".to_string()
            } else {
                domains.into_iter().collect::<Vec<_>>().join(", ")
            };
            let layer_text = if layers.is_empty() {
                "-".to_string()
            } else {
                layers.into_iter().collect::<Vec<_>>().join(", ")
            };
            println!("{path}");
            println!("  absoluto: {}", absolute_root.join(path).display());
            println!("  regiões: {}", sections.len());
            println!("  domínios: {domain_text}");
            println!("  camadas: {layer_text}");
            println!("  intervalo: {start}-{end}");
            for region in sections {
                println!();
                println!("  {}", region.key);
                println!(
                    "    resumo: {}",
                    if region.summary.is_empty() {
                        "-"
                    } else {
                        &region.summary
                    }
                );
                println!("    domínio: {}", region.domain.as_deref().unwrap_or("-"));
                println!("    camada: {}", region.layer.as_deref().unwrap_or("-"));
                println!(
                    "    intervalo: {}-{}",
                    region.content_start, region.content_end
                );
            }
        }
    }
    EXIT_OK
}
// @pinker-nav:end cli.nav.consulta

// @pinker-nav:start cli.nav.sincronizacao-verificacao
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:summary run_nav_sincronizar reescaneia e grava o catálogo somente após validação; run_nav_verificar reutiliza nav::verify_repository e valida em memória os vínculos estruturados do índice de símbolos contra os catálogos de código e documentação, sem escrever e sem duplicar autoridade.
pub(super) fn run_nav_cobertura(repo_root: &Path, json: bool) -> i32 {
    let policy = match nav_coverage::CoveragePolicy::load(repo_root) {
        Ok(policy) => policy,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_POLICY;
        }
    };
    let index = scan_code(repo_root);
    let inventory = match nav_coverage::inventory(repo_root, &index, &policy) {
        Ok(inventory) => inventory,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_SOURCE;
        }
    };
    if json {
        println!("{}", nav_coverage::render_json(&inventory));
    } else {
        print!("{}", nav_coverage::render_text(&inventory));
    }
    let violations = nav_coverage::verify(&inventory, &index, &policy);
    if violations.is_empty() {
        EXIT_OK
    } else {
        for violation in &violations {
            eprintln!("{violation}");
        }
        EXIT_SOURCE
    }
}

pub(super) fn run_nav_sincronizar(repo_root: &Path) -> i32 {
    let doc_config = load_doc_config(repo_root);
    let index = scan_code(repo_root);
    // Validação antes de escrever (§8): não sobrescreve catálogo válido com
    // árvore inválida.
    let problems = index.verify();
    if !problems.is_empty() {
        eprintln!(
            "E-NAV-SYNC: {} divergência(s); catálogo NÃO alterado.",
            problems.len()
        );
        for problem in &problems {
            eprintln!("  - {problem}");
        }
        return EXIT_SOURCE;
    }
    let rendered = index.render_jsonl();
    let path = repo_root.join(&doc_config.generated.code_index);
    if let Err(code) = write_atomic(&path, &rendered) {
        return code;
    }
    println!(
        "Catálogo de código sincronizado: {} ({} regiões).",
        doc_config.generated.code_index,
        index.regions.len()
    );
    EXIT_OK
}

pub(super) fn run_nav_verificar(repo_root: &Path) -> i32 {
    let doc_config = load_doc_config(repo_root);
    let verification = match nav::verify_repository(repo_root, &doc_config.generated.code_index) {
        Ok(verification) => verification,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_FAILURE;
        }
    };
    if !verification.is_ok() {
        eprintln!(
            "E-NAV-VERIFY: {} divergência(s) encontrada(s):",
            verification.total_errors()
        );
        for error in &verification.source_errors {
            eprintln!("  - {error}");
        }
        if verification.catalog_out_of_date {
            eprintln!(
                "  - {}",
                nav::NavVerifyError::IndexOutOfDate {
                    path: doc_config.generated.code_index.clone()
                }
            );
        }
        match &verification.coverage {
            nav::CoverageOutcome::Checked { violations, .. } => {
                for violation in violations {
                    eprintln!("  - {violation}");
                }
            }
            nav::CoverageOutcome::PolicyUnavailable { reason } => {
                eprintln!("  - {reason}");
            }
        }
        return EXIT_SOURCE;
    }

    let code = match nav::CodeCatalog::load(&repo_root.join(&doc_config.generated.code_index)) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    let requires_docs = code
        .regions
        .iter()
        .any(|region| !region.symbol_docs.is_empty());
    let docs = if requires_docs {
        match doc_index::DocCatalog::load(&repo_root.join(&doc_config.generated.docs_index)) {
            Ok(catalog) => Some(catalog),
            Err(error) => {
                eprintln!("{error}");
                return EXIT_CATALOG;
            }
        }
    } else {
        None
    };
    if let Err(error) = symbol_index::locate(&code, docs.as_ref(), "") {
        eprintln!("E-NAV-VERIFY: vínculo explícito de símbolo inválido:\n  - {error}");
        return EXIT_SOURCE;
    }

    println!("Marcadores, vínculos e catálogo de código verificados: ok.");
    EXIT_OK
}
// @pinker-nav:end cli.nav.sincronizacao-verificacao
