//! Controles causais da extração lexical de `pink nav localizar` (#680 / T2).
//!
//! Cada teste ataca uma propriedade distinta do contrato: o extrator encontra
//! declaração ordinária não registrada, recusa declaração falsa escondida em
//! comentário ou literal, mantém homônimo separado por contexto, nunca promove
//! ocorrência textual a declaração nem declaração a identidade semântica,
//! observa o worktree sujo, corresponde trecho e intervalo, e pagina por
//! orçamento determinístico.

// @pinker-nav:start evidencia.simbolos.extracao
// @pinker-nav:domain simbolos
// @pinker-nav:layer evidencia
// @pinker-nav:test-for pinker_v0::symbol_extraction::extend
// @pinker-nav:summary Causal controls for bounded lexical extraction: an unregistered declaration is found, every supported kind keeps its own category and structural context, associated items and homonyms stay separated, fake declarations hidden in comments, normal strings and raw strings never become structural, macro output and cfg attributes stay declared limitations, a dirty or untracked worktree changes the current answer, no cache is materialised, the budget truncates deterministically with a usable continuation, explicit identity keeps precedence, unstable source refuses to pass silently, textual fallback stays outside the structural counts, a hand-edited derived catalog manufactures no source authority, and binary provenance stays observable.

use pinker_v0::symbol_extraction;
use pinker_v0::symbol_index::{LocateReport, TextualOccurrence};
use std::fs;
use std::io;
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

/// Fonte registrada: existe identidade explícita cujo nome coincide com a
/// declaração Rust, para provar que a precedência do contrato antigo continua
/// valendo e que o candidato extraído não duplica a identidade.
const REGISTERED: &str = r#"// @pinker-nav:start codigo.registrado
// @pinker-nav:domain fixture
// @pinker-nav:layer modelo
// @pinker-nav:symbol pkg::registered_fn|registered_fn|rust-function|declaration
// @pinker-nav:summary Declaração registrada explicitamente pela fixture.
fn registered_fn() {}
// @pinker-nav:end codigo.registrado
"#;

/// Fonte deliberadamente não registrada. Nenhuma linha aqui possui marcador:
/// tudo o que o extrator encontrar veio da leitura da fonte corrente.
const PLAIN: &str = r####"pub fn unregistered_helper(value: usize) -> usize {
    value
}

pub struct PlainStruct;

pub enum PlainEnum {
    One,
}

pub trait PlainTrait {}

pub type PlainAlias = usize;

pub mod inner_module {
    pub fn homonym() {}
}

pub mod other_module {
    pub fn homonym() {}
}

pub struct Holder;

impl Holder {
    pub fn associated_item(&self) {}
}

pub trait WithAssoc {
    fn associated_item(&self);
}

// fn fake_in_line_comment() {}
/* fn fake_in_block_comment() {} */
/* outer /* struct FakeInNestedComment; */ ainda comentário */

pub fn strings_holder() -> &'static str {
    let _normal = "fn fake_in_normal_string() {}";
    let _raw = r#"fn fake_in_raw_string() {}"#;
    let _raw_hashes = r###"struct FakeInRawHashString;"###;
    let _delimiter = '{';
    let _also = "enum FakeInNormalString {}";
    "done"
}

macro_rules! declare_thing {
    ($name:ident) => {
        pub fn $name() {}
    };
}

declare_thing!(macro_made_fn);

#[cfg(feature = "absent")]
pub fn cfg_gated_fn() {}

pub fn calls_only() {
    unregistered_helper(1);
    textual_only_name();
}

macro_rules! template_with_metavariable {
    ($outer:ident) => {
        pub fn $metavariable_template() {}
        pub struct TemplateOnlyStruct;
    };
}

some_macro! {
    pub fn $invocation_metavariable() {}
}

macro_rules! paren_body_macro (
    () => {
        pub struct GhostParenBody;
    };
);

macro_rules! bracket_body_macro [
    () => {
        pub struct GhostBracketBody;
    };
];

macro_rules! split_header_macro
{
    () => {
        pub struct GhostSplitHeader;
    };
}

some_macro! {
    pub struct OrdinaryLookingTokenTreeItem;
}

paren_invocation! (
    pub struct GhostParenInvocation;
);

bracket_invocation! [
    pub struct GhostBracketInvocation;
];

pub struct DeclaredAfterMacro;

pub fn never_returns() -> ! {
    loop {}
}

pub fn negation_and_inequality(flag: bool) -> bool {
    !flag && 1 != 2
}

pub struct AfterNeverType;
"####;

/// Fonte dedicada à fronteira lexical de macro medida por posição, não por
/// linha. Cada bloco existe para uma propriedade distinta: abertura na mesma
/// linha do primeiro token interno, fechamento seguido de código externo na
/// mesma linha, aninhamento, máscara lexical e contaminação de contexto.
const MACRO_BOUNDARY: &str = r####"some_macro!
{ pub struct GhostCurlySameLine; }

some_macro!
(pub struct GhostParenSameLine;);

some_macro!
[pub struct GhostBracketSameLine;];

some_macro!
{
    pub struct InternalOnly;
}

pub struct LegitimateAfterMacro;

outer_macro! {
    inner_macro! {
        pub struct NestedGhost;
    }
}

pub struct AfterNestedMacro;

pub fn lexical_mask_holder() -> usize {
    let a = "{";
    let b = "some_macro! { pub struct GhostInString; }";
    // some_macro! { pub struct GhostInLineComment; }
    /* some_macro! [ pub struct GhostInBlockComment; ] */
    a.len() + b.len()
}

pub struct AfterLexicalMask;

contaminating_macro! {
    mod FakeContainer {
        impl FakeThing {
            pub fn ghost() {}
} } } pub fn real_after_macro() {}

