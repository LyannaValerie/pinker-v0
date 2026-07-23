//! Trama Pinker — consultas por catálogo, contratos CLI e códigos de saída.
//!
//! Cobre (§20): consulta lendo JSONL sem revarrer Markdown/Rust (1, 2),
//! `mostrar` detectando âncora/hash divergente (3), saída JSON estável (4),
//! limites de resultados (7), códigos de saída (8), catálogo ausente (9),
//! catálogo inválido (10) e ausência de resultados (11).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.trama.query.fixture-config
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Agrupa a configuração documental, os documentos sintéticos e a fonte Rust marcada usados pelas consultas de documento e código.
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

const PORTAL: &str = "---\npinker-doc: 1\nid: rosa\ndomain: rosa\nkind: portal\nstatus: active\nparent: atlas\n---\n\n# Rosa\n\nPortal.\n";

const CORE: &str = "---\npinker-doc: 1\nid: rosa.core\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# Core\n\n<!-- @pinker-doc:start\nid: rosa.identity\ntags: [rosa, identidade]\naliases:\n  - quem e rosa\nsummary: Identidade de Rosa.\n-->\n## Identidade\n\nRosa e a guia estetica.\n<!-- @pinker-doc:end rosa.identity -->\n";

const SRC: &str = "// @pinker-nav:start rosa.identidade.core\n// @pinker-nav:domain rosa\n// @pinker-nav:layer core\n// @pinker-nav:summary Consulta compartilhada de identidade no codigo.\nfn identidade() {\n    let _x = 1;\n}\n// @pinker-nav:end rosa.identidade.core\n// @pinker-nav:start alfa.execucao.cli\n// @pinker-nav:domain engine\n// @pinker-nav:layer cli\n// @pinker-nav:summary Consulta compartilhada de execucao no codigo.\nfn executar() {\n    let _y = 2;\n}\n// @pinker-nav:end alfa.execucao.cli\n";

const TEST_SRC: &str = "// @pinker-nav:start beta.consulta.evidencia\n// @pinker-nav:domain trama\n// @pinker-nav:layer evidencia\n// @pinker-nav:summary Consulta compartilhada como evidencia.\nfn observar() {\n    let _z = 3;\n}\n// @pinker-nav:end beta.consulta.evidencia\n";
// @pinker-nav:end evidencia.trama.query.fixture-config

// @pinker-nav:start evidencia.trama.query.process-support
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Fornece repositório temporário, escrita de arquivos, montagem da fixture, execução de pink doc e pink nav e extração de códigos de saída.
fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_q_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fixture(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
    write(root, "docs/rosa/README.md", PORTAL);
    write(root, "docs/rosa/core.md", CORE);
    write(root, "src/rosa.rs", SRC);
    write(root, "runtime/pinker_rt/src/lib.rs", "pub fn _rt() {}\n");
    write(root, "tests/mapa.rs", TEST_SRC);
}

