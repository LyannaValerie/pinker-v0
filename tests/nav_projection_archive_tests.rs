//! Propriedades do arquivo histórico materializado (TA/#697).
//!
//! POT/LPT: INVARIANT LEGITIMATE_CURRENT_CHANGE -> ZERO_HISTORICAL_MAINTENANCE
//! POT/LPT: INVARIANT CURRENT_CATALOG_DRIFT != HISTORICAL_ARCHIVE_DRIFT
//! POT/LPT: AUTHORITY #697 §5, §10, §11
//!
//! A história congelada deixou de ser reconstruída a partir do catálogo
//! corrente. Estas provas cobrem o que substituiu a reconstrução: forma exata
//! dos bytes materializados, preservação das medidas históricas, integridade
//! SHA-256, detecção de adulteração e ausência, recusa de índice ambíguo, e a
//! propriedade principal — uma mudança legítima do presente não exige nenhuma
//! edição no arquivo.

use pinker_v0::nav_projection_archive as archive;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

const ACCEPTED_SNAPSHOTS: usize = 13;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn index() -> archive::ArchiveIndex {
    archive::load(&repo()).expect("índice do arquivo histórico")
}

/// Um repositório mínimo: só o marcador de raiz, o arquivo materializado e os
/// metadados FROZEN preservados.
///
/// Deliberadamente **sem** `src/navigation.jsonl`, sem receitas e sem mapa de
/// renomeação: se a verificação histórica precisasse de qualquer um deles, ela
/// falharia aqui.
struct MinimalRepo(PathBuf);