pub fn boundary_negation(flag: bool) -> bool {
    !flag && 3 != 4
}

pub fn boundary_never() -> ! {
    loop {}
}

pub struct AfterBoundaryBang;
"####;

struct Repo(PathBuf);

impl Repo {
    fn new(label: &str) -> Repo {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "pinker_extraction_{label}_{}_{}_{}",
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
    write(repo.path(), "src/registered.rs", REGISTERED);
    write(repo.path(), "src/plain.rs", PLAIN);
    write(
        repo.path(),
        "runtime/pinker_rt/src/lib.rs",
        "pub fn runtime() {}\n",
    );
    write(repo.path(), "tests/evidence.rs", "// sem marcadores\n");
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

fn locate_json(root: &Path, symbol: &str) -> String {
    let output = run(root, &["nav", "localizar", symbol, "--json"]);
    assert!(
        stderr(&output).is_empty(),
        "stderr inesperado: {}",
        stderr(&output)
    );
    stdout(&output)
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

/// Conta objetos de uma classe pela etiqueta de classificação, que é o único
/// vocabulário estável que a saída publica.
fn count(json: &str, classification: &str) -> usize {
    json.matches(&format!("\"classification\":\"{classification}\""))
        .count()
}

/// Linha, contada de 1, onde a fixture grafou um nome. Calcular a linha a
/// partir do próprio texto mantém a asserção de intervalo exata sem transformar
/// a fixture num conjunto de números mágicos.
fn line_of(source: &str, needle: &str) -> usize {
    source
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("fixture perdeu {needle}"))
        + 1
}

/// Recorta o objeto JSON de um candidato extraído por caminho e linha inicial,
/// para que uma asserção não dependa da ordem global da página.
fn extracted_at<'a>(json: &'a str, path: &str, start: usize) -> &'a str {
    let needle = format!("\"path\":\"{path}\",\"kind\":");
    let mut cursor = 0;
    while let Some(found) = json[cursor..].find(&needle) {
        let begin = cursor + found;
        let end = json[begin..].find('}').map(|n| begin + n).unwrap();
        let slice = &json[begin..end];
        if slice.contains(&format!("\"start\":{start},")) {
            return slice;
        }
        cursor = end;
    }
    panic!("candidato extraído ausente em {path}:{start}: {json}");
}

// C1 — declaração ordinária sem metadado explícito vira candidato extraído.
#[test]
fn declaracao_ordinaria_nao_registrada_vira_candidato_extraido() {
    let repo = fixture("c1");
    let json = locate_json(repo.path(), "unregistered_helper");
    assert_eq!(count(&json, "EXPLICIT_SYMBOL"), 0, "{json}");
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 1, "{json}");
    let candidate = extracted_at(&json, "src/plain.rs", 1);
    assert!(candidate.contains("\"kind\":\"function\""), "{candidate}");
    assert!(
        candidate.contains("\"name\":\"unregistered_helper\""),
        "{candidate}"
    );
    assert!(
        candidate.contains("pub fn unregistered_helper(value: usize) -> usize {"),
        "{candidate}"
    );
}

// C2 — cada categoria suportada publica kind e contexto corretos.
#[test]
fn cada_categoria_suportada_publica_kind_e_contexto() {
    let repo = fixture("c2");
    for (symbol, kind) in [
        ("PlainStruct", "struct"),
        ("PlainEnum", "enum"),
        ("PlainTrait", "trait"),
        ("PlainAlias", "type_alias"),
        ("inner_module", "module"),
    ] {
        let json = locate_json(repo.path(), symbol);
        assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 1, "{symbol}: {json}");
        assert!(
            json.contains(&format!("\"kind\":\"{kind}\"")),
            "{symbol} esperava {kind}: {json}"
        );
    }
}

// C3 — item associado reconhecível recebe contexto estrutural separado.
#[test]
fn itens_associados_recebem_contexto_estrutural_separado() {
    let repo = fixture("c3");
    let json = locate_json(repo.path(), "associated_item");
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 2, "{json}");
    assert!(json.contains("\"context\":\"impl Holder\""), "{json}");
    assert!(
        json.contains("\"context\":\"pub trait WithAssoc\""),
        "{json}"
    );
}

// C4 — declaração falsa em comentário, string normal e string crua não é
// declaração estrutural. O negativo falha pela causa alvo: o nome existe no
// arquivo, e ainda assim nenhuma classe estrutural o reconhece.
#[test]
fn declaracoes_falsas_em_comentario_e_literal_nao_sao_estruturais() {
    let repo = fixture("c4");
    let plain = fs::read_to_string(repo.path().join("src/plain.rs")).unwrap();
    for fake in [
        "fake_in_line_comment",
        "fake_in_block_comment",
        "FakeInNestedComment",
        "fake_in_normal_string",
        "fake_in_raw_string",
        "FakeInRawHashString",
        "FakeInNormalString",
    ] {
        assert!(plain.contains(fake), "fixture perdeu {fake}");
        let output = run(repo.path(), &["nav", "localizar", fake, "--json"]);
        let json = stdout(&output);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            0,
            "{fake} virou declaração: {json}"
        );
        assert_eq!(
            count(&json, "TEXTUAL_OCCURRENCE"),
            0,
            "{fake} virou ocorrência de código: {json}"
        );
        assert_eq!(code(&output), 4, "{fake}: {json}");
    }
}

// C5 — homônimos em contextos distintos continuam resultados distintos.
#[test]
fn homonimos_permanecem_separados_e_contextualizados() {
    let repo = fixture("c5");
    let json = locate_json(repo.path(), "homonym");
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 2, "{json}");
    assert!(
        json.contains("\"context\":\"pub mod inner_module\""),
        "{json}"
    );
    assert!(
        json.contains("\"context\":\"pub mod other_module\""),
        "{json}"
    );
    let first = json.find("pub mod inner_module").unwrap();
    let second = json.find("pub mod other_module").unwrap();
    assert!(first < second, "ordem não determinística: {json}");
}

