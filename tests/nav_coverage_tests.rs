//! Controles da cobertura corrente da cartografia (T1 — #675).
//!
//! Os controles positivos e negativos atingem o consumidor real: o binário
//! `pink`, pelos mesmos subcomandos que `make ci` executa. Os controles de
//! lifecycle base->candidato atingem `diff_coverage::analyze`, a autoridade
//! que relaciona os dois estados, com catálogos de base explícitos.
//!
//! Nenhuma expectativa deste arquivo é derivada da decisão sob teste: os
//! códigos de erro, as completudes e as disposições estão escritos literais.

use pinker_v0::diff_coverage::{
    analyze, Completeness, CoverageAuthorities, RelationStatus, DIFF_COVERAGE_SCHEMA,
};
use pinker_v0::nav::CodeCatalog;
use pinker_v0::nav_coverage::CoveragePolicy;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
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

const POLICY: &str = concat!(
    r#"{"schema":1,"kind":"scope","root":"src","category":"production","file_enforcement":"required"}"#,
    "\n",
    r#"{"schema":1,"kind":"scope","root":"runtime/pinker_rt/src","category":"production","file_enforcement":"required"}"#,
    "\n",
    r#"{"schema":1,"kind":"scope","root":"tests","category":"evidence","file_enforcement":"inventory"}"#,
    "\n",
    r#"{"schema":1,"kind":"scope","root":"apps","category":"example","file_enforcement":"inventory"}"#,
    "\n",
);

/// Arquivo de produção inteiramente coberto por uma única região coerente.
const COBERTO: &str = "// @pinker-nav:start fixture.alvo.coberto\n\
                       // @pinker-nav:domain fixture\n\
                       // @pinker-nav:layer core\n\
                       // @pinker-nav:summary Responsabilidade coerente do alvo coberto.\n\
                       pub fn alvo() -> i32 {\n\
                       \x20   1\n\
                       }\n\
                       // @pinker-nav:end fixture.alvo.coberto\n";

/// Arquivo de produção com região e código relevante FORA dela.
const PARCIAL: &str = "// @pinker-nav:start fixture.alvo.parcial\n\
                       // @pinker-nav:domain fixture\n\
                       // @pinker-nav:layer core\n\
                       // @pinker-nav:summary Apenas a primeira responsabilidade do arquivo parcial.\n\
                       pub fn coberta() -> i32 {\n\
                       \x20   1\n\
                       }\n\
                       // @pinker-nav:end fixture.alvo.parcial\n\
                       \n\
                       pub fn descoberta() -> i32 {\n\
                       \x20   2\n\
                       }\n";

/// Arquivo de produção sem nenhum marcador.
const SEM_ANCORA: &str = "pub fn invisivel() -> i32 {\n    3\n}\n";

const RUNTIME: &str = "// @pinker-nav:start fixture.runtime.superficie\n\
                       // @pinker-nav:domain fixture\n\
                       // @pinker-nav:layer runtime\n\
                       // @pinker-nav:summary Superficie do runtime da fixture.\n\
                       pub fn runtime() {}\n\
                       // @pinker-nav:end fixture.runtime.superficie\n";

struct Repo(PathBuf);

