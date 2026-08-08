//! Contrato processual de `pink nav localizar` (#434).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const DOC_TOML: &str = r#"schema = 1

[github]
mode = "forward-only"
baseline_pr = 330
baseline_inclusive = false
baseline_commit = "abc"

[generated]
docs_index = "docs/navigation.jsonl"
code_index = "src/navigation.jsonl"
"#;

const DEVELOPMENT_PORTAL: &str = r#"---
pinker-doc: 1
id: development
domain: development
kind: portal
status: active
parent: atlas
---

# Desenvolvimento
"#;

const SYMBOL_DOC: &str = r#"---
pinker-doc: 1
id: development.symbol-index
domain: development
kind: reference
status: active
parent: development
canonical_for:
  - development.symbol-index
---

# Índice de símbolos

<!-- @pinker-doc:start
id: development.symbol-index.contract
tags: [simbolos, navegacao]
aliases:
  - localizar simbolo
summary: Contrato explícito do índice de símbolos.
-->
## Contrato

Relações explícitas.
<!-- @pinker-doc:end development.symbol-index.contract -->
"#;

const CODE: &str = r#"// @pinker-nav:start codigo.alvo.declaracao
// @pinker-nav:domain fixture
// @pinker-nav:layer modelo
// @pinker-nav:symbol pkg_a::igual|igual|rust-type|declaration
// @pinker-nav:symbol-doc pkg_a::igual|development.symbol-index.contract
// @pinker-nav:summary Declara o primeiro homônimo.
struct Igual;
// @pinker-nav:end codigo.alvo.declaracao

// @pinker-nav:start codigo.alvo.implementacao
// @pinker-nav:domain fixture
// @pinker-nav:layer core
// @pinker-nav:symbol pkg_a::igual|igual|rust-type|implementation
// @pinker-nav:summary Implementa o primeiro homônimo em região distinta.
impl Igual { fn executar() {} }
// @pinker-nav:end codigo.alvo.implementacao

// @pinker-nav:start codigo.alvo.relacionada
// @pinker-nav:domain fixture
// @pinker-nav:layer core
// @pinker-nav:related-symbol pkg_a::igual
// @pinker-nav:summary Região explicitamente relacionada.
fn usar_igual() {}
// @pinker-nav:end codigo.alvo.relacionada

// @pinker-nav:start codigo.homonimo.b
// @pinker-nav:domain fixture
// @pinker-nav:layer modelo
// @pinker-nav:symbol pkg_b::igual|igual|rust-function|declaration
// @pinker-nav:summary Segundo homônimo sem implementação, docs ou testes explícitos.
fn igual() {}
// @pinker-nav:end codigo.homonimo.b

// @pinker-nav:start codigo.pinker.funcao
// @pinker-nav:domain fixture
// @pinker-nav:layer modelo
// @pinker-nav:symbol app::principal|principal|pinker-function|declaration
// @pinker-nav:summary Função Pinker publicada explicitamente pela fixture.
fn marcador_pinker() {}
// @pinker-nav:end codigo.pinker.funcao

// @pinker-nav:start codigo.desconhecido
// @pinker-nav:domain fixture
// @pinker-nav:layer modelo
// @pinker-nav:symbol gerado::opaco|opaco|UNKNOWN|declaration
// @pinker-nav:summary Categoria deliberadamente indisponível na autoridade.
fn opaco() {}
// @pinker-nav:end codigo.desconhecido
"#;

const EVIDENCE: &str = r#"// @pinker-nav:start evidencia.simbolo.igual
// @pinker-nav:domain fixture
// @pinker-nav:layer evidencia
// @pinker-nav:test-for pkg_a::igual
// @pinker-nav:summary Evidência explicitamente associada ao primeiro homônimo.
#[test]
fn cobre_igual() { assert_eq!(2 + 2, 4); }
// @pinker-nav:end evidencia.simbolo.igual
"#;

struct Repo(PathBuf);