// C6 — macro e cfg permanecem limitação declarada, nunca identidade falsa.
#[test]
fn macro_e_cfg_permanecem_limitacao_declarada() {
    let repo = fixture("c6");

    let macro_json = locate_json(repo.path(), "macro_made_fn");
    assert_eq!(count(&macro_json, "EXPLICIT_SYMBOL"), 0, "{macro_json}");
    assert_eq!(count(&macro_json, "EXTRACTED_CANDIDATE"), 0, "{macro_json}");
    assert_eq!(count(&macro_json, "TEXTUAL_OCCURRENCE"), 1, "{macro_json}");
    assert!(
        macro_json.contains("macro_generated_declarations_not_expanded"),
        "{macro_json}"
    );

    let cfg_json = locate_json(repo.path(), "cfg_gated_fn");
    assert_eq!(count(&cfg_json, "EXPLICIT_SYMBOL"), 0, "{cfg_json}");
    assert_eq!(count(&cfg_json, "EXTRACTED_CANDIDATE"), 1, "{cfg_json}");
    assert!(cfg_json.contains("\"cfg_present\":true"), "{cfg_json}");
    assert!(
        cfg_json.contains("cfg_attributes_not_evaluated"),
        "{cfg_json}"
    );
}

// C7 — mesmo HEAD, fonte editada e fonte nova mudam o resultado corrente.
#[test]
fn worktree_sujo_e_arquivo_novo_mudam_o_resultado() {
    let repo = fixture("c7");
    let before = run(repo.path(), &["nav", "localizar", "late_arrival", "--json"]);
    assert_eq!(code(&before), 4, "{}", stdout(&before));

    // Arquivo novo, jamais sincronizado ao catálogo derivado.
    write(
        repo.path(),
        "src/untracked.rs",
        "pub fn late_arrival() -> bool {\n    true\n}\n",
    );
    let after = locate_json(repo.path(), "late_arrival");
    assert_eq!(count(&after, "EXTRACTED_CANDIDATE"), 1, "{after}");
    assert!(after.contains("\"path\":\"src/untracked.rs\""), "{after}");

    // Edição não commitada de arquivo existente.
    let plain = fs::read_to_string(repo.path().join("src/plain.rs")).unwrap();
    write(
        repo.path(),
        "src/plain.rs",
        &plain.replace("pub struct PlainStruct;", "pub struct RenamedStruct;"),
    );
    let renamed = locate_json(repo.path(), "RenamedStruct");
    assert_eq!(count(&renamed, "EXTRACTED_CANDIDATE"), 1, "{renamed}");
    let gone = run(repo.path(), &["nav", "localizar", "PlainStruct", "--json"]);
    assert_eq!(count(&stdout(&gone), "EXTRACTED_CANDIDATE"), 0);
}

// C8 — não há cache: a consulta não escreve nada e a próxima leitura já
// enxerga a fonte alterada, sem chave de validade a envelhecer.
#[test]
fn consulta_nao_cria_cache_nem_envelhece_resultado() {
    let repo = fixture("c8");
    let first = locate_json(repo.path(), "unregistered_helper");
    let files_before = inventory(repo.path());
    let repeated = locate_json(repo.path(), "unregistered_helper");
    let files_after = inventory(repo.path());
    assert_eq!(first, repeated, "resultado não determinístico");
    assert_eq!(
        files_before, files_after,
        "a consulta materializou estado novo"
    );

    let plain = fs::read_to_string(repo.path().join("src/plain.rs")).unwrap();
    write(
        repo.path(),
        "src/plain.rs",
        &plain.replace(
            "pub fn unregistered_helper(value: usize) -> usize {",
            "pub fn unregistered_helper(value: usize) -> u64 {",
        ),
    );
    let third = locate_json(repo.path(), "unregistered_helper");
    assert_ne!(first, third, "resultado stale após mudança de conteúdo");
    assert!(third.contains("-> u64"), "{third}");
}

