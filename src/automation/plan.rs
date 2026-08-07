//! Plano imutável, sua serialização canônica e o digest que o autoriza.

use super::path::{Allowlist, RelativePath};
use super::{
    json_string, Failure, HarnessCause, PolicyCause, AUTOMATION_SCHEMA, MAX_PLAN_BYTES,
    MAX_TARGET_BYTES,
};

// @pinker-nav:start automation.plano.modelo
// @pinker-nav:domain plano
// @pinker-nav:layer automation
// @pinker-nav:summary Modelo imutável do plano efêmero: payload opaco com limite explícito por target, target repo-relativo com estado desejado opcional (ausência significa remoção) e construtor que valida schema, produtor, allowlist, duplicidade e o limite somado do plano antes de existir qualquer instância.

/// Conteúdo desejado de um target.
///
/// Opaco de propósito: o payload nunca aparece em relatório, e o único caminho
/// para os bytes é [`Payload::bytes`], usado pela comparação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    bytes: Vec<u8>,
}

impl Payload {
    /// Constrói validando o limite por target.
    pub fn new(bytes: Vec<u8>, path: &RelativePath) -> Result<Payload, PolicyCause> {
        if bytes.len() > MAX_TARGET_BYTES {
            return Err(PolicyCause::TargetLimitExceeded {
                path: path.as_str().to_string(),
                bytes: bytes.len(),
                limit: MAX_TARGET_BYTES,
            });
        }
        Ok(Payload { bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Forma hexadecimal minúscula, usada na serialização canônica.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(self.bytes.len() * 2);
        for byte in &self.bytes {
            out.push_str(&format!("{:02x}", byte));
        }
        out
    }
}

/// Um target do plano.
///
/// `desired` ausente significa **remoção desejada**; presente significa o
/// conteúdo exato que o target deve ter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedTarget {
    path: RelativePath,
    desired: Option<Payload>,
}

impl PlannedTarget {
    pub fn path(&self) -> &RelativePath {
        &self.path
    }

    pub fn desired(&self) -> Option<&Payload> {
        self.desired.as_ref()
    }

    /// Bytes desejados, ou `None` quando o alvo é a remoção.
    pub fn desired_bytes(&self) -> Option<&[u8]> {
        self.desired.as_ref().map(Payload::bytes)
    }
}

/// Plano efêmero e imutável.
///
/// Não é canônico, não é versionado no repositório e nunca é lido de volta: é
/// calculado pelo adaptador, usado e descartado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    schema: u64,
    producer: String,
    targets: Vec<PlannedTarget>,
}

/// Construtor do plano. Toda validação acontece aqui, antes de existir um
/// [`Plan`].
#[derive(Debug, Clone)]
pub struct PlanBuilder {
    schema: u64,
    producer: String,
    allowlist: Allowlist,
    targets: Vec<PlannedTarget>,
}

impl PlanBuilder {
    /// Inicia um plano declarando a origem dos dados e a allowlist lógica.
    pub fn new(producer: &str, allowlist: Allowlist) -> PlanBuilder {
        PlanBuilder {
            schema: AUTOMATION_SCHEMA,
            producer: producer.to_string(),
            allowlist,
            targets: Vec::new(),
        }
    }

    /// Permite exercitar a rejeição de schema desconhecido sem forjar bytes.
    pub fn with_schema(mut self, schema: u64) -> PlanBuilder {
        self.schema = schema;
        self
    }

    /// Declara o conteúdo desejado de um target.
    pub fn desire(mut self, path: &str, bytes: Vec<u8>) -> Result<PlanBuilder, Failure> {
        let path = RelativePath::new(path).map_err(Failure::PolicyViolation)?;
        let payload = Payload::new(bytes, &path).map_err(Failure::PolicyViolation)?;
        self.push(path, Some(payload))?;
        Ok(self)
    }

    /// Declara a remoção desejada de um target.
    pub fn remove(mut self, path: &str) -> Result<PlanBuilder, Failure> {
        let path = RelativePath::new(path).map_err(Failure::PolicyViolation)?;
        self.push(path, None)?;
        Ok(self)
    }

    fn push(&mut self, path: RelativePath, desired: Option<Payload>) -> Result<(), Failure> {
        if !self.allowlist.permits(&path) {
            return Err(Failure::PolicyViolation(PolicyCause::TargetNotAllowed {
                path: path.as_str().to_string(),
            }));
        }
        if self.targets.iter().any(|t| t.path == path) {
            return Err(Failure::HarnessFailure(HarnessCause::DuplicateTarget {
                path: path.as_str().to_string(),
            }));
        }
        self.targets.push(PlannedTarget { path, desired });
        Ok(())
    }

