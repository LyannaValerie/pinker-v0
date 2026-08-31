//! Confinamento no filesystem, observação sem escrita e aplicação local
//! explícita e atômica por arquivo.
//!
//! # O que este módulo promete
//!
//! Atomicidade **por arquivo**: temporário irmão criado com `create_new`,
//! escrita completa, sync quando suportado, revalidação imediatamente antes da
//! substituição, `rename` no mesmo diretório e releitura verificando tamanho e
//! digest.
//!
//! # O que este módulo não promete
//!
//! Não há atomicidade multi-arquivo e não há rollback global. Se A e B forem
//! aplicados e C falhar, o relatório preserva `applied: [A, B]`, `failed: C`,
//! os não tentados e `rollback_performed: false`. A recuperação é observar de
//! novo, executar novo check, produzir novo plano e autorizar novo digest.
//!
//! Também não se promete proteção absoluta contra TOCTOU em filesystem hostil
//! concorrente. O confinamento é lexical mais `symlink_metadata` em cada
//! componente, revalidado imediatamente antes da substituição; ele não é
//! substituto de `openat2` com `RESOLVE_BENEATH`.
//!
//! A implementação operacional host-side é dona de confinamento por descritor.
//! Este módulo conserva somente a política Pinker independente e seus limites.

use super::compare::{check, ChangeKind, CheckReport, Observation, ObservedState};
use super::plan::Plan;
use super::root::RepoRoot;
use super::{
    Authorization, Decision, Failure, FinalDrift, Outcome, PolicyCause, RelativePath,
    MAX_TARGET_BYTES, RECOVERY_PROCEDURE,
};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// Quantas vezes se tenta um nome de temporário antes de desistir.
pub const MAX_TEMP_ATTEMPTS: u32 = 64;

// @pinker-nav:start automation.filesystem.confinamento
// @pinker-nav:domain filesystem
// @pinker-nav:layer automation
// @pinker-nav:summary Confinamento de um path repo-relativo no filesystem: cada ancestral existente e o próprio alvo são inspecionados com symlink_metadata, rejeitando link simbólico em qualquer posição, ancestral que não seja diretório, alvo que não seja arquivo regular e qualquer resultado fora da raiz canônica — política conservadora que se revalida antes de substituir e não promete imunidade a TOCTOU.

/// Resolve um path repo-relativo dentro da raiz, aplicando o confinamento.
///
/// Rejeita link simbólico no alvo e em qualquer ancestral, ancestral que exista
/// e não seja diretório, alvo que exista e não seja arquivo regular, e qualquer
/// caminho que escape da raiz.
pub fn confine(root: &RepoRoot, relative: &RelativePath) -> Result<PathBuf, Failure> {
    let absolute = root.join_relative(relative.as_str());
    if !root.contains(&absolute) {
        return Err(Failure::PolicyViolation(PolicyCause::EscapesRoot {
            path: relative.as_str().to_string(),
        }));
    }

    // Ancestrais: da raiz até o pai do alvo, exclusive o alvo.
    let mut current = root.path().to_path_buf();
    let components: Vec<&str> = relative.as_str().split('/').collect();
    for component in &components[..components.len() - 1] {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Failure::PolicyViolation(PolicyCause::SymlinkAncestor {
                    path: relative.as_str().to_string(),
                    component: (*component).to_string(),
                }))
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(Failure::PolicyViolation(
                    PolicyCause::AncestorNotDirectory {
                        path: relative.as_str().to_string(),
                        component: (*component).to_string(),
                    },
                ))
            }
            Ok(_) => {}
            // Somente `NotFound` significa ausência. A escrita falha depois com
            // causa própria, porque este estágio não cria diretórios.
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            // Qualquer outro erro é operacional e não pode ser lido como
            // ausência: um ancestral sem permissão de travessia não é um
            // ancestral inexistente.
            Err(err) => {
                return Err(Failure::IoFailure {
                    path: relative.as_str().to_string(),
                    msg: format!("ancestral '{}' inacessível: {}", component, err),
                })
            }
        }
    }

    match fs::symlink_metadata(&absolute) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(Failure::PolicyViolation(PolicyCause::SymlinkTarget {
                path: relative.as_str().to_string(),
            }))
        }
        Ok(metadata) if !metadata.is_file() => Err(Failure::PolicyViolation(
            PolicyCause::TargetNotRegularFile {
                path: relative.as_str().to_string(),
            },
        )),
        Ok(_) => Ok(absolute),
        // Ausência é exatamente `NotFound`, e nada mais.
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(absolute),
        Err(err) => Err(Failure::IoFailure {
            path: relative.as_str().to_string(),
            msg: format!("target inacessível: {}", err),
        }),
    }
}
// @pinker-nav:end automation.filesystem.confinamento