impl Repo {
    fn new(label: &str) -> Repo {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pinker_symbol_{label}_{}_{}_{}",
            std::process::id(),
            nonce,
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Repo(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, path: &str, content: &str) {
    let target = root.join(path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, content).unwrap();
}

fn fixture(label: &str) -> Repo {
    let repo = Repo::new(label);
    write(repo.path(), ".pinker/doc.toml", DOC_TOML);
    write(
        repo.path(),
        "docs/development/README.md",
        DEVELOPMENT_PORTAL,
    );
    write(repo.path(), "docs/development/symbol-index.md", SYMBOL_DOC);
    write(repo.path(), "src/alvo.rs", CODE);
    write(
        repo.path(),
        "runtime/pinker_rt/src/lib.rs",
        "pub fn runtime() {}\n",
    );
    write(repo.path(), "tests/evidence.rs", EVIDENCE);
    fs::create_dir_all(repo.path().join("apps")).unwrap();
    assert_success(&run(repo.path(), &["doc", "sincronizar"]));
    assert_success(&run(repo.path(), &["nav", "sincronizar"]));
    repo
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .unwrap()
}

fn locate(root: &Path, symbol: &str, json: bool) -> Output {
    let mut args = vec!["nav", "localizar", symbol];
    if json {
        args.push("--json");
    }
    run(root, &args)
}

fn code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn assert_success(output: &Output) {
    assert_eq!(code(output), 0, "stderr={}", stderr(output));
}

fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    fn walk(root: &Path, current: &Path, out: &mut BTreeMap<String, (Vec<u8>, SystemTime)>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(
                    relative,
                    (
                        fs::read(&path).unwrap(),
                        fs::metadata(&path).unwrap().modified().unwrap(),
                    ),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// @pinker-nav:start evidencia.symbol-index.cli
// @pinker-nav:domain simbolos
// @pinker-nav:layer evidencia
// @pinker-nav:test-for pinker_v0::symbol_index::locate
// @pinker-nav:test-for pinker_v0::symbol_index::LocateReport
// @pinker-nav:summary Prova a CLI de localização, o modelo comum, homônimos, relações explícitas, UNKNOWN/UNAVAILABLE, determinismo entre roots, exits, read-only e ausência estrutural de heurística, rede, Git remoto ou subprocessos no derivador.

#[test]
fn localiza_simbolo_conhecido_e_todas_as_relacoes_explicitas() {
    let repo = fixture("known");
    let human = locate(repo.path(), "pkg_a::igual", false);
    assert_success(&human);
    assert!(human.stderr.is_empty());
    let human = stdout(&human);
    for expected in [
        "pkg_a::igual [rust-type]",
        "declaração: KNOWN",
        "implementação: KNOWN",
        "codigo.alvo.relacionada",
        "development.symbol-index.contract",
        "evidencia.simbolo.igual",
        "src/alvo.rs",
        "tests/evidence.rs",
        "autoridade",
    ] {
        assert!(human.contains(expected), "ausente {expected}: {human}");
    }

    let json = locate(repo.path(), "pkg_a::igual", true);
    assert_success(&json);
    assert!(json.stderr.is_empty());
    let json = stdout(&json);
    assert!(json.starts_with("{\"schema\":1,"));
    for expected in [
        "\"identity\":\"pkg_a::igual\"",
        "\"status\":\"KNOWN\"",
        "\"catalog\":\"src/navigation.jsonl\"",
        "\"catalog\":\"docs/navigation.jsonl\"",
        "\"region\":\"evidencia.simbolo.igual\"",
    ] {
        assert!(json.contains(expected), "ausente {expected}: {json}");
    }
}

#[test]
fn preserva_homonimos_e_unknown_sem_escolha_arbitraria() {
    let repo = fixture("homonyms");
    let json = stdout(&locate(repo.path(), "igual", true));
    let a = json.find("pkg_a::igual").unwrap();
    let b = json.find("pkg_b::igual").unwrap();
    assert!(a < b, "ordem não determinística: {json}");
    assert_eq!(json.matches("\"identity\":").count(), 2);
    assert!(json[b..].contains("\"implementation\":{\"status\":\"UNKNOWN\""));
    assert!(json[b..].contains("\"documentation\":{\"status\":\"UNKNOWN\""));
    assert!(json[b..].contains("\"tests\":{\"status\":\"UNKNOWN\""));
}

#[test]
fn cobre_cada_categoria_publicada_e_categoria_unknown() {
    let repo = fixture("kinds");
    for (symbol, kind) in [
        ("pkg_a::igual", "rust-type"),
        ("pkg_b::igual", "rust-function"),
        ("app::principal", "pinker-function"),
        ("gerado::opaco", "UNKNOWN"),
    ] {
        let output = locate(repo.path(), symbol, true);
        assert_success(&output);
        assert!(stdout(&output).contains(&format!("\"kind\":\"{kind}\"")));
    }
}

#[test]
fn inexistente_retorna_quatro_com_json_unico_e_humano_equivalente() {
    let repo = fixture("missing");
    let json = locate(repo.path(), "nao_existe", true);
    assert_eq!(code(&json), 4);
    assert!(json.stderr.is_empty());
    assert_eq!(
        stdout(&json),
        "{\"schema\":1,\"query\":\"nao_existe\",\"candidates\":[]}\n"
    );
    let human = locate(repo.path(), "nao_existe", false);
    assert_eq!(code(&human), 4);
    assert!(human.stdout.is_empty());
    assert!(stderr(&human).contains("Nenhum símbolo estruturado"));
}

#[test]
fn repeticao_e_roots_absolutos_distintos_produzem_json_byte_identico() {
    let first = fixture("root_a");
    let second = fixture("root_b");
    let a = stdout(&locate(first.path(), "igual", true));
    let repeated = stdout(&locate(first.path(), "igual", true));
    let b = stdout(&locate(second.path(), "igual", true));
    assert_eq!(a, repeated);
    assert_eq!(a, b);
    assert!(!a.contains(first.path().to_string_lossy().as_ref()));
    assert!(!a.contains(second.path().to_string_lossy().as_ref()));
}

#[test]
fn consulta_nao_escreve_nem_altera_mtime() {
    let repo = fixture("read_only");
    let before = snapshot(repo.path());
    assert_success(&locate(repo.path(), "igual", false));
    assert_success(&locate(repo.path(), "igual", true));
    assert_eq!(before, snapshot(repo.path()));
}

#[test]
fn catalogo_de_codigo_ausente_ou_invalido_retorna_tres() {
    let missing = fixture("catalog_missing");
    fs::remove_file(missing.path().join("src/navigation.jsonl")).unwrap();
    let output = locate(missing.path(), "igual", false);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("E-NAV-CATALOG"));

    let invalid = fixture("catalog_invalid");
    write(invalid.path(), "src/navigation.jsonl", "{invalid\n");
    let output = locate(invalid.path(), "igual", false);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("E-NAV-CATALOG"));

    let wrong_type = fixture("catalog_wrong_type");
    let path = wrong_type.path().join("src/navigation.jsonl");
    let catalog = fs::read_to_string(&path).unwrap();
    let corrupted = catalog.replace(
        "\"related_symbols\":[\"pkg_a::igual\"]",
        "\"related_symbols\":\"pkg_a::igual\"",
    );
    assert_ne!(catalog, corrupted);
    fs::write(path, corrupted).unwrap();
    let output = locate(wrong_type.path(), "igual", true);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("deve ser lista de strings"));

    let duplicate = fixture("catalog_duplicate");
    let path = duplicate.path().join("src/navigation.jsonl");
    let catalog = fs::read_to_string(&path).unwrap();
    let corrupted = catalog.replace(
        "\"related_symbols\":[\"pkg_a::igual\"]",
        "\"related_symbols\":[\"pkg_a::igual\",\"pkg_a::igual\"]",
    );
    assert_ne!(catalog, corrupted);
    fs::write(path, corrupted).unwrap();
    let output = locate(duplicate.path(), "igual", true);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("metadado de símbolo duplicado"));