fn inventory(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push((
                    path.strip_prefix(root).unwrap().to_string_lossy().into(),
                    fs::read(&path).unwrap(),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

// C9 — orçamento estourado trunca de forma determinística e entrega
// continuação utilizável.
#[test]
fn orcamento_trunca_e_entrega_continuacao_utilizavel() {
    let repo = fixture("c9");
    let total = symbol_extraction::LOCATE_RESULT_BUDGET + 10;
    let mut source = String::new();
    for index in 0..total {
        source.push_str(&format!(
            "pub mod scope_{index:03} {{\n    pub fn budget_target() {{}}\n}}\n\n"
        ));
    }
    write(repo.path(), "src/many.rs", &source);

    let first = locate_json(repo.path(), "budget_target");
    assert_eq!(
        count(&first, "EXTRACTED_CANDIDATE"),
        symbol_extraction::LOCATE_RESULT_BUDGET,
        "{first}"
    );
    assert!(first.contains(&format!("\"total\":{total}")), "{first}");
    assert!(first.contains("\"truncated\":true"), "{first}");
    assert!(
        first.contains(&format!(
            "\"continuation\":{}",
            symbol_extraction::LOCATE_RESULT_BUDGET
        )),
        "{first}"
    );

    let cursor = symbol_extraction::LOCATE_RESULT_BUDGET.to_string();
    let second = run(
        repo.path(),
        &[
            "nav",
            "localizar",
            "budget_target",
            "--json",
            "--desde",
            &cursor,
        ],
    );
    assert_success(&second);
    let second = stdout(&second);
    assert_eq!(count(&second, "EXTRACTED_CANDIDATE"), 10, "{second}");
    assert!(second.contains("\"truncated\":false"), "{second}");
    assert!(second.contains("\"continuation\":null"), "{second}");
    assert!(second.contains("scope_029"), "{second}");
    assert!(!second.contains("scope_000"), "{second}");

    // A janela é uma partição de verdade: a união das páginas devolve cada
    // escopo exatamente uma vez, sem repetição e sem perda. Contar por página
    // não bastaria — uma implementação que sobrepõe e perde resultados passa
    // numa contagem e falha aqui.
    let mut seen: Vec<String> = scopes(&first);
    seen.extend(scopes(&second));
    let mut sorted = seen.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), seen.len(), "páginas se sobrepõem: {seen:?}");
    let expected: Vec<String> = (0..total)
        .map(|i| format!("pub mod scope_{i:03}"))
        .collect();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    assert_eq!(
        sorted, expected_sorted,
        "a paginação perdeu ou inventou escopos"
    );
}

/// Contextos dos candidatos extraídos de uma página, na ordem publicada.
fn scopes(json: &str) -> Vec<String> {
    json.split("\"context\":\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next().map(str::to_string))
        .collect()
}

// C10 — identidade explícita preserva semântica e precedência: ela não é
// rebaixada a candidato extraído nem duplicada por ele.
#[test]
fn identidade_explicita_preserva_semantica_e_precedencia() {
    let repo = fixture("c10");
    let json = locate_json(repo.path(), "registered_fn");
    assert_eq!(count(&json, "EXPLICIT_SYMBOL"), 1, "{json}");
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 0, "{json}");
    assert!(
        json.contains("\"identity\":\"pkg::registered_fn\""),
        "{json}"
    );
    let position_explicit = json.find("EXPLICIT_SYMBOL").unwrap();
    let position_candidates = json.find("\"extracted_candidates\"").unwrap();
    assert!(
        position_explicit < position_candidates,
        "precedência invertida: {json}"
    );
}

// C11 — fonte que muda entre as leituras não vira sucesso silencioso.
#[test]
fn fonte_instavel_nao_passa_em_silencio() {
    let mut reads = 0usize;
    let unstable = symbol_extraction::read_stable_with(|| {
        reads += 1;
        Ok(format!("conteúdo {reads}"))
    })
    .unwrap();
    assert_eq!(unstable, None, "fonte instável devolveu conteúdo");
    assert_eq!(reads, 4, "a releitura de segurança não aconteceu");

    let stable = symbol_extraction::read_stable_with(|| Ok("estável".to_string()))
        .unwrap()
        .unwrap();
    assert_eq!(stable, "estável");

    // Um erro de leitura continua sendo erro, não instabilidade silenciosa.
    let failed: io::Result<Option<String>> =
        symbol_extraction::read_stable_with(|| Err(io::Error::other("falha")));
    assert!(failed.is_err());

    // E a instabilidade é publicada nas duas saídas.
    let mut report = empty_report("alvo");
    report.unstable_sources = vec!["src/instavel.rs".to_string()];
    let human = pinker_v0::symbol_index::render_human(&report);
    let json = pinker_v0::symbol_index::render_json(&report);
    assert!(human.contains("UNSTABLE_SOURCE = TRUE"), "{human}");
    assert!(human.contains("src/instavel.rs"), "{human}");
    assert!(
        json.contains("\"unstable_sources\":[\"src/instavel.rs\"]"),
        "{json}"
    );
}

fn empty_report(query: &str) -> LocateReport {
    LocateReport {
        schema: pinker_v0::symbol_index::SYMBOL_LOCATION_SCHEMA,
        query: query.to_string(),
        candidates: Vec::new(),
        extracted_candidates: Vec::new(),
        textual_occurrences: Vec::new(),
        limitations: Vec::new(),
        unstable_sources: Vec::new(),
        total: 0,
        offset: 0,
        truncated: false,
        continuation: None,
    }
}

// C12 — fallback textual é rotulado e fica fora da contagem estrutural.
#[test]
fn fallback_textual_fica_fora_da_extracao_estrutural() {
    let repo = fixture("c12");
    let json = locate_json(repo.path(), "textual_only_name");
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 0, "{json}");
    assert_eq!(count(&json, "TEXTUAL_OCCURRENCE"), 1, "{json}");
    assert!(
        json.contains("\"limitation\":\"not_a_supported_structural_declaration\""),
        "{json}"
    );
    assert!(json.contains("\"total\":1"), "{json}");
    let extracted_block = json
        .split("\"extracted_candidates\":[")
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap();
    assert!(
        extracted_block.is_empty(),
        "ocorrência textual contada como extração: {extracted_block}"
    );

    // E a ocorrência textual renderiza como ocorrência, nunca como declaração.
    let mut report = empty_report("textual_only_name");
    report.textual_occurrences = vec![TextualOccurrence {
        path: "src/plain.rs".to_string(),
        line: 1,
        snippet: "textual_only_name();".to_string(),
        snippet_truncated: false,
        limitation: "not_a_supported_structural_declaration".to_string(),
    }];
    let human = pinker_v0::symbol_index::render_human(&report);
    assert!(
        human.contains("classificação: TEXTUAL_OCCURRENCE"),
        "{human}"
    );
    assert!(!human.contains("EXTRACTED_CANDIDATE"), "{human}");
}

