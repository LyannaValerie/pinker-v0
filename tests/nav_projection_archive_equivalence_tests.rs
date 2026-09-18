//! TA/#697 — equivalence checkpoint between the reconstruction mechanism and
//! the materialized historical archive.
//!
//! POT/LPT: INVARIANT archive payload bytes = stable_projection(accepted reconstruction)
//! POT/LPT: MUST NOT retire reconstruction before this file proves 13/13
//! POT/LPT: AUTHORITY #697 §4
//!
//! This file is transitional evidence. It exists only while both mechanisms
//! coexist; the cutover commit that removes reconstruction removes it too, and
//! the permanent archive properties live in the archive test file.

use pinker_v0::nav::CodeCatalog;
use pinker_v0::nav_projection_recipe::resolve;
use pinker_v0::nav_projection_snapshot::{fnv1a64_canonical, stable_projection, SnapshotState};
use pinker_v0::nav_projection_store::ProjectionStore;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const ARCHIVE_DIR: &str = ".pinker/archive";
const INDEX_PATH: &str = ".pinker/archive/index.toml";
const ARCHIVE_SCHEMA: u64 = 1;
const PROVENANCE: &str = "materialized bytes are the accepted reconstructed historical projection exported at cutover and are not claimed to have been captured contemporaneously at the original historical date";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sha256(bytes: &[u8]) -> String {
    pinker_sha256_contract::sha256_hex(bytes)
}

/// The accepted reconstruction of every FROZEN snapshot, as exact stable bytes.
///
/// Refuses to produce anything for a snapshot whose reconstruction does not
/// reproduce its own preserved measures: an archive exported from an unverified
/// reconstruction would preserve a wrong fact forever.
fn reconstructed(root: &Path) -> BTreeMap<String, String> {
    let store = ProjectionStore::load(root).expect("projection authority readable");
    assert!(
        store.errors().is_empty(),
        "projection authority has invalid artifacts: {:?}",
        store.errors()
    );
    let library = store.library().expect("projection library resolvable");
    let catalog = CodeCatalog::load(&root.join("src/navigation.jsonl")).expect("current catalog");

    let mut out = BTreeMap::new();
    for stored in store.snapshots() {
        let snapshot = &stored.snapshot;
        assert_eq!(
            snapshot.state,
            SnapshotState::Frozen,
            "snapshot '{}' is not FROZEN; only accepted history may be archived",
            snapshot.id
        );
        let composition = resolve(&library, &snapshot.id, &catalog.regions)
            .unwrap_or_else(|failure| panic!("snapshot '{}': {}", snapshot.id, failure));
        let measures = composition.measures();
        assert_eq!(
            (measures.regions, measures.length, measures.fnv1a64),
            (
                snapshot.measures.regions,
                snapshot.measures.length,
                snapshot.measures.fnv1a64
            ),
            "snapshot '{}' does not reproduce its own preserved measures",
            snapshot.id
        );
        out.insert(
            snapshot.id.clone(),
            stable_projection(composition.regions.iter()),
        );
    }
    out
}

fn toml_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// One-shot materialization of the archive. Run explicitly:
///
/// ```text
/// cargo test --test nav_projection_archive_equivalence_tests -- --ignored --exact export_historical_archive
/// ```
#[test]
#[ignore = "one-shot export; the checkpoint property is the equivalence test"]
fn export_historical_archive() {
    let root = repo();
    let projections = reconstructed(&root);
    let store = ProjectionStore::load(&root).expect("projection authority readable");
    fs::create_dir_all(root.join(ARCHIVE_DIR)).expect("archive directory");

    let mut index = String::new();
    index.push_str(&format!("schema = {ARCHIVE_SCHEMA}\n"));
    index.push_str(&format!(
        "export_source_main = {}\n",
        toml_escape("57e7c5999fe1105d72615f8940847d097397a296")
    ));
    index.push_str(&format!(
        "export_source_tree = {}\n",
        toml_escape("6d11bc40696c790d5e41d94aab45e3a10b10e64d")
    ));
    index.push_str(&format!(
        "export_method = {}\n",
        toml_escape("stable_projection(resolve(frozen snapshot, current catalog)) verified MATCH before export")
    ));
    index.push_str(&format!("provenance = {}\n", toml_escape(PROVENANCE)));

    for (id, payload) in &projections {
        let stored = store.snapshot(id).expect("stored snapshot");
        let payload_rel = format!("{ARCHIVE_DIR}/{id}.stable");
        fs::write(root.join(&payload_rel), payload.as_bytes()).expect("payload written");
        let metadata_bytes = fs::read(root.join(&stored.path)).expect("snapshot metadata");
        index.push_str("\n[[entries]]\n");
        index.push_str(&format!("id = {}\n", toml_escape(id)));
        index.push_str(&format!("metadata_path = {}\n", toml_escape(&stored.path)));
        index.push_str(&format!(
            "metadata_sha256 = {}\n",
            toml_escape(&sha256(&metadata_bytes))
        ));
        index.push_str(&format!("payload_path = {}\n", toml_escape(&payload_rel)));
        index.push_str(&format!("regions = {}\n", payload.lines().count()));
        index.push_str(&format!("length = {}\n", payload.len()));
        index.push_str(&format!(
            "fnv1a64 = {}\n",
            toml_escape(&fnv1a64_canonical(payload.as_bytes()))
        ));
        index.push_str(&format!(
            "sha256 = {}\n",
            toml_escape(&sha256(payload.as_bytes()))
        ));
    }
    fs::write(root.join(INDEX_PATH), index.as_bytes()).expect("index written");
}

