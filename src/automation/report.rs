//! Relatórios determinísticos, os dois derivados do mesmo modelo.
//!
//! O JSON é a superfície de máquina e o Markdown é derivado dele — do mesmo
//! [`CheckReport`], não de uma segunda travessia com regras próprias, para que
//! não possam divergir.
//!
//! Nenhum dos dois carrega o payload completo: um relatório descreve **o que
//! mudaria**, não o conteúdo. Também não carregam root absoluto, porque o
//! modelo só conhece paths repo-relativos.

use super::compare::CheckReport;
use super::{json_string, Failure};

// @pinker-nav:start automation.relatorio.renderizacao
// @pinker-nav:domain relatorio
// @pinker-nav:layer automation
// @pinker-nav:summary Renderização determinística do mesmo modelo de check em JSON de uma linha com ordem de chaves fixa e em Markdown derivado dele, ambos sem payload completo, sem root absoluto e sem códigos ANSI; a falha, quando existe, aparece com código estável e nunca é substituída pelo estado decisório.

/// Relatório JSON de uma linha, com ordem de chaves fixa.
pub fn json_report(report: &CheckReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", report.schema));
    out.push_str(&format!(",\"producer\":{}", json_string(&report.producer)));
    out.push_str(&format!(
        ",\"plan_digest\":{}",
        json_string(&report.plan_digest)
    ));
    out.push_str(&format!(
        ",\"outcome\":{}",
        json_string(report.outcome.as_str())
    ));

    out.push_str(",\"targets\":[");
    for (index, target) in report.targets.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!("{{\"path\":{}", json_string(&target.path)));
        out.push_str(&format!(
            ",\"change\":{}",
            json_string(target.change.as_str())
        ));
        out.push_str(&format!(
            ",\"desired_bytes\":{}",
            optional_number(target.desired_bytes)
        ));
        out.push_str(&format!(
            ",\"desired_digest\":{}",
            optional_text(target.desired_digest.as_deref())
        ));
        out.push_str(&format!(
            ",\"observed_bytes\":{}",
            optional_number(target.observed_bytes)
        ));
        out.push_str(&format!(
            ",\"observed_digest\":{}",
            optional_text(target.observed_digest.as_deref())
        ));
        out.push('}');
    }
    out.push(']');

    out.push_str(",\"summary\":{");
    for (index, (name, count)) in report.summary().iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!("{}:{}", json_string(name), count));
    }
    out.push('}');

    out.push_str(",\"failure\":null");
    match report.decision {
        Some(decision) => {
            out.push_str(&format!(",\"decision\":{}", json_string(decision.as_str())))
        }
        None => out.push_str(",\"decision\":null"),
    }
    out.push('}');
    out
}

/// Relatório JSON de uma falha operacional, com o mesmo envelope.
///
/// A causa aparece sempre; o estado decisório, quando existe, aparece **ao lado**
/// dela e nunca no lugar dela.
pub fn json_failure(
    producer: &str,
    failure: &Failure,
    decision: Option<super::Decision>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", super::AUTOMATION_SCHEMA));
    out.push_str(&format!(",\"producer\":{}", json_string(producer)));
    out.push_str(",\"plan_digest\":null");
    out.push_str(",\"outcome\":null");
    out.push_str(",\"targets\":[]");
    out.push_str(",\"summary\":{\"create\":0,\"replace\":0,\"remove\":0,\"no_change\":0}");
    out.push_str(&format!(
        ",\"failure\":{{\"code\":{},\"message\":{}}}",
        json_string(failure.code()),
        json_string(&failure.to_string())
    ));
    match decision {
        Some(decision) => {
            out.push_str(&format!(",\"decision\":{}", json_string(decision.as_str())))
        }
        None => out.push_str(",\"decision\":null"),
    }
    out.push('}');
    out
}

/// Relatório Markdown, derivado do mesmo modelo do JSON.
pub fn markdown_report(report: &CheckReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Automação — {}\n\n", report.outcome.as_str()));
    out.push_str(&format!("- origem dos dados: `{}`\n", report.producer));
    out.push_str(&format!("- plano: `{}`\n", report.plan_digest));
    let summary: Vec<String> = report
        .summary()
        .iter()
        .map(|(name, count)| format!("{} {}", name, count))
        .collect();
    out.push_str(&format!("- resumo: {}\n", summary.join(", ")));
    match report.decision {
        Some(decision) => out.push_str(&format!("- decisão: {}\n", decision.as_str())),
        None => out.push_str("- decisão: —\n"),
    }

    if report.targets.is_empty() {
        out.push_str("\n_Nenhum target no plano._\n");
        return out;
    }

    out.push_str("\n| Target | Mudança | Desejado | Observado |\n");
    out.push_str("|---|---|---|---|\n");
    for target in &report.targets {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            target.path,
            target.change.as_str(),
            cell(target.desired_bytes, target.desired_digest.as_deref()),
            cell(target.observed_bytes, target.observed_digest.as_deref()),
        ));
    }
    out
}

/// Uma célula descreve tamanho e digest — nunca o conteúdo.
fn cell(bytes: Option<usize>, digest: Option<&str>) -> String {
    match (bytes, digest) {
        (Some(bytes), Some(digest)) => {
            format!("{} B `{}`", bytes, digest.get(..16).unwrap_or(digest))
        }
        _ => "ausente".to_string(),
    }
}

fn optional_number(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |v| v.to_string())
}

fn optional_text(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_string(), json_string)
}
// @pinker-nav:end automation.relatorio.renderizacao

#[cfg(test)]
mod tests {
    use super::super::compare::{check, Observation, ObservedState};
    use super::super::path::Allowlist;
    use super::super::plan::PlanBuilder;
    use super::*;

    #[test]
    fn relatorios_nao_carregam_o_payload() {
        let allowlist = Allowlist::new(&["a.md"]).unwrap();
        let plano = PlanBuilder::new("adaptador", allowlist)
            .desire("a.md", b"conteudo-secreto".to_vec())
            .unwrap()
            .build()
            .unwrap();
        let observado = ObservedState::new()
            .with(Observation::absent("a.md").unwrap())
            .unwrap();
        let report = check(&plano, &observado).unwrap();
        let json = json_report(&report);
        let markdown = markdown_report(&report);
        assert!(!json.contains("conteudo-secreto"));
        assert!(!markdown.contains("conteudo-secreto"));
        // Nem o hexadecimal do payload.
        assert!(!json.contains("636f6e746575646f"));
        assert!(!markdown.contains("636f6e746575646f"));
    }
}