    let dangling = fixture("catalog_dangling");
    let path = dangling.path().join("src/navigation.jsonl");
    let catalog = fs::read_to_string(&path).unwrap();
    let corrupted = catalog.replace(
        "\"related_symbols\":[\"pkg_a::igual\"]",
        "\"related_symbols\":[\"pkg::inexistente\"]",
    );
    assert_ne!(catalog, corrupted);
    fs::write(path, corrupted).unwrap();
    let output = locate(dangling.path(), "igual", true);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("referencia símbolo inexistente"));
}

#[test]
fn autoridade_documental_ausente_e_unavailable_invalida_e_erro_estrutural() {
    let missing = fixture("docs_missing");
    fs::remove_file(missing.path().join("docs/navigation.jsonl")).unwrap();
    let output = locate(missing.path(), "pkg_a::igual", true);
    assert_success(&output);
    assert!(stdout(&output).contains("\"documentation\":{\"status\":\"UNAVAILABLE\""));

    let invalid = fixture("docs_invalid");
    write(invalid.path(), "docs/navigation.jsonl", "{invalid\n");
    let output = locate(invalid.path(), "pkg_a::igual", true);
    assert_eq!(code(&output), 3);
    assert!(stderr(&output).contains("E-DOC-CATALOG"));
}

#[test]
fn metadados_rejeitam_destino_inexistente_duplicidade_e_teste_fabricado() {
    let missing = fixture("target_missing");
    let broken = CODE.replace(
        "// @pinker-nav:related-symbol pkg_a::igual",
        "// @pinker-nav:related-symbol pkg::inexistente",
    );
    let catalog_before = fs::read(missing.path().join("src/navigation.jsonl")).unwrap();
    write(missing.path(), "src/alvo.rs", &broken);
    let sync = run(missing.path(), &["nav", "sincronizar"]);
    assert_eq!(code(&sync), 5);
    assert!(stderr(&sync).contains("referencia símbolo inexistente"));
    assert_eq!(
        catalog_before,
        fs::read(missing.path().join("src/navigation.jsonl")).unwrap()
    );
    let verify = run(missing.path(), &["nav", "verificar"]);
    assert_eq!(code(&verify), 5);
    assert!(stderr(&verify).contains("referencia símbolo inexistente"));

    let duplicate = Repo::new("duplicate");
    write(duplicate.path(), ".pinker/doc.toml", DOC_TOML);
    write(
        duplicate.path(),
        "src/alvo.rs",
        &CODE.replace(
            "// @pinker-nav:symbol pkg_a::igual|igual|rust-type|declaration",
            "// @pinker-nav:symbol pkg_a::igual|igual|rust-type|declaration\n// @pinker-nav:symbol pkg_a::igual|igual|rust-type|declaration",
        ),
    );
    write(
        duplicate.path(),
        "runtime/pinker_rt/src/lib.rs",
        "pub fn runtime() {}\n",
    );
    write(duplicate.path(), "tests/evidence.rs", EVIDENCE);
    fs::create_dir_all(duplicate.path().join("apps")).unwrap();
    let output = run(duplicate.path(), &["nav", "sincronizar"]);
    assert_eq!(code(&output), 5);
    assert!(stderr(&output).contains("metadado malformado"));

    let fabricated = fixture("fabricated");
    write(
        fabricated.path(),
        "tests/evidence.rs",
        &EVIDENCE.replace("layer evidencia", "layer core"),
    );
    let sync = run(fabricated.path(), &["nav", "sincronizar"]);
    assert_eq!(code(&sync), 5);
    assert!(stderr(&sync).contains("exige layer evidencia"));
    let verify = run(fabricated.path(), &["nav", "verificar"]);
    assert_eq!(code(&verify), 5);
    assert!(stderr(&verify).contains("exige layer evidencia"));
}