// C13 — edição manual do índice derivado não fabrica autoridade de fonte.
//
// O ataque é forjar no catálogo derivado um vínculo `symbols` para um nome que
// não existe na fonte. O que a extração produz continua vindo só da fonte, e o
// catálogo adulterado é recusado pela verificação em vez de virar autoridade
// silenciosa. O símbolo explícito forjado ainda aparece porque o catálogo é a
// autoridade declarada dos vínculos explícitos — essa é a semântica preexistente
// que esta unidade preserva, e `pink nav verificar` é o gate que a protege.
#[test]
fn edicao_manual_do_indice_derivado_nao_fabrica_fonte() {
    let repo = fixture("c13");
    let catalog = repo.path().join("src/navigation.jsonl");
    let original = fs::read_to_string(&catalog).unwrap();
    let forged = original.replace(
        "pkg::registered_fn|registered_fn|rust-function|declaration",
        "pkg::forged_only|forged_only|rust-function|declaration",
    );
    assert_ne!(original, forged, "fixture perdeu o vínculo registrado");
    assert!(
        !fs::read_to_string(repo.path().join("src/registered.rs"))
            .unwrap()
            .contains("forged_only"),
        "o nome forjado não pode existir na fonte"
    );
    fs::write(&catalog, &forged).unwrap();

    // A extração lê a fonte, então o nome forjado não vira declaração nem
    // ocorrência: o catálogo não fabricou fonte nenhuma.
    let json = stdout(&run(
        repo.path(),
        &["nav", "localizar", "forged_only", "--json"],
    ));
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 0, "{json}");
    assert_eq!(count(&json, "TEXTUAL_OCCURRENCE"), 0, "{json}");

    // E o catálogo editado à mão é recusado, em vez de permanecer como
    // autoridade independente da fonte que ele diz descrever.
    let verify = run(repo.path(), &["nav", "verificar"]);
    assert_eq!(code(&verify), 5, "{}", stdout(&verify));
    assert!(
        stderr(&verify).contains("dessincronizado"),
        "{}",
        stderr(&verify)
    );

    // O símbolo registrado de verdade continua ausente do catálogo forjado:
    // a adulteração não preserva silenciosamente o vínculo que apagou.
    let gone = stdout(&run(
        repo.path(),
        &["nav", "localizar", "registered_fn", "--json"],
    ));
    assert_eq!(count(&gone, "EXPLICIT_SYMBOL"), 0, "{gone}");
    assert_eq!(count(&gone, "EXTRACTED_CANDIDATE"), 1, "{gone}");
}

// F5 — token tree de macro não é declaração Rust ordinária, qualquer que seja
// o delimitador.
//
// A gramática de Rust é a autoridade: uma invocação é `SimplePath !
// DelimTokenTree`, e uma definição é `macro_rules ! IDENTIFIER MacroRulesDef`.
// Os três delimitadores `()`, `[]` e `{}` valem nos dois casos, e whitespace —
// inclusive quebra de linha — separa tokens normalmente, então o `!` e o
// delimitador podem estar em linhas diferentes. Cada forma é um escape distinto
// e por isso é afirmada separadamente: uma proteção que reconheça só
// `macro_rules!` seguido de `{` na mesma linha passa em algumas e falha nas
// outras.
#[test]
fn token_tree_de_macro_nao_vira_declaracao_estrutural() {
    let repo = fixture("f5");
    let plain = fs::read_to_string(repo.path().join("src/plain.rs")).unwrap();

    for (case, name, expected_limitation) in [
        // Definições. Nenhum destes carrega metavariável: só a fronteira de
        // macro os sustenta, e é por isso que eles vêm primeiro.
        (
            "MACRO_RULES_CURLY_BODY",
            "TemplateOnlyStruct",
            "macro_token_tree_not_a_declaration",
        ),
        (
            "MACRO_RULES_PAREN_BODY",
            "GhostParenBody",
            "macro_token_tree_not_a_declaration",
        ),
        (
            "MACRO_RULES_BRACKET_BODY",
            "GhostBracketBody",
            "macro_token_tree_not_a_declaration",
        ),
        (
            "MACRO_RULES_SPLIT_HEADER",
            "GhostSplitHeader",
            "macro_token_tree_not_a_declaration",
        ),
        // Invocações com item que parece Rust perfeitamente comum.
        (
            "MACRO_INVOCATION_CURLY",
            "OrdinaryLookingTokenTreeItem",
            "macro_token_tree_not_a_declaration",
        ),
        (
            "MACRO_INVOCATION_PAREN",
            "GhostParenInvocation",
            "macro_token_tree_not_a_declaration",
        ),
        (
            "MACRO_INVOCATION_BRACKET",
            "GhostBracketInvocation",
            "macro_token_tree_not_a_declaration",
        ),
        // Metavariáveis: cobertas pela fronteira e também pelo sigilo.
        (
            "MACRO_RULES_METAVARIABLE",
            "metavariable_template",
            "macro_token_tree_not_a_declaration",
        ),
        (
            "MACRO_INVOCATION_METAVARIABLE",
            "invocation_metavariable",
            "macro_token_tree_not_a_declaration",
        ),
    ] {
        assert!(plain.contains(name), "fixture perdeu {name}");
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            0,
            "{case}: {name} virou declaração estrutural: {json}"
        );
        assert_eq!(
            count(&json, "EXPLICIT_SYMBOL"),
            0,
            "{case}: {name} virou identidade: {json}"
        );
        assert_eq!(
            count(&json, "TEXTUAL_OCCURRENCE"),
            1,
            "{case}: {name} perdeu o fallback textual: {json}"
        );
        assert!(
            json.contains(&format!("\"limitation\":\"{expected_limitation}\"")),
            "{case}: {name} sem a limitação {expected_limitation}: {json}"
        );
    }

    // A limitação viaja no relatório, não só na ocorrência.
    let json = locate_json(repo.path(), "GhostParenBody");
    assert!(
        json.contains("macro_token_tree_not_a_declaration"),
        "{json}"
    );

    // DECLARATION_AFTER_MACRO: a fronteira fecha no delimitador correspondente.
    // Se ela vazasse, a declaração seguinte sumiria — e é isso que prova que a
    // proteção recorta o token tree, não o resto do arquivo.
    for name in [
        "DeclaredAfterMacro",
        "AfterNeverType",
        "unregistered_helper",
    ] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            1,
            "DECLARATION_AFTER_MACRO: {name} deixou de ser estrutural: {json}"
        );
    }

    // `-> !`, `!flag` e `1 != 2` não abrem token tree: um `!` só inicia macro
    // quando vem colado a um caminho.
    for name in ["never_returns", "negation_and_inequality"] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            1,
            "{name} foi engolido por uma fronteira de macro inexistente: {json}"
        );
    }

    // E a declaração gerada por macro continua não-expandida, do jeito que C6
    // já exigia: a invocação é ocorrência textual, nunca declaração.
    let invocation = locate_json(repo.path(), "macro_made_fn");
    assert_eq!(count(&invocation, "EXTRACTED_CANDIDATE"), 0, "{invocation}");
    assert_eq!(count(&invocation, "TEXTUAL_OCCURRENCE"), 1, "{invocation}");
}