impl MinimalRepo {
    fn new(label: &str) -> MinimalRepo {
        let path = std::env::temp_dir().join(format!(
            "pinker-archive-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        let source = repo();
        fs::create_dir_all(path.join(".pinker/archive")).unwrap();
        fs::create_dir_all(path.join(".pinker/projections")).unwrap();
        fs::copy(
            source.join(".pinker/doc.toml"),
            path.join(".pinker/doc.toml"),
        )
        .unwrap();
        for relative in [".pinker/archive", ".pinker/projections"] {
            for entry in fs::read_dir(source.join(relative)).unwrap() {
                let file = entry.unwrap().path();
                if file.is_file() {
                    fs::copy(&file, path.join(relative).join(file.file_name().unwrap())).unwrap();
                }
            }
        }
        MinimalRepo(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn payload(&self, id: &str) -> PathBuf {
        self.0.join(format!(".pinker/archive/{id}.stable"))
    }

    fn index_path(&self) -> PathBuf {
        self.0.join(".pinker/archive/index.toml")
    }
}

impl Drop for MinimalRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn pink(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .expect("pink executável")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout UTF-8")
}

// ---------------------------------------------------------------- A1 · forma

/// CASE A1 — os 13 estados aceitos estão materializados como projeção estável.
///
/// A igualdade byte a byte contra `stable_projection()` da reconstrução aceita
/// foi provada no commit de checkpoint de equivalência, enquanto os dois
/// mecanismos coexistiam. O que permanece verificável sem reconstrução é a
/// forma: um registro canônico por linha, terminado em `\n`, com os oito campos
/// estáveis na ordem canônica, e a contagem de registros igual à medida
/// preservada.
#[test]
fn a1_treze_estados_aceitos_sao_projecao_estavel_exata() {
    let index = index();
    assert_eq!(index.entries.len(), ACCEPTED_SNAPSHOTS);

    for entry in &index.entries {
        let payload = fs::read_to_string(repo().join(&entry.payload_path))
            .unwrap_or_else(|_| panic!("payload ausente para '{}'", entry.id));
        assert!(
            payload.ends_with('\n'),
            "{}: a projeção estável termina em registro completo",
            entry.id
        );
        for record in payload.lines() {
            assert!(
                record.starts_with("(1, \"") && record.ends_with("\")"),
                "{}: registro fora da forma canônica: {record}",
                entry.id
            );
        }
        assert_eq!(
            payload.lines().count() as u64,
            entry.regions,
            "{}: contagem de registros diferente da medida preservada",
            entry.id
        );
        let mut ordered: Vec<&str> = payload.lines().collect();
        let declared = ordered.clone();
        ordered.sort_unstable();
        assert_eq!(
            ordered, declared,
            "{}: a projeção estável é ordenada lexicograficamente",
            entry.id
        );
    }
}

// ------------------------------------------------------------- A2 · medidas

/// CASE A2 — as medidas do payload são as medidas históricas preservadas.
///
/// O SHA-256 é evidência adicional: ele não substitui nem recalibra `regions`,
/// `length` ou `fnv1a64`.
#[test]
fn a2_medidas_do_payload_sao_as_medidas_preservadas() {
    for entry in &index().entries {
        let payload = fs::read(repo().join(&entry.payload_path)).expect("payload");
        assert_eq!(payload.len() as u64, entry.length, "{}", entry.id);
        assert_eq!(
            payload.iter().filter(|byte| **byte == b'\n').count() as u64,
            entry.regions,
            "{}",
            entry.id
        );
        assert_eq!(
            archive::fnv1a64_canonical(&payload),
            entry.fnv1a64,
            "{}",
            entry.id
        );
    }
}

/// CASE A2, ancoragem — as medidas do índice são as medidas escritas no TOML
/// FROZEN, não uma cópia que possa ser recalibrada junto com o payload.
///
/// O verificador confere o payload contra o índice e prende os bytes do
/// metadado histórico por `metadata_sha256`. Falta a terceira aresta: que o
/// índice ainda diga o que o TOML congelado sempre disse. Sem ela, editar
/// payload e índice de forma coerente recalibraria a história sem que nada
/// reclamasse. Este controle lê os literais históricos direto do `[measures]`
/// do TOML, sem passar pelo índice.
#[test]
fn medidas_do_indice_sao_os_literais_do_toml_frozen() {
    for entry in &index().entries {
        let toml = fs::read_to_string(repo().join(&entry.metadata_path))
            .unwrap_or_else(|_| panic!("metadado histórico ausente: {}", entry.metadata_path));
        let (regions, length, fnv1a64) = medidas_frozen(&toml, &entry.id);
        assert_eq!(
            regions, entry.regions,
            "regions recalibrado em '{}'",
            entry.id
        );
        assert_eq!(length, entry.length, "length recalibrado em '{}'", entry.id);
        assert_eq!(
            fnv1a64, entry.fnv1a64,
            "fnv1a64 recalibrado em '{}'",
            entry.id
        );
    }
}

/// Lê `regions`, `length` e `fnv1a64` da seção `[measures]` de um TOML FROZEN.
///
/// Deliberadamente escopado à seção: `[[rules]]` também carrega hashes e
/// contagens, e um leitor frouxo confundiria uma regra com uma medida.
fn medidas_frozen(toml: &str, id: &str) -> (u64, u64, String) {
    let mut regions = None;
    let mut length = None;
    let mut fnv1a64 = None;
    let mut dentro = false;
    for linha in toml.lines() {
        let linha = linha.trim();
        if linha.starts_with('[') {
            dentro = linha == "[measures]";
            continue;
        }
        if !dentro {
            continue;
        }
        let Some((chave, valor)) = linha.split_once('=') else {
            continue;
        };
        let valor = valor.trim().trim_matches('"');
        match chave.trim() {
            "regions" => regions = valor.parse::<u64>().ok(),
            "length" => length = valor.parse::<u64>().ok(),
            "fnv1a64" => fnv1a64 = Some(valor.to_string()),
            _ => {}
        }
    }
    (
        regions.unwrap_or_else(|| panic!("'{id}' não declara regions em [measures]")),
        length.unwrap_or_else(|| panic!("'{id}' não declara length em [measures]")),
        fnv1a64.unwrap_or_else(|| panic!("'{id}' não declara fnv1a64 em [measures]")),
    )
}

// ------------------------------------------- R1 · ancoragem em tempo de execução

/// CASE R1 — recalibrar payload e índice de forma coerente não produz INTACT.
///
/// É o controle que separa uma asserção de teste de uma garantia de produto. O
/// mutante edita os bytes materializados e, em seguida, edita o índice para que
/// ele concorde com os novos bytes: comprimento, contagem de registros, FNV-1a64
/// e SHA-256. Contra o índice sozinho tudo fecha. O que não fecha é o
/// `[measures]` do TOML congelado, e é ele que o verificador em produção lê.
#[test]
fn r1_recalibracao_coerente_de_payload_e_indice_nao_e_intacta() {
    let fixture = MinimalRepo::new("r1-recalibracao");
    let id = "onda-8j-anterior";
    let alvo = fixture.payload(id);

    // Um registro a mais: bytes coerentes, medidas todas diferentes.
    let mut payload = fs::read_to_string(&alvo).unwrap();
    payload.push_str("(1, \"zzz-registro-fabricado\")\n");
    fs::write(&alvo, payload.as_bytes()).unwrap();

    let original = index()
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .expect("entrada")
        .clone();
    let texto = fs::read_to_string(fixture.index_path()).unwrap();
    let recalibrado = texto
        .replacen(
            &format!("regions = {}", original.regions),
            &format!("regions = {}", original.regions + 1),
            1,
        )
        .replacen(
            &format!("length = {}", original.length),
            &format!("length = {}", payload.len()),
            1,
        )
        .replacen(
            &original.fnv1a64,
            &archive::fnv1a64_canonical(payload.as_bytes()),
            1,
        )
        .replacen(
            &original.sha256,
            &pinker_sha256_contract::sha256_hex(payload.as_bytes()),
            1,
        );
    fs::write(fixture.index_path(), recalibrado).unwrap();

    // A premissa do mutante: o índice agora concorda consigo mesmo.
    let index = archive::load(fixture.path()).expect("índice");
    let entrada = index
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .expect("entrada recalibrada");
    assert_eq!(
        entrada.length,
        payload.len() as u64,
        "MUTANT_NOT_APPLIED: o índice não foi recalibrado"
    );
    assert_ne!(
        entrada.regions, original.regions,
        "MUTANT_NOT_APPLIED: regions não mudou"
    );

    let verification = archive::verify(fixture.path(), &index);
    assert_eq!(
        verification.outcome(),
        "ALTERED",
        "recalibração coerente aceita como íntegra"
    );
    let archive::EntryOutcome::Altered(divergences) = &verification
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .expect("entrada")
        .outcome
    else {
        panic!("a recalibração não foi classificada como alteração");
    };
    let medidas: Vec<&str> = divergences
        .iter()
        .map(|divergence| divergence.measure)
        .collect();
    for esperada in ["frozen_regions", "frozen_length", "frozen_fnv1a64"] {
        assert!(
            medidas.contains(&esperada),
            "{esperada} não foi ancorada no TOML congelado: {medidas:?}"
        );
    }

    // E o produto, não só a biblioteca: `pink nav projecao verificar` recusa.
    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_eq!(output.status.code(), Some(5), "{}", stdout(&output));
    assert!(!stdout(&output).starts_with("verificar: INTACT"));
}

/// O metadado histórico é a autoridade das medidas, inclusive quando ilegível.
#[test]
fn r1_measures_ilegivel_no_toml_congelado_e_recusado() {
    let fixture = MinimalRepo::new("r1-measures-ilegivel");
    let toml = fixture
        .path()
        .join(".pinker/projections/onda-8j-anterior.toml");
    let texto = fs::read_to_string(&toml).unwrap();
    let sem_measures = texto.replacen("[measures]", "[nao-measures]", 1);
    assert_ne!(texto, sem_measures, "MUTANT_NOT_APPLIED");
    fs::write(&toml, sem_measures).unwrap();

    let index = archive::load(fixture.path()).expect("índice");
    let verification = archive::verify(fixture.path(), &index);
    assert_eq!(verification.outcome(), "ALTERED");
    let archive::EntryOutcome::Altered(divergences) = &verification
        .entries
        .iter()
        .find(|entry| entry.id == "onda-8j-anterior")
        .expect("entrada")
        .outcome
    else {
        panic!("metadado sem [measures] não foi classificado como alteração");
    };
    let medidas: Vec<&str> = divergences
        .iter()
        .map(|divergence| divergence.measure)
        .collect();
    assert!(medidas.contains(&"frozen_measures"), "{medidas:?}");
}

// ------------------------------------------------- R2 · conjunto histórico completo

/// CASE R2A — retirar uma entrada e o seu payload faz a verificação falhar.
///
/// É o buraco que a iteração sobre `index.entries` deixava: quem itera apenas o
/// que o índice declara não tem como notar o que ele deixou de declarar. A
/// cobertura é estabelecida contra os metadados preservados, que são a
/// autoridade do conjunto aceito.
#[test]
fn r2a_estado_aceito_ausente_do_indice_falha() {
    let fixture = MinimalRepo::new("r2a-estado-ausente");
    let id = "onda-8j-anterior";
    let texto = fs::read_to_string(fixture.index_path()).unwrap();
    let sem_entrada = remove_entrada(&texto, id);
    assert_ne!(texto, sem_entrada, "MUTANT_NOT_APPLIED");
    fs::write(fixture.index_path(), sem_entrada).unwrap();
    fs::remove_file(fixture.payload(id)).unwrap();

    match archive::load(fixture.path()) {
        Err(archive::ArchiveFailure::MissingArchivedState { id: ausente }) => {
            assert_eq!(ausente, id);
        }
        outro => panic!("estado aceito omitido foi aceito: {outro:?}"),
    }

    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_ne!(output.status.code(), Some(0), "{}", stdout(&output));
}

/// CASE R2B — uma entrada que não nomeia estado aceito algum é recusada.
#[test]
fn r2b_entrada_desconhecida_falha() {
    let fixture = MinimalRepo::new("r2b-entrada-desconhecida");
    let texto = fs::read_to_string(fixture.index_path()).unwrap();
    let inventado = format!(
        "{texto}\n[[entries]]\n         id = \"onda-inventada\"\n         metadata_path = \".pinker/projections/onda-inventada.toml\"\n         metadata_sha256 = \"{h}\"\n         payload_path = \".pinker/archive/onda-inventada.stable\"\n         regions = 1\n         length = 1\n         fnv1a64 = \"fnv1a64:0000000000000000\"\n         sha256 = \"{h}\"\n",
        h = "0".repeat(64)
    );
    fs::write(fixture.index_path(), inventado).unwrap();

    match archive::load(fixture.path()) {
        Err(archive::ArchiveFailure::UnknownArchivedState { id }) => {
            assert_eq!(id, "onda-inventada");
        }
        outro => panic!("entrada sem metadado aceito foi aceita: {outro:?}"),
    }

    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_ne!(output.status.code(), Some(0), "{}", stdout(&output));
}

/// CASE R2C — redirecionar uma entrada para outra identidade aceita é recusado.
#[test]
fn r2c_entrada_redirecionada_para_outra_identidade_falha() {
    let texto = fs::read_to_string(repo().join(".pinker/archive/index.toml")).unwrap();
    let redirecionado = texto.replacen(
        "metadata_path = \".pinker/projections/onda-8j-anterior.toml\"",
        "metadata_path = \".pinker/projections/onda-8i-anterior.toml\"",
        1,
    );
    assert_ne!(texto, redirecionado, "MUTANT_NOT_APPLIED");
    match archive::parse_index(&redirecionado) {
        Err(archive::ArchiveFailure::UnconfinedPath { field, .. }) => {
            assert_eq!(field, "metadata_path");
        }
        outro => panic!("metadado de outra identidade foi aceito: {outro:?}"),
    }
}

/// Um arquivo estranho na autoridade dos metadados não passa despercebido.
///
/// Sem isto, retirar um estado da história seria só renomear a sua extensão.
#[test]
fn r2_arquivo_estranho_na_autoridade_dos_metadados_falha() {
    let fixture = MinimalRepo::new("r2-metadado-estranho");
    fs::rename(
        fixture
            .path()
            .join(".pinker/projections/onda-8j-anterior.toml"),
        fixture
            .path()
            .join(".pinker/projections/onda-8j-anterior.toml.bak"),
    )
    .unwrap();
    assert!(matches!(
        archive::load(fixture.path()),
        Err(archive::ArchiveFailure::ForeignMetadata { .. })
    ));
}

// ------------------------------------------------------ R3 · confinamento de path

/// CASE R3 — path absoluto, travessia ou autoridade errada falham antes do disco.
///
/// A recusa é lexical e acontece em `parse_index`, que não toca no filesystem:
/// um path que escapa nunca chega a ser um `root.join`.
#[test]
fn r3_paths_fora_da_autoridade_falham_antes_de_ler_o_disco() {
    let base = "schema = 1\n                export_source_main = \"aaa\"\n                export_source_tree = \"bbb\"\n                export_method = \"m\"\n                provenance = \"p\"\n";
    let entrada = |payload: &str, metadata: &str| {
        format!(
            "{base}\n[[entries]]\n             id = \"um\"\n             metadata_path = \"{metadata}\"\n             metadata_sha256 = \"{h}\"\n             payload_path = \"{payload}\"\n             regions = 1\n             length = 1\n             fnv1a64 = \"fnv1a64:0000000000000000\"\n             sha256 = \"{h}\"\n",
            h = "0".repeat(64)
        )
    };
    let canonico_meta = ".pinker/projections/um.toml";
    let canonico_payload = ".pinker/archive/um.stable";

    // A premissa: a forma canônica é aceita.
    assert!(archive::parse_index(&entrada(canonico_payload, canonico_meta)).is_ok());

    // CASE R3A — travessia no payload.
    // CASE R3B — payload absoluto.
    // CASE R3D — diretório repo-local, mas autoridade errada.
    for payload in [
        "../outside",
        "/etc/passwd",
        ".pinker/archive/../../outside.stable",
        "docs/um.stable",
        ".pinker/projections/um.stable",
        ".pinker/archive/./um.stable",
        ".pinker/archive/outro.stable",
        "",
    ] {
        assert!(
            matches!(
                archive::parse_index(&entrada(payload, canonico_meta)),
                Err(archive::ArchiveFailure::UnconfinedPath { .. })
            ),
            "payload_path '{payload}' foi aceito"
        );
    }

    // CASE R3C — travessia e autoridade errada no metadado.
    for metadata in [
        "../outside.toml",
        "/etc/passwd",
        ".pinker/projections/../../outside.toml",
        ".pinker/archive/um.toml",
        "docs/um.toml",
        "",
    ] {
        assert!(
            matches!(
                archive::parse_index(&entrada(canonico_payload, metadata)),
                Err(archive::ArchiveFailure::UnconfinedPath { .. })
            ),
            "metadata_path '{metadata}' foi aceito"
        );
    }
}

// -------------------------------------------------------------- A3 · SHA-256

/// CASE A3 — o SHA-256 declarado corresponde a cada payload.
#[test]
fn a3_sha256_do_indice_corresponde_a_cada_payload() {
    for entry in &index().entries {
        let payload = fs::read(repo().join(&entry.payload_path)).expect("payload");
        assert_eq!(
            pinker_sha256_contract::sha256_hex(&payload),
            entry.sha256,
            "{}",
            entry.id
        );
    }
}

// ------------------------------------------------- A4 e §11 · controle principal

/// CASE A4 e critério de aceite negativo do §11 — mudança legítima corrente,
/// zero edição histórica.
///
/// Renomear uma chave corrente, trocar um resumo autoral, trocar o hash do
/// corpo de uma região e acrescentar uma região são as quatro mudanças que a
/// reconstrução exigia restaurar. Nenhuma delas toca no arquivo.
#[test]
fn a4_mudanca_legitima_corrente_nao_exige_edicao_historica() {
    let fixture = MinimalRepo::new("mudanca-corrente");
    let antes = bytes_do_arquivo(fixture.path());

    // O catálogo corrente sequer existe neste repositório, e mesmo assim a
    // verificação histórica é INTACT. Criá-lo, renomeá-lo e mutá-lo continua
    // sem efeito sobre o arquivo.
    fs::create_dir_all(fixture.path().join("src")).unwrap();
    let catalogo = fixture.path().join("src/navigation.jsonl");
    for corrente in [
        // chave corrente renomeada
        r#"{"schema":1,"key":"archive.corrente.renomeada","kind":"region","domain":"archive","layer":"trama","file":"src/x.rs","start_marker":1,"content_start":2,"content_end":3,"end_marker":4,"summary":"resumo","hash":"fnv1a64:0000000000000000","status":"active"}"#,
        // resumo autoral trocado
        r#"{"schema":1,"key":"archive.corrente.renomeada","kind":"region","domain":"archive","layer":"trama","file":"src/x.rs","start_marker":1,"content_start":2,"content_end":3,"end_marker":4,"summary":"outro resumo","hash":"fnv1a64:0000000000000000","status":"active"}"#,
        // hash do corpo trocado
        r#"{"schema":1,"key":"archive.corrente.renomeada","kind":"region","domain":"archive","layer":"trama","file":"src/x.rs","start_marker":1,"content_start":2,"content_end":3,"end_marker":4,"summary":"outro resumo","hash":"fnv1a64:1111111111111111","status":"active"}"#,
    ] {
        fs::write(&catalogo, format!("{corrente}\n")).unwrap();
        assert_arquivo_intacto(fixture.path());
        assert_eq!(
            bytes_do_arquivo(fixture.path()),
            antes,
            "uma mudança corrente exigiu edição do arquivo histórico"
        );
    }

    // região acrescentada
    let duas = format!(
        "{}\n{}\n",
        r#"{"schema":1,"key":"archive.corrente.renomeada","kind":"region","domain":"archive","layer":"trama","file":"src/x.rs","start_marker":1,"content_start":2,"content_end":3,"end_marker":4,"summary":"outro resumo","hash":"fnv1a64:1111111111111111","status":"active"}"#,
        r#"{"schema":1,"key":"archive.corrente.nova","kind":"region","domain":"archive","layer":"trama","file":"src/y.rs","start_marker":1,"content_start":2,"content_end":3,"end_marker":4,"summary":"região nova","hash":"fnv1a64:2222222222222222","status":"active"}"#
    );
    fs::write(&catalogo, duas).unwrap();
    assert_arquivo_intacto(fixture.path());
    assert_eq!(
        bytes_do_arquivo(fixture.path()),
        antes,
        "acrescentar região corrente exigiu edição do arquivo histórico"
    );
}

// ------------------------------------------------- A5 e A6 · controles negativos

/// CASE A5 — payload adulterado é detectado.
#[test]
fn a5_payload_adulterado_e_detectado() {
    let fixture = MinimalRepo::new("payload-adulterado");
    let alvo = fixture.payload("onda-8j-anterior");
    let mut bytes = fs::read(&alvo).unwrap();
    bytes[42] ^= 0x01;
    fs::write(&alvo, &bytes).unwrap();

    let index = archive::load(fixture.path()).expect("índice");
    let verification = archive::verify(fixture.path(), &index);
    assert_eq!(verification.outcome(), "ALTERED");
    let entrada = verification
        .entries
        .iter()
        .find(|entry| entry.id == "onda-8j-anterior")
        .expect("entrada alterada");
    let archive::EntryOutcome::Altered(divergences) = &entrada.outcome else {
        panic!("payload adulterado não foi classificado como alterado");
    };
    let medidas: Vec<&str> = divergences
        .iter()
        .map(|divergence| divergence.measure)
        .collect();
    assert!(medidas.contains(&"fnv1a64"), "{medidas:?}");
    assert!(medidas.contains(&"sha256"), "{medidas:?}");

    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_eq!(output.status.code(), Some(5), "{}", stdout(&output));
}

/// CASE A6 — payload ausente é detectado.
#[test]
fn a6_payload_ausente_e_detectado() {
    let fixture = MinimalRepo::new("payload-ausente");
    fs::remove_file(fixture.payload("onda-8j-anterior")).unwrap();

    let index = archive::load(fixture.path()).expect("índice");
    let verification = archive::verify(fixture.path(), &index);
    assert_eq!(verification.outcome(), "MISSING");

    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_eq!(output.status.code(), Some(3), "{}", stdout(&output));
}

/// Medida preservada adulterada no índice é detectada.
///
/// É a forma que a recusa de recalibração tomou depois da aposentadoria da
/// reconstrução: mexer na medida declarada não esconde nada, porque a medida é
/// recalculada sobre os bytes materializados.
#[test]
fn medida_preservada_recalibrada_e_recusada() {
    for (de, para) in [
        ("regions = 405", "regions = 406"),
        ("length = 186892", "length = 186893"),
        (
            "fnv1a64 = \"fnv1a64:a77974535bcb999c\"",
            "fnv1a64 = \"fnv1a64:a77974535bcb999d\"",
        ),
    ] {
        let fixture = MinimalRepo::new("medida-recalibrada");
        let texto = fs::read_to_string(fixture.index_path()).unwrap();
        assert!(texto.contains(de), "a fixture não contém '{de}'");
        fs::write(fixture.index_path(), texto.replacen(de, para, 1)).unwrap();

        let index = archive::load(fixture.path()).expect("índice");
        let verification = archive::verify(fixture.path(), &index);
        assert_eq!(
            verification.outcome(),
            "ALTERED",
            "recalibração aceita: {de} -> {para}"
        );
    }
}

/// SHA-256 errado no índice é detectado.
#[test]
fn sha256_errado_e_recusado() {
    let fixture = MinimalRepo::new("sha-errado");
    let texto = fs::read_to_string(fixture.index_path()).unwrap();
    let entrada = index()
        .entries
        .iter()
        .find(|entry| entry.id == "onda-8j-anterior")
        .expect("entrada")
        .clone();
    let errado = format!("{}0", &entrada.sha256[..entrada.sha256.len() - 1]);
    fs::write(
        fixture.index_path(),
        texto.replacen(&entrada.sha256, &errado, 1),
    )
    .unwrap();

    let index = archive::load(fixture.path()).expect("índice");
    let verification = archive::verify(fixture.path(), &index);
    assert_eq!(verification.outcome(), "ALTERED");
}

// ---------------------------------------------------------------- A7 · índice

/// CASE A7 — id ou caminho repetido no índice é recusado.
#[test]
fn a7_id_ou_path_duplicado_e_recusado() {
    let base = "schema = 1\n\
                export_source_main = \"aaa\"\n\
                export_source_tree = \"bbb\"\n\
                export_method = \"m\"\n\
                provenance = \"p\"\n";
    let entrada = |id: &str, path: &str| {
        format!(
            "\n[[entries]]\n\
             id = \"{id}\"\n\
             metadata_path = \".pinker/projections/{id}.toml\"\n\
             metadata_sha256 = \"{h}\"\n\
             payload_path = \"{path}\"\n\
             regions = 1\n\
             length = 1\n\
             fnv1a64 = \"fnv1a64:0000000000000000\"\n\
             sha256 = \"{h}\"\n",
            h = "0".repeat(64)
        )
    };

    let duplicado_id = format!(
        "{base}{}{}",
        entrada("um", ".pinker/archive/um.stable"),
        entrada("um", ".pinker/archive/outro.stable")
    );
    assert!(matches!(
        archive::parse_index(&duplicado_id),
        Err(archive::ArchiveFailure::DuplicateId { .. })
    ));

    let duplicado_path = format!(
        "{base}{}{}",
        entrada("um", ".pinker/archive/um.stable"),
        entrada("dois", ".pinker/archive/um.stable")
    );
    assert!(matches!(
        archive::parse_index(&duplicado_path),
        Err(archive::ArchiveFailure::DuplicatePath { .. })
    ));

    assert!(matches!(
        archive::parse_index(base),
        Err(archive::ArchiveFailure::Empty)
    ));
    assert!(matches!(
        archive::parse_index(&format!("{base}\n[[outra]]\nid = \"x\"\n")),
        Err(archive::ArchiveFailure::Malformed { .. })
    ));
    assert!(matches!(
        archive::parse_index(&format!("schema = 2\n{}", &base[11..])),
        Err(archive::ArchiveFailure::UnsupportedSchema { schema: 2 })
    ));

    // Proveniência declarada mas vazia é recusada: um índice sem origem
    // auditável não descreve um arquivo histórico.
    for campo in [
        "export_source_main",
        "export_source_tree",
        "export_method",
        "provenance",
    ] {
        let vazio = base
            .lines()
            .map(|linha| {
                if linha.starts_with(campo) {
                    format!("{campo} = \"\"")
                } else {
                    linha.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let texto = format!("{vazio}\n{}", entrada("um", ".pinker/archive/um.stable"));
        assert!(
            matches!(
                archive::parse_index(&texto),
                Err(archive::ArchiveFailure::InvalidValue { .. })
            ),
            "'{campo}' vazio foi aceito"
        );
    }
}

/// A proveniência do índice declara o que os bytes são e o que não são.
#[test]
fn a_proveniencia_e_explicita() {
    let index = index();
    assert_eq!(index.schema, 1);
    assert!(!index.export_source_main.is_empty());
    assert!(!index.export_source_tree.is_empty());
    assert!(
        index.provenance.contains("exported at cutover")
            && index
                .provenance
                .contains("not claimed to have been captured contemporaneously"),
        "a proveniência precisa negar a captura contemporânea: {}",
        index.provenance
    );
}

// ---------------------------------------------------------------- A8 · FROZEN

/// CASE A8 — os bytes originais dos TOML FROZEN continuam intactos.
#[test]
fn a8_metadados_frozen_originais_permanecem_intactos() {
    for entry in &index().entries {
        let metadata = fs::read(repo().join(&entry.metadata_path))
            .unwrap_or_else(|_| panic!("metadado histórico ausente: {}", entry.metadata_path));
        assert_eq!(
            pinker_sha256_contract::sha256_hex(&metadata),
            entry.metadata_sha256,
            "os bytes FROZEN de '{}' mudaram",
            entry.id
        );
    }

    let fixture = MinimalRepo::new("frozen-editado");
    let alvo = fixture
        .path()
        .join(".pinker/projections/onda-8j-anterior.toml");
    let texto = fs::read_to_string(&alvo).unwrap();
    fs::write(
        &alvo,
        texto.replacen("state = \"FROZEN\"", "state = \"X\"", 1),
    )
    .unwrap();
    let index = archive::load(fixture.path()).expect("índice");
    let verification = archive::verify(fixture.path(), &index);
    assert_eq!(
        verification.outcome(),
        "ALTERED",
        "editar o metadado FROZEN passou despercebido"
    );
}

// ------------------------------------------------------- A9 e A10 · desacople

/// CASE A9 e A10 — o verificador final não lê o catálogo corrente nem receitas.
///
/// A prova é comportamental, não lexical: o repositório mínimo não tem
/// `src/navigation.jsonl`, não tem `.pinker/projections/recipes/`, não tem mapa
/// de renomeação e não tem sequer um diretório `src/`. A verificação histórica
/// é INTACT mesmo assim.
#[test]
fn a9_a10_verificador_nao_depende_de_catalogo_nem_de_receita() {
    let fixture = MinimalRepo::new("desacoplado");
    assert!(!fixture.path().join("src").exists());
    assert!(!fixture.path().join(".pinker/projections/recipes").exists());

    assert_arquivo_intacto(fixture.path());

    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
    assert!(stdout(&output).starts_with("verificar: INTACT"));

    // Uma receita colocada no lugar antigo não muda nada: ninguém a lê.
    fs::create_dir_all(fixture.path().join(".pinker/projections/recipes")).unwrap();
    fs::write(
        fixture
            .path()
            .join(".pinker/projections/recipes/qualquer.toml"),
        "isto não é uma receita válida\n",
    )
    .unwrap();
    assert_arquivo_intacto(fixture.path());
    let output = pink(fixture.path(), &["nav", "projecao", "verificar"]);
    assert_eq!(output.status.code(), Some(0), "{}", stdout(&output));
}

// ----------------------------------------------------------------- A11 · CLI

/// CASE A11 — listar, mostrar e verificar são determinísticos e somente leitura.
#[test]
fn a11_listar_mostrar_verificar_sao_deterministicos_e_read_only() {
    let fixture = MinimalRepo::new("cli");
    let antes = bytes_do_arquivo(fixture.path());

    for args in [
        vec!["nav", "projecao", "listar"],
        vec!["nav", "projecao", "listar", "--json"],
        vec!["nav", "projecao", "mostrar", "onda-8-convergencia"],
        vec![
            "nav",
            "projecao",
            "mostrar",
            "onda-8-convergencia",
            "--json",
        ],
        vec!["nav", "projecao", "verificar"],
        vec!["nav", "projecao", "verificar", "--json"],
        vec!["nav", "projecao", "verificar", "onda-8-convergencia"],
    ] {
        let primeiro = pink(fixture.path(), &args);
        let segundo = pink(fixture.path(), &args);
        assert_eq!(primeiro.status.code(), Some(0), "{args:?}");
        assert_eq!(
            primeiro.stdout, segundo.stdout,
            "{args:?} não determinístico"
        );
        let texto = stdout(&primeiro);
        assert!(!texto.contains('\u{1b}'), "{args:?} emitiu ANSI");
        assert!(
            !texto.contains(fixture.path().to_str().unwrap()),
            "{args:?} vazou path absoluto"
        );
    }

    assert_eq!(
        bytes_do_arquivo(fixture.path()),
        antes,
        "um comando de leitura escreveu no arquivo"
    );

    let ausente = pink(
        fixture.path(),
        &["nav", "projecao", "mostrar", "nao-existe"],
    );
    assert_eq!(ausente.status.code(), Some(4));

    // Os verbos de reconstrução saíram da superfície pública.
    for retirado in ["preparar", "aceitar", "reconciliar"] {
        let output = pink(fixture.path(), &["nav", "projecao", retirado]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "'{retirado}' ainda é aceito pela CLI"
        );
    }
}

// ---------------------------------------------------------------- A12 · impacto

/// CASE A12 — a análise corrente de diff continua útil sem resolvedor histórico.
#[test]
fn a12_cobertura_de_diff_corrente_permanece_util() {
    let diff = [
        "diff --git a/src/nav_projection_archive.rs b/src/nav_projection_archive.rs",
        "--- a/src/nav_projection_archive.rs",
        "+++ b/src/nav_projection_archive.rs",
        "@@ -1,1 +1,2 @@",
        " //! Arquivo histórico materializado da Trama.",
        "+// linha nova",
        "",
    ]
    .join("\n");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["nav", "cobertura-diff", "--json", "--repo"])
        .arg(repo())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("pink executável");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(diff.as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("saída");
    let json = String::from_utf8(output.stdout).expect("stdout UTF-8");

    assert!(json.starts_with("{\"schema\":3,"), "{json}");
    for presente in [
        "\"covered_intervals\":",
        "\"uncovered_intervals\":",
        "\"completeness\":",
        "\"regions\":",
        "\"path\":\"src/nav_projection_archive.rs\"",
    ] {
        assert!(json.contains(presente), "ausente {presente}: {json}");
    }
    for ausente in ["navigation-snapshot", "projection-store", "reconstruction"] {
        assert!(
            !json.contains(ausente),
            "a análise corrente ainda expõe reconstrução histórica: {ausente}"
        );
    }
}

// ------------------------------------------------------------------- helpers

fn assert_arquivo_intacto(root: &Path) {
    let index = archive::load(root).expect("índice do arquivo histórico");
    let verification = archive::verify(root, &index);
    assert_eq!(
        verification.outcome(),
        "INTACT",
        "o arquivo histórico deixou de estar íntegro"
    );
}

/// Remove do texto do índice o bloco `[[entries]]` de um id.
fn remove_entrada(texto: &str, id: &str) -> String {
    let marcador = format!("id = \"{id}\"");
    let mut out = String::new();
    let mut primeiro = true;
    for bloco in texto.split("[[entries]]") {
        if primeiro {
            primeiro = false;
            out.push_str(bloco);
            continue;
        }
        if bloco.lines().any(|linha| linha.trim() == marcador) {
            continue;
        }
        out.push_str("[[entries]]");
        out.push_str(bloco);
    }
    out
}

/// Todo byte sob `.pinker/archive/`, para provar ausência de edição.
fn bytes_do_arquivo(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let dir = root.join(".pinker/archive");
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("diretório do arquivo")
        .map(|entry| entry.expect("entrada").path())
        .collect();
    entries.sort();
    for path in entries {
        out.push((
            path.file_name().unwrap().to_string_lossy().into_owned(),
            fs::read(&path).expect("arquivo legível"),
        ));
    }
    out
}
