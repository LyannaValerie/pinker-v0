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
use super::fsio::ApplyReport;
use super::{json_string, Failure, FinalDrift};

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

// @pinker-nav:start automation.relatorio.aplicacao
// @pinker-nav:domain relatorio
// @pinker-nav:layer automation
// @pinker-nav:summary Renderização do relatório de aplicação em JSON e Markdown a partir do mesmo modelo: aplicados, item falho, não tentados, rollback_performed sempre falso, drift final medido ou explicitamente desconhecido, causa e estado decisório lado a lado, e o procedimento de recuperação impresso em vez de sugerido.

fn lista_json(itens: &[String]) -> String {
    let partes: Vec<String> = itens.iter().map(|i| json_string(i)).collect();
    format!("[{}]", partes.join(","))
}

/// Relatório JSON de uma aplicação, de uma linha e com ordem de chaves fixa.
pub fn json_apply_report(report: &ApplyReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("{{\"schema\":{}", report.schema));
    out.push_str(&format!(",\"producer\":{}", json_string(&report.producer)));
    out.push_str(&format!(
        ",\"plan_digest\":{}",
        json_string(&report.plan_digest)
    ));
    match report.outcome {
        Some(outcome) => out.push_str(&format!(",\"outcome\":{}", json_string(outcome.as_str()))),
        None => out.push_str(",\"outcome\":null"),
    }
    out.push_str(&format!(",\"applied\":{}", lista_json(&report.applied)));
    match &report.failed {
        Some(path) => out.push_str(&format!(",\"failed\":{}", json_string(path))),
        None => out.push_str(",\"failed\":null"),
    }
    out.push_str(&format!(
        ",\"not_attempted\":{}",
        lista_json(&report.not_attempted)
    ));
    out.push_str(&format!(
        ",\"rollback_performed\":{}",
        report.rollback_performed
    ));
    match &report.final_drift {
        FinalDrift::Measured(outcome) => out.push_str(&format!(
            ",\"final_drift\":{{\"state\":{},\"reason\":null}}",
            json_string(outcome.as_str())
        )),
        FinalDrift::Unknown(reason) => out.push_str(&format!(
            ",\"final_drift\":{{\"state\":\"UNKNOWN\",\"reason\":{}}}",
            json_string(reason)
        )),
    }
    match &report.failure {
        Some(failure) => out.push_str(&format!(
            ",\"failure\":{{\"code\":{},\"message\":{}}}",
            json_string(failure.code()),
            json_string(&failure.to_string())
        )),
        None => out.push_str(",\"failure\":null"),
    }
    match report.decision {
        Some(decision) => {
            out.push_str(&format!(",\"decision\":{}", json_string(decision.as_str())))
        }
        None => out.push_str(",\"decision\":null"),
    }
    out.push_str(&format!(",\"recovery\":{}", json_string(report.recovery)));
    out.push('}');
    out
}

/// Relatório Markdown de uma aplicação, derivado do mesmo modelo do JSON.
pub fn markdown_apply_report(report: &ApplyReport) -> String {
    let estado = report.outcome.map_or("—", |o| o.as_str());
    let mut out = String::new();
    out.push_str(&format!("# Aplicação — {}\n\n", estado));
    out.push_str(&format!("- origem dos dados: `{}`\n", report.producer));
    out.push_str(&format!("- plano: `{}`\n", report.plan_digest));
    out.push_str(&format!("- aplicados: {}\n", lista_humana(&report.applied)));
    out.push_str(&format!(
        "- item falho: {}\n",
        report.failed.as_deref().unwrap_or("—")
    ));
    out.push_str(&format!(
        "- não tentados: {}\n",
        lista_humana(&report.not_attempted)
    ));
    out.push_str(&format!(
        "- rollback executado: {}\n",
        report.rollback_performed
    ));
    match &report.final_drift {
        FinalDrift::Measured(outcome) => {
            out.push_str(&format!("- drift final: {}\n", outcome.as_str()))
        }
        FinalDrift::Unknown(reason) => {
            out.push_str(&format!("- drift final: UNKNOWN ({})\n", reason))
        }
    }
    match &report.failure {
        Some(failure) => out.push_str(&format!("- causa: {}\n", failure)),
        None => out.push_str("- causa: —\n"),
    }
    match report.decision {
        Some(decision) => out.push_str(&format!("- decisão: {}\n", decision.as_str())),
        None => out.push_str("- decisão: —\n"),
    }
    out.push_str(&format!("\nRecuperação: {}.\n", report.recovery));
    out
}

fn lista_humana(itens: &[String]) -> String {
    if itens.is_empty() {
        "—".to_string()
    } else {
        itens
            .iter()
            .map(|i| format!("`{i}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}
// @pinker-nav:end automation.relatorio.aplicacao

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