    /// Fecha o plano, validando schema, produtor e o limite somado.
    ///
    /// Os targets ficam em ordem canônica por path, de modo que a ordem de
    /// declaração não influencia nem a forma serializada nem o digest.
    pub fn build(mut self) -> Result<Plan, Failure> {
        if self.schema != AUTOMATION_SCHEMA {
            return Err(Failure::HarnessFailure(HarnessCause::SchemaUnknown {
                found: self.schema,
            }));
        }
        if self.producer.trim().is_empty() {
            return Err(Failure::HarnessFailure(HarnessCause::ProducerMissing));
        }
        let total: usize = self
            .targets
            .iter()
            .map(|t| t.desired.as_ref().map_or(0, Payload::len))
            .sum();
        if total > MAX_PLAN_BYTES {
            return Err(Failure::PolicyViolation(PolicyCause::PlanLimitExceeded {
                bytes: total,
                limit: MAX_PLAN_BYTES,
            }));
        }
        self.targets.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Plan {
            schema: self.schema,
            producer: self.producer,
            targets: self.targets,
        })
    }
}

impl Plan {
    pub fn schema(&self) -> u64 {
        self.schema
    }

    /// Origem dos dados: qual adaptador calculou este plano.
    pub fn producer(&self) -> &str {
        &self.producer
    }

    /// Targets em ordem canônica por path.
    pub fn targets(&self) -> &[PlannedTarget] {
        &self.targets
    }

    /// Soma dos bytes decodificados desejados.
    pub fn decoded_bytes(&self) -> usize {
        self.targets
            .iter()
            .map(|t| t.desired.as_ref().map_or(0, Payload::len))
            .sum()
    }

    pub fn target(&self, path: &RelativePath) -> Option<&PlannedTarget> {
        self.targets.iter().find(|t| &t.path == path)
    }
}
// @pinker-nav:end automation.plano.modelo

// @pinker-nav:start automation.plano.serializacao
// @pinker-nav:domain plano
// @pinker-nav:layer automation
// @pinker-nav:summary Serialização canônica do plano em JSON de uma linha, com payload hexadecimal minúsculo e remoção representada por null, e digest SHA-256 sobre exatamente esses bytes — de modo que o payload fica coberto pelo digest e nenhum root absoluto entra na forma canônica; não existe parser, porque o plano é efêmero e nunca é lido de volta.

impl Plan {
    /// Forma canônica do plano: JSON de uma linha, ordem de chaves fixa,
    /// targets ordenados por path e payload em hexadecimal minúsculo.
    ///
    /// Só entram paths repo-relativos: nenhum root absoluto alcança esta forma.
    pub fn to_canonical_json(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{{\"schema\":{}", self.schema));
        out.push_str(&format!(",\"producer\":{}", json_string(&self.producer)));
        out.push_str(",\"targets\":[");
        for (index, target) in self.targets.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"path\":{},\"desired\":",
                json_string(target.path.as_str())
            ));
            match &target.desired {
                Some(payload) => out.push_str(&json_string(&payload.to_hex())),
                None => out.push_str("null"),
            }
            out.push('}');
        }
        out.push_str("]}");
        out
    }

    /// Digest de autorização do plano.
    ///
    /// É o SHA-256 da forma canônica, que **inclui o payload hexadecimal**:
    /// alterar um único byte de conteúdo muda o digest. Reutiliza
    /// [`crate::agent::sha256_hex`], a única implementação pública de SHA-256 do
    /// repositório — o contrato proíbe adicionar crate de hash e proíbe
    /// duplicar hashing em silêncio. A dependência é sobre uma função pura de
    /// bytes: o núcleo não conhece, não lê e não traduz nenhum estado do
    /// `pink agente`.
    pub fn digest(&self) -> String {
        crate::agent::sha256_hex(self.to_canonical_json().as_bytes())
    }
}
// @pinker-nav:end automation.plano.serializacao

#[cfg(test)]
mod tests {
    use super::*;

    fn allowlist() -> Allowlist {
        Allowlist::new(&["a.md", "b.md"]).expect("allowlist")
    }

    #[test]
    fn ordem_de_declaracao_nao_muda_a_forma_canonica() {
        let um = PlanBuilder::new("adaptador", allowlist())
            .desire("b.md", b"dois".to_vec())
            .unwrap()
            .desire("a.md", b"um".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let outro = PlanBuilder::new("adaptador", allowlist())
            .desire("a.md", b"um".to_vec())
            .unwrap()
            .desire("b.md", b"dois".to_vec())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(um.to_canonical_json(), outro.to_canonical_json());
        assert_eq!(um.digest(), outro.digest());
    }

    #[test]
    fn payload_entra_no_digest() {
        let base = PlanBuilder::new("adaptador", allowlist())
            .desire("a.md", b"um".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let mudado = PlanBuilder::new("adaptador", allowlist())
            .desire("a.md", b"un".to_vec())
            .unwrap()
            .build()
            .unwrap();
        assert_ne!(base.digest(), mudado.digest());
    }

    #[test]
    fn remocao_e_null_na_forma_canonica() {
        let plano = PlanBuilder::new("adaptador", allowlist())
            .remove("a.md")
            .unwrap()
            .build()
            .unwrap();
        assert!(plano.to_canonical_json().contains("\"desired\":null"));
    }
}