// F6 — a fronteira do token tree de macro é posicional, não por linha.
//
// Uma marca por linha responde apenas "esta linha começa dentro de macro?", e
// isso é granular demais em ambos os sentidos: a mesma linha pode começar fora
// e abrir o token tree logo em seguida, ou começar dentro e continuar com
// código externo depois do fechamento. Cada bloco abaixo ataca uma dessas duas
// mentiras, mais o aninhamento, a máscara lexical e a contaminação de contexto
// estrutural.
#[test]
fn fronteira_de_macro_e_posicional_nao_por_linha() {
    let repo = fixture("f6");
    let path = "src/macro_boundary.rs";
    write(repo.path(), path, MACRO_BOUNDARY);

    // Negativos: o nome só existe dentro do token tree, então ele nunca é
    // declaração — mas continua pesquisável como ocorrência textual limitada.
    for (case, name) in [
        // Abertura na mesma linha do primeiro token interno, nos três
        // delimitadores. É aqui que a marca por linha dizia "fora".
        ("SAME_LINE_CURLY", "GhostCurlySameLine"),
        ("SAME_LINE_PAREN", "GhostParenSameLine"),
        ("SAME_LINE_BRACKET", "GhostBracketSameLine"),
        // Interior de um token tree aberto em linha própria.
        ("MACRO_INTERNAL", "InternalOnly"),
        // Aninhamento: a pilha de delimitadores precisa provar seu valor.
        ("NESTED", "NestedGhost"),
        // Contaminação de contexto: nem o contêiner nem o item interno saem.
        ("CONTAMINATION_MOD", "FakeContainer"),
        ("CONTAMINATION_IMPL", "FakeThing"),
        ("CONTAMINATION_ITEM", "ghost"),
    ] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXPLICIT_SYMBOL"),
            0,
            "{case}: {name} virou identidade: {json}"
        );
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            0,
            "{case}: {name} virou declaração estrutural: {json}"
        );
        assert_eq!(
            count(&json, "TEXTUAL_OCCURRENCE"),
            1,
            "{case}: {name} perdeu o fallback textual: {json}"
        );
        assert!(
            json.contains("\"limitation\":\"macro_token_tree_not_a_declaration\""),
            "{case}: {name} sem a limitação de macro: {json}"
        );
    }

    // O token tree não é apagado da visão pesquisada: o trecho devolvido é a
    // linha original inteira, com os delimitadores no lugar. Apagar o intervalo
    // uniria tokens vizinhos e fabricaria sintaxe que a fonte não tem.
    let curly = locate_json(repo.path(), "GhostCurlySameLine");
    assert!(
        curly.contains("\"snippet\":\"{ pub struct GhostCurlySameLine; }\""),
        "o trecho textual deixou de vir da fonte observada: {curly}"
    );

    // Positivos: a fronteira fecha no delimitador correspondente, e o que vem
    // depois continua sendo Rust ordinário — inclusive na mesma linha do
    // fechamento. Intervalo e contexto estrutural são afirmados junto com a
    // classe: provar exclusão sem provar fechamento não prova fronteira.
    for (case, name, kind, context) in [
        ("AFTER_MACRO", "LegitimateAfterMacro", "struct", "null"),
        ("AFTER_NESTED", "AfterNestedMacro", "struct", "null"),
        ("AFTER_LEXICAL_MASK", "AfterLexicalMask", "struct", "null"),
        // Fechamento e código externo na mesma linha: a representação distingue
        // posição antes e depois do fecho, e o contexto não herda nada do
        // interior do token tree.
        (
            "AFTER_SAME_LINE_CLOSE",
            "real_after_macro",
            "function",
            "null",
        ),
        (
            "NEGATION_AND_INEQUALITY",
            "boundary_negation",
            "function",
            "null",
        ),
        ("NEVER_TYPE", "boundary_never", "function", "null"),
        ("AFTER_BANG_FORMS", "AfterBoundaryBang", "struct", "null"),
    ] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            1,
            "{case}: {name} deixou de ser declaração estrutural: {json}"
        );
        let start = line_of(MACRO_BOUNDARY, name);
        assert!(
            json.contains(&format!(
                "\"classification\":\"EXTRACTED_CANDIDATE\",\"path\":\"{path}\",\"kind\":\"{kind}\",\"name\":\"{name}\",\"start\":{start},\"end\":{start},\"context\":{context},"
            )),
            "{case}: {name} perdeu caminho, intervalo ou contexto: {json}"
        );
        assert!(
            !json.contains("FakeContainer") && !json.contains("FakeThing"),
            "{case}: contexto de dentro do token tree vazou para {name}: {json}"
        );
    }

    // Delimitador dentro de comentário ou literal não abre nem fecha intervalo:
    // a detecção parte da visão lexical já mascarada, e o texto mascarado não é
    // pesquisável nem como ocorrência textual.
    for name in ["GhostInString", "GhostInLineComment", "GhostInBlockComment"] {
        let json = locate_json(repo.path(), name);
        assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 0, "{name}: {json}");
        assert_eq!(count(&json, "TEXTUAL_OCCURRENCE"), 0, "{name}: {json}");
    }
}

