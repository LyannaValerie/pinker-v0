//! Trama Pinker — compatibilidade do template real de PR com o parser.
//!
//! Estes testes leem `.github/pull_request_template.md` versionado e garantem
//! que o bloco `pinker-change` embutido permanece parseável e diagnosticável:
//! um único bloco, sem comentários inline (que quebrariam o parser), com
//! sentinelas que produzem E-CHANGE-PLACEHOLDER e que, uma vez preenchidas,
//! passam por `parse_pr_body` + `validate`. Impede que uma edição futura do
//! template volte a ser incompatível com a automação.

use std::path::PathBuf;

use pinker_v0::change::{Change, ChangeError};

fn template() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".github/pull_request_template.md");
    std::fs::read_to_string(path).expect("template de PR presente")
}

/// Conta as cercas de abertura reais (linha == ```` ```pinker-change ````),
/// ignorando menções inline ao bloco nos comentários HTML.
fn fence_count(body: &str) -> usize {
    body.lines()
        .filter(|l| l.trim() == "```pinker-change")
        .count()
}

/// Extrai o conteúdo entre a cerca de abertura real e a próxima ``` ```.
fn extract_block(body: &str) -> String {
    let mut lines = body.lines();
    // Avança até a linha de cerca real.
    for line in lines.by_ref() {
        if line.trim() == "```pinker-change" {
            break;
        }
    }
    let mut block = String::new();
    for line in lines {
        if line.trim() == "```" {
            return block;
        }
        block.push_str(line);
        block.push('\n');
    }
    panic!("cerca de fechamento ausente");
}

#[test]
fn template_tem_exatamente_um_bloco_pinker_change() {
    let n = fence_count(&template());
    assert_eq!(n, 1, "deve haver exatamente um bloco pinker-change");
}

#[test]
fn bloco_do_template_nao_tem_comentario_inline() {
    // O bloco é lido pela automação, não pelo YAML padrão: um `#` inline
    // vazaria para o valor. O template não pode conter nenhum `#` no bloco.
    let block = extract_block(&template());
    assert!(
        !block.contains('#'),
        "o bloco pinker-change não pode conter '#': {block:?}"
    );
}

#[test]
fn template_cru_falha_com_placeholder() {
    // O template não preenchido deve ser rejeitado com diagnóstico de sentinela,
    // e não com valores padrão que passariam silenciosamente.
    let change = Change::parse_pr_body(&template()).expect("bloco parseável");
    match change.validate() {
        Err(ChangeError::TemplatePlaceholder { field, value }) => {
            assert_eq!(field, "kind");
            assert!(
                value.starts_with('<') && value.ends_with('>'),
                "valor deve ser sentinela: {value:?}"
            );
        }
        other => panic!("esperado TemplatePlaceholder no template cru, veio {other:?}"),
    }
}

#[test]
fn template_nao_traz_valores_padrao_enganosos() {
    // kind/title/status/area precisam ser sentinelas — nunca valores já válidos
    // que produziriam manifesto enganoso sem o autor preencher de fato.
    let change = Change::parse_pr_body(&template()).expect("bloco parseável");
    assert!(change.kind.starts_with('<'), "kind deve ser sentinela");
    assert!(change.title.starts_with('<'), "title deve ser sentinela");
    assert!(change.status.starts_with('<'), "status deve ser sentinela");
    assert!(
        change.area.iter().all(|a| a.starts_with('<')),
        "area deve conter apenas sentinelas"
    );
}

#[test]
fn template_preenchido_passa_parse_e_validate() {
    // Substituindo as sentinelas por valores reais, o bloco deve ser válido.
    let filled = template()
        .replace("<preencher-kind>", "hotfix")
        .replace("<preencher-titulo>", "Título real de mudança")
        .replace("<preencher-status>", "completed")
        .replace("<preencher-area>", "development.trama");
    let change = Change::parse_pr_body(&filled).expect("bloco preenchido parseável");
    change
        .validate()
        .expect("bloco preenchido deve passar na validação");
    assert_eq!(change.kind, "hotfix");
    assert_eq!(change.status, "completed");
    assert_eq!(change.area, vec!["development.trama"]);
}
