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
//! `pinker_v0::nav_projection_lifecycle`). Este arquivo é o adaptador de CLI
//! dessas autoridades, exatamente como era dentro do pai.

use super::*;

// @pinker-nav:start cli.nav.projecao
// @pinker-nav:domain projecoes
// @pinker-nav:layer cli
// @pinker-nav:summary Adaptador final `pink nav projecao`: despacha listar, mostrar, verificar, preparar e aceitar; descobre root pelo automation core, deriva texto e JSON dos mesmos modelos, recalcula planos antes de toda autorização e preserva exits distintos para drift, harness, política e stale.
pub(super) fn run_nav_projecao(repo: &Path, json: bool, command: ProjectionSub) -> i32 {
    let root = match pinker_v0::automation::RepoRoot::discover(repo) {
        Ok(root) => root,
        Err(error) => {
            return print_projection_error("projecao", json, &ProjectionError::Automation(error))
        }
    };
    match command {
        ProjectionSub::Listar => {
            let store = match ProjectionStore::load(root.path()) {
                Ok(store) => store,
                Err(error) => {
                    return print_projection_error(
                        "listar",
                        json,
                        &ProjectionError::Authority(error),
                    )
                }
            };
            if json {
                println!("{}", nav_projection_report::render_inventory_json(&store));
            } else {
                print!("{}", nav_projection_report::render_inventory_human(&store));
            }
            if store.errors().is_empty() {
                EXIT_OK
            } else {
                EXIT_HARNESS
            }
        }
        ProjectionSub::Mostrar { id, observado } => {
            run_projection_show(&root, json, &id, observado)
        }
        ProjectionSub::Verificar { id } => run_projection_verify(&root, json, id.as_deref()),
        ProjectionSub::Preparar {
            id,
            justificativa,
            predecessor,
            autorizar,
        } => {
            let Some(justification) = justificativa else {
                return print_projection_error(
                    "preparar",
                    json,
                    &ProjectionError::Policy {
                        message: "--justificativa é obrigatória".to_string(),
                    },
                );
            };
            let Some(predecessor) = predecessor else {
                return print_projection_error(
                    "preparar",
                    json,
                    &ProjectionError::Policy {
                        message: "--predecessor é obrigatório".to_string(),
                    },
                );
            };
            let catalog = match load_projection_catalog(&root) {
                Ok(catalog) => catalog,
                Err(error) => return print_projection_error("preparar", json, &error),
            };
            let planning = match nav_projection_lifecycle::plan_prepare(
                &root,
                &catalog.regions,
                &id,
                &predecessor,
                &justification,
            ) {
                Ok(planning) => planning,
                Err(error) => return print_projection_error("preparar", json, &error),
            };
            match autorizar {
                None => {
                    if json {
                        println!(
                            "{}",
                            nav_projection_report::render_plan_json("preparar", &planning)
                        );
                    } else {
                        print!("{}", nav_projection_report::render_plan_human(&planning));
                    }
                    EXIT_OK
                }
                Some(digest) => match nav_projection_lifecycle::apply_prepare(
                    &root,
                    &catalog.regions,
                    &planning,
                    &digest,
                ) {
                    Ok(applied) => {
                        if json {
                            println!(
                                "{}",
                                nav_projection_report::render_apply_json("preparar", &applied)
                            );
                        } else {
                            print!("{}", nav_projection_report::render_apply_human(&applied));
                        }
                        EXIT_OK
                    }
                    Err(error) => print_projection_error("preparar", json, &error),
                },
            }
        }
        ProjectionSub::Aceitar { id, autorizar } => {
            let catalog = match load_projection_catalog(&root) {
                Ok(catalog) => catalog,
                Err(error) => return print_projection_error("aceitar", json, &error),
            };
            let planning = match nav_projection_lifecycle::plan_accept(&root, &catalog.regions, &id)
            {
                Ok(planning) => planning,
                Err(error) => return print_projection_error("aceitar", json, &error),
            };
            match autorizar {
                None => {
                    if json {
                        println!(
                            "{}",
                            nav_projection_report::render_plan_json("aceitar", &planning)
                        );
                    } else {
                        print!("{}", nav_projection_report::render_plan_human(&planning));
                    }
                    EXIT_OK
                }
                Some(digest) => match nav_projection_lifecycle::apply_accept(
                    &root,
                    &catalog.regions,
                    &planning,
                    &digest,
                ) {
                    Ok(applied) => {
                        if json {
                            println!(
                                "{}",
                                nav_projection_report::render_apply_json("aceitar", &applied)
                            );
                        } else {
                            print!("{}", nav_projection_report::render_apply_human(&applied));
                        }
                        EXIT_OK
                    }
                    Err(error) => print_projection_error("aceitar", json, &error),
                },
            }
        }
    }
}

