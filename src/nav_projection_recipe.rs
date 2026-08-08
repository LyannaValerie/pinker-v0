//! Trama Pinker — receitas de reconstrução e composição de snapshots (#384).
//!
//! O inventário da cartografia mostrou 15 helpers de reconstrução dispostos num
//! DAG, e apenas 13 estados com medida histórica própria. Oito helpers são nós
//! puramente intermediários: nenhum teste os chama e nenhum produz medida.
//!
//! Inventar snapshots para representá-los seria fabricar história. Este módulo é
//! a autoridade **menor** que esses nós merecem: uma receita é uma transformação
//! reutilizável, e nada além disso.
//!
//! # Duas autoridades, deliberadamente distintas
//!
//! | | snapshot | receita |
//! |---|---|---|
//! | local | `.pinker/projections/<id>.toml` | `.pinker/projections/recipes/<id>.toml` |
//! | medidas | sim | **não** |
//! | estado | sim | **não** |
//! | predecessor | sim | **não** |
//! | compõe | `base_snapshot` + `recipes` | apenas `recipes` |
//!
//! Uma receita **não pode** depender de snapshot. Isso mantém o grafo numa
//! direção só — `snapshot → snapshot` e `snapshot → receita → receita` — e
//! impede que uma transformação genérica adquira dependência histórica
//! escondida.
//!
//! # Namespaces resolvem estruturalmente
//!
//! `base_snapshot` procura somente entre snapshots; `recipes` procura somente
//! entre receitas. Não há resolvedor polimórfico, então um snapshot e uma
//! receita podem até compartilhar o mesmo identificador textual sem ambiguidade
//! — e não existe falha de "base ambígua", porque não existe a ambiguidade.
//!
//! # Versionamento independente
//!
//! O formato de receita nasce agora e estreia em [`RECIPE_SCHEMA`] = 1. O
//! `schema = 2` pertence ao formato de **snapshot**, que foi quem ganhou
//! composição.

use crate::nav::CodeRegion;
use crate::nav_projection_snapshot::{
    apply_rules, build_rule, measure, optional_list, parse_raw, reject_unknown, render_rule_body,
    require_integer, require_text, sort_rules, toml_escape, validate_id, HarnessFailure, Measures,
    Outcome, ProjectionSnapshot, Rule, RuleConsumption, SchemaAuthority, SnapshotState,
    VerifyReport,
};
use std::collections::BTreeMap;

/// Primeira versão do formato de receita.
pub const RECIPE_SCHEMA_V1: u64 = 1;

/// Segunda versão: acrescenta `override-region`, pela mesma razão histórica que
/// levou o snapshot ao schema 3 — a reconstrução real restaura `summary`.
pub const RECIPE_SCHEMA_V2: u64 = 2;

/// Versão máxima aceita do formato de receita.
pub const RECIPE_SCHEMA: u64 = RECIPE_SCHEMA_V2;

/// Diretório repo-relativo canônico das receitas.
pub const RECIPES_DIR: &str = ".pinker/projections/recipes/";

// @pinker-nav:start trama.snapshots.receita
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Receita de reconstrução: autoridade reutilizável e mínima para as transformações intermediárias que não possuem medida histórica própria, sem medidas, sem estado e sem predecessor, capaz de compor apenas outras receitas e nunca snapshots, com versionamento próprio independente do formato de snapshot.

/// Uma transformação reutilizável, sem identidade histórica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub schema: u64,
    pub id: String,
    /// Receitas aplicadas antes das regras locais, **na ordem declarada**.
    pub steps: Vec<String>,
    pub expected_overrides: u64,
    pub expected_exclusions: u64,
    pub rules: Vec<Rule>,
}
// @pinker-nav:end trama.snapshots.receita

// @pinker-nav:start trama.snapshots.receita-serializacao
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Parser estrito e renderer canônico do formato de receita: rejeita seção de medidas, estado e predecessor porque uma receita não tem identidade histórica, valida passos como identificadores sem repetição nem autorreferência, e preserva a ordem declarada dos passos porque ela é procedural.

const RECIPE_ROOT_KEYS: [&str; 2] = ["schema", "id"];
const RECIPE_RECONSTRUCTION_KEYS: [&str; 3] =
    ["steps", "expected_overrides", "expected_exclusions"];

