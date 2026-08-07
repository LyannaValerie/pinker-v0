//! Núcleo comum determinístico e observacional das automações internas (#385).
//!
//! Este módulo é o **automation core** da separação aprovada na Issue #385:
//!
//! ```text
//! pink agente          → orquestração, processos, Git, rede e publicação
//! adaptador de domínio → invariantes e cálculo do estado desejado
//! automation core      → plano, comparação, check local e relatórios
//! ```
//!
//! O core não executa processos, não acessa a rede, não executa Git, não publica
//! e não conhece estados internos do runner nem do `pink agente`.
//!
//! # Fronteiras deste recorte (Estágio B da campanha dos itens 2 e 3 da #417)
//!
//! Este estágio é **puro, library-first e somente leitura**. Ele não toca o
//! filesystem: não descobre a raiz do repositório, não cria temporários, não
//! renomeia, não aplica plano algum. O estado observado entra como **dado**,
//! fornecido pelo chamador; quem o obtém do disco é o estágio seguinte.
//!
//! Também não há CLI, consumidor real, mudança em `src/agent.rs` nem qualquer
//! alteração do contrato congelado `pink-agent-v1`.
//!
//! # O plano é efêmero
//!
//! Um [`Plan`] descreve o estado desejado de um conjunto de arquivos
//! repo-relativos. Ele é calculado, usado e descartado: **não é canônico, não é
//! versionado no repositório e nunca é lido de volta.** Por isso existe
//! serialização canônica e não existe parser — a autorização de uma escrita
//! futura compara o *digest* de um plano recalculado pelo adaptador, jamais um
//! plano desserializado. Não escrever o parser elimina uma superfície inteira de
//! entrada para um formato que nunca é persistido.
//!
//! # Resultados
//!
//! Os resultados de domínio são [`Outcome`] e as falhas operacionais são
//! [`Failure`], separadas de propósito: drift não é erro, e falha de harness
//! nunca vira drift. [`Decision`] é estado decisório e nunca substitui a causa.
//!
//! Neste estágio, o núcleo puro produz operacionalmente apenas [`Outcome::Match`],
//! [`Outcome::Drift`], [`Failure::HarnessFailure`] e [`Failure::PolicyViolation`].
//! Os demais existem no modelo para que o schema seja estável, e não são
//! simulados: não há apply, então não há `APPLIED`, `NO_CHANGE` operacional,
//! `STALE_PLAN`, `IO_FAILURE` nem falha posterior à escrita.

pub mod compare;
pub mod fsio;
pub mod path;
pub mod plan;
pub mod report;
pub mod root;

pub use compare::{check, ChangeKind, CheckReport, Observation, ObservedState, TargetOutcome};
pub use fsio::{apply, confine, observe, observe_target, verify_written, ApplyReport};
pub use path::{Allowlist, RelativePath};
pub use plan::{Payload, Plan, PlanBuilder, PlannedTarget};
pub use report::{
    json_apply_report, json_failure, json_report, markdown_apply_report, markdown_report,
};
pub use root::{RepoRoot, ROOT_MARKER};

use std::fmt;

// @pinker-nav:start automation.contrato.resultados
// @pinker-nav:domain contrato
// @pinker-nav:layer automation
// @pinker-nav:summary Contrato de resultados do núcleo de automação: outcomes de domínio (MATCH, DRIFT, APPLIED, NO_CHANGE), falhas operacionais separadas (HARNESS_FAILURE, POLICY_VIOLATION, STALE_PLAN, IO_FAILURE, VERIFY_AFTER_APPLY_FAILURE) e NEEDS_HUMAN_DECISION como estado decisório que nunca substitui a causa; o estágio puro só produz Match, Drift, HarnessFailure e PolicyViolation.

/// Versão do schema do plano e dos relatórios.
pub const AUTOMATION_SCHEMA: u64 = 1;

/// Limite conservador de bytes **decodificados** por target.
///
/// Constante explícita e coberta por teste, para que qualquer alteração futura
/// seja deliberada.
pub const MAX_TARGET_BYTES: usize = 8 * 1024 * 1024;

/// Limite conservador de bytes **decodificados** somados em um plano.
pub const MAX_PLAN_BYTES: usize = 32 * 1024 * 1024;

/// Comprimento máximo de um path repo-relativo.
pub const MAX_PATH_LEN: usize = 512;