fn run_projection_show(
    root: &pinker_v0::automation::RepoRoot,
    json: bool,
    id: &str,
    observed: bool,
) -> i32 {
    let store = match ProjectionStore::load(root.path()) {
        Ok(store) => store,
        Err(error) => {
            return print_projection_error("mostrar", json, &ProjectionError::Authority(error))
        }
    };
    if let Some(error) = store.snapshot_error(id) {
        return print_projection_error(
            "mostrar",
            json,
            &ProjectionError::Harness {
                path: Some(error.path.clone()),
                message: error.message.clone(),
            },
        );
    }
    let Some(stored) = store.snapshot(id) else {
        return print_projection_error(
            "mostrar",
            json,
            &ProjectionError::NotFound { id: id.to_string() },
        );
    };
    let verification = if observed {
        let catalog = match load_projection_catalog(root) {
            Ok(catalog) => catalog,
            Err(error) => return print_projection_error("mostrar", json, &error),
        };
        match nav_projection_report::verify_one(&store, id, &catalog.regions) {
            Ok(item) => Some(item.report),
            Err(error) => return print_projection_error("mostrar", json, &error),
        }
    } else {
        None
    };
    if json {
        println!(
            "{}",
            nav_projection_report::render_show_json(stored, verification.as_ref())
        );
    } else {
        print!(
            "{}",
            nav_projection_report::render_show_human(stored, verification.as_ref())
        );
    }
    match verification.as_ref().map(|report| &report.outcome) {
        Some(pinker_v0::nav_projection_snapshot::Outcome::Drift(_)) => EXIT_SOURCE,
        Some(pinker_v0::nav_projection_snapshot::Outcome::HarnessFailure(_)) => EXIT_HARNESS,
        _ => EXIT_OK,
    }
}

fn run_projection_verify(
    root: &pinker_v0::automation::RepoRoot,
    json: bool,
    id: Option<&str>,
) -> i32 {
    let store = match ProjectionStore::load(root.path()) {
        Ok(store) => store,
        Err(error) => {
            return print_projection_error("verificar", json, &ProjectionError::Authority(error))
        }
    };
    let catalog = match load_projection_catalog(root) {
        Ok(catalog) => catalog,
        Err(error) => return print_projection_error("verificar", json, &error),
    };
    let batch = if let Some(id) = id {
        let item = match nav_projection_report::verify_one(&store, id, &catalog.regions) {
            Ok(item) => item,
            Err(error) => return print_projection_error("verificar", json, &error),
        };
        nav_projection_report::VerificationBatch {
            results: vec![item],
            causes: Vec::new(),
            errors: Vec::new(),
        }
    } else {
        nav_projection_report::verify_all(&store, &catalog.regions)
    };
    if json {
        println!(
            "{}",
            nav_projection_report::render_verification_json(&batch)
        );
    } else {
        print!(
            "{}",
            nav_projection_report::render_verification_human(&batch)
        );
    }
    match batch.outcome() {
        "MATCH" => EXIT_OK,
        "DRIFT" => EXIT_SOURCE,
        _ => EXIT_HARNESS,
    }
}

fn load_projection_catalog(
    root: &pinker_v0::automation::RepoRoot,
) -> Result<nav::CodeCatalog, ProjectionError> {
    nav::CodeCatalog::load(&root.path().join("src/navigation.jsonl")).map_err(|error| {
        ProjectionError::Harness {
            path: Some("src/navigation.jsonl".to_string()),
            message: error.to_string(),
        }
    })
}

fn print_projection_error(command: &str, json: bool, error: &ProjectionError) -> i32 {
    if json {
        println!(
            "{}",
            nav_projection_report::render_error_json(command, error)
        );
    } else {
        eprintln!("{error}");
    }
    projection_error_exit(error)
}