impl Repo {
    fn new(label: &str) -> Repo {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pinker_nav_coverage_{label}_{}_{}_{}",
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

/// Repositório mínimo válido: todas as raízes oficiais existem, a autoridade
/// de cobertura está presente e o catálogo derivado nasce sincronizado.
fn fixture(label: &str) -> Repo {
    let repo = Repo::new(label);
    write(repo.path(), ".pinker/doc.toml", DOC_TOML);
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        POLICY,
    );
    write(repo.path(), "src/coberto.rs", COBERTO);
    write(repo.path(), "runtime/pinker_rt/src/lib.rs", RUNTIME);
    fs::create_dir_all(repo.path().join("apps")).unwrap();
    fs::create_dir_all(repo.path().join("tests")).unwrap();
    fs::create_dir_all(repo.path().join(".pinker/projections/recipes")).unwrap();
    sincronizar(repo.path());
    repo
}

fn run(root: &Path, args: &[&str], stdin: &str) -> Output {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg("--repo")
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn sincronizar(root: &Path) {
    let output = run(root, &["nav", "sincronizar"], "");
    assert_eq!(
        output.status.code(),
        Some(0),
        "sincronizar falhou: {}",
        stderr(&output)
    );
}

fn verificar(root: &Path) -> Output {
    run(root, &["nav", "verificar"], "")
}

fn cobertura_json(root: &Path) -> String {
    stdout(&run(root, &["nav", "cobertura", "--json"], ""))
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// Recorte determinístico do objeto JSON de um arquivo do inventário.
fn arquivo_json(json: &str, path: &str) -> String {
    let needle = format!("{{\"path\":\"{path}\",");
    let start = json
        .find(&needle)
        .unwrap_or_else(|| panic!("arquivo {path} ausente do inventário: {json}"));
    let mut depth = 0usize;
    for (offset, ch) in json[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return json[start..start + offset + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("objeto JSON de {path} não termina");
}

/// Impressão digital observada do conteúdo descoberto de um arquivo. É um
/// dado do inventário, não a decisão sob teste: os controles a usam para
/// declarar a dívida herdada, e depois provam o comportamento do gate.
fn catalogo(text: &str) -> CodeCatalog {
    CodeCatalog::parse(text, "<fixture>").expect("catálogo de fixture válido")
}

fn registro(key: &str, file: &str, start: usize, end: usize) -> String {
    format!(
        "{{\"schema\":1,\"key\":\"{key}\",\"kind\":\"region\",\"domain\":\"fixture\",\"layer\":\"core\",\"file\":\"{file}\",\"start_marker\":{start},\"content_start\":{},\"content_end\":{},\"end_marker\":{end},\"summary\":\"Responsabilidade coerente.\",\"hash\":\"fnv1a64:0000000000000001\",\"status\":\"current\",\"symbols\":[],\"related_symbols\":[],\"test_for\":[],\"symbol_docs\":[]}}\n",
        start + 1,
        end - 1
    )
}

fn autoridades<'a>(
    code: &'a CodeCatalog,
    base: Option<&'a CodeCatalog>,
    policy: Option<&'a CoveragePolicy>,
) -> CoverageAuthorities<'a> {
    CoverageAuthorities {
        code,
        base_code: base,
        policy,
        docs: None,
        projection_store: None,
        doc_config: None,
        manifests: None,
    }
}

// @pinker-nav:start evidencia.trama.cobertura-corrente
// @pinker-nav:domain cartography-coverage
// @pinker-nav:layer evidencia
// @pinker-nav:test-for pinker_v0::nav_coverage::inventory
// @pinker-nav:test-for pinker_v0::nav_coverage::verify
// @pinker-nav:test-for pinker_v0::nav_coverage::CoveragePolicy
// @pinker-nav:summary Controles C1 a C15 da cobertura corrente atingindo o consumidor real pink nav cobertura/verificar e a autoridade de relacao base-candidato: arquivo zero-ancora visivel e recusado, perda de marcador detectada, intervalo relevante fora de regiao exposto e recusado pelo gate, intersecao parcial nao promovida a completude, movimento e split/merge com disposicao preservada, excecao estreita aceita como unica rota e excecao ampla recusada na carga, base ausente como UNVERIFIABLE, catalogo derivado editado a mao incapaz de fabricar PASS, projecao FROZEN recalibrada recusada, declaracao de lacuna incapaz de devolver o gate ao verde, mesma contagem com conteudo novo ainda recusada por cobertura e nivel de obrigacao de raiz nao declaravel na autoridade.

/// C1 — um arquivo de produção sem nenhum marcador aparece no inventário e o
/// gate o recusa pela causa correta.
#[test]
fn c1_arquivo_zero_ancora_e_inventariado_e_recusado() {
    let repo = fixture("c1");
    write(repo.path(), "src/sem_ancora.rs", SEM_ANCORA);
    sincronizar(repo.path());

    let inventario = arquivo_json(&cobertura_json(repo.path()), "src/sem_ancora.rs");
    assert!(
        inventario.contains("\"regions\":[]"),
        "zero-âncora deveria publicar zero regiões: {inventario}"
    );
    assert!(
        inventario.contains("\"completeness\":\"NONE\""),
        "zero-âncora deveria publicar completude NONE: {inventario}"
    );
    assert!(
        inventario.contains("\"disposition\":\"GAP\""),
        "zero-âncora sem exceção deveria publicar disposição GAP: {inventario}"
    );

    let output = verificar(repo.path());
    assert_ne!(output.status.code(), Some(0), "o gate aceitou zero-âncora");
    assert!(
        stderr(&output).contains("E-COVERAGE-UNCOVERED: src/sem_ancora.rs"),
        "falhou por outra causa: {}",
        stderr(&output)
    );
}

/// C2 — retirar o marcador de um arquivo antes válido é perda de cobertura, e
/// o gate reprova mesmo com o catálogo ressincronizado.
#[test]
fn c2_remover_marcador_e_detectado_como_perda_de_cobertura() {
    let repo = fixture("c2");
    assert_eq!(verificar(repo.path()).status.code(), Some(0));

    write(
        repo.path(),
        "src/coberto.rs",
        "pub fn alvo() -> i32 {\n    1\n}\n",
    );
    sincronizar(repo.path());

    let output = verificar(repo.path());
    assert_ne!(output.status.code(), Some(0), "perda de cobertura aceita");
    assert!(
        stderr(&output).contains("E-COVERAGE-UNCOVERED: src/coberto.rs"),
        "falhou por outra causa: {}",
        stderr(&output)
    );
}

/// C3 — linhas relevantes fora da única região viram intervalo descoberto
/// explícito, com as coordenadas do arquivo.
#[test]
fn c3_linhas_relevantes_fora_da_regiao_viram_intervalo_descoberto() {
    let repo = fixture("c3");
    write(repo.path(), "src/parcial.rs", PARCIAL);
    sincronizar(repo.path());

    let inventario = arquivo_json(&cobertura_json(repo.path()), "src/parcial.rs");
    // A região ocupa as linhas 1..8; a segunda função começa na linha 10 e
    // termina na 12. A linha 9 é vazia e não carrega obrigação.
    assert!(
        inventario.contains("\"covered_intervals\":[{\"start\":1,\"end\":8}]"),
        "intervalo coberto inesperado: {inventario}"
    );
    assert!(
        inventario.contains("\"uncovered_relevant_intervals\":[{\"start\":10,\"end\":12}]"),
        "intervalo descoberto inesperado: {inventario}"
    );
}

/// C4 — interseção parcial nunca é promovida a completude.
#[test]
fn c4_intersecao_parcial_nao_e_completude() {
    let repo = fixture("c4");
    write(repo.path(), "src/parcial.rs", PARCIAL);
    sincronizar(repo.path());

    let inventario = arquivo_json(&cobertura_json(repo.path()), "src/parcial.rs");
    assert!(
        inventario.contains("\"completeness\":\"PARTIAL\""),
        "interseção parcial virou outra completude: {inventario}"
    );
    assert!(
        !inventario.contains("\"completeness\":\"COMPLETE\""),
        "interseção parcial foi promovida a COMPLETE: {inventario}"
    );

    // E não basta publicar PARTIAL: o consumidor de CI precisa recusar o
    // arquivo pela causa correta, sem rota de declaração que o aceite.
    let output = verificar(repo.path());
    assert_ne!(
        output.status.code(),
        Some(0),
        "o gate aceitou interseção parcial não declarada"
    );
    assert!(
        stderr(&output).contains("E-COVERAGE-UNCOVERED-INTERVAL: src/parcial.rs"),
        "falhou por outra causa: {}",
        stderr(&output)
    );

    // O mesmo recorte no diff: a região existe, então o arquivo NÃO é NONE,
    // mas a linha nova fora dela impede completude.
    let code = catalogo(&fs::read_to_string(repo.path().join("src/navigation.jsonl")).unwrap());
    let diff = "--- a/src/parcial.rs\n+++ b/src/parcial.rs\n@@ -11 +11 @@\n-    9\n+    2\n";
    let report = analyze(diff, autoridades(&code, None, None)).expect("diff válido");
    assert_eq!(report.schema, DIFF_COVERAGE_SCHEMA);
    assert_eq!(report.files[0].completeness, Completeness::Absent);
    assert_eq!(
        report.files[0].uncovered_intervals.len(),
        1,
        "linha nova fora de região deveria publicar intervalo descoberto"
    );
}

/// C5 — a mesma chave em outro arquivo é um movimento aceito, com destino
/// explícito e sem ressuscitar o path antigo.
#[test]
fn c5_movimento_legitimo_publica_destino_sem_ressuscitar_path_antigo() {
    let base = catalogo(&registro("fixture.alvo.coberto", "src/antigo.rs", 1, 8));
    let current = catalogo(&registro("fixture.alvo.coberto", "src/novo.rs", 1, 8));
    let diff = "--- a/src/novo.rs\n+++ b/src/novo.rs\n@@ -3 +3 @@\n-    1\n+    2\n";
    let report = analyze(diff, autoridades(&current, Some(&base), None)).expect("diff válido");

    assert_eq!(report.lifecycle.status, RelationStatus::Known);
    let item = &report.lifecycle.items[0];
    assert_eq!(item.key, "fixture.alvo.coberto");
    assert_eq!(item.disposition, "moved");
    assert_eq!(item.base_path, "src/antigo.rs");
    assert_eq!(item.targets, vec!["src/novo.rs".to_string()]);
}

/// C6 — split e merge só são aceitos com disposição declarada na autoridade
/// versionada; a disposição chega intacta ao relatório.
#[test]
fn c6_split_e_merge_preservam_disposicao_declarada() {
    let base = catalogo(&format!(
        "{}{}",
        registro("fixture.origem", "src/origem.rs", 1, 8),
        registro("fixture.fundida", "src/fundida.rs", 1, 8)
    ));
    let current = catalogo(&format!(
        "{}{}{}",
        registro("fixture.parte-a", "src/origem.rs", 1, 8),
        registro("fixture.parte-b", "src/origem.rs", 10, 17),
        registro("fixture.destino", "src/destino.rs", 1, 8)
    ));
    let policy = CoveragePolicy::parse(&format!(
        "{POLICY}{}\n{}\n",
        r#"{"schema":1,"kind":"disposition","key":"fixture.origem","disposition":"split","to":["fixture.parte-a","fixture.parte-b"],"reason":"duas responsabilidades distintas","review":"reavaliar se as partes voltarem a ser uma"}"#,
        r#"{"schema":1,"kind":"disposition","key":"fixture.fundida","disposition":"merged","to":["fixture.destino"],"reason":"responsabilidade absorvida","review":"reavaliar se o destino crescer demais"}"#
    ))
    .expect("política válida");

    let diff = "--- a/src/origem.rs\n+++ b/src/origem.rs\n@@ -3 +3 @@\n-    1\n+    2\n";
    let report =
        analyze(diff, autoridades(&current, Some(&base), Some(&policy))).expect("diff válido");

    assert_eq!(report.lifecycle.status, RelationStatus::Known);
    let dispositions: Vec<(&str, &str, Vec<String>)> = report
        .lifecycle
        .items
        .iter()
        .map(|item| {
            (
                item.key.as_str(),
                item.disposition.as_str(),
                item.targets.clone(),
            )
        })
        .collect();
    assert!(dispositions.contains(&(
        "fixture.origem",
        "split",
        vec!["fixture.parte-a".to_string(), "fixture.parte-b".to_string()]
    )));
    assert!(dispositions.contains(&(
        "fixture.fundida",
        "merged",
        vec!["fixture.destino".to_string()]
    )));

    // Negativo do mesmo eixo: sem a disposição declarada, a mesma remoção fica
    // UNKNOWN — nunca sucesso silencioso.
    let sem_politica = CoveragePolicy::parse(POLICY).expect("política válida");
    let report = analyze(
        diff,
        autoridades(&current, Some(&base), Some(&sem_politica)),
    )
    .expect("diff válido");
    assert_eq!(report.lifecycle.status, RelationStatus::Unknown);
    assert!(report
        .lifecycle
        .items
        .iter()
        .any(|item| item.disposition == "UNDECLARED"));
}

/// C7 — uma exceção estreita aprovada aceita o arquivo, e a disposição fica
/// explícita no inventário em vez de virar cobertura fingida.
#[test]
fn c7_excecao_estreita_aprovada_e_aceita_e_visivel() {
    let repo = fixture("c7");
    write(repo.path(), "src/sem_ancora.rs", SEM_ANCORA);
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &format!(
            "{POLICY}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/sem_ancora.rs","reason":"fixture sem responsabilidade cartografavel","review":"retirar se o arquivo ganhar comportamento proprio"}"#
        ),
    );
    sincronizar(repo.path());

    let output = verificar(repo.path());
    assert_eq!(
        output.status.code(),
        Some(0),
        "exceção estreita recusada: {}",
        stderr(&output)
    );
    let inventario = arquivo_json(&cobertura_json(repo.path()), "src/sem_ancora.rs");
    assert!(
        inventario.contains("\"disposition\":\"EXCEPTION\""),
        "exceção não aparece no inventário: {inventario}"
    );
    assert!(
        inventario.contains("\"completeness\":\"NONE\""),
        "exceção não pode fabricar completude: {inventario}"
    );
}

/// C8 — alargar a exceção para esconder dívida não relacionada é recusado na
/// carga da autoridade, e o gate falha pela causa correta.
#[test]
fn c8_excecao_ampla_nao_esconde_divida() {
    let repo = fixture("c8");
    write(repo.path(), "src/sem_ancora.rs", SEM_ANCORA);
    write(repo.path(), "src/outro_sem_ancora.rs", SEM_ANCORA);
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &format!(
            "{POLICY}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/*.rs","reason":"atalho","review":"nenhuma"}"#
        ),
    );
    sincronizar(repo.path());

    let output = verificar(repo.path());
    assert_ne!(output.status.code(), Some(0), "exceção ampla foi aceita");
    assert!(
        stderr(&output).contains("E-COVERAGE-POLICY-BROAD"),
        "falhou por outra causa: {}",
        stderr(&output)
    );

    // Uma exceção obsoleta também não vira folga silenciosa.
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &format!(
            "{POLICY}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/nunca_existiu.rs","reason":"atalho","review":"nenhuma"}"#
        ),
    );
    let output = verificar(repo.path());
    assert!(
        stderr(&output)
            .contains("E-COVERAGE-EXCEPTION-STALE: exceção declara src/nunca_existiu.rs"),
        "exceção obsoleta passou: {}",
        stderr(&output)
    );
}

