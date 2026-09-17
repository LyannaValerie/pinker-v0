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

// @pinker-nav:start cli.nav.projecao
// @pinker-nav:domain projecoes
// @pinker-nav:layer cli
// @pinker-nav:summary Final `pink nav projecao` adapter: dispatches listing, inspection, verification, lifecycle, and explicit recipe reconciliation; discovers the root through the automation core, derives text and JSON from shared models, recalculates plans before authorization, and preserves distinct drift, harness, policy, and stale exits.
use super::*;
use pinker_v0::automation as core;
use pinker_v0::nav_projection_rename_map::{parse_rename_map, RenameEntry, RenameMap};
use pinker_v0::nav_projection_snapshot::{Rule, SchemaAuthority};
use pinker_v0::symbol_extraction;
use std::process::Command;

pub(super) fn run_nav_projecao(repo: &Path, json: bool, command: ProjectionSub) -> i32 {
    let root = match core::RepoRoot::discover(repo) {
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
        ProjectionSub::Reconciliar {
            autorizar,
            renomeacoes,
        } => run_projection_reconcile(&root, json, autorizar, renomeacoes),
    }
}

#[derive(Debug)]
struct ReconcilePlan {
    plan: core::Plan,
    check: core::CheckReport,
    changes: Vec<String>,
    /// Impressão digital do mapa explícito que produziu este plano, quando há
    /// um. O plano publica de qual relação declarada ele nasceu; o digest é o
    /// que a torna inseparável da autorização.
    rename_map: Option<String>,
}

fn run_projection_reconcile(
    root: &core::RepoRoot,
    json: bool,
    authorization: Option<String>,
    rename_map_path: Option<String>,
) -> i32 {
    let catalog = match load_projection_catalog(root) {
        Ok(catalog) => catalog,
        Err(error) => return print_projection_error("reconciliar", json, &error),
    };
    let renames = match rename_map_path.as_deref().map(load_rename_map) {
        None => None,
        Some(Ok(map)) => Some(map),
        Some(Err(error)) => return print_projection_error("reconciliar", json, &error),
    };
    let planning = match plan_recipe_reconciliation(root, &catalog.regions, renames.as_ref()) {
        Ok(plan) => plan,
        Err(error) => return print_projection_error("reconciliar", json, &error),
    };
    match authorization {
        None => {
            print_reconcile_plan(json, &planning);
            EXIT_OK
        }
        Some(digest) => {
            if digest != planning.plan.digest() {
                return print_projection_error(
                    "reconciliar",
                    json,
                    &ProjectionError::Automation(core::Failure::StalePlan {
                        plan_digest: planning.plan.digest(),
                        msg: "CURRENT_AUTHORITY_MATCHES_PLAN=false; regenerate the reconciliation plan before apply".to_string(),
                    }),
                );
            }
            let report = core::apply(
                root,
                &planning.plan,
                &core::Authorization::for_digest(&digest),
                &planning.check,
            );
            if report.failure.is_some() {
                return print_projection_error(
                    "reconciliar",
                    json,
                    &ProjectionError::Apply(Box::new(report)),
                );
            }
            let store = match ProjectionStore::load(root.path()) {
                Ok(store) => store,
                Err(error) => {
                    return print_projection_error(
                        "reconciliar",
                        json,
                        &ProjectionError::Authority(error),
                    )
                }
            };
            match verify_historical_frozen_authority(root.path(), &store) {
                Ok(())
                    if nav_projection_report::verify_all(&store, &catalog.regions).outcome()
                        == "MATCH" =>
                {
                    if json {
                        println!("{{\"schema\":1,\"command\":\"reconciliar\",\"outcome\":\"APPLIED\",\"digest\":{}}}", json_quote(&planning.plan.digest()));
                    } else {
                        println!("APPLIED\\ndigest: {}", planning.plan.digest());
                    }
                    EXIT_OK
                }
                Ok(()) => print_projection_error(
                    "reconciliar",
                    json,
                    &ProjectionError::VerifyAfterApply {
                        message: "reconstruction verification did not reach MATCH after apply"
                            .to_string(),
                        written: !report.applied.is_empty(),
                    },
                ),
                Err(error) => print_projection_error("reconciliar", json, &error),
            }
        }
    }
}