fn projection_error_exit(error: &ProjectionError) -> i32 {
    use pinker_v0::automation::Failure;
    let failure_exit = |failure: &Failure| match failure {
        Failure::HarnessFailure(pinker_v0::automation::HarnessCause::RootNotFound { .. }) => {
            EXIT_CATALOG
        }
        Failure::HarnessFailure(_) => EXIT_HARNESS,
        Failure::PolicyViolation(_) => EXIT_POLICY,
        Failure::StalePlan { .. } => EXIT_STALE,
        Failure::IoFailure { .. } | Failure::VerifyAfterApplyFailure { .. } => EXIT_FAILURE,
    };
    match error {
        ProjectionError::Authority(_) => EXIT_CATALOG,
        ProjectionError::NotFound { .. } => EXIT_NORESULT,
        ProjectionError::Harness { path, .. }
            if path.as_deref() == Some("src/navigation.jsonl") =>
        {
            EXIT_CATALOG
        }
        ProjectionError::Harness { .. } => EXIT_HARNESS,
        ProjectionError::Policy { .. } => EXIT_POLICY,
        ProjectionError::Drift { .. } => EXIT_SOURCE,
        ProjectionError::Automation(failure) => failure_exit(failure),
        ProjectionError::Apply(report) => {
            report.failure.as_ref().map_or(EXIT_FAILURE, failure_exit)
        }
        ProjectionError::VerifyAfterApply { .. } => EXIT_FAILURE,
    }
}
// @pinker-nav:end cli.nav.projecao

// @pinker-nav:start cli.nav.consulta
// @pinker-nav:domain nav
// @pinker-nav:layer cli
// @pinker-nav:related-symbol pinker_v0::symbol_index::locate
// @pinker-nav:related-symbol pinker_v0::diff_coverage::analyze
// @pinker-nav:summary Consultas nav read-only carregam catálogo e símbolos; cobertura-diff analisa stdin e impacto compõe git diff limitado com as autoridades correntes sem mutar o repositório.
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

pub(super) fn run_nav_mostrar(repo_root: &Path, key: &str, json: bool) -> i32 {
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
    let content = nav::extract_region_content(&source, region);
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
        out.push_str(&format!(
            ",\"content\":{}",
            json_escape(&content.join("\n"))
        ));
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
        println!();
        for line in &content {
            println!("{line}");
        }
    }
    EXIT_OK
}

pub(super) fn run_nav_buscar(
    repo_root: &Path,
    consulta: &str,
    json: bool,
    limite: Option<usize>,
) -> i32 {
    let catalog = match load_code_catalog(repo_root) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let limit = clamp_limit(limite, LIMIT_DEFAULT_BUSCAR);
    let hits = catalog.search(consulta);
    if hits.is_empty() {
        if json {
            println!(
                "{{\"schema\":1,\"query\":{},\"normalized\":{},\"results\":[]}}",
                json_escape(consulta),
                json_escape(&pinker_v0::text_norm::normalize(consulta))
            );
        } else {
            eprintln!("Nenhuma região encontrada para: {consulta}");
        }
        return EXIT_NORESULT;
    }
    let shown: Vec<&nav::CodeRegion> = hits.into_iter().take(limit).collect();
    if json {
        let results: Vec<String> = shown
            .iter()
            .map(|r| {
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
                o.push('}');
                o
            })
            .collect();
        println!(
            "{{\"schema\":1,\"query\":{},\"normalized\":{},\"results\":[{}]}}",
            json_escape(consulta),
            json_escape(&pinker_v0::text_norm::normalize(consulta)),
            results.join(",")
        );
    } else {
        for region in shown {
            println!("{}", region.key);
            if !region.summary.is_empty() {
                println!("   {}", region.summary);
            }
            println!(
                "   {}:{}-{}",
                region.file, region.content_start, region.content_end
            );
        }
    }
    EXIT_OK
}

pub(super) fn run_nav_localizar(repo_root: &Path, symbol: &str, json: bool) -> i32 {
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
    let report = match symbol_index::locate(&code, docs.as_ref(), symbol) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_CATALOG;
        }
    };
    if json {
        println!("{}", symbol_index::render_json(&report));
    } else if report.found() {
        print!("{}", symbol_index::render_human(&report));
    } else {
        eprint!("{}", symbol_index::render_human(&report));
    }
    if report.found() {
        EXIT_OK
    } else {
        EXIT_NORESULT
    }
}

pub(super) fn run_nav_cobertura_diff(repo_root: &Path, json: bool) -> i32 {
    let root = match pinker_v0::automation::RepoRoot::discover(repo_root) {
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
    let projection_store = ProjectionStore::load(root.path()).ok();
    let manifests = change::Manifests::load(&root.path().join(".pinker/changes"));
    let report = match diff_coverage::analyze(
        input,
        diff_coverage::CoverageAuthorities {
            code: &code,
            docs: docs.as_ref(),
            projection_store: projection_store.as_ref(),
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

pub(super) fn run_nav_impacto(repo_root: &Path, diff: &str, json: bool) -> i32 {
    match tooling::collect_impact(repo_root, diff) {
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
