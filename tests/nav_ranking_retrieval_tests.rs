//! Evidência da #672 (T0): relevância do `pink nav buscar` e recuperação
//! compacta verificada do `pink nav mostrar`.
//!
//! Os testes exercitam a superfície pública real (processo `pink`), porque a
//! propriedade a provar pertence ao agente que consulta, não a um helper.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
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

/// Região implementadora do alvo conceitual.
const SRC_ALVO: &str = "// @pinker-nav:start alvo.relogio.monotonico\n// @pinker-nav:domain relogio\n// @pinker-nav:layer alvo\n// @pinker-nav:summary Fonte monotonica de tempo usada pelo agendador.\nfn relogio() -> u64 {\n    7\n}\n// @pinker-nav:end alvo.relogio.monotonico\n";

/// Competidora: cita o termo comum em toda parte, mas não o termo raro.
const SRC_RUIDO: &str = "// @pinker-nav:start ruido.tempo.contabilidade\n// @pinker-nav:domain tempo\n// @pinker-nav:layer ruido\n// @pinker-nav:summary Contabilidade de tempo de execucao usada pelo tempo de relatorio de tempo.\nfn ruido() -> u64 {\n    1\n}\n// @pinker-nav:end ruido.tempo.contabilidade\n// @pinker-nav:start ruido.tempo.segunda\n// @pinker-nav:domain tempo\n// @pinker-nav:layer ruido\n// @pinker-nav:summary Outra regiao de tempo sem qualquer mencao ao termo raro.\nfn outra() -> u64 {\n    2\n}\n// @pinker-nav:end ruido.tempo.segunda\n";

const RUNTIME_LIB: &str = "// @pinker-nav:start runtime.tempo.terceira\n// @pinker-nav:domain tempo\n// @pinker-nav:layer runtime\n// @pinker-nav:summary Terceira regiao de tempo, tambem sem o termo raro.\npub fn terceira() -> i32 {\n    3\n}\n// @pinker-nav:end runtime.tempo.terceira\n";

fn pink() -> &'static str {
    env!("CARGO_BIN_EXE_pink")
}

fn repo_root() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

fn temp_repo(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_navrank_{name}_{now}"))
}

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fixture(root: &Path) {
    write(root, ".pinker/doc.toml", DOC_TOML);
    write(root, "src/alvo.rs", SRC_ALVO);
    write(root, "src/ruido.rs", SRC_RUIDO);
    write(root, "runtime/pinker_rt/src/lib.rs", RUNTIME_LIB);
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::create_dir_all(root.join("apps")).unwrap();
    let sync = nav(root, &["sincronizar"]);
    assert!(
        sync.status.success(),
        "{}",
        String::from_utf8_lossy(&sync.stderr)
    );
}

fn nav(root: &Path, args: &[&str]) -> Output {
    Command::new(pink())
        .arg("nav")
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("executar pink nav")
}

fn nav_real(args: &[&str]) -> Output {
    Command::new(pink())
        .arg("nav")
        .arg("--repo")
        .arg(repo_root())
        .args(args)
        .output()
        .expect("executar pink nav")
}

/// Extrai, em ordem, as chaves de uma saída JSON de `nav buscar` sem depender
/// de parser externo: o formato é estável e as chaves aparecem uma vez por
/// resultado.
fn ordered_keys(stdout: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(stdout);
    let Some(results) = text.split("\"results\":[").nth(1) else {
        return Vec::new();
    };
    results
        .split("{\"key\":\"")
        .skip(1)
        .filter_map(|chunk| chunk.split('"').next().map(str::to_string))
        .collect()
}

fn json_field(stdout: &[u8], field: &str) -> String {
    let text = String::from_utf8_lossy(stdout).to_string();
    let needle = format!("\"{field}\":");
    let rest = text
        .split(&needle)
        .nth(1)
        .unwrap_or_else(|| panic!("campo '{field}' ausente em {text}"))
        .to_string();
    let rest = rest.trim_start();
    if let Some(stripped) = rest.strip_prefix('"') {
        stripped.split('"').next().unwrap_or_default().to_string()
    } else {
        rest.chars()
            .take_while(|c| c.is_ascii_digit() || c.is_ascii_alphabetic() || *c == '-')
            .collect()
    }
}