// C14 — a proveniência do binário é observável, então um `pink` plausível de
// outro commit não pode se passar pela autoridade da Task.
#[test]
fn proveniencia_do_binario_e_observavel() {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--version-json")
        .output()
        .unwrap();
    assert_success(&output);
    let json = stdout(&output);
    assert!(json.contains("\"binary_commit\":"), "{json}");
    assert!(json.contains("\"binary_path\":"), "{json}");
}

// Trecho e intervalo correspondem, e o corte é declarado em vez de disfarçado.
#[test]
fn trecho_corresponde_ao_intervalo_e_declara_o_corte() {
    let repo = fixture("snippet");
    let long_name = "a".repeat(symbol_extraction::SNIPPET_CHAR_BUDGET + 40);
    write(
        repo.path(),
        "src/long.rs",
        &format!(
            "pub fn wide_signature(\n    first: usize,\n    // {long_name}\n    second: usize,\n) -> usize {{\n    first + second\n}}\n"
        ),
    );
    let json = locate_json(repo.path(), "wide_signature");
    let candidate = extracted_at(&json, "src/long.rs", 1);
    assert!(candidate.contains("\"end\":5"), "{candidate}");
    assert!(
        candidate.contains("\"snippet_truncated\":true"),
        "{candidate}"
    );
    assert!(!candidate.contains("full"), "{candidate}");

    let source = fs::read_to_string(repo.path().join("src/long.rs")).unwrap();
    let interval = source.lines().take(5).collect::<Vec<_>>().join("\n");
    let prefix: String = interval
        .trim()
        .chars()
        .take(symbol_extraction::SNIPPET_CHAR_BUDGET)
        .collect();
    let escaped = prefix
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    assert!(
        candidate.contains(&escaped),
        "trecho não corresponde ao intervalo observado: {candidate}"
    );
}

// As limitações viajam como dado estável em inglês, não como prosa opcional.
#[test]
fn limitacoes_sao_dado_estavel_em_ingles() {
    let repo = fixture("limitations");
    let json = locate_json(repo.path(), "unregistered_helper");
    for limitation in symbol_extraction::DECLARED_LIMITATIONS {
        assert!(json.contains(limitation), "ausente {limitation}: {json}");
        assert!(
            limitation
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()),
            "limitação fora do vocabulário canônico: {limitation}"
        );
    }
}

// Nome grafado não é identidade resolvida: o candidato extraído nunca carrega
// identidade, estabilidade ou vínculo explícito.
#[test]
fn candidato_extraido_nunca_vira_identidade_semantica() {
    let repo = fixture("identity");
    let json = locate_json(repo.path(), "unregistered_helper");
    let block = json
        .split("\"extracted_candidates\":[")
        .nth(1)
        .unwrap()
        .split("],\"textual_occurrences\"")
        .next()
        .unwrap();
    for forbidden in [
        "\"identity\"",
        "\"stability\"",
        "\"documentation\"",
        "\"tests\"",
        "\"regions\"",
        "EXPLICIT_SYMBOL",
    ] {
        assert!(
            !block.contains(forbidden),
            "candidato extraído carrega {forbidden}: {block}"
        );
    }
}

/// Separadores léxicos entre o caminho, o `!` e o delimitador de uma macro.
///
/// A sequência reconhecida é de tokens, não de bytes adjacentes: espaço, quebra
/// de linha e comentário comum separam `SimplePath`, `!` e `DelimTokenTree` sem
/// desfazer a sequência. Comentário de documentação é atributo em Rust e por
/// isso a desfaz. Palavra-chave não é identificador e nunca abre um
/// `SimplePath`, então `return !` e `if !` continuam sendo negação.
const MACRO_SEPARATION: &str = r####"some_macro ! {
    pub struct GhostSpace;
}

pub struct RealAfterSpace;

some_macro
!
(
    pub struct GhostNewline;
);

pub struct RealAfterNewline;

some_macro /* ordinary comment */ ! [
    pub struct GhostComment;
];

pub struct RealAfterComment;

macro_rules ! spaced_definition {
    () => {
        pub struct GhostDefinition;
    };
}

pub struct RealAfterDefinition;

foo::bar ! {
    pub struct GhostQualified;
}

pub struct RealAfterQualified;

r#macro_name ! {
    pub struct GhostRaw;
}

pub struct RealAfterRaw;

r#type ! {
    pub struct GhostRawKeyword;
}

pub struct RealAfterRawKeyword;

not_a_macro
/// Atributo de documentação, não espaço em branco.
!
{
    pub struct DocSeparated;
}

pub fn separation_return() -> bool {
    return !{
        struct RealInsideBlock;
        false
    };
}

pub fn separation_paren() -> bool {
    return !({
        struct RealInsideParen;
        false
    });
}

pub fn separation_if(flag: bool) -> bool {
    if !{
        struct RealInsideCondition;
        flag
    } {
        return true;
    }
    false
}

pub fn separation_bang_forms(flag: bool) -> bool {
    !flag && 1 != 2
}

pub fn separation_never() -> ! {
    loop {}
}

pub struct RealAfterBangForms;
"####;

/// A mesma invocação válida, variando só o separador. A classificação precisa
/// ser a mesma nas cinco formas: os deslocamentos mudam, a fronteira não.
const MACRO_SEPARATORS: &str = r####"foo!{
    pub struct MetaCompact;
}