// @pinker-nav:start automation.filesystem.observacao
// @pinker-nav:domain filesystem
// @pinker-nav:layer automation
// @pinker-nav:summary Observação estritamente sem escrita do estado corrente de cada target do plano, atravessando o confinamento, tratando ausência como observação válida e aplicando o limite de bytes por target na leitura; é o único caminho pelo qual o disco entra no núcleo.

/// Observa um único target. Estritamente somente leitura.
pub fn observe_target(root: &RepoRoot, relative: &RelativePath) -> Result<Observation, Failure> {
    let absolute = confine(root, relative)?;
    match fs::symlink_metadata(&absolute) {
        // Só `NotFound` é ausência observada. Qualquer outro erro é falha
        // operacional: tratá-lo como ausência inventaria um `CREATE` e faria
        // uma falha de I/O virar drift.
        Err(err) if err.kind() == ErrorKind::NotFound => {
            Observation::absent(relative.as_str()).map_err(Failure::PolicyViolation)
        }
        Err(err) => Err(Failure::IoFailure {
            path: relative.as_str().to_string(),
            msg: format!("observação falhou: {}", err),
        }),
        Ok(metadata) => {
            let len = metadata.len();
            if len > MAX_TARGET_BYTES as u64 {
                return Err(Failure::PolicyViolation(PolicyCause::TargetLimitExceeded {
                    path: relative.as_str().to_string(),
                    bytes: len as usize,
                    limit: MAX_TARGET_BYTES,
                }));
            }
            let bytes = fs::read(&absolute).map_err(|err| Failure::IoFailure {
                path: relative.as_str().to_string(),
                msg: err.to_string(),
            })?;
            Observation::present(relative.as_str(), bytes).map_err(Failure::PolicyViolation)
        }
    }
}

/// Observa todos os targets de um plano. Estritamente somente leitura.
pub fn observe(root: &RepoRoot, plan: &Plan) -> Result<ObservedState, Failure> {
    let mut state = ObservedState::new();
    for target in plan.targets() {
        state = state.with(observe_target(root, target.path())?)?;
    }
    Ok(state)
}
// @pinker-nav:end automation.filesystem.observacao

// @pinker-nav:start automation.filesystem.aplicacao
// @pinker-nav:domain filesystem
// @pinker-nav:layer automation
// @pinker-nav:summary Aplicação local explícita: exige autorização por digest exato do plano, revalida as precondições observadas antes de escrever, detecta plano obsoleto, e por arquivo cria temporário irmão exclusivo com create_new, escreve, sincroniza quando suportado, revalida o confinamento, substitui por rename e verifica tamanho e digest relendo — preservando progresso parcial explícito, item falho, itens não tentados e rollback_performed sempre falso.

/// Relatório de uma aplicação. Progresso parcial é explícito por construção.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReport {
    pub schema: u64,
    pub producer: String,
    pub plan_digest: String,
    /// `Applied` ou `NoChange` quando a aplicação concluiu; ausente quando parou.
    pub outcome: Option<Outcome>,
    /// A causa. Nunca é substituída pelo estado decisório.
    pub failure: Option<Failure>,
    /// Estado decisório, ao lado da causa.
    pub decision: Option<Decision>,
    /// Targets efetivamente aplicados e verificados, em ordem de aplicação.
    pub applied: Vec<String>,
    /// O target em que a aplicação parou, se parou.
    pub failed: Option<String>,
    /// Targets que nem chegaram a ser tentados.
    pub not_attempted: Vec<String>,
    /// Sempre falso: este módulo não desfaz nada.
    pub rollback_performed: bool,
    /// Drift depois da aplicação: medido, ou desconhecido com a razão.
    pub final_drift: FinalDrift,
    /// Procedimento de recuperação, constante e explícito.
    pub recovery: &'static str,
}