/// Interpreta o texto de uma receita. Não toca no filesystem.
pub fn parse_recipe(text: &str) -> Result<Recipe, HarnessFailure> {
    let raw = parse_raw(text).map_err(HarnessFailure::Toml)?;

    // Uma receita não tem medidas, estado nem predecessor. A ausência é
    // estrutural, e declará-los é erro **nomeado** — a checagem vem antes da
    // rejeição genérica de chave desconhecida, para que o diagnóstico diga o
    // que de fato aconteceu.
    if raw.measures.is_some() {
        return Err(HarnessFailure::RecipeHasSnapshotField {
            field: "measures".to_string(),
        });
    }
    for proibido in ["state", "predecessor", "justification"] {
        if raw.root.get(proibido).is_some() {
            return Err(HarnessFailure::RecipeHasSnapshotField {
                field: proibido.to_string(),
            });
        }
    }
    if let Some(reconstruction) = &raw.reconstruction {
        for proibido in ["base_snapshot", "recipes"] {
            if reconstruction.get(proibido).is_some() {
                return Err(HarnessFailure::RecipeHasSnapshotField {
                    field: format!("reconstruction.{}", proibido),
                });
            }
        }
    }
    reject_unknown(&raw.root, &RECIPE_ROOT_KEYS, "")?;

    let schema = match raw.root.get("schema") {
        Some(escalar) => match escalar.as_integer() {
            Some(value) => value,
            None => {
                return Err(HarnessFailure::InvalidField {
                    field: "schema".to_string(),
                    msg: "esperado inteiro, não texto".to_string(),
                })
            }
        },
        None => {
            return Err(HarnessFailure::SchemaUnknown {
                authority: SchemaAuthority::Recipe,
                found: 0,
            })
        }
    };
    if !(RECIPE_SCHEMA_V1..=RECIPE_SCHEMA_V2).contains(&schema) {
        return Err(HarnessFailure::SchemaUnknown {
            authority: SchemaAuthority::Recipe,
            found: schema,
        });
    }

    let id = require_text(&raw.root, "id", "")?;
    validate_id(&id, "id")?;

    let Some(reconstruction) = raw.reconstruction else {
        return Err(HarnessFailure::MissingField {
            field: "reconstruction".to_string(),
        });
    };
    reject_unknown(
        &reconstruction,
        &RECIPE_RECONSTRUCTION_KEYS,
        "reconstruction.",
    )?;
    let steps = optional_list(&reconstruction, "steps", "reconstruction.")?;
    for (posicao, passo) in steps.iter().enumerate() {
        validate_id(passo, &format!("reconstruction.steps[{}]", posicao))?;
        if passo == &id {
            return Err(HarnessFailure::RecipeSelfStep { id: id.clone() });
        }
        if steps[..posicao].contains(passo) {
            return Err(HarnessFailure::InvalidField {
                field: format!("reconstruction.steps[{}]", posicao),
                msg: format!("passo '{}' declarado duas vezes no mesmo escopo", passo),
            });
        }
    }
    let expected_overrides =
        require_integer(&reconstruction, "expected_overrides", "reconstruction.")?;
    let expected_exclusions =
        require_integer(&reconstruction, "expected_exclusions", "reconstruction.")?;

    let mut rules = Vec::with_capacity(raw.rules.len());
    for (index, table) in raw.rules.iter().enumerate() {
        let rule = build_rule(table, index)?;
        // A matriz de capacidades é por autoridade: a mesma operação pode exigir
        // versões diferentes em snapshot e em receita.
        let exigido = rule.min_schema(SchemaAuthority::Recipe);
        if exigido > schema {
            return Err(HarnessFailure::CapabilityRequiresSchema {
                authority: SchemaAuthority::Recipe,
                capability: format!("op '{}'", rule.op()),
                found_schema: schema,
                required_schema: exigido,
            });
        }
        rules.push(rule);
    }
    let encontrados_override = rules.iter().filter(|r| r.is_override()).count() as u64;
    let encontradas_exclusoes = rules.len() as u64 - encontrados_override;
    if expected_overrides != encontrados_override {
        return Err(if expected_overrides > encontrados_override {
            HarnessFailure::OverrideMissing {
                declared: expected_overrides,
                found: encontrados_override,
            }
        } else {
            HarnessFailure::OverrideExcess {
                declared: expected_overrides,
                found: encontrados_override,
            }
        });
    }
    if expected_exclusions != encontradas_exclusoes {
        return Err(if expected_exclusions > encontradas_exclusoes {
            HarnessFailure::ExclusionMissing {
                declared: expected_exclusions,
                found: encontradas_exclusoes,
            }
        } else {
            HarnessFailure::ExclusionExcess {
                declared: expected_exclusions,
                found: encontradas_exclusoes,
            }
        });
    }
    sort_rules(&mut rules);

    Ok(Recipe {
        schema,
        id,
        steps,
        expected_overrides,
        expected_exclusions,
        rules,
    })
}

