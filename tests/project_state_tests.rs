use pinker_v0::project_state::{
    collect, DomainDetails, DomainId, ProjectState, StateStatus, PROJECT_STATE_SCHEMA,
};
use pinker_v0::project_state_report::{render_human, render_json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

// @pinker-nav:start evidencia.project-state.contrato
// @pinker-nav:domain estado
// @pinker-nav:layer evidencia
// @pinker-nav:summary Exercita schema, ordem, disponibilidade parcial, drift e harness das autoridades reais, CLI, independência de root e invariância somente leitura do estado consolidado.

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn copy(label: &str) -> Fixture {
        let path = std::env::temp_dir().join(format!(
            "pinker-project-state-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        for relative in [
            ".pinker",
            "docs",
            "src",
            "tests",
            "apps",
            "runtime/pinker_rt/src",
        ] {
            copy_tree(&source.join(relative), &path.join(relative));
        }
        Fixture(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    let metadata = fs::symlink_metadata(source).unwrap();
    if metadata.is_dir() {
        fs::create_dir_all(destination).unwrap();
        let mut entries = fs::read_dir(source)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            copy_tree(&entry.path(), &destination.join(entry.file_name()));
        }
    } else {
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(source, destination).unwrap();
    }
}

fn state(root: &Path) -> ProjectState {
    collect(root).expect("estado estrutural")
}

fn status(state: &ProjectState, id: DomainId) -> StateStatus {
    state.domain(id).expect("domínio").status
}

fn projection_details(state: &ProjectState) -> &pinker_v0::project_state::ProjectionsState {
    match &state.domain(DomainId::Projections).unwrap().details {
        DomainDetails::Projections(details) => details,
        _ => panic!("detalhe de projeção"),
    }
}

fn replace_once(path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains(from), "fixture não contém {from}");
    fs::write(path, text.replacen(from, to, 1)).unwrap();
}

fn json_is_valid(input: &str) -> bool {
    fn ws(bytes: &[u8], pos: &mut usize) {
        while bytes.get(*pos).is_some_and(u8::is_ascii_whitespace) {
            *pos += 1;
        }
    }
    fn string(bytes: &[u8], pos: &mut usize) -> bool {
        if bytes.get(*pos) != Some(&b'"') {
            return false;
        }
        *pos += 1;
        while let Some(&ch) = bytes.get(*pos) {
            *pos += 1;
            match ch {
                b'"' => return true,
                b'\\' => {
                    let Some(&escaped) = bytes.get(*pos) else {
                        return false;
                    };
                    *pos += 1;
                    if escaped == b'u' {
                        for _ in 0..4 {
                            if !bytes.get(*pos).is_some_and(u8::is_ascii_hexdigit) {
                                return false;
                            }
                            *pos += 1;
                        }
                    } else if !matches!(
                        escaped,
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't'
                    ) {
                        return false;
                    }
                }
                0..=0x1f => return false,
                _ => {}
            }
        }
        false
    }
    fn value(bytes: &[u8], pos: &mut usize) -> bool {
        ws(bytes, pos);
        match bytes.get(*pos) {
            Some(b'"') => string(bytes, pos),
            Some(b'{') => object(bytes, pos),
            Some(b'[') => array(bytes, pos),
            Some(b't') if bytes.get(*pos..*pos + 4) == Some(b"true") => {
                *pos += 4;
                true
            }
            Some(b'f') if bytes.get(*pos..*pos + 5) == Some(b"false") => {
                *pos += 5;
                true
            }
            Some(b'n') if bytes.get(*pos..*pos + 4) == Some(b"null") => {
                *pos += 4;
                true
            }
            Some(b'-' | b'0'..=b'9') => {
                if bytes.get(*pos) == Some(&b'-') {
                    *pos += 1;
                }
                let start = *pos;
                while bytes.get(*pos).is_some_and(u8::is_ascii_digit) {
                    *pos += 1;
                }
                *pos > start
            }
            _ => false,
        }
    }
    fn array(bytes: &[u8], pos: &mut usize) -> bool {
        *pos += 1;
        ws(bytes, pos);
        if bytes.get(*pos) == Some(&b']') {
            *pos += 1;
            return true;
        }
        loop {
            if !value(bytes, pos) {
                return false;
            }
            ws(bytes, pos);
            match bytes.get(*pos) {
                Some(b',') => *pos += 1,
                Some(b']') => {
                    *pos += 1;
                    return true;
                }
                _ => return false,
            }
        }
    }
    fn object(bytes: &[u8], pos: &mut usize) -> bool {
        *pos += 1;
        ws(bytes, pos);
        if bytes.get(*pos) == Some(&b'}') {
            *pos += 1;
            return true;
        }
        loop {
            ws(bytes, pos);
            if !string(bytes, pos) {
                return false;
            }
            ws(bytes, pos);
            if bytes.get(*pos) != Some(&b':') {
                return false;
            }
            *pos += 1;
            if !value(bytes, pos) {
                return false;
            }
            ws(bytes, pos);
            match bytes.get(*pos) {
                Some(b',') => *pos += 1,
                Some(b'}') => {
                    *pos += 1;
                    return true;
                }
                _ => return false,
            }
        }
    }
    let bytes = input.as_bytes();
    let mut pos = 0;
    value(bytes, &mut pos) && {
        ws(bytes, &mut pos);
        pos == bytes.len()
    }
}

#[test]
fn schema_ordem_fontes_e_estado_saudavel_sao_deterministicos() {
    let state = state(Path::new(env!("CARGO_MANIFEST_DIR")));
    assert_eq!(state.schema, PROJECT_STATE_SCHEMA);
    assert_eq!(
        state
            .domains
            .iter()
            .map(|domain| domain.id)
            .collect::<Vec<_>>(),
        vec![
            DomainId::Repository,
            DomainId::Trama,
            DomainId::Documentation,
            DomainId::Projections,
            DomainId::LocalChecks,
            DomainId::Diagnostics,
        ]
    );
    for id in [
        DomainId::Trama,
        DomainId::Documentation,
        DomainId::Projections,
    ] {
        assert_eq!(status(&state, id), StateStatus::Ok);
        assert!(!state.domain(id).unwrap().source.authority.is_empty());
    }
    assert_eq!(state.overall, StateStatus::Ok);
    let projections = projection_details(&state);
    assert_eq!(projections.frozen, 13);
    assert_eq!(projections.candidate, 0);
    assert_eq!(projections.verification, "MATCH");
    assert!(projections.items.iter().all(|item| item.outcome == "MATCH"));

    let json = render_json(&state);
    assert!(json_is_valid(&json));
    assert!(json.starts_with("{\"schema\":1,\"overall\":\"OK\",\"domains\":"));
    assert!(!json.contains('\u{1b}'));
    assert!(!json.contains(env!("CARGO_MANIFEST_DIR")));
    assert_eq!(json, render_json(&state));
}

#[test]
fn documentacao_invalida_ou_com_drift_preserva_outros_dominios() {
    let drift = Fixture::copy("docs-drift");
    replace_once(
        &drift.path().join("docs/development/README.md"),
        "# Desenvolvimento",
        "# Desenvolvimento alterado",
    );
    let observed = state(drift.path());
    assert_eq!(
        status(&observed, DomainId::Documentation),
        StateStatus::Warning
    );
    assert!(observed
        .pending_operations
        .iter()
        .any(|operation| operation.reason == "documentation_drift"));

    let invalid = Fixture::copy("docs-invalid");
    fs::write(invalid.path().join("docs/navigation.jsonl"), "{invalid\n").unwrap();
    let observed = state(invalid.path());
    assert_eq!(
        status(&observed, DomainId::Documentation),
        StateStatus::Blocked
    );
    assert_eq!(status(&observed, DomainId::Trama), StateStatus::Ok);
    assert!(projection_details(&observed).frozen > 0);
}

#[test]
fn trama_ausente_invalida_e_divergente_sao_estados_distintos() {
    let missing = Fixture::copy("trama-missing");
    fs::remove_file(missing.path().join("src/navigation.jsonl")).unwrap();
    let observed = state(missing.path());
    assert_eq!(status(&observed, DomainId::Trama), StateStatus::Blocked);
    assert_eq!(
        status(&observed, DomainId::Projections),
        StateStatus::Partial
    );
    assert!(projection_details(&observed)
        .items
        .iter()
        .all(|item| item.outcome == "UNKNOWN"));

    let invalid = Fixture::copy("trama-invalid");
    fs::write(invalid.path().join("src/navigation.jsonl"), "não-json\n").unwrap();
    let observed = state(invalid.path());
    assert_eq!(status(&observed, DomainId::Trama), StateStatus::Blocked);

    let drift = Fixture::copy("trama-drift");
    replace_once(
        &drift.path().join("src/project_state.rs"),
        "pub enum StateStatus {",
        "pub enum StateStatusChanged {",
    );
    let observed = state(drift.path());
    assert_eq!(status(&observed, DomainId::Trama), StateStatus::Warning);
    assert!(observed
        .pending_operations
        .iter()
        .any(|operation| operation.reason == "code_catalog_out_of_date"));
}

#[test]
fn projection_drift_harness_candidate_e_causas_nao_sao_ocultados() {
    let drift = Fixture::copy("projection-drift");
    replace_once(
        &drift
            .path()
            .join(".pinker/projections/onda-8j-anterior.toml"),
        "regions = 405",
        "regions = 406",
    );
    let observed = state(drift.path());
    assert_eq!(projection_details(&observed).verification, "DRIFT");
    assert_eq!(
        status(&observed, DomainId::Projections),
        StateStatus::Warning
    );

    let harness = Fixture::copy("projection-harness");
    replace_once(
        &harness
            .path()
            .join(".pinker/projections/onda-pink-agente-d.toml"),
        "state = \"FROZEN\"",
        "state = \"CANDIDATE\"",
    );
    let observed = state(harness.path());
    let projections = projection_details(&observed);
    assert_eq!(projections.verification, "HARNESS_FAILURE");
    assert_eq!(
        status(&observed, DomainId::Projections),
        StateStatus::Blocked
    );
    assert!(projections
        .items
        .iter()
        .any(|item| item.failure_code.as_deref() == Some("E-SNAP-CONGELADO-SOBRE-CANDIDATO")));
    assert!(observed
        .pending_operations
        .iter()
        .any(|operation| operation.reason == "projection_candidate_pending"));

    let grouped = Fixture::copy("projection-causes");
    replace_once(
        &grouped
            .path()
            .join(".pinker/projections/onda-pink-agente-d.toml"),
        "regions = 484",
        "regions = 485",
    );
    let observed = state(grouped.path());
    let projections = projection_details(&observed);
    assert_eq!(projections.verification, "HARNESS_FAILURE");
    assert!(projections
        .causes
        .iter()
        .any(|cause| cause.cause == "onda-pink-agente-d" && !cause.blocked.is_empty()));
}

fn authority_snapshot(root: &Path) -> BTreeMap<String, (u64, u64, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, out: &mut BTreeMap<String, (u64, u64, Vec<u8>)>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let metadata = fs::metadata(&path).unwrap();
                let modified = metadata
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    (metadata.len(), modified, fs::read(&path).unwrap()),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    for relative in [
        ".pinker",
        "docs",
        "src",
        "tests",
        "apps",
        "runtime/pinker_rt/src",
    ] {
        walk(root, &root.join(relative), &mut out);
    }
    out
}

#[test]
fn coleta_repetida_e_renderers_sao_somente_leitura() {
    let fixture = Fixture::copy("read-only");
    let before = authority_snapshot(fixture.path());
    for _ in 0..3 {
        let observed = state(fixture.path());
        assert!(!render_human(&observed).is_empty());
        assert!(json_is_valid(&render_json(&observed)));
    }
    assert_eq!(authority_snapshot(fixture.path()), before);
}

#[test]
fn dois_roots_absolutos_produzem_json_byte_identico() {
    let first = Fixture::copy("root-a");
    let second = Fixture::copy("root-b");
    let first_json = render_json(&state(first.path()));
    let second_json = render_json(&state(second.path()));
    assert_eq!(first_json, second_json);
    assert!(!first_json.contains(first.path().to_str().unwrap()));
    assert!(!second_json.contains(second.path().to_str().unwrap()));
}

fn pink(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .output()
        .expect("pink")
}

#[test]
fn cli_estado_cobre_help_flags_streams_e_exits() {
    for args in [
        ["help", "estado"].as_slice(),
        ["estado", "--help"].as_slice(),
        ["estado", "-h"].as_slice(),
    ] {
        let output = pink(args);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("pink estado"));
    }
    let root = env!("CARGO_MANIFEST_DIR");
    let output = pink(&["estado", "--repo", root, "--json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.ends_with('\n'));
    assert!(json_is_valid(stdout.trim_end()));
    let output = pink(&["estado", "--repo", root]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .starts_with("Pinker — OK\n"));

    for args in [
        vec!["estado", "--desconhecida"],
        vec!["estado", "--repo"],
        vec!["estado", "--repo", root, "--repo", root],
        vec!["estado", "--json", "--json"],
        vec!["estado", "inesperado"],
    ] {
        let output = pink(&args);
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }

    let output = pink(&["estado", "--repo", "/definitely/not/a/pinker/repository"]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn sensibilidade_protege_reuso_read_only_e_renderer_unico() {
    let collector = include_str!("../src/project_state.rs");
    let renderer = include_str!("../src/project_state_report.rs");
    assert!(collector.contains("RepoRoot::discover"));
    assert!(collector.contains("nav::verify_repository"));
    assert!(collector.contains("doc::verify_repository"));
    assert!(collector.contains("nav_projection_report::verify_all"));
    assert!(!collector.contains("agent::observe_status"));
    assert!(!collector.contains("Command::new"));
    assert!(!collector.contains("fs::write"));
    assert!(!collector.contains("SystemTime::now"));
    assert!(!collector.contains("http://"));
    assert!(!collector.contains("https://"));
    assert!(collector.contains("StateStatus::Unknown | StateStatus::Unavailable"));
    assert!(renderer.contains("json_string(state.overall.as_str())"));
    assert!(!renderer.contains("collect("));
    assert!(!renderer.contains("derive_overall"));
    assert!(!renderer.contains("SystemTime"));
}

// @pinker-nav:end evidencia.project-state.contrato
