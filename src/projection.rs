//! Trama Pinker — projeções documentais determinísticas (§12).
//!
//! Manifestos versionados (`.pinker/changes/pr-N.yaml`) são a fonte estrutural
//! das mudanças. Esta camada projeta essa fonte em **regiões geradas**
//! explícitas dentro de documentos humanos:
//!
//! ```text
//! <!-- @pinker-generated:start change.history -->
//! conteúdo gerado
//! <!-- @pinker-generated:end change.history -->
//! ```
//!
//! Regras (§12):
//! 1. Toda região gerada é propriedade da ferramenta.
//! 2. Texto fora da região é propriedade humana e nunca é tocado.
//! 3. A ferramenta nunca cria narrativa técnica por inferência — apenas projeta
//!    campos declarados nos manifestos.
//! 4. Se `updates.<flag> = true` num manifesto, precisa existir um consumidor
//!    (`[projections.<flag>]`) com região presente; caso contrário, falha.
//! 5. Projeções são idempotentes e reprodutíveis (sem timestamps).
//!
//! Projeções suportadas com conteúdo completo: `history`, `state`, `roadmap`.
//! Para outros nomes (ex.: `readme`, `manual`), só se gera dentro de uma região
//! explícita; se a região não existir, a flag é recusada — nunca inventada.

use crate::change::{Change, Manifests};
use crate::doc::{DocConfig, DocProjection};
use std::fmt;
use std::path::{Path, PathBuf};

const GEN_START: &str = "@pinker-generated:start";
const GEN_END: &str = "@pinker-generated:end";

/// Falhas ao planejar projeções.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    /// Uma flag `updates.<flag>` verdadeira não tem projeção configurada.
    FlagWithoutConsumer { flag: String },
    /// A projeção existe, mas o arquivo de destino não pôde ser lido.
    TargetUnreadable {
        name: String,
        file: String,
        msg: String,
    },
    /// A região explícita não existe no arquivo (não se inventa onde escrever).
    RegionMissing {
        name: String,
        file: String,
        region: String,
    },
    /// A região tem abertura sem fechamento (ou vice-versa).
    RegionUnbalanced {
        name: String,
        file: String,
        region: String,
    },
    /// Projeção nomeada sem gerador conhecido (não é `history`/`state`/`roadmap`
    /// e a região existe, mas a ferramenta não sabe o que projetar nela).
    UnknownGenerator { name: String },
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectionError::FlagWithoutConsumer { flag } => write!(
                f,
                "E-PROJ-CONSUMER\nflag updates.{} verdadeira sem consumidor: configure [projections.{}] em .pinker/doc.toml",
                flag, flag
            ),
            ProjectionError::TargetUnreadable { name, file, msg } => write!(
                f,
                "E-PROJ-TARGET\nprojeção '{}' não pôde ler '{}': {}",
                name, file, msg
            ),
            ProjectionError::RegionMissing {
                name,
                file,
                region,
            } => write!(
                f,
                "E-PROJ-REGION\nprojeção '{}' sem região segura '{}' em {}; adicione @pinker-generated:start/end {} (a ferramenta não inventa onde escrever)",
                name, region, file, region
            ),
            ProjectionError::RegionUnbalanced {
                name,
                file,
                region,
            } => write!(
                f,
                "E-PROJ-REGION\nprojeção '{}' com região '{}' desbalanceada em {}",
                name, region, file
            ),
            ProjectionError::UnknownGenerator { name } => write!(
                f,
                "E-PROJ-GENERATOR\nprojeção '{}' não tem gerador conhecido (suportados: history, state, roadmap)",
                name
            ),
        }
    }
}

/// Uma escrita planejada para um arquivo (conteúdo completo desejado).
#[derive(Debug, Clone)]
pub struct WritePlan {
    pub name: String,
    pub path: PathBuf,
    pub file: String,
    pub content: String,
    /// Verdadeiro se o conteúdo desejado difere do que está em disco.
    pub changed: bool,
}

/// Plano completo de projeções.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub writes: Vec<WritePlan>,
}

impl Plan {
    /// Resumo curto das projeções que mudaram (para a mensagem de sync).
    pub fn summary(&self) -> String {
        let names: Vec<&str> = self
            .writes
            .iter()
            .filter(|w| w.changed)
            .map(|w| w.name.as_str())
            .collect();
        if names.is_empty() {
            "nenhuma alteração".to_string()
        } else {
            names.join(", ")
        }
    }

    /// Divergências para `verificar`: projeções dessincronizadas em disco.
    pub fn drift(&self) -> Vec<String> {
        self.writes
            .iter()
            .filter(|w| w.changed)
            .map(|w| {
                format!(
                    "projeção '{}' dessincronizada em {}; rode `pink doc sincronizar`",
                    w.name, w.file
                )
            })
            .collect()
    }
}