/// C9 — quando a propriedade depende da base e a base não pode ser
/// estabelecida, o resultado é UNVERIFIABLE, nunca PASS.
#[test]
fn c9_base_necessaria_e_indisponivel_vira_unverifiable() {
    let current = catalogo(&registro("fixture.alvo.coberto", "src/alvo.rs", 1, 8));
    let diff = "--- a/src/alvo.rs\n+++ b/src/alvo.rs\n@@ -3 +2,0 @@\n-    1\n";
    let report = analyze(diff, autoridades(&current, None, None)).expect("diff válido");

    assert_eq!(
        report.files[0].completeness,
        Completeness::Unverifiable {
            reason: "a propriedade depende da cartografia da base, que está indisponível"
                .to_string()
        }
    );
    assert_eq!(
        report.files[0].base_regions.status,
        RelationStatus::Unavailable
    );
    assert_eq!(report.lifecycle.status, RelationStatus::Unavailable);

    // Com a base disponível, a MESMA deleção deixa de ser indecidível: as
    // coordenadas removidas são relacionadas contra a cartografia da base.
    let base = catalogo(&registro("fixture.alvo.coberto", "src/alvo.rs", 1, 8));
    let report = analyze(diff, autoridades(&current, Some(&base), None)).expect("diff válido");
    assert_eq!(report.files[0].base_regions.status, RelationStatus::Known);
    assert_ne!(
        report.files[0].completeness,
        Completeness::Unverifiable {
            reason: "a propriedade depende da cartografia da base, que está indisponível"
                .to_string()
        }
    );
}

