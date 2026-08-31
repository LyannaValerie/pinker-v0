//! Renderização do estado consolidado.
//!
//! Human e JSON recebem exclusivamente [`ProjectState`]. Nenhum renderer
//! consulta autoridades, recalcula overall ou executa I/O.

use crate::project_state::{
    AuthorityAvailability, Diagnostic, DocumentationState, DomainDetails, DomainState, Finding,
    LocalCheck, PendingOperation, ProjectState, ProjectionCause, ProjectionItem, ProjectionsState,
    RepositoryState, Source, TramaState,
};

// @pinker-nav:start project-state.renderizacao
// @pinker-nav:domain estado
// @pinker-nav:layer relatorios
// @pinker-nav:summary Renderers humano e JSON determinísticos derivados exclusivamente de ProjectState, com ordem fixa, UTF-8, paths repo-relativos e ausência de ANSI, timestamps e root absoluto.

pub fn render_json(state: &ProjectState) -> String {
    let domains = state
        .domains
        .iter()
        .map(domain_json)
        .collect::<Vec<_>>()
        .join(",");
    let warnings = state
        .warnings
        .iter()
        .map(finding_json)
        .collect::<Vec<_>>()
        .join(",");
    let blockers = state
        .blockers
        .iter()
        .map(finding_json)
        .collect::<Vec<_>>()
        .join(",");
    let pending = state
        .pending_operations
        .iter()
        .map(pending_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"overall\":{},\"domains\":[{}],\"warnings\":[{}],\"blockers\":[{}],\"pending_operations\":[{}]}}",
        state.schema,
        json_string(state.overall.as_str()),
        domains,
        warnings,
        blockers,
        pending
    )
}

pub fn render_human(state: &ProjectState) -> String {
    let mut out = String::new();
    out.push_str(&format!("Pinker — {}\n", state.overall.as_str()));
    for domain in &state.domains {
        out.push_str(&format!(
            "- {:<14} {}\n",
            human_domain(domain.id.as_str()),
            domain.status.as_str()
        ));
        append_domain_summary(&mut out, domain);
    }
    append_findings_human(&mut out, "Warnings", &state.warnings);
    append_findings_human(&mut out, "Blockers", &state.blockers);
    out.push_str("\nOperações pendentes\n");
    if state.pending_operations.is_empty() {
        out.push_str("- nenhuma\n");
    } else {
        for operation in &state.pending_operations {
            out.push_str(&format!(
                "- [{}] {} ({})\n",
                operation.domain.as_str(),
                operation.summary,
                operation.reason
            ));
        }
    }
    out
}