/// Carrega o mapa explícito de renomeação a partir do caminho fornecido.
///
/// O mapa é entrada do operador, não autoridade do acervo: nada o lê por
/// omissão, ele não vive sob `.pinker/projections/` e não sobrevive à
/// invocação. Falha de leitura e falha de formato recusam pela mesma porta,
/// porque as duas significam a mesma coisa para o plano — a relação declarada
/// não pôde ser estabelecida.
fn load_rename_map(path: &str) -> Result<RenameMap, ProjectionError> {
    let text = std::fs::read_to_string(path).map_err(|erro| ProjectionError::Policy {
        message: format!("RENAME_MAP_UNREADABLE: {path}: {erro}"),
    })?;
    parse_rename_map(&text).map_err(|erro| ProjectionError::Policy {
        message: format!("RENAME_MAP_INVALID: {path}: {erro}"),
    })
}

/// A região corrente que responde por um seletor de regra.
enum Resolved<'a> {
    /// A chave da regra ainda nomeia exatamente uma região corrente, e o mapa
    /// não declara nada sobre ela.
    Direct(&'a pinker_v0::nav::CodeRegion),
    /// A chave da regra ainda nomeia exatamente uma região corrente, e o mapa
    /// declara a identidade histórica dessa mesma região. É o caso da
    /// renomeação que mudou só `domain` ou `layer`: a chave não se mexeu e a
    /// identidade medida pela projeção congelada mudou assim mesmo.
    DirectRenamed(&'a pinker_v0::nav::CodeRegion, &'a RenameEntry),
    /// A chave da regra é a identidade histórica declarada por uma entrada do
    /// mapa, e a região corrente é outra.
    Mapped(&'a pinker_v0::nav::CodeRegion, &'a RenameEntry),
    /// Nenhuma região corresponde e nenhuma entrada de mapa a reivindica.
    Absent,
}

fn regions_with_key<'a>(
    catalog: &'a [pinker_v0::nav::CodeRegion],
    key: &str,
) -> Vec<&'a pinker_v0::nav::CodeRegion> {
    catalog.iter().filter(|region| region.key == key).collect()
}

/// Resolve o seletor de uma regra contra o catálogo corrente e o mapa.
///
/// São duas perguntas diferentes, e confundi-las foi o que deixou a renomeação
/// só de metadata invisível: *qual região a regra seleciona* é decidida pelo
/// catálogo corrente, sempre; *que identidade histórica aquela região precisa
/// de volta* é decidida pelo mapa, e a chave corrente pode não ter mudado.
/// O mapa nunca escolhe a região — só declara a relação da região que o
/// catálogo já apontou.
fn resolve_selector<'a>(
    catalog: &'a [pinker_v0::nav::CodeRegion],
    key: &str,
    renames: Option<&'a RenameMap>,
) -> Result<Resolved<'a>, ProjectionError> {
    let diretas = regions_with_key(catalog, key);
    match diretas.as_slice() {
        [region] => {
            return Ok(match renames.and_then(|map| map.by_current_key(key)) {
                Some(entry) => Resolved::DirectRenamed(region, entry),
                None => Resolved::Direct(region),
            })
        }
        [] => {}
        varias => {
            return Err(ProjectionError::Policy {
                message: format!(
                    "SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} matches {} current regions",
                    varias.len()
                ),
            })
        }
    }
    let Some(entry) = renames.and_then(|map| map.by_historical_key(key)) else {
        return Ok(Resolved::Absent);
    };
    let correntes = regions_with_key(catalog, &entry.current_key);
    match correntes.as_slice() {
        [region] => Ok(Resolved::Mapped(region, entry)),
        [] => Err(ProjectionError::Policy {
            message: format!(
                "MAPPING_CURRENT_ABSENT: current_key={} declared for historical_key={key} does not exist in the current catalog",
                entry.current_key
            ),
        }),
        varias => Err(ProjectionError::Policy {
            message: format!(
                "MAPPING_CURRENT_AMBIGUOUS: current_key={} matches {} current regions",
                entry.current_key,
                varias.len()
            ),
        }),
    }
}