/// C10 — editar o catálogo derivado à mão não fabrica cobertura: o inventário
/// parte da varredura das raízes, não do arquivo versionado.
#[test]
fn c10_catalogo_derivado_editado_a_mao_nao_fabrica_pass() {
    let repo = fixture("c10");
    write(repo.path(), "src/sem_ancora.rs", SEM_ANCORA);
    sincronizar(repo.path());

    let catalog_path = repo.path().join("src/navigation.jsonl");
    let mut text = fs::read_to_string(&catalog_path).unwrap();
    text.push_str(&registro("fixture.mentira", "src/sem_ancora.rs", 1, 3));
    fs::write(&catalog_path, text).unwrap();

    let output = verificar(repo.path());
    assert_ne!(
        output.status.code(),
        Some(0),
        "catálogo editado à mão fabricou sucesso"
    );
    assert!(
        stderr(&output).contains("E-COVERAGE-UNCOVERED: src/sem_ancora.rs"),
        "a cobertura passou a vir do catálogo versionado: {}",
        stderr(&output)
    );
}

/// C11 — recalibrar a medida de uma projeção FROZEN é recusado.
#[test]
fn c11_projecao_frozen_recalibrada_e_recusada() {
    let repo = fixture("c11");
    let catalog = CodeCatalog::load(&repo.path().join("src/navigation.jsonl")).unwrap();
    let measures = pinker_v0::nav_projection_snapshot::measure(catalog.regions.iter());
    let snapshot = pinker_v0::nav_projection_snapshot::ProjectionSnapshot {
        schema: pinker_v0::nav_projection_snapshot::SNAPSHOT_SCHEMA_V1,
        id: "fixture-frozen".to_string(),
        state: pinker_v0::nav_projection_snapshot::SnapshotState::Frozen,
        predecessor: None,
        justification: Some("fixture congelada".to_string()),
        measures,
        expected_overrides: 0,
        expected_exclusions: 0,
        expected_materializations: 0,
        base_snapshot: None,
        recipes: Vec::new(),
        rules: Vec::new(),
    };
    let rendered = pinker_v0::nav_projection_snapshot::render(&snapshot);
    write(
        repo.path(),
        ".pinker/projections/fixture-frozen.toml",
        &rendered,
    );
    let output = run(repo.path(), &["nav", "projecao", "verificar"], "");
    assert_eq!(
        output.status.code(),
        Some(0),
        "projeção íntegra recusada: {}",
        stderr(&output)
    );

    // Recalibrar o digest para esconder drift: a medida deixa de bater com a
    // reconstrução, e o resultado NÃO é MATCH.
    let recalibrado = rendered.replace(
        &format!("regions = {}", snapshot.measures.regions),
        &format!("regions = {}", snapshot.measures.regions + 1),
    );
    assert_ne!(recalibrado, rendered, "a fixture não mudou a medida");
    write(
        repo.path(),
        ".pinker/projections/fixture-frozen.toml",
        &recalibrado,
    );
    let output = run(repo.path(), &["nav", "projecao", "verificar"], "");
    assert_ne!(
        output.status.code(),
        Some(0),
        "recalibração de medida FROZEN aceita: {}",
        stdout(&output)
    );
}