// @pinker-nav:start trama.projecoes.geracao
// @pinker-nav:domain projecoes
// @pinker-nav:layer trama
// @pinker-nav:summary Projeta os manifestos versionados em regiões geradas explícitas (`@pinker-generated`) para `history`, `state` e `roadmap`: valida que toda flag `updates.*` verdadeira tem consumidor, gera conteúdo determinístico e preserva o texto humano fora da região.
/// Planeja todas as projeções configuradas e valida as flags dos manifestos.
pub fn plan(
    repo_root: &Path,
    config: &DocConfig,
    manifests: &Manifests,
) -> Result<Plan, ProjectionError> {
    // (1) Toda flag `updates.<flag>` verdadeira precisa de consumidor.
    let mut true_flags: Vec<String> = Vec::new();
    for change in &manifests.changes {
        for (flag, value) in &change.updates {
            if *value && !true_flags.contains(flag) {
                true_flags.push(flag.clone());
            }
        }
    }
    for flag in &true_flags {
        if !config.projections.iter().any(|p| &p.name == flag) {
            return Err(ProjectionError::FlagWithoutConsumer { flag: flag.clone() });
        }
    }

    // (2) Gera cada projeção configurada dentro de sua região.
    let mut plan = Plan::default();
    for projection in &config.projections {
        let generated = generate(&projection.name, manifests)?;
        let path = repo_root.join(&projection.file);
        let current =
            std::fs::read_to_string(&path).map_err(|err| ProjectionError::TargetUnreadable {
                name: projection.name.clone(),
                file: projection.file.clone(),
                msg: err.to_string(),
            })?;
        let content = splice_region(projection, &current, &generated)?;
        let changed = content != current;
        plan.writes.push(WritePlan {
            name: projection.name.clone(),
            path,
            file: projection.file.clone(),
            content,
            changed,
        });
    }
    Ok(plan)
}

/// Substitui o conteúdo entre os marcadores gerados, preservando tudo fora.
fn splice_region(
    projection: &DocProjection,
    current: &str,
    generated: &str,
) -> Result<String, ProjectionError> {
    let lines: Vec<&str> = current.lines().collect();
    let start_needle = format!("{} {}", GEN_START, projection.region);
    let end_needle = format!("{} {}", GEN_END, projection.region);
    let mut start_idx = None;
    let mut end_idx = None;
    for (i, line) in lines.iter().enumerate() {
        if line.contains(&start_needle) {
            start_idx = Some(i);
        } else if line.contains(&end_needle) {
            end_idx = Some(i);
        }
    }
    let (Some(start), end) = (start_idx, end_idx) else {
        return Err(ProjectionError::RegionMissing {
            name: projection.name.clone(),
            file: projection.file.clone(),
            region: projection.region.clone(),
        });
    };
    let Some(end) = end else {
        return Err(ProjectionError::RegionUnbalanced {
            name: projection.name.clone(),
            file: projection.file.clone(),
            region: projection.region.clone(),
        });
    };
    if end <= start {
        return Err(ProjectionError::RegionUnbalanced {
            name: projection.name.clone(),
            file: projection.file.clone(),
            region: projection.region.clone(),
        });
    }

    let mut out = String::new();
    // Cabeçalho humano + marcador de abertura (preservados).
    for line in &lines[..=start] {
        out.push_str(line);
        out.push('\n');
    }
    // Conteúdo gerado (propriedade da ferramenta).
    if !generated.is_empty() {
        out.push_str(generated);
        if !generated.ends_with('\n') {
            out.push('\n');
        }
    }
    // Marcador de fechamento + rodapé humano (preservados).
    for line in &lines[end..] {
        out.push_str(line);
        out.push('\n');
    }
    // Preserva a ausência de newline final se o original não terminava em '\n'.
    if !current.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

/// Gera o conteúdo de uma projeção nomeada a partir dos manifestos.
fn generate(name: &str, manifests: &Manifests) -> Result<String, ProjectionError> {
    match name {
        "history" => Ok(generate_history(manifests)),
        "state" => Ok(generate_state(manifests)),
        "roadmap" => Ok(generate_roadmap(manifests)),
        other => Err(ProjectionError::UnknownGenerator {
            name: other.to_string(),
        }),
    }
}

fn sorted_changes(manifests: &Manifests) -> Vec<&Change> {
    let mut changes: Vec<&Change> = manifests.changes.iter().collect();
    changes.sort_by_key(|c| c.source.as_ref().map(|s| s.number).unwrap_or(0));
    changes
}

fn cell(value: &str) -> String {
    if value.is_empty() {
        "—".to_string()
    } else {
        value.replace('|', "\\|")
    }
}

fn opt_num(value: Option<u64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string())
}