foo ! {
    pub struct MetaSpace;
}

foo
!
{
    pub struct MetaNewline;
}

foo /* ordinary comment */ ! {
    pub struct MetaCommentBefore;
}

foo ! /* ordinary comment */ {
    pub struct MetaCommentAfter;
}

pub struct RealAfterSeparators;
"####;

// Espaço em branco, quebra de linha e comentário comum entre `SimplePath`, `!`
// e o delimitador não desfazem a invocação; comentário de documentação e
// palavra-chave desfazem.
#[test]
fn separador_lexical_entre_caminho_e_bang_nao_desfaz_a_macro() {
    let repo = fixture("separation");
    let path = "src/macro_separation.rs";
    write(repo.path(), path, MACRO_SEPARATION);

    // Negativos: o nome só existe dentro do token tree, então perder o caminho
    // candidato por causa do separador o promoveria a declaração inventada.
    for (case, name) in [
        ("SPACE_BEFORE_BANG", "GhostSpace"),
        ("NEWLINE_AROUND_BANG", "GhostNewline"),
        ("COMMENT_BEFORE_BANG", "GhostComment"),
        ("SPACED_MACRO_RULES", "GhostDefinition"),
        ("QUALIFIED_PATH", "GhostQualified"),
        ("RAW_IDENTIFIER", "GhostRaw"),
        ("RAW_IDENTIFIER_SPELLED_AS_KEYWORD", "GhostRawKeyword"),
    ] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXPLICIT_SYMBOL"),
            0,
            "{case}: {name} virou identidade: {json}"
        );
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            0,
            "{case}: {name} virou declaração estrutural: {json}"
        );
        assert_eq!(
            count(&json, "TEXTUAL_OCCURRENCE"),
            1,
            "{case}: {name} perdeu o fallback textual: {json}"
        );
        assert!(
            json.contains("\"limitation\":\"macro_token_tree_not_a_declaration\""),
            "{case}: {name} sem a limitação de macro: {json}"
        );
    }

    // Positivos: a declaração seguinte continua estrutural nos três
    // delimitadores, e o que a linguagem recusa como `SimplePath !` continua
    // sendo Rust ordinário observável.
    for (case, name) in [
        ("AFTER_SPACE_CURLY", "RealAfterSpace"),
        ("AFTER_NEWLINE_PAREN", "RealAfterNewline"),
        ("AFTER_COMMENT_BRACKET", "RealAfterComment"),
        ("AFTER_SPACED_MACRO_RULES", "RealAfterDefinition"),
        ("AFTER_QUALIFIED_PATH", "RealAfterQualified"),
        ("AFTER_RAW_IDENTIFIER", "RealAfterRaw"),
        ("AFTER_RAW_KEYWORD", "RealAfterRawKeyword"),
        // Comentário de documentação tem semântica de atributo: ele não separa
        // `not_a_macro` do `!`, então nenhuma macro foi reconhecida ali.
        ("DOC_COMMENT_IS_NOT_WHITESPACE", "DocSeparated"),
        // `return` e `if` são palavras-chave, não identificadores de caminho.
        ("KEYWORD_RETURN_BLOCK", "RealInsideBlock"),
        ("KEYWORD_RETURN_PAREN", "RealInsideParen"),
        ("KEYWORD_IF_CONDITION", "RealInsideCondition"),
        // `!flag`, `1 != 2` e `-> !` seguem fora do reconhecimento.
        ("BANG_FORMS_PRESERVED", "RealAfterBangForms"),
    ] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            1,
            "{case}: {name} deixou de ser declaração estrutural: {json}"
        );
        let start = line_of(MACRO_SEPARATION, name);
        assert!(
            json.contains(&format!(
                "\"classification\":\"EXTRACTED_CANDIDATE\",\"path\":\"{path}\",\"kind\":\"struct\",\"name\":\"{name}\",\"start\":{start},\"end\":{start},\"context\":null,"
            )),
            "{case}: {name} perdeu caminho, intervalo ou contexto: {json}"
        );
    }
}

// Metamórfico: uma invocação válida, cinco separadores, uma classificação.
#[test]
fn classificacao_de_macro_e_invariante_ao_separador() {
    let repo = fixture("separators");
    let path = "src/macro_separators.rs";
    write(repo.path(), path, MACRO_SEPARATORS);

    for name in [
        "MetaCompact",
        "MetaSpace",
        "MetaNewline",
        "MetaCommentBefore",
        "MetaCommentAfter",
    ] {
        let json = locate_json(repo.path(), name);
        assert_eq!(
            count(&json, "EXTRACTED_CANDIDATE"),
            0,
            "{name}: o separador mudou a classe: {json}"
        );
        assert_eq!(
            count(&json, "TEXTUAL_OCCURRENCE"),
            1,
            "{name}: o separador apagou o fallback textual: {json}"
        );
        assert!(
            json.contains("\"limitation\":\"macro_token_tree_not_a_declaration\""),
            "{name}: o separador apagou a limitação de macro: {json}"
        );
    }

    let json = locate_json(repo.path(), "RealAfterSeparators");
    let start = line_of(MACRO_SEPARATORS, "RealAfterSeparators");
    assert_eq!(count(&json, "EXTRACTED_CANDIDATE"), 1, "{json}");
    assert!(
        json.contains(&format!(
            "\"classification\":\"EXTRACTED_CANDIDATE\",\"path\":\"{path}\",\"kind\":\"struct\",\"name\":\"RealAfterSeparators\",\"start\":{start},\"end\":{start},\"context\":null,"
        )),
        "a última fronteira não fechou: {json}"
    );
}
// @pinker-nav:end evidencia.simbolos.extracao