#[test]
fn termo_raro_vence_termo_comum_repetido() {
    let root = temp_repo("raridade");
    fixture(&root);

    let out = nav(&root, &["buscar", "relogio monotonico", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let keys = ordered_keys(&out.stdout);
    assert_eq!(
        keys.first().map(String::as_str),
        Some("alvo.relogio.monotonico"),
        "ordem observada: {keys:?}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn termo_repetido_nao_amplifica_relevancia() {
    let root = temp_repo("repeticao");
    fixture(&root);

    let uma = nav(&root, &["buscar", "tempo", "--json"]);
    let tres = nav(&root, &["buscar", "tempo tempo tempo", "--json"]);
    assert_eq!(ordered_keys(&uma.stdout), ordered_keys(&tres.stdout));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn consulta_vazia_nao_devolve_o_catalogo_inteiro() {
    let root = temp_repo("vazia");
    fixture(&root);

    let out = nav(&root, &["buscar", "", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(ordered_keys(&out.stdout).is_empty());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn toda_chave_estavel_continua_recuperavel_em_primeiro_lugar() {
    let catalogo = fs::read_to_string(Path::new(repo_root()).join("src/navigation.jsonl"))
        .expect("catálogo real");
    let chaves: Vec<String> = catalogo
        .lines()
        .filter_map(|line| line.split("\"key\":\"").nth(1))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_string)
        .collect();
    assert!(chaves.len() > 100, "catálogo real inesperadamente pequeno");

    let indice =
        pinker_v0::nav::CodeCatalog::load(&Path::new(repo_root()).join("src/navigation.jsonl"))
            .expect("carregar catálogo");
    let mut regressoes: Vec<&str> = Vec::new();
    for chave in &chaves {
        let hits = indice.search_ranked(chave);
        match hits.hits.first() {
            Some(primeiro) if primeiro.region.key == *chave => {}
            _ => regressoes.push(chave),
        }
    }
    assert!(
        regressoes.is_empty(),
        "chaves que deixaram de ser recuperáveis em primeiro lugar: {regressoes:?}"
    );
}

#[test]
fn a_mesma_consulta_produz_a_mesma_ordem() {
    let primeira = nav_real(&["buscar", "alocador de memoria", "--json"]);
    let segunda = nav_real(&["buscar", "alocador de memoria", "--json"]);
    assert_eq!(primeira.stdout, segunda.stdout);
    assert!(!ordered_keys(&primeira.stdout).is_empty());
}

#[test]
fn consultas_conceituais_acertam_a_regiao_implementadora() {
    let casos = [
        ("normalizacao de consultas", "trama.consultas.normalizacao"),
        ("leitor de elf", "build.elf.leitor"),
        ("gramatica de tipos", "parser.tipos.gramatica"),
        ("canonicalizacao de unioes", "union.unioes.canonicalizacao"),
        (
            "rotulo injetivo de simbolo nativo",
            "nativo.simbolo.rotulo-injetivo",
        ),
    ];
    let mut falhas: Vec<String> = Vec::new();
    for (consulta, esperado) in casos {
        let out = nav_real(&["buscar", consulta, "--json", "--limite", "5"]);
        let keys = ordered_keys(&out.stdout);
        if keys.first().map(String::as_str) != Some(esperado) {
            falhas.push(format!("{consulta} -> {keys:?} (esperado {esperado})"));
        }
    }
    assert!(
        falhas.is_empty(),
        "consultas sem a região correta em 1º: {falhas:#?}"
    );
}

#[test]
fn buscar_declara_contagem_truncamento_e_continuacao() {
    let out = nav_real(&["buscar", "lowering", "--json", "--limite", "3"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(json_field(&out.stdout, "returned_results"), "3");
    assert_eq!(json_field(&out.stdout, "truncated"), "true");
    assert_eq!(json_field(&out.stdout, "continuation_desde"), "3");
    let total: usize = json_field(&out.stdout, "total_results").parse().unwrap();
    assert!(total > 3);

    let pagina1 = ordered_keys(&out.stdout);
    let seguinte = nav_real(&[
        "buscar", "lowering", "--json", "--limite", "3", "--desde", "3",
    ]);
    let pagina2 = ordered_keys(&seguinte.stdout);
    assert_eq!(json_field(&seguinte.stdout, "offset"), "3");
    assert_eq!(pagina2.len(), 3);
    for chave in &pagina2 {
        assert!(!pagina1.contains(chave), "continuação repetiu {chave}");
    }

    let texto = nav_real(&["buscar", "lowering", "--limite", "3"]);
    let saida = String::from_utf8_lossy(&texto.stdout);
    assert!(
        saida.contains("3 de "),
        "saída textual sem contagem: {saida}"
    );
    assert!(
        saida.contains("--desde 3"),
        "saída textual sem continuação: {saida}"
    );
}

#[test]
fn buscar_sem_truncamento_ainda_declara_a_contagem() {
    let root = temp_repo("contagem");
    fixture(&root);

    let out = nav(&root, &["buscar", "relogio monotonico", "--limite", "20"]);
    let saida = String::from_utf8_lossy(&out.stdout);
    assert!(saida.contains(" de "), "{saida}");
    assert!(
        !saida.contains("--desde"),
        "não truncou, não deve oferecer continuação: {saida}"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mostrar_resumo_verifica_sem_devolver_corpo() {
    let out = nav_real(&[
        "mostrar",
        "trama.consultas.normalizacao",
        "--resumo",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let texto = String::from_utf8_lossy(&out.stdout);
    assert_eq!(json_field(&out.stdout, "body_included"), "false");
    assert_eq!(json_field(&out.stdout, "verified"), "true");
    assert_eq!(json_field(&out.stdout, "truncated"), "false");
    assert!(
        !texto.contains("\"content\":"),
        "resumo não deve trazer corpo: {texto}"
    );
    assert!(
        texto.contains("\"summary\":"),
        "resumo deve trazer o summary: {texto}"
    );
    assert!(
        texto.contains("\"hash\":"),
        "resumo deve trazer o hash verificado: {texto}"
    );

    let completo = nav_real(&["mostrar", "trama.consultas.normalizacao", "--json"]);
    let total_completo: usize = json_field(&completo.stdout, "total_lines").parse().unwrap();
    let total_resumo: usize = json_field(&out.stdout, "total_lines").parse().unwrap();
    assert_eq!(total_completo, total_resumo);
    assert!(out.stdout.len() < completo.stdout.len());
}

#[test]
fn mostrar_com_orcamento_declara_truncamento_e_reconstroi_o_corpo() {
    let completo = nav_real(&["mostrar", "trama.consultas.normalizacao", "--json"]);
    assert_eq!(json_field(&completo.stdout, "truncated"), "false");
    assert_eq!(json_field(&completo.stdout, "body_included"), "true");
    let total: usize = json_field(&completo.stdout, "total_lines").parse().unwrap();
    assert!(total > 4);

    let parcial = nav_real(&[
        "mostrar",
        "trama.consultas.normalizacao",
        "--json",
        "--linhas",
        "2",
    ]);
    assert_eq!(json_field(&parcial.stdout, "truncated"), "true");
    assert_eq!(json_field(&parcial.stdout, "returned_lines"), "2");
    assert_eq!(json_field(&parcial.stdout, "returned_from"), "1");
    assert_eq!(json_field(&parcial.stdout, "continuation_desde"), "3");

    let resto = nav_real(&[
        "mostrar",
        "trama.consultas.normalizacao",
        "--json",
        "--desde",
        "3",
    ]);
    assert_eq!(json_field(&resto.stdout, "truncated"), "false");
    assert_eq!(json_field(&resto.stdout, "returned_from"), "3");
    let devolvidas: usize = json_field(&resto.stdout, "returned_lines").parse().unwrap();
    assert_eq!(
        devolvidas + 2,
        total,
        "as páginas devem reconstruir o corpo inteiro"
    );

    let texto = nav_real(&["mostrar", "trama.consultas.normalizacao", "--linhas", "2"]);
    let saida = String::from_utf8_lossy(&texto.stdout);
    assert!(
        saida.contains("truncado:"),
        "truncamento textual não declarado: {saida}"
    );
    assert!(
        saida.contains("--desde 3"),
        "continuação textual ausente: {saida}"
    );
}

#[test]
fn resumo_recusa_regiao_derivada_da_fonte() {
    let root = temp_repo("drift");
    fixture(&root);

    // A fonte muda depois do catálogo: o hash da região deixa de bater.
    let derivado = SRC_ALVO.replace("    7\n", "    8\n");
    write(&root, "src/alvo.rs", &derivado);

    let completo = nav(&root, &["mostrar", "alvo.relogio.monotonico"]);
    assert_eq!(completo.status.code(), Some(5));

    let resumo = nav(
        &root,
        &["mostrar", "alvo.relogio.monotonico", "--resumo", "--json"],
    );
    assert_eq!(
        resumo.status.code(),
        Some(5),
        "modo compacto não pode declarar verificado o que derivou: {}",
        String::from_utf8_lossy(&resumo.stdout)
    );
    assert!(String::from_utf8_lossy(&resumo.stderr).contains("E-NAV-SOURCE"));
    assert!(!String::from_utf8_lossy(&resumo.stdout).contains("\"verified\":true"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn estrito_abstem_quando_o_catalogo_desconhece_o_vocabulario() {
    let out = nav_real(&[
        "buscar",
        "coletor de lixo geracional do interpretador",
        "--estrito",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "abstenção usa o código de 'sem resultado': {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(json_field(&out.stdout, "abstained"), "true");
    assert_eq!(
        json_field(&out.stdout, "abstention_reason"),
        "vocabulario_desconhecido"
    );
    assert!(ordered_keys(&out.stdout).is_empty());

    // O comportamento padrão da MESMA consulta não mudou: continua devolvendo
    // o que o ranqueamento encontrar, sem abstenção.
    let padrao = nav_real(&["buscar", "coletor de lixo geracional do interpretador"]);
    assert_eq!(padrao.status.code(), Some(0), "{}", stderr_de(&padrao));
    assert!(!padrao.stdout.is_empty());
}

#[test]
fn estrito_abstem_quando_nenhuma_regiao_cobre_a_consulta_inteira() {
    // Todos os termos existem no catálogo; nenhuma região cobre todos eles.
    let out = nav_real(&[
        "buscar",
        "contagem de referencias para liberar memoria",
        "--estrito",
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(json_field(&out.stdout, "abstained"), "true");
    assert_eq!(
        json_field(&out.stdout, "abstention_reason"),
        "cobertura_insuficiente"
    );

    let padrao = nav_real(&["buscar", "contagem de referencias para liberar memoria"]);
    assert_eq!(padrao.status.code(), Some(0), "{}", stderr_de(&padrao));
}

#[test]
fn estrito_nao_rejeita_consulta_fraca_mas_valida() {
    let casos = [
        ("aleatorio", "runtime.aleatorio.gerador"),
        (
            "alinhamento e offsets de campos de struct",
            "layout.tipos.memoria",
        ),
        ("normalizacao de consultas", "trama.consultas.normalizacao"),
    ];
    for (consulta, esperado) in casos {
        let out = nav_real(&["buscar", consulta, "--estrito", "--json", "--limite", "5"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "modo estrito abstendo de consulta respondível '{consulta}': {}",
            String::from_utf8_lossy(&out.stdout)
        );
        let keys = ordered_keys(&out.stdout);
        assert_eq!(
            keys.first().map(String::as_str),
            Some(esperado),
            "estrito devolveu {keys:?} para '{consulta}'"
        );
    }
}

#[test]
fn estrito_pertence_somente_a_buscar() {
    let out = nav_real(&["mostrar", "trama.consultas.normalizacao", "--estrito"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr_de(&out));
    assert!(stderr_de(&out).contains("--estrito"));
}

fn stderr_de(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