fn doc(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("doc")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

fn nav(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("nav")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink")
}

fn code(out: &std::process::Output) -> i32 {
    out.status.code().unwrap_or(-1)
}
// @pinker-nav:end evidencia.trama.query.process-support

// @pinker-nav:start evidencia.trama.query.nav-map
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa pink nav mapa agrupando o catálogo por arquivo, preservando seleções ambíguas, ordem determinística, JSON schema 1, códigos de saída e leitura exclusiva do catálogo.
fn stdout(out: &std::process::Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

fn sync_nav(root: &Path) {
    let out = nav(root, &["sincronizar"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
}

fn json_is_valid(input: &str) -> bool {
    fn ws(bytes: &[u8], pos: &mut usize) {
        while bytes
            .get(*pos)
            .is_some_and(|c| matches!(c, b' ' | b'\n' | b'\r' | b'\t'))
        {
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
                            let Some(hex) = bytes.get(*pos) else {
                                return false;
                            };
                            if !hex.is_ascii_hexdigit() {
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
    let valid = value(bytes, &mut pos);
    ws(bytes, &mut pos);
    valid && pos == bytes.len()
}

#[test]
fn nav_mapa_completo_e_agrupado() {
    let root = temp_repo("mapa_completo");
    fixture(&root);
    sync_nav(&root);
    let out = nav(&root, &["mapa"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("src/rosa.rs\n"));
    assert!(text.contains("tests/mapa.rs\n"));
    assert!(text.contains("  regiões: 2"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_seleciona_caminho_repo_relativo_exato() {
    let root = temp_repo("mapa_exato");
    fixture(&root);
    sync_nav(&root);
    let out = nav(&root, &["mapa", "src/rosa.rs"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("rosa.identidade.core"));
    assert!(text.contains("alfa.execucao.cli"));
    assert!(!text.contains("tests/mapa.rs"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_faz_fallback_para_consulta_textual() {
    let root = temp_repo("mapa_texto");
    fixture(&root);
    sync_nav(&root);
    let out = nav(&root, &["mapa", "execucao"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("alfa.execucao.cli"));
    assert!(!text.contains("rosa.identidade.core"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_preserva_multiplos_resultados_textuais() {
    let root = temp_repo("mapa_ambiguo");
    fixture(&root);
    sync_nav(&root);
    let out = nav(&root, &["mapa", "consulta", "compartilhada"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("rosa.identidade.core"));
    assert!(text.contains("alfa.execucao.cli"));
    assert!(text.contains("beta.consulta.evidencia"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_sem_resultado_retorna_quatro() {
    let root = temp_repo("mapa_vazio");
    fixture(&root);
    sync_nav(&root);
    let out = nav(&root, &["mapa", "zzzznaoexistemesmo"]);
    assert_eq!(code(&out), 4);
    assert!(out.stdout.is_empty());
    assert!(stderr(&out).contains("Nenhuma região"));
    let json = nav(&root, &["mapa", "zzzznaoexistemesmo", "--json"]);
    assert_eq!(code(&json), 4);
    assert!(stderr(&json).is_empty());
    assert_eq!(
        stdout(&json),
        "{\"schema\":1,\"filter\":\"zzzznaoexistemesmo\",\"files\":[]}\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_ordena_arquivos_por_caminho() {
    let root = temp_repo("mapa_ordem_arquivos");
    fixture(&root);
    sync_nav(&root);
    let text = stdout(&nav(&root, &["mapa"]));
    assert!(text.find("src/rosa.rs\n").unwrap() < text.find("tests/mapa.rs\n").unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_ordena_regioes_pelo_intervalo_fisico() {
    let root = temp_repo("mapa_ordem_regioes");
    fixture(&root);
    sync_nav(&root);
    let text = stdout(&nav(&root, &["mapa", "src/rosa.rs"]));
    assert!(text.find("rosa.identidade.core").unwrap() < text.find("alfa.execucao.cli").unwrap());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_json_schema_um_e_valido() {
    let root = temp_repo("mapa_json_valido");
    fixture(&root);
    sync_nav(&root);
    let out = nav(&root, &["mapa", "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(json_is_valid(text.trim()));
    assert!(text.starts_with("{\"schema\":1,\"filter\":null,\"files\":["));
    assert!(text.contains("\"sections\":["));
    assert!(text.contains("\"path\":\"src/rosa.rs\""));
    assert!(text.contains("\"path\":\"tests/mapa.rs\""));
    assert!(!text.contains("\"absolute_path\""));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_json_e_byte_identical() {
    let first_root = temp_repo("mapa_json_estavel_primeiro_root");
    let second_root = temp_repo("mapa_json_estavel_segundo_root");
    fixture(&first_root);
    fixture(&second_root);
    sync_nav(&first_root);
    sync_nav(&second_root);
    let first = nav(
        &first_root,
        &["mapa", "consulta", "compartilhada", "--json"],
    );
    let second = nav(
        &second_root,
        &["mapa", "consulta", "compartilhada", "--json"],
    );
    assert_eq!(code(&first), 0);
    assert_eq!(code(&second), 0);
    assert_eq!(first.stdout, second.stdout);
    let text = stdout(&first);
    assert!(!text.contains(first_root.to_string_lossy().as_ref()));
    assert!(!text.contains(second_root.to_string_lossy().as_ref()));
    fs::remove_dir_all(first_root).unwrap();
    fs::remove_dir_all(second_root).unwrap();
}

#[test]
fn nav_mapa_resume_multiplos_dominios() {
    let root = temp_repo("mapa_dominios");
    fixture(&root);
    sync_nav(&root);
    let text = stdout(&nav(&root, &["mapa", "src/rosa.rs"]));
    assert!(text.contains("  domínios: engine, rosa"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_resume_multiplas_camadas() {
    let root = temp_repo("mapa_camadas");
    fixture(&root);
    sync_nav(&root);
    let text = stdout(&nav(&root, &["mapa", "src/rosa.rs"]));
    assert!(text.contains("  camadas: cli, core"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_catalogo_ausente_retorna_tres() {
    let root = temp_repo("mapa_catalogo_ausente");
    fixture(&root);
    let out = nav(&root, &["mapa"]);
    assert_eq!(code(&out), 3);
    assert!(stderr(&out).contains("E-NAV-CATALOG"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_catalogo_invalido_retorna_tres() {
    let root = temp_repo("mapa_catalogo_invalido");
    fixture(&root);
    write(&root, "src/navigation.jsonl", "{isto nao e json valido\n");
    let out = nav(&root, &["mapa"]);
    assert_eq!(code(&out), 3);
    assert!(stderr(&out).contains("E-NAV-CATALOG"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_help_inclui_mapa() {
    let root = temp_repo("mapa_help");
    fixture(&root);
    let out = nav(&root, &["--help"]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("mapa [filtro]"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_flag_desconhecida_retorna_dois() {
    let root = temp_repo("mapa_flag");
    fixture(&root);
    let out = nav(&root, &["mapa", "--desconhecida"]);
    assert_eq!(code(&out), 2);
    assert!(stderr(&out).contains("Flag desconhecida"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_le_somente_catalogo_sem_fontes() {
    let root = temp_repo("mapa_catalog_only");
    fixture(&root);
    sync_nav(&root);
    fs::remove_file(root.join("src/rosa.rs")).unwrap();
    fs::remove_file(root.join("tests/mapa.rs")).unwrap();
    let out = nav(&root, &["mapa"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("beta.consulta.evidencia"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_deriva_caminho_absoluto_do_root() {
    let root = temp_repo("mapa_absoluto");
    fixture(&root);
    sync_nav(&root);
    let text = stdout(&nav(&root, &["mapa", "src/rosa.rs"]));
    assert!(text.contains(&format!(
        "  absoluto: {}",
        root.canonicalize().unwrap().join("src/rosa.rs").display()
    )));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mapa_nao_modifica_arquivos() {
    let root = temp_repo("mapa_read_only");
    fixture(&root);
    sync_nav(&root);
    let watched = [
        ".pinker/doc.toml",
        "src/navigation.jsonl",
        "src/rosa.rs",
        "tests/mapa.rs",
    ];
    let before: Vec<Vec<u8>> = watched
        .iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .collect();
    let out = nav(&root, &["mapa", "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let after: Vec<Vec<u8>> = watched
        .iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .collect();
    assert_eq!(before, after);
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.nav-map

// @pinker-nav:start evidencia.trama.query.catalog-only
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa consultas documentais e de código continuando a localizar resultados pelo JSONL após a remoção das fontes Markdown e Rust usadas na fixture.
#[test]
fn consulta_documental_le_catalogo_sem_revarrer_markdown() {
    let root = temp_repo("doc_catalog_only");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());

    // Remove as fontes Markdown, preservando apenas o catálogo JSONL.
    fs::remove_file(root.join("docs/rosa/README.md")).unwrap();
    fs::remove_file(root.join("docs/rosa/core.md")).unwrap();

    // `buscar` continua respondendo — prova de que lê o catálogo, não a árvore.
    let out = doc(&root, &["buscar", "identidade"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("rosa.identity"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn consulta_de_codigo_le_catalogo_sem_revarrer_rust() {
    let root = temp_repo("nav_catalog_only");
    fixture(&root);
    assert!(nav(&root, &["sincronizar"]).status.success());

    fs::remove_file(root.join("src/rosa.rs")).unwrap();

    let out = nav(&root, &["buscar", "identidade"]);
    assert_eq!(code(&out), 0, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("rosa.identidade.core"));

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.catalog-only

// @pinker-nav:start evidencia.trama.query.source-drift
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa pink doc mostrar detectando deriva de âncora e pink nav mostrar detectando divergência de hash com código de saída de fonte.
#[test]
fn mostrar_detecta_ancora_divergente() {
    let root = temp_repo("doc_drift");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());

    // Desloca o conteúdo inserindo linhas antes da âncora: o intervalo
    // registrado deixa de ser delimitado pela âncora esperada.
    let drifted = format!("linha nova\noutra linha\n{CORE}");
    write(&root, "docs/rosa/core.md", &drifted);

    let out = doc(&root, &["mostrar", "rosa.identity"]);
    assert_eq!(code(&out), 5, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-SOURCE"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nav_mostrar_detecta_hash_divergente() {
    let root = temp_repo("nav_drift");
    fixture(&root);
    assert!(nav(&root, &["sincronizar"]).status.success());

    // Altera o conteúdo da região mantendo os marcadores: o hash diverge.
    let drifted = SRC.replace("let _x = 1;", "let _x = 999;");
    write(&root, "src/rosa.rs", &drifted);

    let out = nav(&root, &["mostrar", "rosa.identidade.core"]);
    assert_eq!(code(&out), 5, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("Hash divergente"));

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.source-drift

// @pinker-nav:start evidencia.trama.query.json-stability
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa a rota documental em JSON produzindo saída repetível com os campos consultados pela suíte e o resultado rosa.identity.
#[test]
fn saida_json_e_valida_e_estavel() {
    let root = temp_repo("json");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());

    let a = doc(&root, &["rota", "quem e rosa", "--json"]);
    let b = doc(&root, &["rota", "quem e rosa", "--json"]);
    assert_eq!(code(&a), 0);
    let sa = String::from_utf8_lossy(&a.stdout);
    let sb = String::from_utf8_lossy(&b.stdout);
    assert_eq!(sa, sb, "saída JSON deve ser estável");
    assert!(sa.contains("\"query\""));
    assert!(sa.contains("\"normalized\""));
    assert!(sa.contains("\"results\""));
    assert!(sa.contains("\"next\""));
    assert!(sa.contains("\"score\""));
    assert!(sa.contains("rosa.identity"));

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.json-stability

// @pinker-nav:start evidencia.trama.query.result-limit
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa pink doc buscar respeitando limite explícito de dois resultados e aceitando limite acima do máximo por clamp.
#[test]
fn limite_de_resultados_respeita_contornos() {
    let root = temp_repo("limite");
    fixture(&root);
    // Vários documentos com seções distintas casando o mesmo termo.
    for i in 0..6 {
        let content = format!(
            "---\npinker-doc: 1\nid: rosa.d{i}\ndomain: rosa\nkind: reference\nstatus: active\nparent: rosa\n---\n\n# D{i}\n\n<!-- @pinker-doc:start\nid: rosa.item{i}\ntags: [comum]\nsummary: Item comum {i}.\n-->\n## Item {i}\ncomum\n<!-- @pinker-doc:end rosa.item{i} -->\n"
        );
        write(&root, &format!("docs/rosa/d{i}.md"), &content);
    }
    assert!(doc(&root, &["sincronizar"]).status.success());

    let out = doc(&root, &["buscar", "comum", "--limite", "2"]);
    assert_eq!(code(&out), 0);
    let n = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("rosa.item"))
        .count();
    assert_eq!(n, 2, "limite deve cortar em 2");

    // Limite acima do máximo é fixado em 20 (não erro).
    let out_max = doc(&root, &["buscar", "comum", "--limite", "999"]);
    assert_eq!(code(&out_max), 0);

    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.result-limit

// @pinker-nav:start evidencia.trama.query.catalog-errors
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa consultas documentais rejeitando catálogo ausente e catálogo JSONL inválido com código de saída 3 e diagnóstico E-DOC-CATALOG.
#[test]
fn catalogo_ausente_falha_com_codigo_3() {
    let root = temp_repo("missing_catalog");
    fixture(&root);
    // Não sincroniza: o catálogo não existe.
    let out = doc(&root, &["mostrar", "rosa.identity"]);
    assert_eq!(code(&out), 3, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-CATALOG"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn catalogo_invalido_falha_com_codigo_3() {
    let root = temp_repo("invalid_catalog");
    fixture(&root);
    write(&root, "docs/navigation.jsonl", "{isto nao e json valido\n");
    let out = doc(&root, &["buscar", "rosa"]);
    assert_eq!(code(&out), 3, "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("E-DOC-CATALOG"));
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.catalog-errors

// @pinker-nav:start evidencia.trama.query.query-exit-codes
// @pinker-nav:domain trama
// @pinker-nav:layer evidencia
// @pinker-nav:summary Observa consulta sem resultado retornando código 4 e uso de subcomando inválido retornando código 2.
#[test]
fn ausencia_de_resultados_nao_e_sucesso_silencioso() {
    let root = temp_repo("noresult");
    fixture(&root);
    assert!(doc(&root, &["sincronizar"]).status.success());
    let out = doc(&root, &["buscar", "zzzznaoexistemesmo"]);
    assert_eq!(code(&out), 4, "consulta sem resultado deve sair 4");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn uso_invalido_sai_com_codigo_2() {
    let root = temp_repo("usage");
    fixture(&root);
    // Subcomando inexistente.
    let out = doc(&root, &["subcomando-inexistente"]);
    assert_eq!(code(&out), 2);
    fs::remove_dir_all(root).unwrap();
}
// @pinker-nav:end evidencia.trama.query.query-exit-codes