/// C12 — candidato de produção integralmente coberto passa, e o inventário
/// publica completude COMPLETE sem lacuna nem exceção.
#[test]
fn c12_candidato_integralmente_coberto_passa() {
    let repo = fixture("c12");
    let output = verificar(repo.path());
    assert_eq!(
        output.status.code(),
        Some(0),
        "candidato válido recusado: {}",
        stderr(&output)
    );

    let json = cobertura_json(repo.path());
    let inventario = arquivo_json(&json, "src/coberto.rs");
    assert!(
        inventario.contains("\"completeness\":\"COMPLETE\""),
        "candidato coberto sem completude: {inventario}"
    );
    assert!(
        inventario.contains("\"disposition\":\"COVERED\""),
        "candidato coberto sem disposição: {inventario}"
    );
    assert!(
        !json.contains("\"disposition\":\"GAP\""),
        "candidato válido publicou lacuna: {json}"
    );

    // O diff do mesmo candidato: linha nova dentro da região é COMPLETE.
    let code = catalogo(&fs::read_to_string(repo.path().join("src/navigation.jsonl")).unwrap());
    let diff = "--- a/src/coberto.rs\n+++ b/src/coberto.rs\n@@ -6 +6 @@\n-    0\n+    1\n";
    let report = analyze(diff, autoridades(&code, None, None)).expect("diff válido");
    assert_eq!(report.files[0].completeness, Completeness::Complete);
    assert!(report.files[0].uncovered_intervals.is_empty());
    assert_eq!(report.files[0].covered_intervals.len(), 1);
}