/// Visão humana cronológica do ledger (projeção `history`).
fn generate_history(manifests: &Manifests) -> String {
    let changes = sorted_changes(manifests);
    if changes.is_empty() {
        return "_Nenhuma mudança registrada após o marco #330._".to_string();
    }
    let mut out = String::new();
    out.push_str("| PR | Tipo | Fase | Bloco | Título | Status |\n");
    out.push_str("|---|---|---|---|---|---|\n");
    for change in changes {
        let pr = change.source.as_ref().map(|s| s.number).unwrap_or(0);
        out.push_str(&format!(
            "| #{} | {} | {} | {} | {} | {} |\n",
            pr,
            cell(&change.kind),
            opt_num(change.phase),
            opt_num(change.block),
            cell(&change.title),
            cell(&change.status),
        ));
    }
    out.pop(); // remove o '\n' final; splice_region recoloca
    out
}

/// Estado corrente derivado dos manifestos (projeção `state`).
fn generate_state(manifests: &Manifests) -> String {
    let changes = sorted_changes(manifests);
    if changes.is_empty() {
        return "_Sem mudanças registradas após o marco #330._".to_string();
    }
    let last = changes.last().unwrap();
    let last_pr = last.source.as_ref().map(|s| s.number).unwrap_or(0);
    let mut implemented: Vec<String> = Vec::new();
    for change in &changes {
        for id in &change.implemented {
            if !implemented.contains(id) {
                implemented.push(id.clone());
            }
        }
    }
    implemented.sort();
    let mut out = String::new();
    out.push_str(&format!("- Manifestos processados: {}\n", changes.len()));
    out.push_str(&format!(
        "- Última mudança: PR #{} — {} (fase {}, bloco {})\n",
        last_pr,
        cell(&last.title),
        opt_num(last.phase),
        opt_num(last.block),
    ));
    if implemented.is_empty() {
        out.push_str("- Seções implementadas: —");
    } else {
        out.push_str(&format!(
            "- Seções implementadas: {}",
            implemented.join(", ")
        ));
    }
    out
}

/// Tabela mecânica de entregas por status (projeção `roadmap`).
fn generate_roadmap(manifests: &Manifests) -> String {
    let changes = sorted_changes(manifests);
    if changes.is_empty() {
        return "_Sem entregas registradas após o marco #330._".to_string();
    }
    let mut out = String::new();
    out.push_str("| PR | Fase | Bloco | Título | Status |\n");
    out.push_str("|---|---|---|---|---|\n");
    for change in changes {
        let pr = change.source.as_ref().map(|s| s.number).unwrap_or(0);
        out.push_str(&format!(
            "| #{} | {} | {} | {} | {} |\n",
            pr,
            opt_num(change.phase),
            opt_num(change.block),
            cell(&change.title),
            cell(&change.status),
        ));
    }
    out.pop();
    out
}
// @pinker-nav:end trama.projecoes.geracao

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::Source;

    fn manifest(pr: u64, title: &str) -> Change {
        Change {
            schema: 1,
            source: Some(Source {
                kind: "github-pr".to_string(),
                number: pr,
                repository: None,
            }),
            kind: "phase".to_string(),
            phase: Some(241),
            block: Some(20),
            title: title.to_string(),
            status: "completed".to_string(),
            updates: vec![("history".to_string(), true)],
            implemented: vec!["result.predeclared".to_string()],
            ..Default::default()
        }
    }

    fn proj(name: &str, region: &str) -> DocProjection {
        DocProjection {
            name: name.to_string(),
            file: format!("docs/{}.md", name),
            region: region.to_string(),
        }
    }

    #[test]
    fn splice_preserves_human_text() {
        let projection = proj("history", "change.history");
        let current = "# Humano\n\n<!-- @pinker-generated:start change.history -->\nvelho\n<!-- @pinker-generated:end change.history -->\n\nrodapé humano\n";
        let out = splice_region(&projection, current, "novo").unwrap();
        assert!(out.contains("# Humano"));
        assert!(out.contains("rodapé humano"));
        assert!(out.contains("novo"));
        assert!(!out.contains("velho"));
    }

    #[test]
    fn splice_is_idempotent() {
        let projection = proj("history", "change.history");
        let current = "<!-- @pinker-generated:start change.history -->\nx\n<!-- @pinker-generated:end change.history -->\n";
        let once = splice_region(&projection, current, "gerado").unwrap();
        let twice = splice_region(&projection, &once, "gerado").unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn missing_region_is_error() {
        let projection = proj("history", "change.history");
        let current = "# sem região\n";
        assert!(matches!(
            splice_region(&projection, current, "x"),
            Err(ProjectionError::RegionMissing { .. })
        ));
    }

    #[test]
    fn history_table_is_deterministic() {
        let mut manifests = Manifests::default();
        manifests.changes.push(manifest(341, "Resultado"));
        manifests.changes.push(manifest(340, "Anterior"));
        let a = generate_history(&manifests);
        let b = generate_history(&manifests);
        assert_eq!(a, b);
        // Ordenado por PR.
        let idx340 = a.find("#340").unwrap();
        let idx341 = a.find("#341").unwrap();
        assert!(idx340 < idx341);
    }
}