/// Resultado de domínio de uma operação do núcleo.
///
/// `Applied` e `NoChange` descrevem o resultado de uma aplicação. Este estágio
/// não aplica nada e, portanto, nunca os produz — eles existem para que o schema
/// não mude quando o apply chegar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Observação e estado desejado coincidem em todos os targets.
    Match,
    /// Ao menos um target diverge do estado desejado.
    Drift,
    /// Escrita concluída e verificada. Não alcançável neste estágio.
    Applied,
    /// Aplicação executada sem nada a fazer. Não alcançável neste estágio.
    NoChange,
}

impl Outcome {
    /// Nome canônico, estável nos relatórios.
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Match => "MATCH",
            Outcome::Drift => "DRIFT",
            Outcome::Applied => "APPLIED",
            Outcome::NoChange => "NO_CHANGE",
        }
    }

    /// Verdadeiro para os resultados que o núcleo **puro** consegue produzir,
    /// isto é, sem tocar o filesystem.
    ///
    /// Serve de invariante executável: `check` nunca devolve um outcome fora
    /// deste conjunto, mesmo depois de o apply existir.
    pub fn reachable_by_pure_core(&self) -> bool {
        matches!(self, Outcome::Match | Outcome::Drift)
    }

    /// Verdadeiro para os resultados que a aplicação local consegue produzir.
    pub fn reachable_by_apply(&self) -> bool {
        matches!(self, Outcome::Applied | Outcome::NoChange)
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Falha operacional. Nunca é drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Erro estrutural do próprio harness: schema, duplicidade, observação
    /// ausente ou incoerente.
    HarnessFailure(HarnessCause),
    /// Violação de política: path lexicalmente inválido, target fora da
    /// allowlist ou limite de tamanho excedido.
    PolicyViolation(PolicyCause),
    /// Plano autorizado deixou de corresponder ao estado observado.
    StalePlan { plan_digest: String, msg: String },
    /// Falha de entrada e saída.
    IoFailure { path: String, msg: String },
    /// Verificação posterior à escrita reprovou.
    VerifyAfterApplyFailure { path: String, msg: String },
}

impl Failure {
    /// Código estável da falha.
    pub fn code(&self) -> &'static str {
        match self {
            Failure::HarnessFailure(_) => "HARNESS_FAILURE",
            Failure::PolicyViolation(_) => "POLICY_VIOLATION",
            Failure::StalePlan { .. } => "STALE_PLAN",
            Failure::IoFailure { .. } => "IO_FAILURE",
            Failure::VerifyAfterApplyFailure { .. } => "VERIFY_AFTER_APPLY_FAILURE",
        }
    }

    /// Verdadeiro para as falhas que o núcleo **puro** consegue produzir, isto
    /// é, sem tocar o filesystem.
    pub fn reachable_by_pure_core(&self) -> bool {
        matches!(
            self,
            Failure::HarnessFailure(_) | Failure::PolicyViolation(_)
        )
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::HarnessFailure(cause) => write!(f, "{}: {}", self.code(), cause),
            Failure::PolicyViolation(cause) => write!(f, "{}: {}", self.code(), cause),
            Failure::StalePlan { plan_digest, msg } => write!(
                f,
                "{}: plano {} não corresponde mais: {}",
                self.code(),
                plan_digest,
                msg
            ),
            Failure::IoFailure { path, msg } => {
                write!(f, "{}: {}: {}", self.code(), path, msg)
            }
            Failure::VerifyAfterApplyFailure { path, msg } => {
                write!(f, "{}: {}: {}", self.code(), path, msg)
            }
        }
    }
}

/// Causa estrutural de uma falha de harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessCause {
    /// Schema ausente ou diferente de [`AUTOMATION_SCHEMA`].
    SchemaUnknown { found: u64 },
    /// Identificador de produtor vazio: a origem dos dados é obrigatória.
    ProducerMissing,
    /// Dois targets com o mesmo path.
    DuplicateTarget { path: String },
    /// Duas observações para o mesmo path.
    DuplicateObservation { path: String },
    /// Um target do plano não recebeu observação: sem ela não há classificação.
    MissingObservation { path: String },
    /// Uma observação não corresponde a nenhum target do plano.
    ObservationWithoutTarget { path: String },
    /// A raiz canônica do repositório não pôde ser determinada.
    RootNotFound { start: String, msg: String },
}