/// A mudança da própria autoridade de escopo/exceções é observável no
/// relatório de diff, em vez de passar como alteração comum.
#[test]
fn mudanca_da_politica_e_observavel_no_diff() {
    let current = catalogo(&registro("fixture.alvo.coberto", "src/alvo.rs", 1, 8));
    let diff = "--- a/.pinker/cartography/coverage-policy-v1.jsonl\n\
                +++ b/.pinker/cartography/coverage-policy-v1.jsonl\n\
                @@ -5 +5 @@\n\
                -antigo\n\
                +novo\n";
    let report = analyze(diff, autoridades(&current, None, None)).expect("diff válido");
    assert!(
        report.policy_changed,
        "mudança de política passou invisível"
    );

    let diff = "--- a/src/alvo.rs\n+++ b/src/alvo.rs\n@@ -3 +3 @@\n-    1\n+    2\n";
    let report = analyze(diff, autoridades(&current, None, None)).expect("diff válido");
    assert!(!report.policy_changed);
}

/// C13 — a causalidade completa do gate de intervalo, em quatro passos.
///
/// STEP A: arquivo integralmente coberto passa.
/// STEP B: acrescentar trecho relevante fora de região falha POR COBERTURA.
/// STEP C: tentar declarar essa lacuna na autoridade NÃO produz PASS — não
///         existe registro que transforme código descoberto em aceito.
/// STEP D: cartografar de fato a responsabilidade nova e sincronizar o
///         catálogo pelo caminho oficial passa.
///
/// Este é o ataque que a revisão adversarial de #535 executou contra o
/// candidato anterior, onde declarar a lacuna como dívida devolvia o gate ao
/// verde sem cobrir uma linha sequer.
#[test]
fn c13_intervalo_novo_em_arquivo_ancorado_e_recusado_pelo_gate() {
    let repo = fixture("c13");

    // STEP A.
    assert_eq!(
        verificar(repo.path()).status.code(),
        Some(0),
        "candidato inicial deveria passar"
    );

    // STEP B.
    let com_lacuna = format!("{COBERTO}\npub fn responsabilidade_nova() -> i32 {{\n    9\n}}\n");
    write(repo.path(), "src/coberto.rs", &com_lacuna);
    sincronizar(repo.path());

    let output = verificar(repo.path());
    assert_ne!(
        output.status.code(),
        Some(0),
        "intervalo novo entrou sem o gate ver"
    );
    assert!(
        stderr(&output).contains(
            "E-COVERAGE-UNCOVERED-INTERVAL: src/coberto.rs tem 3 linha(s) relevante(s) em 1 intervalo(s)"
        ),
        "falhou por outra causa: {}",
        stderr(&output)
    );

    // STEP C — declarar a lacuna não é rota de aceitação. A autoridade recusa
    // o registro, e o que importa é que o candidato continua reprovado: a
    // cobertura não passa a existir porque alguém a declarou.
    let politica_com_declaracao = format!(
        "{POLICY}{}\n",
        r#"{"schema":1,"kind":"debt","path":"src/coberto.rs","uncovered_relevant_lines":3,"reason":"responsabilidade nova ainda nao cartografada","review":"reduzir a zero"}"#
    );
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &politica_com_declaracao,
    );
    let declarado = verificar(repo.path());
    assert_ne!(
        declarado.status.code(),
        Some(0),
        "declarar a lacuna devolveu o gate ao verde"
    );
    assert!(
        stderr(&declarado).contains("E-COVERAGE-POLICY-KIND"),
        "a autoridade aceitou um registro de dívida: {}",
        stderr(&declarado)
    );

    // STEP D — cartografar de fato, com a autoridade de volta à forma válida.
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        POLICY,
    );
    let cartografado = format!(
        "{COBERTO}\n\
         // @pinker-nav:start fixture.alvo.responsabilidade-nova\n\
         // @pinker-nav:domain fixture\n\
         // @pinker-nav:layer core\n\
         // @pinker-nav:summary Responsabilidade nova do alvo, publicada como regiao propria.\n\
         pub fn responsabilidade_nova() -> i32 {{\n\
         \x20   9\n\
         }}\n\
         // @pinker-nav:end fixture.alvo.responsabilidade-nova\n"
    );
    write(repo.path(), "src/coberto.rs", &cartografado);
    sincronizar(repo.path());
    assert_eq!(
        verificar(repo.path()).status.code(),
        Some(0),
        "cartografia real foi recusada: {}",
        stderr(&verificar(repo.path()))
    );
}