/// Serializa uma receita na forma canônica.
///
/// A ordem declarada de `steps` é preservada: ela é procedural, e ordená-la por
/// nome mudaria o significado. As regras locais, que são independentes entre si,
/// seguem a mesma canonicalização do formato de snapshot.
pub fn render_recipe(recipe: &Recipe) -> String {
    let mut out = String::new();
    out.push_str(&format!("schema = {}\n", recipe.schema));
    out.push_str(&format!("id = {}\n", toml_escape(&recipe.id)));

    out.push_str("\n[reconstruction]\n");
    if !recipe.steps.is_empty() {
        let itens: Vec<String> = recipe.steps.iter().map(|p| toml_escape(p)).collect();
        out.push_str(&format!("steps = [{}]\n", itens.join(", ")));
    }
    out.push_str(&format!(
        "expected_overrides = {}\n",
        recipe.expected_overrides
    ));
    out.push_str(&format!(
        "expected_exclusions = {}\n",
        recipe.expected_exclusions
    ));

    let mut rules = recipe.rules.clone();
    sort_rules(&mut rules);
    for rule in &rules {
        out.push_str("\n[[rules]]\n");
        out.push_str(&render_rule_body(rule));
    }
    out
}

/// Path repo-relativo canônico do arquivo desta receita.
impl Recipe {
    pub fn relative_path(&self) -> String {
        format!("{}{}.toml", RECIPES_DIR, self.id)
    }
}
// @pinker-nav:end trama.snapshots.receita-serializacao

// @pinker-nav:start trama.snapshots.biblioteca
// @pinker-nav:domain snapshots
// @pinker-nav:layer trama
// @pinker-nav:summary Biblioteca que reúne snapshots e receitas em namespaces separados e resolve a composição: base ausente, autorreferência, ciclo no grafo completo, receita ausente, receita repetida no mesmo escopo e dependência transitiva de FROZEN para CANDIDATE são todas falhas de harness distintas.

/// Conjunto de snapshots e receitas, em namespaces separados.
#[derive(Debug, Clone, Default)]
pub struct Library {
    snapshots: BTreeMap<String, ProjectionSnapshot>,
    recipes: BTreeMap<String, Recipe>,
}

impl Library {
    pub fn new() -> Library {
        Library::default()
    }

    /// Acrescenta um snapshot. Identificador repetido é falha de harness.
    pub fn with_snapshot(
        mut self,
        snapshot: ProjectionSnapshot,
    ) -> Result<Library, HarnessFailure> {
        if self.snapshots.contains_key(&snapshot.id) {
            return Err(HarnessFailure::DuplicateSnapshot {
                id: snapshot.id.clone(),
            });
        }
        self.snapshots.insert(snapshot.id.clone(), snapshot);
        Ok(self)
    }

    /// Acrescenta uma receita. Namespace separado: um snapshot com o mesmo
    /// identificador textual não colide.
    pub fn with_recipe(mut self, recipe: Recipe) -> Result<Library, HarnessFailure> {
        if self.recipes.contains_key(&recipe.id) {
            return Err(HarnessFailure::DuplicateRecipe {
                id: recipe.id.clone(),
            });
        }
        self.recipes.insert(recipe.id.clone(), recipe);
        Ok(self)
    }

    pub fn snapshot(&self, id: &str) -> Option<&ProjectionSnapshot> {
        self.snapshots.get(id)
    }

    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipes.get(id)
    }

    /// Identificadores de snapshot em ordem canônica.
    pub fn snapshot_ids(&self) -> Vec<&str> {
        self.snapshots.keys().map(String::as_str).collect()
    }

    /// Identificadores de receita em ordem canônica.
    pub fn recipe_ids(&self) -> Vec<&str> {
        self.recipes.keys().map(String::as_str).collect()
    }
}

/// Resultado de uma composição resolvida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    /// Regiões do estado reconstruído.
    pub regions: Vec<CodeRegion>,
    /// Consumo por escopo, na ordem de aplicação.
    pub ledger: Vec<ScopeConsumption>,
    /// Snapshots de base verificados durante a resolução, do mais profundo ao
    /// mais raso.
    pub verified_bases: Vec<String>,
}

impl Composition {
    pub fn measures(&self) -> Measures {
        measure(self.regions.iter())
    }
}

/// Consumo de um escopo — base, receita ou local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeConsumption {
    pub scope: String,
    pub entries: Vec<RuleConsumption>,
}