impl fmt::Display for HarnessCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HarnessCause::SchemaUnknown { found } => write!(
                f,
                "schema {} desconhecido; esta versão aceita somente {}",
                found, AUTOMATION_SCHEMA
            ),
            HarnessCause::ProducerMissing => {
                write!(f, "origem dos dados ausente: 'producer' é obrigatório")
            }
            HarnessCause::DuplicateTarget { path } => {
                write!(f, "target duplicado: {}", path)
            }
            HarnessCause::DuplicateObservation { path } => {
                write!(f, "observação duplicada: {}", path)
            }
            HarnessCause::MissingObservation { path } => write!(
                f,
                "observação ausente para o target {}: sem observação não há classificação",
                path
            ),
            HarnessCause::ObservationWithoutTarget { path } => write!(
                f,
                "observação de {} não corresponde a nenhum target do plano",
                path
            ),
            HarnessCause::RootNotFound { start, msg } => write!(
                f,
                "raiz canônica não determinada a partir de '{}': {}",
                start, msg
            ),
        }
    }
}

/// Causa de uma violação de política.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyCause {
    /// Path vazio.
    PathEmpty,
    /// Path absoluto onde só se aceita repo-relativo.
    PathAbsolute { path: String },
    /// Componente `..`.
    PathTraversal { path: String },
    /// Componente `.` ou componente vazio (`a//b`).
    PathDegenerateComponent { path: String, component: String },
    /// Barra invertida: separador ambíguo entre plataformas.
    PathBackslash { path: String },
    /// Caractere de controle no path.
    PathControlChar { path: String },
    /// Path mais longo que [`MAX_PATH_LEN`].
    PathTooLong { path: String, len: usize },
    /// Target ausente da allowlist lógica.
    TargetNotAllowed { path: String },
    /// Payload de um target maior que [`MAX_TARGET_BYTES`].
    TargetLimitExceeded {
        path: String,
        bytes: usize,
        limit: usize,
    },
    /// Soma dos payloads maior que [`MAX_PLAN_BYTES`].
    PlanLimitExceeded { bytes: usize, limit: usize },
    /// O digest autorizado não é o do plano apresentado.
    AuthorizationMismatch { expected: String, provided: String },
    /// O caminho resolvido cai fora da raiz canônica.
    EscapesRoot { path: String },
    /// O próprio target é um link simbólico.
    SymlinkTarget { path: String },
    /// Um ancestral do target é um link simbólico.
    SymlinkAncestor { path: String, component: String },
    /// Um ancestral existe e não é diretório.
    AncestorNotDirectory { path: String, component: String },
    /// O target existe e não é arquivo regular.
    TargetNotRegularFile { path: String },
}

impl fmt::Display for PolicyCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyCause::PathEmpty => write!(f, "path vazio"),
            PolicyCause::PathAbsolute { path } => {
                write!(f, "path absoluto '{}': esperado repo-relativo", path)
            }
            PolicyCause::PathTraversal { path } => write!(f, "travessia em '{}'", path),
            PolicyCause::PathDegenerateComponent { path, component } => {
                write!(f, "componente degenerado '{}' em '{}'", component, path)
            }
            PolicyCause::PathBackslash { path } => {
                write!(f, "barra invertida em '{}'", path)
            }
            PolicyCause::PathControlChar { path } => {
                write!(f, "caractere de controle em '{}'", path)
            }
            PolicyCause::PathTooLong { path, len } => write!(
                f,
                "path com {} caracteres excede o limite de {}: '{}'",
                len, MAX_PATH_LEN, path
            ),
            PolicyCause::TargetNotAllowed { path } => {
                write!(f, "target '{}' fora da allowlist", path)
            }
            PolicyCause::TargetLimitExceeded { path, bytes, limit } => write!(
                f,
                "target '{}' com {} bytes decodificados excede o limite de {}",
                path, bytes, limit
            ),
            PolicyCause::PlanLimitExceeded { bytes, limit } => write!(
                f,
                "plano com {} bytes decodificados excede o limite de {}",
                bytes, limit
            ),
            PolicyCause::AuthorizationMismatch { expected, provided } => write!(
                f,
                "autorização não corresponde ao plano: esperado {}, apresentado {}",
                expected, provided
            ),
            PolicyCause::EscapesRoot { path } => {
                write!(f, "'{}' resolve fora da raiz canônica", path)
            }
            PolicyCause::SymlinkTarget { path } => {
                write!(f, "target '{}' é link simbólico", path)
            }
            PolicyCause::SymlinkAncestor { path, component } => write!(
                f,
                "ancestral '{}' de '{}' é link simbólico",
                component, path
            ),
            PolicyCause::AncestorNotDirectory { path, component } => write!(
                f,
                "ancestral '{}' de '{}' existe e não é diretório",
                component, path
            ),
            PolicyCause::TargetNotRegularFile { path } => {
                write!(f, "target '{}' existe e não é arquivo regular", path)
            }
        }
    }
}