/// C14 — a exceção estreita aprovada é a ÚNICA rota que retira um arquivo da
/// obrigação, e ela vale para o arquivo inteiro. Não existe rota por
/// intervalo: um arquivo que já tem região e ainda tem linha relevante fora
/// dela continua reprovado mesmo com exceção declarada, e a própria exceção é
/// denunciada como desnecessária.
#[test]
fn c14_excecao_e_a_unica_rota_e_nao_alcanca_intervalo() {
    let repo = fixture("c14");

    // Arquivo sem nenhuma região: a exceção estreita aprovada o retira da
    // obrigação inteira.
    write(repo.path(), "src/sem_ancora.rs", SEM_ANCORA);
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &format!(
            "{POLICY}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/sem_ancora.rs","reason":"fixture sem responsabilidade cartografavel","review":"retirar se ganhar comportamento proprio"}"#
        ),
    );
    sincronizar(repo.path());
    assert_eq!(
        verificar(repo.path()).status.code(),
        Some(0),
        "exceção aprovada de arquivo inteiro foi recusada: {}",
        stderr(&verificar(repo.path()))
    );

    // Arquivo PARCIAL: a exceção não alcança o intervalo descoberto.
    write(repo.path(), "src/parcial.rs", PARCIAL);
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &format!(
            "{POLICY}{}\n",
            r#"{"schema":1,"kind":"exception","path":"src/parcial.rs","reason":"tentativa de retirar um intervalo da obrigacao","review":"nao deveria ser aceita"}"#
        ),
    );
    sincronizar(repo.path());
    let output = verificar(repo.path());
    assert_ne!(
        output.status.code(),
        Some(0),
        "exceção retirou um intervalo da obrigação"
    );
    assert!(
        stderr(&output).contains("E-COVERAGE-UNCOVERED-INTERVAL: src/parcial.rs"),
        "falhou por outra causa: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("E-COVERAGE-EXCEPTION-UNNECESSARY: src/parcial.rs"),
        "exceção sobre arquivo com região não foi denunciada: {}",
        stderr(&output)
    );
}

/// C15 — contagem igual com conteúdo diferente continua recusada, e a causa é
/// cobertura. Trocar as linhas descobertas por outras, mantendo o número,
/// não muda nada: não existe contagem tolerada para comparar.
#[test]
fn c15_mesma_contagem_com_conteudo_novo_continua_recusada() {
    let repo = fixture("c15");

    let primeira =
        format!("{COBERTO}\npub const PRIMEIRA_A: i32 = 1;\npub const PRIMEIRA_B: i32 = 2;\n");
    write(repo.path(), "src/coberto.rs", &primeira);
    sincronizar(repo.path());
    let antes = verificar(repo.path());
    assert_ne!(
        antes.status.code(),
        Some(0),
        "duas linhas descobertas passaram"
    );
    assert!(
        stderr(&antes).contains("E-COVERAGE-UNCOVERED-INTERVAL: src/coberto.rs tem 2 linha(s)"),
        "falhou por outra causa: {}",
        stderr(&antes)
    );

    // Mesma contagem, conteúdo (responsabilidade) diferente.
    let trocada = format!("{COBERTO}\npub const TROCA_A: i32 = 7;\npub const TROCA_B: i32 = 8;\n");
    write(repo.path(), "src/coberto.rs", &trocada);
    sincronizar(repo.path());

    let depois = verificar(repo.path());
    assert_ne!(
        depois.status.code(),
        Some(0),
        "troca de responsabilidade descoberta por contagem igual passou"
    );
    assert!(
        stderr(&depois).contains("E-COVERAGE-UNCOVERED-INTERVAL: src/coberto.rs tem 2 linha(s)"),
        "a recusa precisa vir da cobertura: {}",
        stderr(&depois)
    );
}