/// Resolve e verifica a reconstrução de um snapshot.
///
/// A ordem de aplicação é fixa:
///
/// 1. resolve a base recursivamente **e verifica as medidas dela**;
/// 2. aplica as receitas na ordem declarada, cada uma resolvendo seus próprios
///    passos antes das próprias regras;
/// 3. aplica as exclusões locais;
/// 4. aplica os overrides locais.
///
/// Uma base não é apenas reconstruída: ela é conferida contra as próprias
/// medidas congeladas antes de servir de fundação. Sem isso, uma base quebrada
/// poderia ser compensada por coincidência pelas regras do descendente, e a
/// separação entre erro de harness e drift — que é o ponto da Issue #384 —
/// deixaria de valer.
pub fn resolve(
    library: &Library,
    snapshot_id: &str,
    catalog: &[CodeRegion],
) -> Result<Composition, HarnessFailure> {
    let mut visitando: Vec<String> = Vec::new();
    resolve_snapshot(library, snapshot_id, catalog, &mut visitando)
}

fn resolve_snapshot(
    library: &Library,
    snapshot_id: &str,
    catalog: &[CodeRegion],
    visitando: &mut Vec<String>,
) -> Result<Composition, HarnessFailure> {
    let marca = format!("snapshot:{}", snapshot_id);
    if visitando.contains(&marca) {
        return Err(HarnessFailure::CompositionCycle {
            path: ciclo(visitando, &marca),
        });
    }
    let Some(snapshot) = library.snapshot(snapshot_id) else {
        return Err(HarnessFailure::BaseSnapshotMissing {
            id: snapshot_id.to_string(),
        });
    };
    visitando.push(marca);

    let mut ledger: Vec<ScopeConsumption> = Vec::new();
    let mut verified_bases: Vec<String> = Vec::new();

    // (1) base, resolvida e verificada contra as próprias medidas.
    let mut regions: Vec<CodeRegion> = match &snapshot.base_snapshot {
        None => catalog.to_vec(),
        Some(base_id) => {
            let base = resolve_snapshot(library, base_id, catalog, visitando)?;
            let base_snapshot = library
                .snapshot(base_id)
                .expect("resolvida acima, portanto presente");
            let medidas = base.measures();
            if medidas != base_snapshot.measures {
                return Err(HarnessFailure::BaseMeasuresDiverged {
                    id: base_id.to_string(),
                    expected: base_snapshot.measures,
                    observed: medidas,
                });
            }
            verified_bases.extend(base.verified_bases.iter().cloned());
            verified_bases.push(base_id.clone());
            ledger.extend(base.ledger);
            base.regions
        }
    };

    // (2) receitas, na ordem declarada.
    for recipe_id in &snapshot.recipes {
        let saida = resolve_recipe(library, recipe_id, regions, visitando)?;
        regions = saida.0;
        ledger.extend(saida.1);
    }

    // (3) e (4) regras locais, exclusões antes de overrides.
    let (regions, entries) = apply_rules(regions, &snapshot.rules)?;
    ledger.push(ScopeConsumption {
        scope: format!("snapshot:{}", snapshot_id),
        entries,
    });

    visitando.pop();
    Ok(Composition {
        regions,
        ledger,
        verified_bases,
    })
}

type SaidaReceita = (Vec<CodeRegion>, Vec<ScopeConsumption>);

fn resolve_recipe(
    library: &Library,
    recipe_id: &str,
    entrada: Vec<CodeRegion>,
    visitando: &mut Vec<String>,
) -> Result<SaidaReceita, HarnessFailure> {
    let marca = format!("recipe:{}", recipe_id);
    if visitando.contains(&marca) {
        return Err(HarnessFailure::CompositionCycle {
            path: ciclo(visitando, &marca),
        });
    }
    let Some(recipe) = library.recipe(recipe_id) else {
        return Err(HarnessFailure::RecipeMissing {
            id: recipe_id.to_string(),
        });
    };
    visitando.push(marca);

    let mut regions = entrada;
    let mut ledger: Vec<ScopeConsumption> = Vec::new();
    for passo in &recipe.steps {
        let saida = resolve_recipe(library, passo, regions, visitando)?;
        regions = saida.0;
        ledger.extend(saida.1);
    }
    let (regions, entries) = apply_rules(regions, &recipe.rules)?;
    ledger.push(ScopeConsumption {
        scope: format!("recipe:{}", recipe_id),
        entries,
    });

    visitando.pop();
    Ok((regions, ledger))
}

fn ciclo(visitando: &[String], repetida: &str) -> String {
    let mut caminho: Vec<&str> = visitando.iter().map(String::as_str).collect();
    caminho.push(repetida);
    caminho.join(" → ")
}