impl ApplyReport {
    fn stopped(plan: &Plan, failure: Failure, pending: Vec<String>) -> ApplyReport {
        ApplyReport {
            schema: plan.schema(),
            producer: plan.producer().to_string(),
            plan_digest: plan.digest(),
            outcome: None,
            failure: Some(failure),
            decision: Some(Decision::NeedsHumanDecision),
            applied: Vec::new(),
            failed: None,
            not_attempted: pending,
            rollback_performed: false,
            final_drift: FinalDrift::Unknown("aplicação não foi iniciada".to_string()),
            recovery: RECOVERY_PROCEDURE,
        }
    }
}

/// Aplica um plano autorizado. Único ponto de escrita do núcleo.
///
/// Exige a autorização pelo digest exato, revalida as precondições observadas em
/// `precondition` e só então escreve, arquivo por arquivo, em ordem canônica.
///
/// O recálculo do estado desejado é do adaptador: o núcleo não sabe derivar o
/// plano, então valida o que recebe — digest autorizado e precondições ainda
/// verdadeiras — em vez de fingir que recalculou.
pub fn apply(
    root: &RepoRoot,
    plan: &Plan,
    authorization: &Authorization,
    precondition: &CheckReport,
) -> ApplyReport {
    let todos: Vec<String> = plan
        .targets()
        .iter()
        .map(|t| t.path().as_str().to_string())
        .collect();

    if authorization.digest() != plan.digest() {
        return ApplyReport::stopped(
            plan,
            Failure::PolicyViolation(PolicyCause::AuthorizationMismatch {
                expected: plan.digest(),
                provided: authorization.digest().to_string(),
            }),
            todos,
        );
    }
    if precondition.plan_digest != plan.digest() {
        return ApplyReport::stopped(
            plan,
            Failure::StalePlan {
                plan_digest: plan.digest(),
                msg: "a precondição pertence a outro plano".to_string(),
            },
            todos,
        );
    }

    let current = match observe(root, plan) {
        Ok(state) => state,
        Err(failure) => return ApplyReport::stopped(plan, failure, todos),
    };
    let current_report = match check(plan, &current) {
        Ok(report) => report,
        Err(failure) => return ApplyReport::stopped(plan, failure, todos),
    };
    if let Some(divergente) = precondicao_divergente(precondition, &current_report) {
        return ApplyReport::stopped(
            plan,
            Failure::StalePlan {
                plan_digest: plan.digest(),
                msg: format!("o estado observado de '{}' mudou desde o check", divergente),
            },
            todos,
        );
    }

    let pendentes: Vec<&super::compare::TargetOutcome> = current_report
        .targets
        .iter()
        .filter(|t| t.change.is_divergent())
        .collect();

    if pendentes.is_empty() {
        return ApplyReport {
            schema: plan.schema(),
            producer: plan.producer().to_string(),
            plan_digest: plan.digest(),
            outcome: Some(Outcome::NoChange),
            failure: None,
            decision: None,
            applied: Vec::new(),
            failed: None,
            not_attempted: Vec::new(),
            rollback_performed: false,
            final_drift: FinalDrift::Measured(Outcome::Match),
            recovery: RECOVERY_PROCEDURE,
        };
    }

    let mut applied: Vec<String> = Vec::new();
    let mut failed: Option<String> = None;
    let mut falha: Option<Failure> = None;
    let mut not_attempted: Vec<String> = Vec::new();

    for (indice, alvo) in pendentes.iter().enumerate() {
        let relative = match RelativePath::new(&alvo.path) {
            Ok(path) => path,
            Err(cause) => {
                falha = Some(Failure::PolicyViolation(cause));
                failed = Some(alvo.path.clone());
                not_attempted = pendentes[indice + 1..]
                    .iter()
                    .map(|t| t.path.clone())
                    .collect();
                break;
            }
        };
        let desired = plan
            .target(&relative)
            .and_then(|t| t.desired_bytes())
            .map(<[u8]>::to_vec);
        let resultado = match alvo.change {
            ChangeKind::Remove => remove_one(root, &relative),
            _ => write_one(root, &relative, desired.as_deref().unwrap_or(&[])),
        };
        match resultado {
            Ok(()) => applied.push(alvo.path.clone()),
            Err(erro) => {
                falha = Some(erro);
                failed = Some(alvo.path.clone());
                not_attempted = pendentes[indice + 1..]
                    .iter()
                    .map(|t| t.path.clone())
                    .collect();
                break;
            }
        }
    }

    let final_drift = measure_final_drift(root, plan);

    let houve_falha = falha.is_some();
    ApplyReport {
        schema: plan.schema(),
        producer: plan.producer().to_string(),
        plan_digest: plan.digest(),
        outcome: if houve_falha {
            None
        } else {
            Some(Outcome::Applied)
        },
        failure: falha,
        decision: if houve_falha {
            Some(Decision::NeedsHumanDecision)
        } else {
            None
        },
        applied,
        failed,
        not_attempted,
        rollback_performed: false,
        final_drift,
        recovery: RECOVERY_PROCEDURE,
    }
}