#[test]
fn remover_vinculo_explicito_remove_relacao_sem_promover_inferencia() {
    let repo = fixture("sensitivity");
    let before = stdout(&locate(repo.path(), "pkg_a::igual", true));
    assert!(before.contains("\"tests\":{\"status\":\"KNOWN\""));
    write(
        repo.path(),
        "tests/evidence.rs",
        &EVIDENCE.replace("// @pinker-nav:test-for pkg_a::igual\n", ""),
    );
    assert_success(&run(repo.path(), &["nav", "sincronizar"]));
    let after = stdout(&locate(repo.path(), "pkg_a::igual", true));
    assert!(after.contains("\"tests\":{\"status\":\"UNKNOWN\""));
    assert!(!after.contains("evidencia.simbolo.igual"));
}

#[test]
fn ajuda_e_uso_publicam_apenas_flags_pertinentes() {
    for args in [
        vec!["help", "nav"],
        vec!["nav", "--help"],
        vec!["nav", "-h"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(args)
            .output()
            .unwrap();
        assert_success(&output);
        assert!(stdout(&output).contains("localizar SÍMBOLO"));
    }
    let repo = fixture("usage");
    let output = run(repo.path(), &["nav", "localizar", "igual", "--limite", "1"]);
    assert_eq!(code(&output), 2);
    assert!(stderr(&output).contains("não pertence a nav localizar"));
}

#[test]
fn derivador_nao_contem_io_rede_git_subprocesso_busca_ou_normalizacao() {
    let source = include_str!("../src/symbol_index.rs");
    let start = source.find("pub fn locate(").unwrap();
    let end = source[start..].find("fn missing_target(").unwrap() + start;
    let body = &source[start..end];
    for forbidden in [
        "std::fs",
        "Command::new",
        "TcpStream",
        "UdpSocket",
        "git ",
        "github",
        ".search(",
        "text_norm",
        "contains(query)",
    ] {
        assert!(!body.contains(forbidden), "derivador contém {forbidden}");
    }
    assert!(body.contains("candidate.identity == query || candidate.name == query"));

    let main = include_str!("../src/main.rs");
    let start = main.find("fn run_nav_localizar(").unwrap();
    let end = main[start..].find("fn run_nav_listar(").unwrap() + start;
    let adapter = &main[start..end];
    for forbidden in [
        "sincronizar",
        "write_atomic",
        "Command::new",
        "git ",
        "github",
        "automation::apply",
    ] {
        assert!(!adapter.contains(forbidden), "adaptador contém {forbidden}");
    }
}
// @pinker-nav:end evidencia.symbol-index.cli