fn append_domain_summary(out: &mut String, domain: &DomainState) {
    match &domain.details {
        DomainDetails::Repository(details) => out.push_str(&format!(
            "  root={} autoridades={}\n",
            details.root_discovered,
            details.authorities.len()
        )),
        DomainDetails::Trama(details) => out.push_str(&format!(
            "  regiões={} catálogo={} sincronizado={}\n",
            human_option(details.regions),
            human_validity(details.catalog_valid),
            human_bool(details.synchronized)
        )),
        DomainDetails::Documentation(details) => out.push_str(&format!(
            "  documentos={} seções={} drift={}\n",
            human_option(details.documents),
            human_option(details.sections),
            details.known_drift
        )),
        DomainDetails::Projections(details) => out.push_str(&format!(
            "  FROZEN={} CANDIDATE={} verificar={}\n",
            details.frozen, details.candidate, details.verification
        )),
        DomainDetails::LocalChecks(details) => out.push_str(&format!(
            "  checks={}\n",
            details
                .checks
                .iter()
                .map(|check| format!("{}={}", check.id, check.status.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        DomainDetails::Diagnostics(details) => {
            out.push_str(&format!("  entradas={}\n", details.entries.len()))
        }
    }
}

fn append_findings_human(out: &mut String, title: &str, findings: &[Finding]) {
    out.push_str(&format!("\n{title}\n"));
    if findings.is_empty() {
        out.push_str("- nenhum\n");
    } else {
        for finding in findings {
            out.push_str(&format!(
                "- [{}] {} ({})\n",
                finding.domain.as_str(),
                finding.summary,
                finding.reason
            ));
        }
    }
}

fn human_domain(id: &str) -> &str {
    match id {
        "repository" => "Repositório",
        "trama" => "Trama",
        "documentation" => "Documentação",
        "projections" => "Projeções",
        "local_checks" => "Checks locais",
        "diagnostics" => "Diagnósticos",
        _ => id,
    }
}

fn human_option(value: Option<usize>) -> String {
    value.map_or_else(|| "—".to_string(), |value| value.to_string())
}

fn human_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "sim",
        Some(false) => "não",
        None => "—",
    }
}

fn human_validity(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "válido",
        Some(false) => "inválido",
        None => "—",
    }
}

fn domain_json(domain: &DomainState) -> String {
    format!(
        "{{\"id\":{},\"status\":{},\"source\":{},\"details\":{}}}",
        json_string(domain.id.as_str()),
        json_string(domain.status.as_str()),
        source_json(&domain.source),
        details_json(&domain.details)
    )
}

fn details_json(details: &DomainDetails) -> String {
    match details {
        DomainDetails::Repository(details) => repository_json(details),
        DomainDetails::Trama(details) => trama_json(details),
        DomainDetails::Documentation(details) => documentation_json(details),
        DomainDetails::Projections(details) => projections_json(details),
        DomainDetails::LocalChecks(details) => {
            let checks = details
                .checks
                .iter()
                .map(local_check_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"checks\":[{checks}]}}")
        }
        DomainDetails::Diagnostics(details) => {
            let entries = details
                .entries
                .iter()
                .map(diagnostic_json)
                .collect::<Vec<_>>()
                .join(",");
            format!("{{\"entries\":[{entries}]}}")
        }
    }
}

fn repository_json(details: &RepositoryState) -> String {
    let authorities = details
        .authorities
        .iter()
        .map(authority_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"root_discovered\":{},\"marker\":{},\"authorities\":[{}]}}",
        details.root_discovered,
        json_string(&details.marker),
        authorities
    )
}

fn authority_json(authority: &AuthorityAvailability) -> String {
    format!(
        "{{\"id\":{},\"status\":{},\"source\":{}}}",
        json_string(&authority.id),
        json_string(authority.status.as_str()),
        source_json(&authority.source)
    )
}

fn trama_json(details: &TramaState) -> String {
    format!(
        "{{\"catalog_path\":{},\"catalog_available\":{},\"catalog_valid\":{},\"regions\":{},\"source_valid\":{},\"synchronized\":{}}}",
        option_string(details.catalog_path.as_deref()),
        details.catalog_available,
        option_bool(details.catalog_valid),
        option_usize(details.regions),
        option_bool(details.source_valid),
        option_bool(details.synchronized)
    )
}

fn documentation_json(details: &DocumentationState) -> String {
    format!(
        "{{\"catalog_path\":{},\"catalog_available\":{},\"catalog_valid\":{},\"documents\":{},\"sections\":{},\"source_valid\":{},\"known_drift\":{}}}",
        option_string(details.catalog_path.as_deref()),
        details.catalog_available,
        option_bool(details.catalog_valid),
        option_usize(details.documents),
        option_usize(details.sections),
        option_bool(details.source_valid),
        details.known_drift
    )
}

fn projections_json(details: &ProjectionsState) -> String {
    let items = details
        .items
        .iter()
        .map(projection_item_json)
        .collect::<Vec<_>>()
        .join(",");
    let causes = details
        .causes
        .iter()
        .map(projection_cause_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"frozen\":{},\"candidate\":{},\"recipes\":{},\"verification\":{},\"items\":[{}],\"causes\":[{}]}}",
        details.frozen,
        details.candidate,
        details.recipes,
        json_string(&details.verification),
        items,
        causes
    )
}

fn projection_item_json(item: &ProjectionItem) -> String {
    format!(
        "{{\"id\":{},\"state\":{},\"path\":{},\"outcome\":{},\"failure_code\":{}}}",
        json_string(&item.id),
        json_string(&item.state),
        json_string(&item.path),
        json_string(&item.outcome),
        option_string(item.failure_code.as_deref())
    )
}

fn projection_cause_json(cause: &ProjectionCause) -> String {
    format!(
        "{{\"cause\":{},\"blocked\":{}}}",
        json_string(&cause.cause),
        string_array(&cause.blocked)
    )
}

fn local_check_json(check: &LocalCheck) -> String {
    format!(
        "{{\"id\":{},\"status\":{},\"source\":{}}}",
        json_string(&check.id),
        json_string(check.status.as_str()),
        source_json(&check.source)
    )
}

fn diagnostic_json(diagnostic: &Diagnostic) -> String {
    format!(
        "{{\"id\":{},\"domain\":{},\"status\":{},\"summary\":{},\"reason\":{},\"source\":{}}}",
        json_string(&diagnostic.id),
        json_string(diagnostic.domain.as_str()),
        json_string(diagnostic.status.as_str()),
        json_string(&diagnostic.summary),
        json_string(&diagnostic.reason),
        source_json(&diagnostic.source)
    )
}

fn finding_json(finding: &Finding) -> String {
    format!(
        "{{\"id\":{},\"domain\":{},\"summary\":{},\"source\":{},\"reason\":{}}}",
        json_string(&finding.id),
        json_string(finding.domain.as_str()),
        json_string(&finding.summary),
        source_json(&finding.source),
        json_string(&finding.reason)
    )
}

fn pending_json(operation: &PendingOperation) -> String {
    format!(
        "{{\"id\":{},\"domain\":{},\"kind\":{},\"summary\":{},\"source\":{},\"reason\":{}}}",
        json_string(&operation.id),
        json_string(operation.domain.as_str()),
        json_string(&operation.kind),
        json_string(&operation.summary),
        source_json(&operation.source),
        json_string(&operation.reason)
    )
}

fn source_json(source: &Source) -> String {
    format!(
        "{{\"kind\":{},\"path\":{},\"authority\":{}}}",
        json_string(source.kind.as_str()),
        option_string(source.path.as_deref()),
        json_string(&source.authority)
    )
}

fn option_string(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_string(), json_string)
}

fn option_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn option_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "null",
    }
}

fn string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

// @pinker-nav:end project-state.renderizacao

#[cfg(test)]
mod tests {
    use super::json_string;

    #[test]
    fn json_escape_cobre_controles_sem_ansi() {
        assert_eq!(json_string("a\n\"b"), "\"a\\n\\\"b\"");
    }
}