/// Verifica que nenhum snapshot `FROZEN` depende, direta ou transitivamente, de
/// um `CANDIDATE`.
///
/// A proibição é transitiva de propósito: um congelado que dependa de um
/// congelado que dependa de um candidato continua apoiado em algo que ainda pode
/// mudar. Receitas não têm estado, então são neutras aqui e não podem violar a
/// regra — o que é uma consequência de não lhes darmos estado, não uma exceção.
pub fn verify_frozen_dependencies(library: &Library) -> Result<(), HarnessFailure> {
    for id in library.snapshot_ids() {
        verify_snapshot_dependencies(library, id)?;
    }
    Ok(())
}

/// Verifica a política de dependências apenas para um alvo e sua cadeia.
pub fn verify_snapshot_dependencies(library: &Library, id: &str) -> Result<(), HarnessFailure> {
    let Some(snapshot) = library.snapshot(id) else {
        return Err(HarnessFailure::BaseSnapshotMissing { id: id.to_string() });
    };
    if snapshot.state != SnapshotState::Frozen {
        return Ok(());
    }
    let mut atual: Option<String> = snapshot.base_snapshot.clone();
    let mut visitados: Vec<String> = vec![id.to_string()];
    while let Some(base_id) = atual.take() {
        if visitados.contains(&base_id) {
            return Err(HarnessFailure::CompositionCycle {
                path: ciclo(&visitados, &base_id),
            });
        }
        let Some(base) = library.snapshot(&base_id) else {
            return Err(HarnessFailure::BaseSnapshotMissing { id: base_id });
        };
        if base.state == SnapshotState::Candidate {
            return Err(HarnessFailure::FrozenDependsOnCandidate {
                frozen: id.to_string(),
                candidate: base_id,
            });
        }
        visitados.push(base_id);
        atual.clone_from(&base.base_snapshot);
    }
    Ok(())
}

/// Verifica um snapshot pela composição real (`base_snapshot`, recipes e
/// regras locais), reutilizando [`resolve`] como única autoridade.
pub fn verify_composed(library: &Library, id: &str, catalog: &[CodeRegion]) -> VerifyReport {
    let Some(snapshot) = library.snapshot(id) else {
        return missing_report(id);
    };
    if let Err(failure) = verify_snapshot_dependencies(library, id) {
        return failure_report(snapshot, failure);
    }
    let composition = match resolve(library, id, catalog) {
        Ok(composition) => composition,
        Err(failure) => return failure_report(snapshot, failure),
    };
    let observed = composition.measures();
    let mut divergences = Vec::new();
    if observed.regions != snapshot.measures.regions {
        divergences.push(crate::nav_projection_snapshot::Divergence {
            measure: "regions",
            expected: snapshot.measures.regions.to_string(),
            observed: observed.regions.to_string(),
        });
    }
    if observed.length != snapshot.measures.length {
        divergences.push(crate::nav_projection_snapshot::Divergence {
            measure: "length",
            expected: snapshot.measures.length.to_string(),
            observed: observed.length.to_string(),
        });
    }
    if observed.fnv1a64 != snapshot.measures.fnv1a64 {
        divergences.push(crate::nav_projection_snapshot::Divergence {
            measure: "fnv1a64",
            expected: snapshot.measures.fnv1a64_canonical(),
            observed: observed.fnv1a64_canonical(),
        });
    }
    VerifyReport {
        snapshot_id: snapshot.id.clone(),
        state: snapshot.state,
        predecessor: snapshot.predecessor.clone(),
        expected: snapshot.measures,
        observed: Some(observed),
        outcome: if divergences.is_empty() {
            Outcome::Match
        } else {
            Outcome::Drift(divergences)
        },
        ledger: composition
            .ledger
            .into_iter()
            .flat_map(|scope| scope.entries)
            .collect(),
    }
}

fn failure_report(snapshot: &ProjectionSnapshot, failure: HarnessFailure) -> VerifyReport {
    VerifyReport {
        snapshot_id: snapshot.id.clone(),
        state: snapshot.state,
        predecessor: snapshot.predecessor.clone(),
        expected: snapshot.measures,
        observed: None,
        outcome: Outcome::HarnessFailure(failure),
        ledger: Vec::new(),
    }
}

fn missing_report(id: &str) -> VerifyReport {
    VerifyReport {
        snapshot_id: id.to_string(),
        state: SnapshotState::Candidate,
        predecessor: None,
        expected: Measures {
            regions: 0,
            length: 0,
            fnv1a64: 0,
        },
        observed: None,
        outcome: Outcome::HarnessFailure(HarnessFailure::BaseSnapshotMissing {
            id: id.to_string(),
        }),
        ledger: Vec::new(),
    }
}
// @pinker-nav:end trama.snapshots.biblioteca