/// Estado decisório, separado da causa.
///
/// Nunca substitui uma [`Failure`]: um relatório pode carregar causa e decisão
/// ao mesmo tempo, e a decisão sozinha não explica nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    NeedsHumanDecision,
}

impl Decision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Decision::NeedsHumanDecision => "NEEDS_HUMAN_DECISION",
        }
    }
}

impl fmt::Display for Decision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
// @pinker-nav:end automation.contrato.resultados

// @pinker-nav:start automation.contrato.autorizacao
// @pinker-nav:domain contrato
// @pinker-nav:layer automation
// @pinker-nav:summary Autorização de escrita por digest exato do plano, drift final medido ou explicitamente desconhecido, e o procedimento de recuperação constante — observar de novo, novo check, novo plano, novo digest — que substitui qualquer promessa de rollback ou retry cego.

/// Autorização explícita para aplicar um plano.
///
/// O tipo é a prova: não existe caminho de escrita que não receba uma
/// autorização, então "apply sem digest" não é um erro em tempo de execução —
/// é uma expressão que não compila. O valor guardado é comparado com o digest
/// do plano apresentado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorization {
    digest: String,
}

impl Authorization {
    /// Autoriza exatamente o plano cujo digest é este.
    pub fn for_digest(digest: &str) -> Authorization {
        Authorization {
            digest: digest.to_string(),
        }
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Estado do drift depois de uma aplicação.
///
/// `Unknown` carrega a razão: um relatório que não conseguiu medir precisa
/// dizer isso, não omitir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalDrift {
    Measured(Outcome),
    Unknown(String),
}

impl FinalDrift {
    pub fn as_str(&self) -> &str {
        match self {
            FinalDrift::Measured(outcome) => outcome.as_str(),
            FinalDrift::Unknown(_) => "UNKNOWN",
        }
    }
}

/// Procedimento de recuperação depois de uma aplicação parcial.
///
/// Não há rollback global e não há retry cego: um plano que parou no meio deixa
/// o repositório num estado que só uma nova observação descreve.
pub const RECOVERY_PROCEDURE: &str =
    "observar novamente; executar novo check; produzir novo plano; autorizar novo digest";
// @pinker-nav:end automation.contrato.autorizacao

/// Escapa um texto para string JSON.
///
/// As implementações existentes no repositório (`src/nav.rs`, `src/main.rs`,
/// `src/doc_index.rs`, `src/change.rs`, `src/agent.rs`) são todas privadas dos
/// seus módulos: não há autoridade pública a reutilizar, e promover uma delas
/// mudaria a superfície de outro domínio para acomodar este. A cópia é declarada
/// e coberta por teste de escaping.
pub(crate) fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limites_sao_os_valores_aprovados() {
        assert_eq!(MAX_TARGET_BYTES, 8 * 1024 * 1024);
        assert_eq!(MAX_PLAN_BYTES, 32 * 1024 * 1024);
        assert_eq!(MAX_PLAN_BYTES / MAX_TARGET_BYTES, 4);
    }

    #[test]
    fn apenas_match_e_drift_sao_alcancaveis_pelo_nucleo_puro() {
        assert!(Outcome::Match.reachable_by_pure_core());
        assert!(Outcome::Drift.reachable_by_pure_core());
        assert!(!Outcome::Applied.reachable_by_pure_core());
        assert!(!Outcome::NoChange.reachable_by_pure_core());
    }

    #[test]
    fn decisao_nao_substitui_causa() {
        // A decisão é um tipo próprio: não há como construí-la a partir de uma
        // falha nem usá-la onde uma causa é esperada.
        assert_eq!(
            Decision::NeedsHumanDecision.as_str(),
            "NEEDS_HUMAN_DECISION"
        );
        assert_eq!(
            Failure::HarnessFailure(HarnessCause::ProducerMissing).code(),
            "HARNESS_FAILURE"
        );
    }
}
