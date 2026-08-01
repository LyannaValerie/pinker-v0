//! Paridade de contabilidade de uniões entre interpretador e backend nativo.
//!
//! O contrato verificado aqui é o observável de fora: uma chamada bem-sucedida
//! de `alocar` consome exatamente uma identidade pública, construir e extrair
//! uniões agregadas não consome nenhuma, e os dois backends produzem o mesmo
//! stdout e o mesmo exit para a mesma sequência de construções.
//!
//! A contagem interna de cada domínio é provada nos testes de unidade do
//! interpretador (`src/interpreter.rs`) e do runtime (`runtime/pinker_rt`),
//! que enxergam o estado. Esta suíte guarda a fronteira externa e a forma do
//! código nativo.

mod common;

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.unioes.contabilidade-paridade
// @pinker-nav:domain unioes
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência externa da paridade de contabilidade de uniões: os exemplos de domínios independentes e de reinjeção produzem o mesmo stdout e o mesmo exit no interpretador e no binário nativo, o backend materializa o binding de extração em slot do frame sem chamar o alocador público, e a construção de união não emite chamada ao alocador público em nenhum ponto do caminho de injeção.

/// Executa o exemplo pelo CLI para observar exatamente o stdout do
/// interpretador, que é o mesmo canal comparado com o binário nativo.
fn interpretado(exemplo: &str) -> (Vec<String>, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", exemplo])
        .output()
        .expect("execução do interpretador");
    assert!(
        output.stderr.is_empty(),
        "stderr interpretado deveria ser vazio: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let linhas = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    (linhas, output.status.code())
}

/// Compila e executa o exemplo pelo caminho nativo, devolvendo stdout e exit.
fn nativo(exemplo: &str) -> Option<(Vec<String>, Option<i32>)> {
    let (_driver, Some(runtime_lib)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)?
    else {
        return None;
    };
    let pink = env!("CARGO_BIN_EXE_pink");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_uniao_contabilidade_{nanos}"));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nome = std::path::Path::new(exemplo)
        .file_stem()
        .expect("nome do exemplo");
    let run = Command::new(out_dir.join(nome))
        .output()
        .expect("falha ao executar binário nativo");
    assert!(
        run.stderr.is_empty(),
        "stderr nativo deveria ser vazio: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let linhas = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    let codigo = run.status.code();

    let _ = std::fs::remove_dir_all(&out_dir);
    Some((linhas, codigo))
}

/// Confere que os dois backends concordam em stdout e exit, e que o stdout é o
/// esperado. Sem evidência nativa disponível, o interpretador ainda é exigido.
fn paridade(exemplo: &str, esperado: &[&str]) {
    let (linhas_interpretadas, exit_interpretado) = interpretado(exemplo);
    assert_eq!(
        linhas_interpretadas, esperado,
        "stdout interpretado divergente"
    );
    let Some((linhas_nativas, exit_nativo)) = nativo(exemplo) else {
        return;
    };
    assert_eq!(linhas_nativas, esperado, "stdout nativo divergente");
    assert_eq!(
        exit_interpretado, exit_nativo,
        "os dois backends devem concordar no exit"
    );
}

// ---------------------------------------------------------------------------
// Paridade observável
// ---------------------------------------------------------------------------

/// Oito construções agregadas seguidas não reduzem a capacidade pública: a
/// alocação feita depois do laço continua funcionando nos dois backends.
#[test]
fn dominios_independentes_tem_paridade_de_stdout_e_exit() {
    paridade(
        "examples/uniao_contabilidade_dominios_valido.pink",
        &["808", "7"],
    );
}

/// Extrair e reinjetar o binding não duplica consumo nem muda o valor
/// observado, nos dois backends.
#[test]
fn reinjecao_de_binding_tem_paridade_de_stdout_e_exit() {
    paridade(
        "examples/uniao_contabilidade_reinjecao_valido.pink",
        &["31", "31"],
    );
}

// ---------------------------------------------------------------------------
// Limites: uma política, duas cópias na fronteira da ABI
// ---------------------------------------------------------------------------

/// Os tetos do domínio interno de união são duplicados no runtime nativo porque
/// o runtime não depende do compilador — a duplicação é a fronteira da ABI, não
/// uma segunda política. Sem este guardião, os dois lados podem divergir em
/// silêncio e a mesma sequência de construções passa a atingir limites
/// diferentes em cada backend.
#[test]
fn os_limites_do_dominio_interno_sao_identicos_nos_dois_backends() {
    let fonte = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/pinker_rt/src/lib.rs"),
    )
    .expect("fonte do runtime nativo");

    let constante_nativa = |nome: &str| -> u64 {
        let prefixo = format!("const {nome}: u64 = ");
        let linha = fonte
            .lines()
            .map(str::trim)
            .find(|linha| linha.starts_with(&prefixo))
            .unwrap_or_else(|| panic!("constante '{nome}' ausente no runtime nativo"));
        let expressao: String = linha[prefixo.len()..]
            .trim_end_matches(';')
            .chars()
            .filter(|caractere| *caractere != '_' && *caractere != ' ')
            .collect();
        // As formas usadas são literais e produtos de literais.
        expressao
            .split('*')
            .map(|fator| {
                fator
                    .parse::<u64>()
                    .unwrap_or_else(|_| panic!("fator não literal em '{nome}': {fator}"))
            })
            .product()
    };

    for (nome, canonico) in [
        (
            "MAX_UNION_PAYLOAD_BYTES",
            pinker_v0::union_payload::MAX_UNION_PAYLOAD_BYTES,
        ),
        (
            "MAX_UNION_PAYLOAD_ALIGN",
            pinker_v0::union_payload::MAX_UNION_PAYLOAD_ALIGN,
        ),
        (
            "MAX_UNION_DESCRIPTORS",
            pinker_v0::union_payload::MAX_UNION_DESCRIPTORS,
        ),
        (
            "MAX_UNION_TOTAL_PAYLOAD_BYTES",
            pinker_v0::union_payload::MAX_UNION_TOTAL_PAYLOAD_BYTES,
        ),
        (
            "UNION_DESCRIPTOR_METADATA_BYTES",
            pinker_v0::union_payload::UNION_DESCRIPTOR_METADATA_BYTES,
        ),
    ] {
        assert_eq!(
            constante_nativa(nome),
            canonico,
            "o teto '{nome}' divergiu entre o compilador e o runtime nativo: a mesma sequência de \
             construções atingiria limites diferentes em cada backend"
        );
    }
}

// ---------------------------------------------------------------------------
// Forma do código nativo
// ---------------------------------------------------------------------------

/// O binding de extração é um slot do frame. Se o backend passasse a
/// materializá-lo por `pinker_publico_alocar`, o storage interno voltaria a
/// consumir identidade pública no nativo — este teste recusa exatamente isso.
#[test]
fn extracao_agregada_nao_chama_o_alocador_publico_no_nativo() {
    let asm = common::render_backend_s_external_subset_nativo(include_str!(
        "../examples/uniao_contabilidade_reinjecao_valido.pink"
    ))
    .expect("assembly");

    assert!(asm.contains("call pinker_uniao_criar"), "{asm}");
    assert!(asm.contains("call pinker_uniao_copiar_payload"), "{asm}");

    // O exemplo tem exatamente uma chamada pública de `alocar`, a da origem.
    let chamadas_publicas = asm.matches("call pinker_publico_alocar").count();
    assert_eq!(
        chamadas_publicas, 1,
        "só o `alocar` da origem pode chamar o alocador público:\n{asm}"
    );

    // Logo depois de cada cópia de payload agregado, o endereço devolvido vem
    // do frame — nunca de uma chamada ao alocador público.
    for (indice, _) in asm.match_indices("call pinker_uniao_copiar_payload") {
        let seguintes: Vec<&str> = asm[indice..]
            .lines()
            .skip(1)
            .take(3)
            .map(str::trim)
            .collect();
        assert!(
            !seguintes
                .iter()
                .any(|linha| linha.contains("pinker_publico_alocar")),
            "a extração não pode alocar região pública: {seguintes:?}"
        );
    }
}

/// A injeção lê a origem e cria o descritor pelo runtime de uniões; nenhuma
/// identidade pública é reservada no caminho da construção.
#[test]
fn construcao_agregada_nao_chama_o_alocador_publico_no_nativo() {
    let asm = common::render_backend_s_external_subset_nativo(include_str!(
        "../examples/hr3_uniao_extracoes_independentes_valido.pink"
    ))
    .expect("assembly");

    let chamadas_publicas = asm.matches("call pinker_publico_alocar").count();
    assert_eq!(
        chamadas_publicas, 1,
        "só o `alocar` da origem pode chamar o alocador público:\n{asm}"
    );
    for (indice, _) in asm.match_indices("call pinker_uniao_criar") {
        let seguintes: Vec<&str> = asm[indice..]
            .lines()
            .skip(1)
            .take(3)
            .map(str::trim)
            .collect();
        assert!(
            !seguintes
                .iter()
                .any(|linha| linha.contains("pinker_publico_alocar")),
            "a construção não pode alocar região pública: {seguintes:?}"
        );
    }
}
// @pinker-nav:end evidencia.unioes.contabilidade-paridade