/// Mede o drift depois da aplicação.
///
/// Se a observação final falhar, o resultado é `Unknown` **com a razão**: um
/// relatório que não conseguiu medir precisa dizer isso, e nunca apresentar
/// `Measured` a partir de uma observação que não aconteceu.
pub fn measure_final_drift(root: &RepoRoot, plan: &Plan) -> FinalDrift {
    match observe(root, plan).and_then(|estado| check(plan, &estado)) {
        Ok(report) => FinalDrift::Measured(report.outcome),
        Err(erro) => FinalDrift::Unknown(erro.to_string()),
    }
}

/// Qual target teve a observação alterada entre o check e a aplicação.
fn precondicao_divergente(antes: &CheckReport, agora: &CheckReport) -> Option<String> {
    for anterior in &antes.targets {
        let Some(atual) = agora.targets.iter().find(|t| t.path == anterior.path) else {
            return Some(anterior.path.clone());
        };
        if atual.observed_bytes != anterior.observed_bytes
            || atual.observed_digest != anterior.observed_digest
        {
            return Some(anterior.path.clone());
        }
    }
    None
}

/// Escreve um target de forma atômica por arquivo.
fn write_one(root: &RepoRoot, relative: &RelativePath, bytes: &[u8]) -> Result<(), Failure> {
    let absolute = confine(root, relative)?;
    let parent = absolute.parent().ok_or_else(|| Failure::IoFailure {
        path: relative.as_str().to_string(),
        msg: "target sem diretório pai".to_string(),
    })?;
    if !parent.is_dir() {
        return Err(Failure::IoFailure {
            path: relative.as_str().to_string(),
            msg: "diretório pai ausente; este núcleo não cria diretórios".to_string(),
        });
    }

    let (temporary, mut file) = create_temporary(relative, &absolute, parent)?;

    let escrita = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|err| Failure::IoFailure {
            path: relative.as_str().to_string(),
            msg: err.to_string(),
        });
    // Sync quando suportado: durabilidade é melhor-esforço, e a garantia real
    // é a releitura depois da substituição.
    let _ = file.sync_all();
    drop(file);
    if let Err(erro) = escrita {
        let _ = fs::remove_file(&temporary);
        return Err(erro);
    }

    // Revalidação imediatamente antes de substituir: o alvo não pode ter virado
    // link simbólico enquanto o temporário era escrito.
    if let Err(erro) = confine(root, relative) {
        let _ = fs::remove_file(&temporary);
        return Err(erro);
    }

    if let Err(err) = fs::rename(&temporary, &absolute) {
        let _ = fs::remove_file(&temporary);
        return Err(Failure::IoFailure {
            path: relative.as_str().to_string(),
            msg: err.to_string(),
        });
    }
    // Sync do diretório, quando suportado.
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }

    verify_written(root, relative, Some(bytes))
}

