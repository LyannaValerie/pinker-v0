//! Testes de unidade do núcleo somente leitura dos snapshots históricos,
//! movidos de `src/nav_projection_snapshot.rs` pela unidade NPS-2 do
//! inventário da #601 (Task #617).
//!
//! Só o arquivo mudou: as nove asserções continuam exatamente como estavam, e
//! continuam se chamando `nav_projection_snapshot::tests::*`. `mod tests` era
//! privado e `#[cfg(test)]` no pai e continua sendo; `super` continua sendo o
//! módulo `nav_projection_snapshot`, porque o módulo desceu de bloco para
//! arquivo sem descer de nível.

use super::*;

fn region(key: &str, file: &str, hash: &str) -> CodeRegion {
    CodeRegion {
        key: key.to_string(),
        kind: "region".to_string(),
        domain: Some("dominio".to_string()),
        layer: Some("camada".to_string()),
        phase: None,
        file: file.to_string(),
        start_marker: 1,
        content_start: 2,
        content_end: 3,
        end_marker: 4,
        summary: format!("Resumo de {}.", key),
        hash: hash.to_string(),
        status: "active".to_string(),
        symbols: Vec::new(),
        related_symbols: Vec::new(),
        test_for: Vec::new(),
        symbol_docs: Vec::new(),
    }
}

fn base_catalog() -> Vec<CodeRegion> {
    vec![
        region("a.b.um", "src/um.rs", "fnv1a64:0000000000000001"),
        region("a.b.dois", "src/dois.rs", "fnv1a64:0000000000000002"),
        region("posterior.novo", "src/novo.rs", "fnv1a64:0000000000000003"),
    ]
}

const VALID: &str = concat!(
    "schema = 1\n",
    "id = \"exemplo-historico\"\n",
    "state = \"FROZEN\"\n",
    "predecessor = \"exemplo-anterior\"\n",
    "justification = \"fixture sintetica\"\n",
    "\n[reconstruction]\n",
    "expected_overrides = 1\n",
    "expected_exclusions = 1\n",
    "\n[measures]\n",
    "regions = 2\n",
    "length = 0\n",
    "fnv1a64 = \"fnv1a64:0000000000000000\"\n",
    "\n[[rules]]\n",
    "op = \"exclude-key\"\n",
    "key = \"posterior.novo\"\n",
    "expected_matches = 1\n",
    "\n[[rules]]\n",
    "op = \"override-hash\"\n",
    "key = \"a.b.um\"\n",
    "from = \"fnv1a64:0000000000000001\"\n",
    "to = \"fnv1a64:00000000000000ff\"\n",
);

#[test]
fn parse_aceita_snapshot_valido() {
    let snapshot = parse(VALID).expect("snapshot valido");
    // A fixture declara `schema = 1` e assim permanece: o significado do
    // schema 1 é preservado mesmo depois de a composição chegar.
    assert_eq!(snapshot.schema, SNAPSHOT_SCHEMA_V1);
    assert_eq!(snapshot.base_snapshot, None);
    assert!(snapshot.recipes.is_empty());
    assert_eq!(snapshot.id, "exemplo-historico");
    assert_eq!(snapshot.state, SnapshotState::Frozen);
    assert_eq!(snapshot.predecessor.as_deref(), Some("exemplo-anterior"));
    assert_eq!(snapshot.rules.len(), 2);
    // Ordem canônica: exclusões antes de overrides.
    assert_eq!(snapshot.rules[0].op(), "exclude-key");
    assert_eq!(snapshot.rules[1].op(), "override-hash");
}

#[test]
fn render_e_parse_sao_estaveis() {
    let snapshot = parse(VALID).expect("snapshot valido");
    let rendered = render(&snapshot);
    let reparsed = parse(&rendered).expect("render canônico volta a interpretar");
    assert_eq!(snapshot, reparsed);
    assert_eq!(rendered, render(&reparsed));
}

#[test]
fn reconstrucao_consome_regras_exatamente() {
    let snapshot = parse(VALID).expect("snapshot valido");
    let reconstruction = reconstruct(&base_catalog(), &snapshot).expect("reconstrucao valida");
    assert_eq!(reconstruction.regions.len(), 2);
    assert_eq!(
        reconstruction
            .regions
            .iter()
            .find(|region| region.key == "a.b.um")
            .map(|region| region.hash.as_str()),
        Some("fnv1a64:00000000000000ff")
    );
    assert_eq!(reconstruction.ledger.len(), 2);
    assert!(reconstruction
        .ledger
        .iter()
        .all(|entry| entry.consumed == entry.expected));
}

#[test]
fn medida_e_independente_da_ordem_de_entrada() {
    let mut invertido = base_catalog();
    invertido.reverse();
    assert_eq!(measure(base_catalog().iter()), measure(invertido.iter()));
}

#[test]
fn schema_desconhecido_e_falha_de_harness() {
    let text = VALID.replace("schema = 1", &format!("schema = {}", SNAPSHOT_SCHEMA + 1));
    match parse(&text) {
        Err(HarnessFailure::SchemaUnknown { found, .. }) => {
            assert_eq!(found, SNAPSHOT_SCHEMA + 1)
        }
        outro => panic!("esperado schema desconhecido, veio {outro:?}"),
    }
}

#[test]
fn chave_desconhecida_e_rejeitada() {
    let text = format!("{}extra = 1\n", VALID);
    assert!(matches!(
        parse(&text),
        Err(HarnessFailure::InvalidField { .. })
    ));
}

#[test]
fn chave_duplicada_e_rejeitada() {
    let text = VALID.replace(
        "state = \"FROZEN\"",
        "state = \"FROZEN\"\nstate = \"FROZEN\"",
    );
    assert!(matches!(parse(&text), Err(HarnessFailure::Toml(_))));
}

#[test]
fn hash_invalido_e_rejeitado() {
    let text = VALID.replace("fnv1a64:0000000000000000", "fnv1a64:XYZ");
    assert!(matches!(
        parse(&text),
        Err(HarnessFailure::HashInvalid { .. })
    ));
}

#[test]
fn falha_de_harness_nao_produz_medida_observada() {
    let text = VALID.replace("key = \"a.b.um\"", "key = \"a.b.inexistente\"");
    let snapshot = parse(&text).expect("snapshot valido");
    let report = verify(&snapshot, &base_catalog());
    assert!(matches!(report.outcome, Outcome::HarnessFailure(_)));
    assert!(report.observed.is_none());
}