/// Confere a entrada do mapa contra a região corrente que ela seleciona.
///
/// O mapa é declaração do operador e envelhece: se ele diz que o domínio
/// corrente é `X` e a região já mostra `Y`, a relação descrita não é a que
/// existe, e aplicar mesmo assim restauraria identidade a partir de uma origem
/// que ninguém conferiu.
fn check_entry_guards(
    entry: &RenameEntry,
    region: &pinker_v0::nav::CodeRegion,
) -> Result<(), ProjectionError> {
    for (campo, declarado, observado) in [
        ("domain", &entry.current_domain, region.domain.as_deref()),
        ("layer", &entry.current_layer, region.layer.as_deref()),
    ] {
        if let Some(declarado) = declarado {
            if Some(declarado.as_str()) != observado {
                return Err(ProjectionError::Policy {
                    message: format!(
                        "MAPPING_GUARD_STALE: current_key={} current_{campo} declared={declarado} observed={}",
                        entry.current_key,
                        observado.unwrap_or("—")
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Confere que a identidade histórica declarada pelo mapa é a mesma que a regra
/// já esperava.
///
/// A regra existente guarda o valor que a reconstrução espera. Se o operador
/// declarar outro, uma das duas afirmações está errada, e escolher entre elas
/// seria o reconciliador decidindo história.
fn check_rule_agrees_with_entry(
    owner: &str,
    rule_key: &str,
    expect_domain: Option<&str>,
    expect_layer: Option<&str>,
    entry: &RenameEntry,
    region: &pinker_v0::nav::CodeRegion,
) -> Result<(), ProjectionError> {
    for (campo, declarado_pela_regra, historico_do_mapa, corrente) in [
        (
            "domain",
            expect_domain,
            entry.historical_domain.as_deref(),
            region.domain.as_deref(),
        ),
        (
            "layer",
            expect_layer,
            entry.historical_layer.as_deref(),
            region.layer.as_deref(),
        ),
    ] {
        let Some(pela_regra) = declarado_pela_regra else {
            continue;
        };
        let esperado = historico_do_mapa.or(corrente);
        if Some(pela_regra) != esperado {
            return Err(ProjectionError::Policy {
                message: format!(
                    "MAPPING_CONTRADICTS_AUTHORITY: owner_file={owner} rule_key={rule_key} expect_{campo}={pela_regra} map_historical_{campo}={}",
                    historico_do_mapa.unwrap_or("—")
                ),
            });
        }
    }
    Ok(())
}

/// Constrói a regra reconciliada de uma região renomeada.
///
/// A regra resultante é sempre `override-region`: ela é a única operação capaz
/// de restaurar mais de um campo como uma unidade, e uma renomeação de
/// identidade quase nunca é de um campo só. Promover `override-hash` não muda o
/// orçamento, porque as duas contam como override.
#[allow(clippy::too_many_arguments)]
fn reconciled_identity_rule(
    entry: &RenameEntry,
    region: &pinker_v0::nav::CodeRegion,
    from_hash: Option<String>,
    to_hash: Option<String>,
    to_summary: Option<String>,
    expect_file: Option<String>,
    to_file: Option<String>,
    expect_domain: Option<String>,
    expect_layer: Option<String>,
) -> Rule {
    // A guarda declarada pela regra que já existia é preservada, e só é
    // substituída quando a restauração a exige. Reconciliar nunca é ocasião
    // para uma regra sair mais fraca do que entrou.
    let guarda = |restaura: bool, anterior: Option<String>, corrente: Option<String>| {
        if restaura {
            corrente
        } else {
            anterior
        }
    };
    Rule::OverrideRegion {
        key: entry.current_key.clone(),
        to_key: entry.historical_key.clone(),
        from_hash,
        to_hash,
        from_summary: to_summary.as_ref().map(|_| region.summary.clone()),
        to_summary,
        expect_file,
        to_file,
        expect_domain: guarda(
            entry.historical_domain.is_some(),
            expect_domain,
            region.domain.clone(),
        ),
        to_domain: entry.historical_domain.clone(),
        expect_layer: guarda(
            entry.historical_layer.is_some(),
            expect_layer,
            region.layer.clone(),
        ),
        to_layer: entry.historical_layer.clone(),
    }
}

fn plan_recipe_reconciliation(
    root: &core::RepoRoot,
    catalog: &[pinker_v0::nav::CodeRegion],
    renames: Option<&RenameMap>,
) -> Result<ReconcilePlan, ProjectionError> {
    let store = ProjectionStore::load(root.path())?;
    verify_historical_frozen_authority(root.path(), &store)?;
    let mut desired = Vec::new();
    let mut changes = Vec::new();
    let mut used: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // O lado corrente de cada entrada é conferido antes de qualquer
    // planejamento: um mapa que nomeia região inexistente ou ambígua descreve
    // uma relação que não existe, e descobrir isso só no fim confundiria a
    // causa com "entrada sem uso".
    if let Some(map) = renames {
        for entry in &map.entries {
            match regions_with_key(catalog, &entry.current_key).as_slice() {
                [_] => {}
                [] => {
                    return Err(ProjectionError::Policy {
                        message: format!(
                            "MAPPING_CURRENT_ABSENT: current_key={} does not exist in the current catalog",
                            entry.current_key
                        ),
                    })
                }
                varias => {
                    return Err(ProjectionError::Policy {
                        message: format!(
                            "MAPPING_CURRENT_AMBIGUOUS: current_key={} matches {} current regions",
                            entry.current_key,
                            varias.len()
                        ),
                    })
                }
            }
        }
    }

    for stored in store.recipes() {
        let mut recipe = stored.recipe.clone();
        for rule in &mut recipe.rules {
            match rule {
                Rule::ExcludeKey { key, .. } => {
                    // Uma exclusão só é candidata quando sua chave deixou de
                    // existir: enquanto ela casa, o orçamento declarado decide,
                    // e mexer nela seria inventar trabalho.
                    if !regions_with_key(catalog, key).is_empty() {
                        continue;
                    }
                    if let Resolved::Mapped(_, entry) = resolve_selector(catalog, key, renames)? {
                        // A região excluída não chega a projeção histórica
                        // nenhuma. Declarar identidade histórica de domínio ou
                        // camada para ela é declarar uma restauração que nada
                        // aplica, e aceitar em silêncio seria ignorar o mapa.
                        if entry.historical_domain.is_some() || entry.historical_layer.is_some() {
                            return Err(ProjectionError::Policy {
                                message: format!(
                                    "MAPPING_RESTORES_EXCLUDED_REGION: current_key={} is excluded from every historical projection and has no domain/layer identity to restore",
                                    entry.current_key
                                ),
                            });
                        }
                        changes.push(format!(
                            "owner_file={} rule_key={} planned_allowed_mutation=recipe.exclude_key historical_key={} current_key={} reason=EXPLICIT_RENAME_MAPPED",
                            stored.path, key, key, entry.current_key
                        ));
                        used.insert(entry.current_key.clone());
                        key.clone_from(&entry.current_key);
                    }
                }
                Rule::OverrideHash {
                    key,
                    from,
                    to,
                    expect_file,
                    expect_domain,
                    expect_layer,
                } => {
                    match resolve_selector(catalog, key, renames)? {
                        Resolved::Direct(region) => {
                            if region.hash != *from {
                                if expect_file.as_deref() != Some(region.file.as_str())
                                    || expect_domain.as_deref() != region.domain.as_deref()
                                    || expect_layer.as_deref() != region.layer.as_deref()
                                {
                                    return Err(ProjectionError::Policy { message: format!("SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} structural identity changed") });
                                }
                                changes.push(format!("owner_file={} rule_key={} path={} domain={} layer={} expected={} observed={} planned_allowed_mutation=recipe.from reason=MECHANICALLY_RECONCILABLE", stored.path, key, region.file, region.domain.as_deref().unwrap_or("—"), region.layer.as_deref().unwrap_or("—"), from, region.hash));
                                from.clone_from(&region.hash);
                            }
                        }
                        Resolved::Mapped(region, entry) | Resolved::DirectRenamed(region, entry) => {
                            check_entry_guards(entry, region)?;
                            check_rule_agrees_with_entry(
                                &stored.path,
                                key,
                                expect_domain.as_deref(),
                                expect_layer.as_deref(),
                                entry,
                                region,
                            )?;
                            if expect_file.is_some()
                                && expect_file.as_deref() != Some(region.file.as_str())
                            {
                                return Err(ProjectionError::Policy { message: format!("SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} structural identity changed") });
                            }
                            changes.push(format!(
                                "owner_file={} rule_key={} path={} planned_allowed_mutation=recipe.override_region historical_key={} current_key={} historical_domain={} historical_layer={} reason=EXPLICIT_RENAME_MAPPED",
                                stored.path,
                                key,
                                region.file,
                                entry.historical_key.as_deref().unwrap_or("—"),
                                entry.current_key,
                                entry.historical_domain.as_deref().unwrap_or("—"),
                                entry.historical_layer.as_deref().unwrap_or("—")
                            ));
                            used.insert(entry.current_key.clone());
                            *rule = reconciled_identity_rule(
                                entry,
                                region,
                                Some(region.hash.clone()),
                                Some(to.clone()),
                                None,
                                expect_file.clone(),
                                None,
                                expect_domain.clone(),
                                expect_layer.clone(),
                            );
                        }
                        Resolved::Absent => {
                            return Err(ProjectionError::Policy { message: format!("SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} requires exactly one current region") })
                        }
                    }
                }
                Rule::OverrideRegion {
                    key,
                    from_hash,
                    to_hash,
                    from_summary,
                    to_summary,
                    expect_file,
                    to_file,
                    expect_domain,
                    expect_layer,
                    ..
                } => {
                    match resolve_selector(catalog, key, renames)? {
                        Resolved::Direct(region) => {
                            let hash_changed =
                                from_hash.as_deref().is_some_and(|hash| hash != region.hash);
                            let summary_changed = from_summary
                                .as_deref()
                                .is_some_and(|summary| summary != region.summary);
                            if hash_changed || summary_changed {
                                if expect_file.as_deref() != Some(region.file.as_str())
                                    || expect_domain.as_deref() != region.domain.as_deref()
                                    || expect_layer.as_deref() != region.layer.as_deref()
                                {
                                    return Err(ProjectionError::Policy { message: format!("SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} structural identity changed") });
                                }
                                changes.push(format!("owner_file={} rule_key={} path={} domain={} layer={} expected_hash={} observed_hash={} expected_summary={} observed_summary={} planned_allowed_mutation=recipe.from_* reason=MECHANICALLY_RECONCILABLE", stored.path, key, region.file, region.domain.as_deref().unwrap_or("—"), region.layer.as_deref().unwrap_or("—"), from_hash.as_deref().unwrap_or("—"), region.hash, from_summary.as_deref().unwrap_or("—"), region.summary));
                                if let Some(hash) = from_hash {
                                    hash.clone_from(&region.hash);
                                }
                                if let Some(summary) = from_summary {
                                    summary.clone_from(&region.summary);
                                }
                            }
                        }
                        Resolved::Mapped(region, entry) | Resolved::DirectRenamed(region, entry) => {
                            check_entry_guards(entry, region)?;
                            check_rule_agrees_with_entry(
                                &stored.path,
                                key,
                                expect_domain.as_deref(),
                                expect_layer.as_deref(),
                                entry,
                                region,
                            )?;
                            if expect_file.is_some()
                                && expect_file.as_deref() != Some(region.file.as_str())
                            {
                                return Err(ProjectionError::Policy { message: format!("SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} structural identity changed") });
                            }
                            changes.push(format!(
                                "owner_file={} rule_key={} path={} planned_allowed_mutation=recipe.override_region historical_key={} current_key={} historical_domain={} historical_layer={} reason=EXPLICIT_RENAME_MAPPED",
                                stored.path,
                                key,
                                region.file,
                                entry.historical_key.as_deref().unwrap_or("—"),
                                entry.current_key,
                                entry.historical_domain.as_deref().unwrap_or("—"),
                                entry.historical_layer.as_deref().unwrap_or("—")
                            ));
                            used.insert(entry.current_key.clone());
                            *rule = reconciled_identity_rule(
                                entry,
                                region,
                                from_hash.as_ref().map(|_| region.hash.clone()),
                                to_hash.clone(),
                                to_summary.clone(),
                                expect_file.clone(),
                                to_file.clone(),
                                expect_domain.clone(),
                                expect_layer.clone(),
                            );
                        }
                        Resolved::Absent => {
                            return Err(ProjectionError::Policy { message: format!("SEMANTIC_AMBIGUITY_BLOCK: rule_key={key} requires exactly one current region") })
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(map) = renames {
            create_missing_identity_rules(
                &stored.path,
                &mut recipe,
                catalog,
                map,
                &mut used,
                &mut changes,
            )?;
        }

        // A versão sobe quando — e só quando — a receita passa a usar
        // capacidade que a versão declarada não possui. Uma receita que não
        // ganhou capacidade nova continua exatamente na versão em que estava.
        let exigido = recipe
            .rules
            .iter()
            .map(|rule| rule.min_schema(SchemaAuthority::Recipe))
            .max()
            .unwrap_or(recipe.schema);
        if exigido > recipe.schema {
            changes.push(format!(
                "owner_file={} planned_allowed_mutation=recipe.schema from={} to={exigido} reason=CAPABILITY_REQUIRES_SCHEMA",
                stored.path, recipe.schema
            ));
            recipe.schema = exigido;
        }

        if recipe != stored.recipe {
            desired.push((
                stored.path.clone(),
                pinker_v0::nav_projection_recipe::render_recipe(&recipe).into_bytes(),
            ));
        }
    }

    // Entrada declarada e não usada não é ruído: ou o operador descreveu uma
    // renomeação que não aconteceu, ou descreveu a errada. Ignorá-la deixaria
    // passar um mapa que o autor acredita ter efeito e não tem.
    if let Some(map) = renames {
        for entry in &map.entries {
            if !used.contains(&entry.current_key) {
                return Err(ProjectionError::Policy {
                    message: format!(
                        "MAPPING_ENTRY_UNUSED: current_key={} produced no reconciliation",
                        entry.current_key
                    ),
                });
            }
        }
    }

    let paths: Vec<_> = desired.iter().map(|(path, _)| path.as_str()).collect();
    let allowlist = core::Allowlist::new(&paths).map_err(|cause| ProjectionError::Policy {
        message: cause.to_string(),
    })?;
    // O mapa entra no produtor do plano, e o produtor entra na forma canônica
    // que o digest assina. Dois mapas distintos nunca autorizam o mesmo plano,
    // ainda que por acaso produzissem os mesmos bytes de receita.
    let producer = match renames {
        None => "nav.projecao.reconcile".to_string(),
        Some(map) => format!("nav.projecao.reconcile+renames:{}", map.fingerprint()),
    };
    let mut builder = core::PlanBuilder::new(&producer, allowlist);
    for (path, bytes) in desired {
        builder = builder
            .desire(&path, bytes)
            .map_err(ProjectionError::Automation)?;
    }
    let plan = builder.build().map_err(ProjectionError::Automation)?;
    let observed = core::observe(root, &plan).map_err(ProjectionError::Automation)?;
    let check = core::check(&plan, &observed).map_err(ProjectionError::Automation)?;
    Ok(ReconcilePlan {
        plan,
        check,
        changes,
        rename_map: renames.map(RenameMap::fingerprint),
    })
}

/// Cria a regra de restauração das entradas que nenhuma regra existente cobre.
///
/// Uma região histórica não precisa ter regra: enquanto sua identidade corrente
/// é a histórica, a reconstrução a carrega de graça. Renomeá-la é justamente o
/// que cria a necessidade da regra, e nenhuma regra antiga nomeia a região nova.
///
/// Região que a própria receita exclui não recebe restauração: exclusões correm
/// antes dos overrides, e um override sobre região excluída não teria o que
/// consumir.
fn create_missing_identity_rules(
    owner: &str,
    recipe: &mut pinker_v0::nav_projection_recipe::Recipe,
    catalog: &[pinker_v0::nav::CodeRegion],
    map: &RenameMap,
    used: &mut std::collections::BTreeSet<String>,
    changes: &mut Vec<String>,
) -> Result<(), ProjectionError> {
    let sobreviventes = match recipe.keys_surviving_exclusions(catalog) {
        Ok(chaves) => chaves,
        Err(failure) => {
            return Err(ProjectionError::Policy {
                message: format!("RECIPE_EXCLUSIONS_UNRESOLVED: owner_file={owner} {failure}"),
            })
        }
    };

    let mut novas = Vec::new();
    for entry in &map.entries {
        if used.contains(&entry.current_key) {
            continue;
        }
        if !sobreviventes.contains(&entry.current_key) {
            continue;
        }
        let correntes = regions_with_key(catalog, &entry.current_key);
        let [region] = correntes.as_slice() else {
            continue;
        };
        check_entry_guards(entry, region)?;
        changes.push(format!(
            "owner_file={owner} rule_key={} path={} planned_allowed_mutation=recipe.new_override_region historical_key={} current_key={} historical_domain={} historical_layer={} reason=EXPLICIT_RENAME_MAPPED",
            entry.current_key,
            region.file,
            entry.historical_key.as_deref().unwrap_or("—"),
            entry.current_key,
            entry.historical_domain.as_deref().unwrap_or("—"),
            entry.historical_layer.as_deref().unwrap_or("—")
        ));
        used.insert(entry.current_key.clone());
        novas.push(reconciled_identity_rule(
            entry, region, None, None, None, None, None, None, None,
        ));
    }
    if !novas.is_empty() {
        recipe.expected_overrides += novas.len() as u64;
        recipe.rules.extend(novas);
    }
    Ok(())
}

fn print_reconcile_plan(json: bool, planning: &ReconcilePlan) {
    if json {
        let changes: Vec<_> = planning
            .changes
            .iter()
            .map(|change| json_quote(change))
            .collect();
        println!("{{\"schema\":1,\"command\":\"reconciliar\",\"outcome\":{},\"digest\":{},\"producer\":{},\"rename_map\":{},\"changes\":[{}]}}", json_quote(if planning.changes.is_empty() { "NO_CHANGE" } else { "MECHANICALLY_RECONCILABLE" }), json_quote(&planning.plan.digest()), json_quote(planning.plan.producer()), planning.rename_map.as_deref().map_or("null".to_string(), json_quote), changes.join(","));
    } else {
        println!(
            "{}\\ndigest: {}",
            if planning.changes.is_empty() {
                "NO_CHANGE"
            } else {
                "MECHANICALLY_RECONCILABLE"
            },
            planning.plan.digest()
        );
        println!("producer: {}", planning.plan.producer());
        if let Some(fingerprint) = &planning.rename_map {
            println!("rename_map: {fingerprint}");
        }
        for change in &planning.changes {
            println!("{change}");
        }
    }
}

fn json_quote(value: &str) -> String {
    format!("{value:?}")
}

fn run_projection_show(root: &core::RepoRoot, json: bool, id: &str, observed: bool) -> i32 {
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

fn run_projection_verify(root: &core::RepoRoot, json: bool, id: Option<&str>) -> i32 {
    let store = match ProjectionStore::load(root.path()) {
        Ok(store) => store,
        Err(error) => {
            return print_projection_error("verificar", json, &ProjectionError::Authority(error))
        }
    };
    if let Err(error) = verify_historical_frozen_authority(root.path(), &store) {
        return print_projection_error("verificar", json, &error);
    }
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

/// A reconstrução prova consistência; a base confiável prova que a autoridade
/// FROZEN não foi recalibrada junto com a recipe no checkout candidato.
fn verify_historical_frozen_authority(
    root: &Path,
    store: &ProjectionStore,
) -> Result<(), ProjectionError> {
    let repository = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map_err(|error| ProjectionError::Harness {
            path: None,
            message: format!(
                "HISTORICAL_AUTHORITY_UNVERIFIABLE: cannot inspect repository authority: {error}"
            ),
        })?;
    if !repository.status.success() {
        return Err(ProjectionError::Harness {
            path: None,
            message: format!(
                "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted Git historical reference is unavailable ({})",
                String::from_utf8_lossy(&repository.stderr).trim()
            ),
        });
    }
    let listing = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-tree",
            "-r",
            "--name-only",
            "origin/main",
            "--",
            ".pinker/projections",
        ])
        .output()
        .map_err(|error| ProjectionError::Harness {
            path: None,
            message: format!(
                "HISTORICAL_AUTHORITY_UNVERIFIABLE: cannot list trusted origin/main reference: {error}"
            ),
        })?;
    if !listing.status.success() {
        return Err(ProjectionError::Harness {
            path: None,
            message: format!(
                "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main reference is unavailable ({})",
                String::from_utf8_lossy(&listing.stderr).trim()
            ),
        });
    }
    let historical_paths: Vec<_> = String::from_utf8_lossy(&listing.stdout)
        .lines()
        .filter(|path| path.ends_with(".toml") && !path.starts_with(".pinker/projections/recipes/"))
        .map(str::to_owned)
        .collect();
    for path in &historical_paths {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("show")
            .arg(format!("origin/main:{path}"))
            .output()
            .map_err(|error| ProjectionError::Harness {
                path: Some(path.to_string()),
                message: format!(
                    "HISTORICAL_AUTHORITY_UNVERIFIABLE: cannot read trusted origin/main snapshot: {error}"
                ),
            })?;
        if !output.status.success() {
            return Err(ProjectionError::Harness {
                path: Some(path.to_string()),
                message:
                    "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main snapshot is unavailable"
                        .to_string(),
            });
        }
        let historical = pinker_v0::nav_projection_snapshot::parse(
            std::str::from_utf8(&output.stdout).map_err(|error| ProjectionError::Harness {
                path: Some(path.to_string()),
                message: format!(
                    "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main snapshot is not UTF-8: {error}"
                ),
            })?,
        )
        .map_err(|error| ProjectionError::Harness {
            path: Some(path.to_string()),
            message: format!(
                "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main snapshot is invalid: {error}"
            ),
        })?;
        if historical.state == pinker_v0::nav_projection_snapshot::SnapshotState::Frozen
            && !store.snapshots().any(|stored| stored.path == *path)
        {
            return Err(ProjectionError::Harness {
                path: Some(path.to_string()),
                message: format!(
                    "HISTORICAL_AUTHORITY_MUTATED: protected FROZEN authority is absent; owner_file={path} safe_recovery=restore FROZEN authority from origin/main"
                ),
            });
        }
    }
    for stored in store.snapshots().filter(|stored| {
        stored.snapshot.state == pinker_v0::nav_projection_snapshot::SnapshotState::Frozen
            && historical_paths.contains(&stored.path)
    }) {
        let spec = format!("origin/main:{}", stored.path);
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("show")
            .arg(&spec)
            .output()
            .map_err(|error| ProjectionError::Harness {
                path: Some(stored.path.clone()),
                message: format!(
                    "HISTORICAL_AUTHORITY_UNVERIFIABLE: cannot read trusted origin/main reference: {error}"
                ),
            })?;
        if !output.status.success() {
            return Err(ProjectionError::Harness {
                path: Some(stored.path.clone()),
                message: format!(
                    "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main reference is unavailable; restore the FROZEN authority from origin/main, then reconcile the recipe through the explicit plan/apply path ({})",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            });
        }
        let historical = pinker_v0::nav_projection_snapshot::parse(
            std::str::from_utf8(&output.stdout).map_err(|error| ProjectionError::Harness {
                path: Some(stored.path.clone()),
                message: format!(
                    "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main snapshot is not UTF-8: {error}"
                ),
            })?,
        )
        .map_err(|error| ProjectionError::Harness {
            path: Some(stored.path.clone()),
            message: format!(
                "HISTORICAL_AUTHORITY_UNVERIFIABLE: trusted origin/main snapshot is invalid: {error}"
            ),
        })?;
        if historical != stored.snapshot {
            return Err(ProjectionError::Harness {
                path: Some(stored.path.clone()),
                message: format!(
                    "HISTORICAL_AUTHORITY_MUTATED: protected FROZEN fields differ from origin/main; owner_file={} safe_recovery=restore FROZEN authority, then reconcile recipe only through the explicit plan/apply path",
                    stored.path
                ),
            });
        }
    }
    Ok(())
}

fn load_projection_catalog(root: &core::RepoRoot) -> Result<nav::CodeCatalog, ProjectionError> {
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
    use core::Failure;
    let failure_exit = |failure: &Failure| match failure {
        Failure::HarnessFailure(core::HarnessCause::RootNotFound { .. }) => EXIT_CATALOG,
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
    let projection_store = ProjectionStore::load(root.path()).ok();
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
