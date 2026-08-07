//! Comparação de bytes, classificação da mudança e check somente leitura.
//!
//! O estado observado entra como **dado**. Este estágio não lê o disco: quem
//! observa é o chamador, e no estágio seguinte será o adaptador com acesso ao
//! filesystem. A consequência é que o check aqui é trivialmente sem escrita —
//! não há caminho de escrita a evitar, porque não há filesystem.

use super::path::RelativePath;
use super::plan::Plan;
use super::{Decision, Failure, HarnessCause, Outcome, PolicyCause};

// @pinker-nav:start automation.comparacao.classificacao
// @pinker-nav:domain comparacao
// @pinker-nav:layer automation
// @pinker-nav:summary Observação do estado corrente como dado de entrada, classificação por comparação de bytes em create/replace/remove/no-change e check somente leitura que exige observação para cada target, rejeita observação órfã ou duplicada e produz apenas MATCH ou DRIFT — falha de harness nunca é reclassificada como drift.

/// Como um target difere do estado desejado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Desejado presente, observado ausente.
    Create,
    /// Ambos presentes, bytes diferentes.
    Replace,
    /// Desejado ausente, observado presente.
    Remove,
    /// Já coincidem — inclusive quando ambos estão ausentes.
    NoChange,
}

impl ChangeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeKind::Create => "CREATE",
            ChangeKind::Replace => "REPLACE",
            ChangeKind::Remove => "REMOVE",
            ChangeKind::NoChange => "NO_CHANGE",
        }
    }

    /// Verdadeiro quando o target exige alguma escrita para convergir.
    pub fn is_divergent(&self) -> bool {
        !matches!(self, ChangeKind::NoChange)
    }
}

/// Classifica um target por comparação de bytes.
pub fn classify(desired: Option<&[u8]>, observed: Option<&[u8]>) -> ChangeKind {
    match (desired, observed) {
        (Some(_), None) => ChangeKind::Create,
        (Some(d), Some(o)) if d != o => ChangeKind::Replace,
        (Some(_), Some(_)) => ChangeKind::NoChange,
        (None, Some(_)) => ChangeKind::Remove,
        (None, None) => ChangeKind::NoChange,
    }
}

/// O que se observou em um path. `bytes` ausente significa arquivo inexistente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    path: RelativePath,
    bytes: Option<Vec<u8>>,
}

impl Observation {
    /// Observa conteúdo presente.
    pub fn present(path: &str, bytes: Vec<u8>) -> Result<Observation, PolicyCause> {
        Ok(Observation {
            path: RelativePath::new(path)?,
            bytes: Some(bytes),
        })
    }

    /// Observa ausência.
    pub fn absent(path: &str) -> Result<Observation, PolicyCause> {
        Ok(Observation {
            path: RelativePath::new(path)?,
            bytes: None,
        })
    }

    pub fn path(&self) -> &RelativePath {
        &self.path
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

/// Conjunto de observações, sem duplicatas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservedState {
    entries: Vec<Observation>,
}

impl ObservedState {
    pub fn new() -> ObservedState {
        ObservedState::default()
    }

    /// Acrescenta uma observação, rejeitando path repetido.
    pub fn with(mut self, observation: Observation) -> Result<ObservedState, Failure> {
        if self.entries.iter().any(|e| e.path == observation.path) {
            return Err(Failure::HarnessFailure(
                HarnessCause::DuplicateObservation {
                    path: observation.path.as_str().to_string(),
                },
            ));
        }
        self.entries.push(observation);
        Ok(self)
    }

    pub fn get(&self, path: &RelativePath) -> Option<&Observation> {
        self.entries.iter().find(|e| &e.path == path)
    }

    pub fn entries(&self) -> &[Observation] {
        &self.entries
    }
}

/// Resultado por target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOutcome {
    pub path: String,
    pub change: ChangeKind,
    pub desired_bytes: Option<usize>,
    pub desired_digest: Option<String>,
    pub observed_bytes: Option<usize>,
    pub observed_digest: Option<String>,
}

/// Relatório de um check somente leitura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub schema: u64,
    pub producer: String,
    pub plan_digest: String,
    pub outcome: Outcome,
    pub targets: Vec<TargetOutcome>,
    pub decision: Option<Decision>,
}

impl CheckReport {
    /// Quantidade de targets por classificação, em ordem fixa.
    pub fn summary(&self) -> [(&'static str, usize); 4] {
        let count = |kind: ChangeKind| self.targets.iter().filter(|t| t.change == kind).count();
        [
            ("create", count(ChangeKind::Create)),
            ("replace", count(ChangeKind::Replace)),
            ("remove", count(ChangeKind::Remove)),
            ("no_change", count(ChangeKind::NoChange)),
        ]
    }
}

/// Compara o plano com o estado observado. Somente leitura e puro.
///
/// Exige observação para **cada** target: sem ela não há classificação
/// possível, e inventar ausência transformaria um erro de harness em `CREATE`.
/// Observação sem target correspondente também falha, porque indica que o
/// chamador observou algo que o plano não declara.
pub fn check(plan: &Plan, observed: &ObservedState) -> Result<CheckReport, Failure> {
    for observation in observed.entries() {
        if plan.target(observation.path()).is_none() {
            return Err(Failure::HarnessFailure(
                HarnessCause::ObservationWithoutTarget {
                    path: observation.path().as_str().to_string(),
                },
            ));
        }
    }

    let mut targets = Vec::with_capacity(plan.targets().len());
    for target in plan.targets() {
        let Some(observation) = observed.get(target.path()) else {
            return Err(Failure::HarnessFailure(HarnessCause::MissingObservation {
                path: target.path().as_str().to_string(),
            }));
        };
        let desired = target.desired_bytes();
        let observed_bytes = observation.bytes();
        targets.push(TargetOutcome {
            path: target.path().as_str().to_string(),
            change: classify(desired, observed_bytes),
            desired_bytes: desired.map(<[u8]>::len),
            desired_digest: desired.map(crate::agent::sha256_hex),
            observed_bytes: observed_bytes.map(<[u8]>::len),
            observed_digest: observed_bytes.map(crate::agent::sha256_hex),
        });
    }

    let outcome = if targets.iter().any(|t| t.change.is_divergent()) {
        Outcome::Drift
    } else {
        Outcome::Match
    };

    Ok(CheckReport {
        schema: plan.schema(),
        producer: plan.producer().to_string(),
        plan_digest: plan.digest(),
        outcome,
        targets,
        decision: None,
    })
}
// @pinker-nav:end automation.comparacao.classificacao

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classificacao_cobre_as_quatro_formas() {
        assert_eq!(classify(Some(b"a"), None), ChangeKind::Create);
        assert_eq!(classify(Some(b"a"), Some(b"b")), ChangeKind::Replace);
        assert_eq!(classify(Some(b"a"), Some(b"a")), ChangeKind::NoChange);
        assert_eq!(classify(None, Some(b"a")), ChangeKind::Remove);
        assert_eq!(classify(None, None), ChangeKind::NoChange);
    }
}