/// Minimal reader for the checkpoint: the archive index as declared fields per
/// entry. The permanent parser belongs to the product and arrives with cutover.
fn read_index(root: &Path) -> (BTreeMap<String, String>, Vec<BTreeMap<String, String>>) {
    let text = fs::read_to_string(root.join(INDEX_PATH)).expect("archive index present");
    let mut header = BTreeMap::new();
    let mut entries: Vec<BTreeMap<String, String>> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[entries]]" {
            entries.push(BTreeMap::new());
            continue;
        }
        let (key, raw) = line.split_once(" = ").expect("declared pair");
        let value = raw
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .map(|text| text.replace("\\\"", "\"").replace("\\\\", "\\"))
            .unwrap_or_else(|| raw.to_string());
        match entries.last_mut() {
            Some(entry) => {
                entry.insert(key.to_string(), value);
            }
            None => {
                header.insert(key.to_string(), value);
            }
        }
    }
    (header, entries)
}

#[test]
fn archive_payload_equals_accepted_reconstruction() {
    let root = repo();
    let projections = reconstructed(&root);
    let (_, entries) = read_index(&root);

    assert_eq!(
        projections.len(),
        13,
        "the accepted historical authority is 13 snapshots"
    );
    assert_eq!(
        entries.len(),
        projections.len(),
        "every accepted snapshot has exactly one archive entry"
    );

    for entry in &entries {
        let id = entry.get("id").expect("entry id");
        let expected = projections
            .get(id)
            .unwrap_or_else(|| panic!("archive entry '{id}' has no accepted reconstruction"));
        let payload_path = entry.get("payload_path").expect("payload path");
        let payload = fs::read(root.join(payload_path))
            .unwrap_or_else(|_| panic!("archive payload missing for '{id}'"));

        assert_eq!(
            payload,
            expected.as_bytes(),
            "archive payload for '{id}' is not the exact stable projection of the accepted reconstruction"
        );
    }
}

#[test]
fn archive_index_preserves_historical_measures() {
    let root = repo();
    let store = ProjectionStore::load(&root).expect("projection authority readable");
    let (header, entries) = read_index(&root);

    assert_eq!(header.get("schema").map(String::as_str), Some("1"));
    assert_eq!(
        header.get("export_source_main").map(String::as_str),
        Some("57e7c5999fe1105d72615f8940847d097397a296")
    );
    assert_eq!(
        header.get("export_source_tree").map(String::as_str),
        Some("6d11bc40696c790d5e41d94aab45e3a10b10e64d")
    );
    assert_eq!(
        header.get("provenance").map(String::as_str),
        Some(PROVENANCE)
    );

    for entry in &entries {
        let id = entry.get("id").expect("entry id");
        let stored = store
            .snapshot(id)
            .unwrap_or_else(|| panic!("archive entry '{id}' has no snapshot metadata"));
        let measures = &stored.snapshot.measures;

        assert_eq!(
            entry.get("regions").map(String::as_str),
            Some(measures.regions.to_string().as_str()),
            "regions recalibrated for '{id}'"
        );
        assert_eq!(
            entry.get("length").map(String::as_str),
            Some(measures.length.to_string().as_str()),
            "length recalibrated for '{id}'"
        );
        assert_eq!(
            entry.get("fnv1a64").map(String::as_str),
            Some(measures.fnv1a64_canonical().as_str()),
            "fnv1a64 recalibrated for '{id}'"
        );
        assert_eq!(
            entry.get("metadata_path").map(String::as_str),
            Some(stored.path.as_str())
        );
        assert_eq!(
            entry.get("metadata_sha256").map(String::as_str),
            Some(sha256(&stored.bytes).as_str()),
            "FROZEN metadata bytes changed for '{id}'"
        );

        let payload = fs::read(root.join(entry.get("payload_path").expect("payload path")))
            .unwrap_or_else(|_| panic!("archive payload missing for '{id}'"));
        assert_eq!(
            payload.len() as u64,
            measures.length,
            "payload length for '{id}' does not equal the preserved length"
        );
        assert_eq!(
            payload.iter().filter(|byte| **byte == b'\n').count() as u64,
            measures.regions,
            "payload record count for '{id}' does not equal the preserved regions"
        );
        assert_eq!(
            fnv1a64_canonical(&payload),
            measures.fnv1a64_canonical(),
            "payload FNV-1a64 for '{id}' does not equal the preserved measure"
        );
        assert_eq!(
            entry.get("sha256").map(String::as_str),
            Some(sha256(&payload).as_str()),
            "archive SHA-256 does not match the payload for '{id}'"
        );
    }
}