/// Cria o temporário irmão, exclusivo por `create_new`.
fn create_temporary(
    relative: &RelativePath,
    absolute: &Path,
    parent: &Path,
) -> Result<(PathBuf, fs::File), Failure> {
    let nome = absolute
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Failure::IoFailure {
            path: relative.as_str().to_string(),
            msg: "target sem nome de arquivo".to_string(),
        })?;
    let mut ultimo = String::new();
    for tentativa in 0..MAX_TEMP_ATTEMPTS {
        let candidato = parent.join(format!(".{nome}.pinker-tmp-{tentativa}"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidato)
        {
            Ok(file) => return Ok((candidato, file)),
            // Só colisão justifica tentar o próximo nome.
            Err(err) if err.kind() == ErrorKind::AlreadyExists => ultimo = err.to_string(),
            // Qualquer outro erro já vale para todos os nomes: insistir 64
            // vezes só esconderia a causa real atrás de uma mensagem de
            // exaustão.
            Err(err) => {
                return Err(Failure::IoFailure {
                    path: relative.as_str().to_string(),
                    msg: format!("temporário não pôde ser criado: {}", err),
                })
            }
        }
    }
    Err(Failure::IoFailure {
        path: relative.as_str().to_string(),
        msg: format!(
            "nenhum temporário exclusivo disponível em {} tentativas: {}",
            MAX_TEMP_ATTEMPTS, ultimo
        ),
    })
}

/// Remove um target, revalidando o confinamento antes.
fn remove_one(root: &RepoRoot, relative: &RelativePath) -> Result<(), Failure> {
    let absolute = confine(root, relative)?;
    fs::remove_file(&absolute).map_err(|err| Failure::IoFailure {
        path: relative.as_str().to_string(),
        msg: err.to_string(),
    })?;
    if let Some(Ok(dir)) = absolute.parent().map(fs::File::open) {
        let _ = dir.sync_all();
    }
    verify_written(root, relative, None)
}

/// Relê o target e verifica tamanho e digest contra o esperado.
///
/// `expected` ausente significa que o target deve ter deixado de existir.
/// É a verificação posterior à substituição, e é ela — não o `sync` — que
/// sustenta a garantia por arquivo.
pub fn verify_written(
    root: &RepoRoot,
    relative: &RelativePath,
    expected: Option<&[u8]>,
) -> Result<(), Failure> {
    let observado =
        observe_target(root, relative).map_err(|erro| Failure::VerifyAfterApplyFailure {
            path: relative.as_str().to_string(),
            msg: format!("releitura falhou: {erro}"),
        })?;
    match (expected, observado.bytes()) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(Failure::VerifyAfterApplyFailure {
            path: relative.as_str().to_string(),
            msg: "o target deveria ter sido removido e ainda existe".to_string(),
        }),
        (Some(_), None) => Err(Failure::VerifyAfterApplyFailure {
            path: relative.as_str().to_string(),
            msg: "o target deveria existir e está ausente".to_string(),
        }),
        (Some(esperado), Some(lido)) => {
            if esperado.len() != lido.len() {
                return Err(Failure::VerifyAfterApplyFailure {
                    path: relative.as_str().to_string(),
                    msg: format!(
                        "tamanho divergente: esperado {}, lido {}",
                        esperado.len(),
                        lido.len()
                    ),
                });
            }
            let esperado_digest = pinker_sha256_contract::sha256_hex(esperado);
            let lido_digest = pinker_sha256_contract::sha256_hex(lido);
            if esperado_digest != lido_digest {
                return Err(Failure::VerifyAfterApplyFailure {
                    path: relative.as_str().to_string(),
                    msg: format!(
                        "digest divergente: esperado {}, lido {}",
                        esperado_digest, lido_digest
                    ),
                });
            }
            Ok(())
        }
    }
}
// @pinker-nav:end automation.filesystem.aplicacao

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::HarnessCause;

    #[test]
    fn harness_cause_de_raiz_e_formatavel() {
        let falha = Failure::HarnessFailure(HarnessCause::RootNotFound {
            start: "/x".to_string(),
            msg: "sem marcador".to_string(),
        });
        assert_eq!(falha.code(), "HARNESS_FAILURE");
        assert!(falha.to_string().contains("/x"));
    }
}