/// C16 — o nível de obrigação de uma raiz não é declarável na autoridade.
///
/// Rebaixar `src` de `required` para `inventory` na política versionada
/// desligaria o gate de intervalo para toda a produção de uma vez — uma rota
/// de aceitação por registro de política mais ampla do que a dívida que a
/// #675 recusou. A categoria e o nível de cada raiz oficial são contrato do
/// código; a autoridade declara as raízes para que o escopo seja auditável,
/// nunca para decidir quanto se exige delas.
#[test]
fn c16_nivel_de_obrigacao_de_raiz_nao_e_declaravel_na_autoridade() {
    let repo = fixture("c16");
    write(repo.path(), "src/parcial.rs", PARCIAL);
    sincronizar(repo.path());

    // Sanidade: com a autoridade na forma canônica, o gate recusa o arquivo
    // parcial pela causa de cobertura.
    let antes = verificar(repo.path());
    assert_ne!(
        antes.status.code(),
        Some(0),
        "fixture deveria estar vermelha"
    );
    assert!(
        stderr(&antes).contains("E-COVERAGE-UNCOVERED-INTERVAL: src/parcial.rs"),
        "falhou por outra causa: {}",
        stderr(&antes)
    );

    // Rebaixamento do nível: recusado na carga, e o gate continua vermelho.
    let rebaixada = POLICY.replace(
        r#"{"schema":1,"kind":"scope","root":"src","category":"production","file_enforcement":"required"}"#,
        r#"{"schema":1,"kind":"scope","root":"src","category":"production","file_enforcement":"inventory"}"#,
    );
    assert_ne!(rebaixada, POLICY, "a substituição precisa ter acontecido");
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &rebaixada,
    );
    let output = verificar(repo.path());
    assert_ne!(
        output.status.code(),
        Some(0),
        "rebaixar a raiz de produção desligou o gate"
    );
    assert!(
        stderr(&output).contains("E-COVERAGE-POLICY-SCOPE-CONTRACT: a raiz 'src'"),
        "falhou por outra causa: {}",
        stderr(&output)
    );

    // Trocar a categoria é recusado pelo mesmo contrato.
    let recategorizada = POLICY.replace(
        r#""root":"src","category":"production""#,
        r#""root":"src","category":"evidence""#,
    );
    write(
        repo.path(),
        ".pinker/cartography/coverage-policy-v1.jsonl",
        &recategorizada,
    );
    assert!(
        stderr(&verificar(repo.path())).contains("E-COVERAGE-POLICY-SCOPE-CONTRACT"),
        "categoria trocada foi aceita"
    );
}

/// `--base REF` é o caminho real pelo qual a cartografia da base entra: o
/// adaptador lê o catálogo derivado daquele commit, sem mutar o repositório.
/// Sem `--base`, a mesma deleção pura permanece UNVERIFIABLE.
#[test]
fn base_explicita_torna_a_delecao_pura_verificavel_pela_cli() {
    let repo = fixture("base");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .current_dir(repo.path())
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} falhou");
    };
    git(&["init", "--quiet"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["config", "user.name", "fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "--quiet", "-m", "base"]);

    let diff = "--- a/src/coberto.rs\n+++ b/src/coberto.rs\n@@ -6 +5,0 @@\n-    1\n";

    let sem_base = run(repo.path(), &["nav", "cobertura-diff", "--json"], diff);
    assert_eq!(sem_base.status.code(), Some(0));
    assert!(
        stdout(&sem_base).contains("\"completeness\":\"UNVERIFIABLE\""),
        "sem base a deleção pura deveria ser UNVERIFIABLE: {}",
        stdout(&sem_base)
    );

    let com_base = run(
        repo.path(),
        &["nav", "cobertura-diff", "--base", "HEAD", "--json"],
        diff,
    );
    assert_eq!(com_base.status.code(), Some(0), "{}", stderr(&com_base));
    let json = stdout(&com_base);
    assert!(
        !json.contains("\"completeness\":\"UNVERIFIABLE\""),
        "com base a deleção pura continuou indecidível: {json}"
    );
    assert!(
        json.contains("\"base_regions\":{\"status\":\"KNOWN\""),
        "a cartografia da base não foi consultada: {json}"
    );
    assert!(
        json.contains("\"lifecycle\":{\"status\":\"KNOWN\""),
        "o lifecycle base->candidato não foi estabelecido: {json}"
    );

    // Uma base inexistente é recusada, não silenciosamente ignorada.
    let invalida = run(
        repo.path(),
        &[
            "nav",
            "cobertura-diff",
            "--base",
            "nao-existe-ref",
            "--json",
        ],
        diff,
    );
    assert_ne!(invalida.status.code(), Some(0));
    assert!(
        stderr(&invalida).contains("E-BASE-GIT"),
        "base inválida falhou por outra causa: {}",
        stderr(&invalida)
    );
}

/// A classificação de relevância conserva obrigação: só linha vazia e linha
/// inteiramente comentada saem, e comentário dentro de string não engana.
#[test]
fn classificacao_de_relevancia_conserva_obrigacao() {
    use pinker_v0::nav::{relevant_source_lines, MarkerDialect};
    let fonte = "fn a() {\n\
                 \x20   let s = \"// nao e comentario\";\n\
                 \n\
                 \x20   // comentario de linha\n\
                 \x20   let t = r#\"/* nao abre bloco */\"#;\n\
                 }\n";
    assert_eq!(
        relevant_source_lines(fonte, MarkerDialect::Rust),
        vec![true, true, false, false, true, true]
    );
}
// @pinker-nav:end evidencia.trama.cobertura-corrente
