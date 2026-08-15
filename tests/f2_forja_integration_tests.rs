//! Evidência da Parte F2 (#489): os registros que o publicador deriva precisam
//! ser aceitos pelas autoridades que os consomem.
//!
//! A F1 só verificava o TEXTO do `scripts/pink-baseline` por substring, e foi
//! por isso que um manifest sintaticamente inválido chegou a ser publicado sem
//! nenhum teste vermelho. Aqui a evidência incide sobre a SAÍDA renderizada.
//!
//! Nada neste arquivo escreve em `/opt`, exige root ou toca o catálogo real: os
//! modos `manifest` e `ficha` são read-only e o `--release-root` aponta para um
//! diretório temporário do próprio teste.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.tooling.f2.registros-derivados
// @pinker-nav:domain tooling
// @pinker-nav:layer evidence
// @pinker-nav:summary Contratos positivos e negativos dos registros derivados pelo publicador do baseline: manifest bem-formado e em forma canônica, cobertura de checksums, identidade de verificação, ficha de catálogo com launcher declarado e idempotente, e recusa de bundle inválido; e a transação de publicação, que valida o candidato integralmente antes de ativar e restaura o estado vivo anterior exatamente quando a conferência posterior reprova.

const COMMIT: &str = "9e53cbf286f9500114bd6141bfeace21a7b5f7c3";
const SHA256: &str = "d63f71c6218f392080672edaab472c145d83a1b0d129ab57cc2baa2d2eb9b363";
const DATA: &str = "2026-08-11";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn temp(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("pinker_f2_{name}_{stamp}"));
    fs::create_dir_all(&path).expect("criar diretório temporário");
    path
}

/// Bundle sintético completo. O binário é um stub executável: os modos
/// read-only leem `identity.env` e nunca executam o payload.
fn bundle(dir: &Path, sha256: &str) -> PathBuf {
    let bundle = dir.join("bundle");
    fs::create_dir_all(&bundle).expect("criar bundle");
    let binary = bundle.join("pink");
    fs::write(&binary, "#!/bin/sh\nexit 0\n").expect("escrever stub");
    make_executable(&binary);
    fs::write(
        bundle.join("identity.env"),
        format!(
            "schema=forja-pink-bundle-v1\nversion=0.1.0\ncommit={COMMIT}\nsha256={sha256}\nsource={}\n",
            root().display()
        ),
    )
    .expect("escrever identity.env");
    bundle
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

fn baseline(args: &[&str]) -> Output {
    Command::new(root().join("scripts/pink-baseline"))
        .args(args)
        .current_dir(root())
        .output()
        .expect("executar pink-baseline")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout UTF-8")
}

fn render_manifest(dir: &Path, release_root: &Path) -> String {
    let bundle = bundle(dir, SHA256);
    let output = baseline(&[
        "manifest",
        "--bundle",
        bundle.to_str().unwrap(),
        "--release-root",
        release_root.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "render do manifest falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout(&output)
}

// ---------------------------------------------------------------------------
// Oráculo de JSON.
//
// O oráculo não é escrito aqui. Ele é o módulo `json` do Python: o mesmo parser
// que `pinker-manifest-verify` usa para aceitar ou recusar um manifest, no mesmo
// interpretador que `scripts/pink-baseline` já exige para serializar. Nada novo
// entra no fluxo.
//
// O que existia antes era um scanner próprio, e um scanner próprio custa caro
// exatamente onde parece barato: ele respondia "válido" para `1.2.3`, `--1`,
// `1e`, `01` e `\u` sem os quatro dígitos hex. Um teste que afirma "isto é JSON
// válido" apoiado em um oráculo que aceita o inválido não afirma nada. O
// compilador continua zero-dependência: nenhuma crate entrou; a suíte apenas
// chama a autoridade que o próprio fluxo já exige.
//
// Uma diferença deliberada em relação ao validador canônico: `json.loads` aceita
// `NaN` e `Infinity`, que JSON não tem. Aqui elas são recusadas, porque a
// propriedade sob teste é "é JSON", não "é o que este validador tolera".
// ---------------------------------------------------------------------------

const PRELUDIO_JSON: &str = "\
import json, sys

def constante(nome):
    raise ValueError('constante nao-JSON: ' + nome)

dados = json.loads(sys.stdin.read(), parse_constant=constante)
";

/// Roda um programa Python com o texto em stdin. Erro do parser vem como Err.
fn python_json(texto: &str, programa: &str, args: &[&str]) -> Result<String, String> {
    use std::io::Write;
    let mut filho = Command::new("python3")
        .arg("-c")
        .arg(format!("{PRELUDIO_JSON}{programa}"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("executar python3");
    filho
        .stdin
        .as_mut()
        .expect("stdin do python3")
        .write_all(texto.as_bytes())
        .expect("escrever no python3");
    let saida = filho.wait_with_output().expect("aguardar python3");
    if saida.status.success() {
        Ok(String::from_utf8(saida.stdout).expect("stdout UTF-8"))
    } else {
        Err(String::from_utf8_lossy(&saida.stderr)
            .lines()
            .last()
            .unwrap_or_default()
            .to_string())
    }
}

fn separados(saida: String) -> Vec<String> {
    if saida.is_empty() {
        Vec::new()
    } else {
        saida.split('\0').map(str::to_string).collect()
    }
}

/// Chaves de topo na ordem declarada pelo documento, ou o erro real do parser.
fn chaves_de_topo(texto: &str) -> Result<Vec<String>, String> {
    python_json(
        texto,
        "\
if not isinstance(dados, dict):
    raise ValueError('documento de topo nao e objeto')
sys.stdout.write('\\0'.join(dados))
",
        &[],
    )
    .map(separados)
}

/// A forma canônica do documento, pela regra literal da autoridade validadora:
/// `json.dumps(dados, ensure_ascii=False, indent=2, sort_keys=True) + "\n"`.
///
/// Comparar a saída com isto é a mesma decisão que `pinker-manifest-verify` toma
/// ao recusar um manifest por "serialização não determinística".
fn forma_canonica(texto: &str) -> Result<String, String> {
    python_json(
        texto,
        "\
sys.stdout.write(
    json.dumps(dados, ensure_ascii=False, indent=2, sort_keys=True) + '\\n'
)
",
        &[],
    )
}

/// Todos os valores string associados à chave, em qualquer profundidade.
///
/// A travessia é estrutural: buscar `"chave":` no texto encontraria a mesma
/// sequência dentro de um valor string — e `expected_contains` é literalmente um
/// trecho de JSON dentro de uma string.
fn valores_string_de(texto: &str, chave: &str) -> Vec<String> {
    python_json(
        texto,
        "\
alvo = sys.argv[1]
achados = []

def andar(no):
    if isinstance(no, dict):
        for chave, valor in no.items():
            if chave == alvo and isinstance(valor, str):
                achados.append(valor)
            andar(valor)
    elif isinstance(no, list):
        for item in no:
            andar(item)

andar(dados)
sys.stdout.write('\\0'.join(achados))
",
        &[chave],
    )
    .map(separados)
    .expect("o documento precisa ser JSON válido antes de ter campos lidos")
}

// ---------------------------------------------------------------------------
// O oráculo julgado antes de julgar
// ---------------------------------------------------------------------------

#[test]
fn o_oraculo_recusa_json_realmente_invalido() {
    // As cinco primeiras formas são as que o scanner parcial anterior aceitava
    // por engano — número com dois pontos, sinal repetido, expoente vazio, zero
    // à esquerda e `\u` sem os quatro dígitos hex. Se o oráculo aceita isso,
    // "o manifest é JSON válido" deixa de ser uma afirmação sobre JSON.
    for invalido in [
        r#"{"a": 1.2.3}"#,
        r#"{"a": --1}"#,
        r#"{"a": 1e}"#,
        r#"{"a": 01}"#,
        r#"{"a": "\u12"}"#,
        r#"{"a": "\q"}"#,
        r#"{"a": 1,}"#,
        r#"{"a" 1}"#,
        r#"{"a": "sem fim}"#,
        r#"{"a": 1} lixo"#,
        // JSON não tem estas constantes, ainda que `json.loads` as tolere por
        // default: a propriedade sob teste é "é JSON".
        r#"{"a": NaN}"#,
        r#"{"a": Infinity}"#,
        // O defeito histórico: a aspa mal escapada deixa um token nu no lugar
        // do valor. Foi exatamente esta forma que a F1 publicou em /opt.
        r#"{"expected_contains": ""binary_commit":"abc""}"#,
    ] {
        assert!(
            chaves_de_topo(invalido).is_err(),
            "o oráculo aceitou JSON inválido: {invalido}"
        );
    }

    // E continua aceitando JSON válido, inclusive as formas que uma checagem
    // ingênua confundiria com as de cima.
    let valido = r#"{"a": [1, -2.5e+3, 0, null, true, "é \" \\"], "b": {"c": {}}}"#;
    assert_eq!(
        chaves_de_topo(valido).expect("documento válido"),
        vec!["a".to_string(), "b".to_string()]
    );
}

#[test]
fn o_oraculo_le_campos_por_estrutura_e_nao_por_texto() {
    // `expected_contains` carrega `"binary_commit":"..."` DENTRO de uma string.
    // Uma busca textual por `"binary_commit":` acharia essa ocorrência e leria
    // um campo que não existe; a travessia estrutural não.
    let documento = r#"{"verification": {"expected_contains": "\"binary_commit\":\"abc\""}}"#;
    assert!(
        valores_string_de(documento, "binary_commit").is_empty(),
        "um trecho dentro de uma string não é um campo"
    );
    assert_eq!(
        valores_string_de(documento, "expected_contains"),
        vec!["\"binary_commit\":\"abc\"".to_string()]
    );
}

// ---------------------------------------------------------------------------
// Positivos — manifest
// ---------------------------------------------------------------------------

#[test]
fn manifest_renderizado_e_json_bem_formado() {
    let dir = temp("manifest_bem_formado");
    let release_root = dir.join("release");
    let rendered = render_manifest(&dir, &release_root);
    let keys = chaves_de_topo(&rendered).unwrap_or_else(|error| {
        panic!("manifest renderizado não é JSON válido: {error}\n---\n{rendered}")
    });
    assert!(!keys.is_empty(), "manifest sem chaves de topo");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn manifest_declara_exatamente_os_campos_exigidos_e_ordenados() {
    let dir = temp("manifest_campos");
    let release_root = dir.join("release");
    let rendered = render_manifest(&dir, &release_root);
    let keys = chaves_de_topo(&rendered).expect("manifest válido");
    let esperado = vec![
        "checksums",
        "exposed_commands",
        "installation_root",
        "installed_executables",
        "name",
        "ownership_policy",
        "schema",
        "source_or_provenance",
        "verification",
        "version",
        "wrappers",
    ];
    assert_eq!(keys, esperado, "campos de topo divergem do exigido");
    // A autoridade validadora compara o texto com uma serialização ordenada e
    // indentada; chave fora de ordem é rejeitada mesmo com JSON válido.
    let mut ordenado = keys.clone();
    ordenado.sort();
    assert_eq!(keys, ordenado, "chaves de topo fora de ordem");
    // E a forma canônica é conferida pela regra da própria autoridade, não por
    // aproximações textuais dela. Terminar em `\n`, não usar tab e indentar com
    // dois espaços são consequências desta igualdade — checá-las uma a uma
    // deixaria de fora tudo o que ninguém lembrou de enumerar.
    assert_eq!(
        rendered,
        forma_canonica(&rendered).expect("manifest válido"),
        "serialização não determinística — é assim que a autoridade recusa um manifest"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_forma_canonica_recusa_o_que_a_autoridade_recusaria() {
    // O oráculo de canonicidade precisa distinguir o correto do plausível: as
    // quatro variações abaixo são JSON válido e são exatamente as que o
    // validador rejeita por "serialização não determinística" — o mesmo motivo
    // pelo qual outro manifest vivo deste host é reprovado hoje.
    let canonico = "{\n  \"a\": 1,\n  \"b\": 2\n}\n";
    assert_eq!(canonico, forma_canonica(canonico).expect("válido"));
    for divergente in [
        "{\n  \"b\": 2,\n  \"a\": 1\n}\n",     // fora de ordem
        "{\n    \"a\": 1,\n    \"b\": 2\n}\n", // indentação de quatro
        "{\"a\": 1, \"b\": 2}\n",              // sem indentação
        "{\n  \"a\": 1,\n  \"b\": 2\n}",       // sem a quebra final
    ] {
        assert_ne!(
            divergente,
            forma_canonica(divergente).expect("válido"),
            "aceitou uma serialização que a autoridade recusaria: {divergente:?}"
        );
    }
}

#[test]
fn checksums_cobrem_todos_os_executaveis_e_comandos_declarados() {
    let dir = temp("manifest_checksums");
    let release_root = dir.join("release");
    let rendered = render_manifest(&dir, &release_root);

    let declarados: BTreeSet<String> = valores_string_de(&rendered, "path")
        .into_iter()
        .chain(valores_string_de(&rendered, "installation_root"))
        .collect();
    let release_binary = format!("{}/bin/pink", release_root.display());
    assert!(
        declarados.contains(&release_binary),
        "checksums não cobrem o binário da release"
    );
    assert!(
        declarados.contains("/opt/pinker/bin/pink"),
        "checksums não cobrem o comando exposto — foi exatamente essa a lacuna do manifest publicado"
    );
    assert_eq!(
        valores_string_de(&rendered, "sha256"),
        vec![SHA256.to_string(), SHA256.to_string()],
        "o comando exposto é symlink para o binário: o digest tem de ser o mesmo"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn verificacao_do_manifest_casa_com_a_identidade_declarada() {
    let dir = temp("manifest_verificacao");
    let release_root = dir.join("release");
    let rendered = render_manifest(&dir, &release_root);
    let esperado = format!("\"binary_commit\":\"{COMMIT}\"");
    let contains = valores_string_de(&rendered, "expected_contains");
    assert_eq!(
        contains,
        vec![esperado],
        "expected_contains precisa reproduzir o trecho literal de --version-json"
    );
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Positivos — ficha de catálogo
// ---------------------------------------------------------------------------

fn render_ficha(dir: &Path, release_root: &Path) -> String {
    let bundle = bundle(dir, SHA256);
    let output = baseline(&[
        "ficha",
        "--bundle",
        bundle.to_str().unwrap(),
        "--release-root",
        release_root.to_str().unwrap(),
        "--perfil",
        "amara",
        "--data",
        DATA,
    ]);
    assert!(
        output.status.success(),
        "render da ficha falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout(&output)
}

/// Lê o frontmatter pelas mesmas regras do catálogo: bloco `---` inicial, uma
/// chave por linha, sem duplicatas, terminado por `---`.
fn frontmatter(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("---"), "frontmatter inicial ausente");
    let mut fields = Vec::new();
    let mut seen = BTreeSet::new();
    for line in lines {
        if line == "---" {
            return fields;
        }
        let (key, value) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("linha de frontmatter inválida: {line:?}"));
        let key = key.trim().to_string();
        assert!(!key.is_empty(), "chave vazia no frontmatter");
        assert!(seen.insert(key.clone()), "campo duplicado: {key}");
        fields.push((key, value.trim().to_string()));
    }
    panic!("frontmatter final ausente");
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> &'a str {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or_else(|| panic!("campo ausente na ficha: {key}"))
}

#[test]
fn ficha_declara_launcher_identidade_e_linguagem_derivados() {
    let dir = temp("ficha_campos");
    let release_root = dir.join("release");
    let rendered = render_ficha(&dir, &release_root);
    let fields = frontmatter(&rendered);

    assert_eq!(field(&fields, "tipo"), "ferramenta");
    assert_eq!(field(&fields, "nome"), "pink");
    assert_eq!(field(&fields, "versao"), "0.1.0");
    assert_eq!(field(&fields, "estado"), "instalado");
    assert_eq!(field(&fields, "linguagem"), "Rust");
    // O launcher entra na identidade do registro: sem isso a saúde derivada
    // aprova a entrada apenas por existir metadata.
    assert_eq!(field(&fields, "arquivo_principal"), "/opt/pinker/bin/pink");
    assert_eq!(field(&fields, "executavel"), "sim");
    assert_eq!(field(&fields, "interface_padrao"), "sim");
    assert!(
        field(&fields, "local").starts_with("/opt/pinker/bin/pink"),
        "o primeiro caminho absoluto de `local` é o que o catálogo resolve"
    );
    assert!(
        field(&fields, "local").contains(&release_root.display().to_string()),
        "a release resolvida precisa aparecer na procedência"
    );
    // `motivo_pinker` é obrigatório na convenção mesmo quando a resposta é não.
    assert_eq!(field(&fields, "pinker"), "nao");
    assert!(!field(&fields, "motivo_pinker").is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn ficha_e_reproduzivel_byte_a_byte() {
    let dir = temp("ficha_idempotente");
    let release_root = dir.join("release");
    let primeira = render_ficha(&dir, &release_root);
    let segunda = render_ficha(&dir, &release_root);
    // Um registro derivado só se distingue de uma entrada digitada à mão se
    // puder ser reproduzido a qualquer momento a partir da mesma autoridade.
    assert_eq!(primeira, segunda, "ficha não é reproduzível");
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Negativos
// ---------------------------------------------------------------------------

#[test]
fn bundle_invalido_e_recusado_antes_de_qualquer_render() {
    let casos: [(&str, &str, &str); 4] = [
        ("schema", "schema=errado", "schema do bundle inválido"),
        ("versao", "version=0.1", "versão inválida"),
        ("commit", "commit=abc", "commit inválido"),
        ("sha256", "sha256=xyz", "SHA-256 esperado inválido"),
    ];
    for (nome, substituicao, esperado) in casos {
        let dir = temp(&format!("bundle_invalido_{nome}"));
        let bundle_dir = bundle(&dir, SHA256);
        let identity = fs::read_to_string(bundle_dir.join("identity.env")).unwrap();
        let campo = substituicao.split_once('=').unwrap().0;
        let alterado: String = identity
            .lines()
            .map(|line| {
                if line.starts_with(&format!("{campo}=")) {
                    substituicao.to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(bundle_dir.join("identity.env"), format!("{alterado}\n")).unwrap();

        let output = baseline(&["manifest", "--bundle", bundle_dir.to_str().unwrap()]);
        assert!(!output.status.success(), "{nome}: render deveria falhar");
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        assert!(
            stderr.contains(esperado),
            "{nome}: causa errada. esperado {esperado:?}, obtido {stderr:?}"
        );
        assert!(
            stdout(&output).is_empty(),
            "{nome}: nada deve ser renderizado quando a identidade é inválida"
        );
        fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn campo_desconhecido_no_bundle_nao_e_ignorado() {
    let dir = temp("bundle_campo_extra");
    let bundle_dir = bundle(&dir, SHA256);
    let identity = fs::read_to_string(bundle_dir.join("identity.env")).unwrap();
    fs::write(
        bundle_dir.join("identity.env"),
        format!("{identity}inesperado=1\n"),
    )
    .unwrap();
    let output = baseline(&["manifest", "--bundle", bundle_dir.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("campo desconhecido no bundle"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn ficha_sem_origem_recusa_inventar_data() {
    let dir = temp("ficha_sem_origem");
    let bundle_dir = bundle(&dir, SHA256);
    let identity = fs::read_to_string(bundle_dir.join("identity.env")).unwrap();
    let alterado: String = identity
        .lines()
        .map(|line| {
            if line.starts_with("source=") {
                "source=/inexistente/origem".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(bundle_dir.join("identity.env"), format!("{alterado}\n")).unwrap();

    let output = baseline(&["ficha", "--bundle", bundle_dir.to_str().unwrap()]);
    // A data vem do commit da release. Sem origem para derivá-la, a ferramenta
    // para e pede o valor: usar a data de hoje tornaria o registro irreprodutível.
    assert!(
        !output.status.success(),
        "data não derivável deveria falhar"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("data não derivável"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn modos_de_render_nao_escrevem_fora_da_saida_padrao() {
    let dir = temp("render_read_only");
    let release_root = dir.join("release-inexistente");
    let _ = render_manifest(&dir, &release_root);
    let _ = render_ficha(&dir, &release_root);
    // O `--release-root` é apenas um caminho declarado: renderizar não pode
    // criar árvore de release nem exigir que ela exista.
    assert!(
        !release_root.exists(),
        "render criou estado fora da saída padrão"
    );
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// Regressão da F1 e não-duplicação de autoridade
// ---------------------------------------------------------------------------

#[test]
fn publish_continua_exigindo_root_e_os_modos_read_only_nao() {
    let dir = temp("publish_root");
    let bundle_dir = bundle(&dir, SHA256);
    let publish = baseline(&["publish", "--bundle", bundle_dir.to_str().unwrap()]);
    assert!(!publish.status.success());
    assert!(String::from_utf8_lossy(&publish.stderr).contains("publish exige root"));

    let manifest = baseline(&["manifest", "--bundle", bundle_dir.to_str().unwrap()]);
    assert!(
        manifest.status.success(),
        "o modo read-only não pode herdar a exigência de root"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn publicador_nao_mantem_lista_manual_de_capacidades_do_pink() {
    let script = fs::read_to_string(root().join("scripts/pink-baseline")).unwrap();
    // Os comandos do `pink` pertencem a `pink doctor`/`pink --help`, que os
    // derivam da própria CLI. Transcrevê-los aqui criaria uma segunda lista a
    // envelhecer sozinha.
    for capacidade in ["available_subcommands", "pink nav projecao", "pink repl"] {
        assert!(
            !script.contains(capacidade),
            "o publicador não deve duplicar capacidades já derivadas: {capacidade}"
        );
    }
}

#[test]
fn manifest_e_ficha_derivam_da_mesma_identidade() {
    let dir = temp("mesma_identidade");
    let release_root = dir.join("release");
    let manifest = render_manifest(&dir, &release_root);
    let ficha = render_ficha(&dir, &release_root);
    let fields = frontmatter(&ficha);
    // Um único fato, duas vistas derivadas: se divergirem, voltou a existir
    // mais de uma autoridade para a mesma identidade.
    assert!(manifest.contains(COMMIT) && ficha.contains(COMMIT));
    assert!(manifest.contains(&format!("\"version\": \"{}\"", field(&fields, "versao"))));
    assert!(manifest.contains(field(&fields, "arquivo_principal")));
    fs::remove_dir_all(&dir).ok();
}
// ---------------------------------------------------------------------------
// Transação de publicação
//
// `publish` altera três coisas: o diretório da release, o manifest ativo e o
// comando exposto. Só a última é ativação. A conferência do manifest pela
// autoridade canônica vinha depois da troca do launcher e sem volta, então uma
// rejeição deixava estado inválido vivo — e era justamente esse o caso real, já
// que o manifest publicado pela F1 é recusado pelo validador.
//
// Nada aqui toca `/opt`: a publicação roda em raiz isolada, e as duas
// autoridades externas — o launcher e o validador — são stubs que contam as
// próprias invocações. Contar invocação é o que permite exigir a falha em uma
// fase escolhida, em vez de torcer para ela cair no lugar certo.
// ---------------------------------------------------------------------------

#[cfg(unix)]
const COMMIT_TX: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
#[cfg(unix)]
const COMMIT_OUTRO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[cfg(unix)]
const LAUNCHER_STUB: &str = r#"#!/bin/sh
n=$(( $(cat "$PINK_TESTE_CONTADOR" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$PINK_TESTE_CONTADOR"
if [ "$n" = "${PINK_TESTE_BLOQUEAR_LAUNCHER:-0}" ]; then
  [ -n "${PINK_TESTE_SABOTAR_BIN:-}" ] && chmod 500 "$PINK_TESTE_SABOTAR_BIN"
  echo "launcher:$n" > "$PINK_TESTE_ALCANCOU"
  cat "$PINK_TESTE_PORTA" > /dev/null
fi
if [ "$1" = "--version-json" ]; then
  if [ "$n" = "${PINK_TESTE_MENTIR:-0}" ]; then
    printf '{"binary_commit":"@OUTRO@"}\n'
  else
    printf '{"binary_commit":"@COMMIT@"}\n'
  fi
fi
exit 0
"#;

#[cfg(unix)]
const VERIFICADOR_STUB: &str = r#"#!/bin/sh
# Sem argumentos, o validador canônico julga a COLEÇÃO inteira; com um caminho,
# julga aquele arquivo. O stub precisa dessa distinção para que a diferença entre
# "o arquivo é válido" e "a autoridade aceita o estado vivo" seja exercível.
if [ $# -eq 0 ]; then
  [ -n "${PINK_TESTE_COLECAO_RUIM:-}" ] && exit 1
  exit 0
fi
n=$(( $(cat "$PINK_TESTE_VERIFY_CONTADOR" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$PINK_TESTE_VERIFY_CONTADOR"
if [ "$n" = "${PINK_TESTE_BLOQUEAR_VERIFY:-0}" ]; then
  echo "verify:$n" > "$PINK_TESTE_ALCANCOU"
  cat "$PINK_TESTE_PORTA" > /dev/null
fi
if [ "$n" = "${PINK_TESTE_REPROVAR:-0}" ]; then
  exit 1
fi
exit 0
"#;

/// python3 que falha. Serve a um caso só: `render_manifest` é chamado DEPOIS de
/// a release entrar por rename e usa `fail`, que é `exit 1` puro — uma saída
/// inesperada que não passa por `abort_publication`.
#[cfg(unix)]
const PYTHON_QUE_FALHA: &str = "#!/bin/sh\nexit 7\n";

#[cfg(unix)]
fn sha256_de(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("executar sha256sum");
    assert!(output.status.success(), "sha256sum falhou em {path:?}");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("digest")
        .to_string()
}

#[cfg(unix)]
struct Cenario {
    dir: PathBuf,
    raiz: PathBuf,
    stubs: PathBuf,
    bundle: PathBuf,
    manifest: PathBuf,
    link: PathBuf,
    release: PathBuf,
}

#[cfg(unix)]
fn cenario(nome: &str) -> Cenario {
    let dir = temp(nome);
    let raiz = dir.join("raiz");
    let stubs = dir.join("stubs");
    let bundle_dir = dir.join("bundle");
    fs::create_dir_all(raiz.join("pinker")).expect("criar raiz isolada");
    fs::create_dir_all(&stubs).expect("criar stubs");
    fs::create_dir_all(&bundle_dir).expect("criar bundle");

    let launcher = bundle_dir.join("pink");
    fs::write(
        &launcher,
        LAUNCHER_STUB
            .replace("@COMMIT@", COMMIT_TX)
            .replace("@OUTRO@", COMMIT_OUTRO),
    )
    .expect("escrever launcher stub");
    make_executable(&launcher);
    let sha = sha256_de(&launcher);
    fs::write(
        bundle_dir.join("identity.env"),
        format!(
            "schema=forja-pink-bundle-v1\nversion=0.1.0\ncommit={COMMIT_TX}\nsha256={sha}\nsource={}\n",
            root().display()
        ),
    )
    .expect("escrever identity.env");

    let verificador = stubs.join("pinker-manifest-verify");
    fs::write(&verificador, VERIFICADOR_STUB).expect("escrever verificador stub");
    make_executable(&verificador);

    Cenario {
        manifest: raiz.join("pinker/manifests/pink-0.1.0.json"),
        link: raiz.join("pinker/bin/pink"),
        release: raiz.join(format!("pinker/releases/pink/{COMMIT_TX}")),
        dir,
        raiz,
        stubs,
        bundle: bundle_dir,
    }
}

#[cfg(unix)]
impl Cenario {
    fn publicar(&self, extra: &[(&str, &str)]) -> Output {
        // Contadores zerados por publicação: a fase alvo é indexada a partir da
        // primeira invocação desta tentativa, não da vida inteira do cenário.
        fs::remove_file(self.dir.join("contador")).ok();
        fs::remove_file(self.dir.join("verify-contador")).ok();
        let path = format!(
            "{}:{}",
            self.stubs.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = Command::new(root().join("scripts/pink-baseline"));
        command
            .args(["publish", "--bundle", self.bundle.to_str().unwrap()])
            .current_dir(root())
            .env("PATH", path)
            .env("PINK_BASELINE_ROOT", &self.raiz)
            .env("PINK_TESTE_CONTADOR", self.dir.join("contador"))
            .env(
                "PINK_TESTE_VERIFY_CONTADOR",
                self.dir.join("verify-contador"),
            );
        for (chave, valor) in extra {
            command.env(chave, valor);
        }
        command.output().expect("executar publish")
    }

    /// Publica em sessão própria, espera a fase alvo ser REALMENTE alcançada e
    /// só então sinaliza o grupo.
    ///
    /// A sincronização é handshake, não espera cega: o stub da fase alvo escreve
    /// um arquivo e bloqueia lendo um FIFO, e o sinal só sai depois que esse
    /// arquivo aparece. O poll existe para observar o handshake, não para
    /// adivinhar o tempo da fase.
    ///
    /// O sinal vai ao GRUPO porque o bash pai fica preso dentro da substituição
    /// de comando enquanto o stub bloqueia; sinalizar só o pai não o alcançaria
    /// na fase desejada.
    fn publicar_e_sinalizar(&self, sinal: &str, extra: &[(&str, &str)]) -> (i32, String) {
        let porta = self.dir.join("porta");
        let alcancou = self.dir.join("alcancou");
        let pidfile = self.dir.join("pgid");
        for caminho in [&porta, &alcancou, &pidfile] {
            fs::remove_file(caminho).ok();
        }
        fs::remove_file(self.dir.join("contador")).ok();
        fs::remove_file(self.dir.join("verify-contador")).ok();
        assert!(
            Command::new("mkfifo")
                .arg(&porta)
                .status()
                .expect("mkfifo")
                .success(),
            "criar FIFO de handshake"
        );

        let path = format!(
            "{}:{}",
            self.stubs.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        // `setsid sh -c` faz do sh o líder de sessão: $$ é o pgid, e o `exec`
        // preserva esse pid ao virar o publish.
        let roteiro = format!(
            "echo $$ > {pid}; exec {script} publish --bundle {bundle}",
            pid = pidfile.display(),
            script = root().join("scripts/pink-baseline").display(),
            bundle = self.bundle.display()
        );
        let mut command = Command::new("setsid");
        command
            .args(["sh", "-c", &roteiro])
            .current_dir(root())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PATH", path)
            .env("PINK_BASELINE_ROOT", &self.raiz)
            .env("PINK_TESTE_CONTADOR", self.dir.join("contador"))
            .env(
                "PINK_TESTE_VERIFY_CONTADOR",
                self.dir.join("verify-contador"),
            )
            .env("PINK_TESTE_PORTA", &porta)
            .env("PINK_TESTE_ALCANCOU", &alcancou);
        for (chave, valor) in extra {
            command.env(chave, valor);
        }
        let mut filho = command.spawn().expect("iniciar publish");

        let mut fase = String::new();
        for _ in 0..600 {
            if let Ok(conteudo) = fs::read_to_string(&alcancou) {
                fase = conteudo.trim().to_string();
                break;
            }
            if filho.try_wait().expect("try_wait").is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(
            !fase.is_empty(),
            "a fase alvo não foi alcançada — o sinal seria enviado às cegas"
        );

        let pgid = fs::read_to_string(&pidfile)
            .expect("pgid")
            .trim()
            .to_string();
        Command::new("kill")
            .args([&format!("-{sinal}"), &format!("-{pgid}")])
            .status()
            .expect("enviar sinal ao grupo");
        // Destrava o FIFO caso o stub tenha sobrevivido; limitado no tempo para
        // que abrir para escrita sem leitor nunca pendure o teste.
        Command::new("timeout")
            .args(["5", "sh", "-c", &format!("echo > {}", porta.display())])
            .status()
            .ok();

        let saida = filho.wait_with_output().expect("aguardar publish");
        fs::remove_file(&porta).ok();
        let erro = String::from_utf8_lossy(&saida.stderr).to_string();
        // O handler devolve a semântica do sinal matando o próprio processo com
        // ele, então o filho morre PELO sinal e não tem código de saída. A
        // convenção 128+n é a tradução usual, e é ela que prova que o sinal foi
        // preservado em vez de virar um exit qualquer.
        use std::os::unix::process::ExitStatusExt;
        let codigo = match (saida.status.code(), saida.status.signal()) {
            (Some(c), _) => c,
            (None, Some(s)) => 128 + s,
            (None, None) => -1,
        };
        (codigo, format!("{fase}\n{erro}"))
    }

    /// Estado vivo anterior deliberadamente diferente do que a republicação
    /// produziria. Restauração só é observável contra um anterior distinto.
    fn com_estado_anterior_distinto(&self) -> (String, PathBuf) {
        for pai in [self.manifest.parent(), self.link.parent()]
            .into_iter()
            .flatten()
        {
            fs::create_dir_all(pai).expect("criar diretório do estado anterior");
        }
        fs::write(&self.manifest, "manifest anterior invalido {\n").expect("manifest anterior");
        let decoy = self.raiz.join("pinker/releases/decoy/bin/pink");
        fs::create_dir_all(decoy.parent().unwrap()).expect("criar decoy");
        fs::write(&decoy, "#!/bin/sh\nexit 0\n").expect("escrever decoy");
        make_executable(&decoy);
        fs::remove_file(&self.link).ok();
        std::os::unix::fs::symlink(&decoy, &self.link).expect("apontar link para o decoy");
        (sha256_de(&self.manifest), decoy)
    }

    fn residuos(&self) -> Vec<PathBuf> {
        fn varrer(dir: &Path, achados: &mut Vec<PathBuf>) {
            let Ok(entradas) = fs::read_dir(dir) else {
                return;
            };
            for entrada in entradas.flatten() {
                let caminho = entrada.path();
                let nome = entrada.file_name().to_string_lossy().to_string();
                if nome.starts_with(".publish-")
                    || nome.starts_with(".pink-next-")
                    || nome.starts_with(".pink-restore-")
                    || nome.starts_with(".staging-")
                    || nome.contains(".tmp-")
                {
                    achados.push(caminho.clone());
                }
                if caminho.is_dir() && !caminho.is_symlink() {
                    varrer(&caminho, achados);
                }
            }
        }
        let mut achados = Vec::new();
        varrer(&self.raiz, &mut achados);
        achados
    }

    fn exigir_estado_anterior(&self, sha_manifest: &str, alvo_link: &Path, caso: &str) {
        assert_eq!(
            sha256_de(&self.manifest),
            sha_manifest,
            "{caso}: o manifest anterior precisa voltar byte a byte, não ser substituído por algo plausível"
        );
        assert!(
            self.link.is_symlink(),
            "{caso}: o comando exposto era symlink e precisa continuar symlink"
        );
        assert_eq!(
            fs::read_link(&self.link).expect("ler link"),
            alvo_link,
            "{caso}: o symlink anterior precisa voltar ao alvo exato"
        );
        assert!(
            self.residuos().is_empty(),
            "{caso}: resíduo enganoso deixado para trás: {:?}",
            self.residuos()
        );
    }

    fn limpar(self) {
        fs::remove_dir_all(self.dir).ok();
    }
}

#[cfg(unix)]
#[test]
fn publicacao_limpa_ativa_release_manifest_e_link_sem_residuo() {
    let cenario = cenario("publicacao_limpa");
    let saida = cenario.publicar(&[]);
    assert!(
        saida.status.success(),
        "publicação limpa falhou: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let relato = stdout(&saida);
    assert!(relato.contains("status=PUBLISHED"));
    // O relato precisa dizer QUEM validou antes de ativar: uma publicação que
    // não conseguiu validar nada antes não pode se parecer com uma que validou.
    assert!(
        relato.contains("pre_activation_validation=pinker-manifest-verify"),
        "o relato deve nomear a autoridade que julgou o candidato: {relato}"
    );
    assert!(cenario.manifest.is_file());
    assert_eq!(
        fs::read_link(&cenario.link).expect("ler link"),
        cenario.release.join("bin/pink")
    );
    assert!(cenario.residuos().is_empty());
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn candidato_rejeitado_nao_chega_a_ativar_nada() {
    let cenario = cenario("candidato_rejeitado");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // Reprovar na PRIMEIRA invocação do validador é reprovar o candidato, que
    // por construção acontece antes da troca do launcher.
    let saida = cenario.publicar(&[("PINK_TESTE_REPROVAR", "1")]);
    assert!(!saida.status.success());
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        erro.contains("antes da ativação"),
        "o erro precisa dizer que a reprovação foi anterior à ativação: {erro}"
    );
    cenario.exigir_estado_anterior(&sha, &decoy, "candidato rejeitado");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn launcher_publicado_que_nao_declara_o_commit_restaura_o_estado_anterior() {
    let cenario = cenario("identidade_pos_ativacao");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // Invocação 1 do launcher = conferência do bundle; invocação 2 = conferência
    // do comando exposto, já ativado. Mentir só na segunda coloca a falha
    // exatamente depois da ativação, sem mexer nos bytes conferidos por sha256.
    let saida = cenario.publicar(&[("PINK_TESTE_MENTIR", "2")]);
    assert!(!saida.status.success());
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(erro.contains("não declara"), "erro inesperado: {erro}");
    assert!(
        erro.contains("RESTORED_EXACTLY"),
        "a restauração precisa ser afirmada por observação, não presumida: {erro}"
    );
    cenario.exigir_estado_anterior(&sha, &decoy, "identidade divergente");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn manifest_rejeitado_depois_da_ativacao_restaura_o_estado_anterior() {
    let cenario = cenario("manifest_pos_ativacao");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // Invocação 2 do validador é a conferência do manifest já instalado: é o
    // caso vivo da F2, em que o registro publicado é recusado pela autoridade.
    let saida = cenario.publicar(&[("PINK_TESTE_REPROVAR", "2")]);
    assert!(!saida.status.success());
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        erro.contains("rejeitado por pinker-manifest-verify"),
        "erro inesperado: {erro}"
    );
    assert!(erro.contains("RESTORED_EXACTLY"), "erro inesperado: {erro}");
    cenario.exigir_estado_anterior(&sha, &decoy, "manifest rejeitado");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn publicacao_falha_em_raiz_virgem_nao_deixa_release_nem_manifest() {
    let cenario = cenario("raiz_virgem");
    let saida = cenario.publicar(&[("PINK_TESTE_REPROVAR", "1")]);
    assert!(!saida.status.success());
    // Nada existia antes; a falha não pode inventar meia instalação. Uma release
    // órfã é exatamente o resíduo que faz a próxima publicação achar que o
    // trabalho já estava feito.
    assert!(!cenario.release.exists(), "release órfã sobreviveu à falha");
    assert!(
        !cenario.manifest.exists(),
        "manifest apareceu apesar da falha"
    );
    assert!(
        !cenario.link.exists(),
        "launcher foi exposto apesar da falha"
    );
    assert!(cenario.residuos().is_empty(), "{:?}", cenario.residuos());
    cenario.limpar();
}

// ---------------------------------------------------------------------------
// Término inesperado
//
// A transação anterior cobria só as falhas que chegavam a `abort_publication`.
// O único trap era `EXIT -> publish_cleanup_scratch`, e o scratch é justamente
// onde vivem os backups: qualquer saída que não fosse pela rota explícita
// apagava a única forma de voltar. Reproduzido antes de corrigir — SIGTERM
// durante a conferência pós-ativação deixava manifest e launcher ativados,
// não conferidos, e sem estado anterior.
//
// Agora há um funil só. `set -e` já transforma falha inesperada em saída, então
// EXIT é o funil natural; INT e TERM apenas convertem o sinal em saída e depois
// devolvem a semântica dele. O estado da transação decide o que desfazer, e a
// limpeza do scratch é sempre a última coisa.
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn t1_sinal_antes_da_ativacao_nao_deixa_nada_ativo_nem_release_orfa() {
    let cenario = cenario("t1_term_pre_ativacao");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // Bloquear a primeira invocação do validador é parar exatamente na
    // prevalidation: a release já entrou por rename, nada foi ativado.
    let (codigo, relato) =
        cenario.publicar_e_sinalizar("TERM", &[("PINK_TESTE_BLOQUEAR_VERIFY", "1")]);
    assert!(
        relato.starts_with("verify:1"),
        "fase alvo inesperada: {relato}"
    );
    assert_eq!(codigo, 143, "TERM precisa preservar a semântica do sinal");

    cenario.exigir_estado_anterior(&sha, &decoy, "T1");
    assert!(
        !cenario.release.exists(),
        "a release preparada por esta tentativa não pode ficar órfã"
    );
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn t2_sigterm_depois_da_ativacao_restaura_o_estado_anterior() {
    let cenario = cenario("t2_term_pos_ativacao");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // Invocação 1 do launcher é a conferência do bundle; a 2 é a do comando
    // exposto, já ativado. Bloquear ali põe o sinal na janela em que o estado
    // vivo mudou e ainda não foi conferido — a janela que estava descoberta.
    let (codigo, relato) =
        cenario.publicar_e_sinalizar("TERM", &[("PINK_TESTE_BLOQUEAR_LAUNCHER", "2")]);
    assert!(
        relato.starts_with("launcher:2"),
        "fase alvo inesperada: {relato}"
    );
    assert_eq!(codigo, 143);
    assert!(
        relato.contains("RESTORED_EXACTLY"),
        "a restauração precisa ser afirmada por observação: {relato}"
    );
    cenario.exigir_estado_anterior(&sha, &decoy, "T2");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn t3_sigint_em_fase_material_restaura_igualmente() {
    let cenario = cenario("t3_int");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let (codigo, relato) =
        cenario.publicar_e_sinalizar("INT", &[("PINK_TESTE_BLOQUEAR_LAUNCHER", "2")]);
    assert!(
        relato.starts_with("launcher:2"),
        "fase alvo inesperada: {relato}"
    );
    assert_eq!(codigo, 130, "INT precisa preservar a semântica do sinal");
    cenario.exigir_estado_anterior(&sha, &decoy, "T3");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn t4_falha_inesperada_sob_set_e_restaura_em_vez_de_so_limpar() {
    let cenario = cenario("t4_set_e");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // `render_manifest` roda depois de a release entrar por rename e chama
    // `fail`, que é `exit 1` puro: uma saída que nunca passou por
    // `abort_publication`. Antes, isso deixava release órfã e apagava backups.
    let python = cenario.stubs.join("python3");
    fs::write(&python, PYTHON_QUE_FALHA).expect("escrever python3 que falha");
    make_executable(&python);

    let saida = cenario.publicar(&[]);
    assert!(!saida.status.success());
    cenario.exigir_estado_anterior(&sha, &decoy, "T4");
    assert!(
        !cenario.release.exists(),
        "falha não roteada também precisa desfazer a release preparada"
    );
    fs::remove_file(&python).ok();
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn t5_caminho_de_sucesso_nao_executa_rollback() {
    let cenario = cenario("t5_sucesso");
    cenario.publicar(&[]);
    // Estado anterior distinto para que um rollback indevido seja visível: se o
    // funil desfizesse depois do commit, o decoy voltaria.
    let (_sha, decoy) = cenario.com_estado_anterior_distinto();

    let saida = cenario.publicar(&[]);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let relato = stdout(&saida);
    assert!(relato.contains("status=PUBLISHED"));
    assert!(
        !String::from_utf8_lossy(&saida.stderr).contains("estado anterior"),
        "sucesso não pode emitir veredito de restauração"
    );
    assert_eq!(
        fs::read_link(&cenario.link).expect("ler link"),
        cenario.release.join("bin/pink"),
        "o estado novo precisa permanecer ativo"
    );
    assert_ne!(
        fs::read_link(&cenario.link).expect("ler link"),
        decoy,
        "o rollback não pode ter rodado no caminho de sucesso"
    );
    assert!(cenario.residuos().is_empty(), "{:?}", cenario.residuos());
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn t6_falha_durante_o_proprio_rollback_termina_como_divergente() {
    let cenario = cenario("t6_rollback_falho");
    cenario.publicar(&[]);
    let (_sha, _decoy) = cenario.com_estado_anterior_distinto();

    // O stub sabota o diretório do launcher ANTES de bloquear, então a
    // restauração do symlink não tem como funcionar. O contrato aqui não é
    // "restaurar sempre" — é não mentir: sem recursão, sem laço, e com terminal
    // de alta severidade dizendo que o anterior não voltou.
    let bin = cenario.raiz.join("pinker/bin");
    let (codigo, relato) = cenario.publicar_e_sinalizar(
        "TERM",
        &[
            ("PINK_TESTE_BLOQUEAR_LAUNCHER", "2"),
            ("PINK_TESTE_SABOTAR_BIN", bin.to_str().unwrap()),
        ],
    );
    make_executable(&bin); // devolve a permissão para poder limpar
    assert!(
        relato.starts_with("launcher:2"),
        "fase alvo inesperada: {relato}"
    );
    assert!(
        relato.contains("RESTORATION_DIVERGED"),
        "restauração impossível precisa terminar como divergente: {relato}"
    );
    assert_eq!(
        codigo, 3,
        "divergência de restauração é mais grave que o sinal e precisa de terminal próprio"
    );
    assert_eq!(
        relato.matches("estado anterior").count(),
        1,
        "o rollback não pode rodar duas vezes nem recursar: {relato}"
    );
    cenario.limpar();
}

// ---------------------------------------------------------------------------
// Lifecycle do manifest na coleção ativa
//
// Toda entrada de /opt/pinker/manifests é autoridade viva: o validador canônico
// faz glob de *.json e valida cada uma, parando na primeira falha. Não há
// índice, pointer nem seletor — estar no diretório É ser ativo.
//
// A F1 batizou o manifest Pinker com o commit no nome. Como o nome é a chave da
// coleção e a política manda substituir atomicamente, isso converteu "substituir"
// em "acrescentar": cada publicação deixaria para trás uma autoridade viva
// descrevendo uma instalação que não está mais exposta — e que falha justamente
// por isso. O nome voltou à convenção de todo software aprovado.
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn o_manifest_usa_a_convencao_de_nome_da_colecao() {
    let cenario = cenario("nome_canonico");
    let saida = cenario.publicar(&[]);
    assert!(
        saida.status.success(),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );

    let nome = cenario
        .manifest
        .file_name()
        .and_then(|n| n.to_str())
        .expect("nome do manifest");
    assert_eq!(
        nome, "pink-0.1.0.json",
        "o nome precisa ser <software>-<versão>.json, como todo manifest da coleção"
    );
    assert!(
        !nome.contains(&COMMIT_TX[..12]),
        "o commit no nome do arquivo transforma substituição em acúmulo"
    );
    assert!(cenario.manifest.is_file());
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn republicar_substitui_o_manifest_em_vez_de_acumular_autoridades() {
    let cenario = cenario("substituicao");
    cenario.publicar(&[]);
    let primeiro = sha256_de(&cenario.manifest);

    // Segunda publicação do mesmo software: a coleção não pode crescer.
    let saida = cenario.publicar(&[]);
    assert!(saida.status.success());
    let manifests: Vec<_> = fs::read_dir(cenario.raiz.join("pinker/manifests"))
        .expect("ler coleção")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        manifests,
        vec!["pink-0.1.0.json".to_string()],
        "a coleção ativa precisa ter uma autoridade Pinker, não uma por publicação"
    );
    assert_eq!(
        sha256_de(&cenario.manifest),
        primeiro,
        "substituição idempotente"
    );
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn autoridade_pinker_concorrente_bloqueia_a_publicacao_e_e_nomeada() {
    let cenario = cenario("supersessao");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    // Um manifest Pinker legado, sob o nome antigo. É JSON válido e declara
    // `name: Pinker CLI` — a chave de identidade real, já que o validador nunca
    // interpreta o nome do arquivo.
    let legado = cenario
        .raiz
        .join("pinker/manifests/pink-0.1.0-abcabcabcabc.json");
    fs::write(&legado, "{\n  \"name\": \"Pinker CLI\"\n}\n").expect("manifest legado");

    let saida = cenario.publicar(&[]);
    assert!(
        !saida.status.success(),
        "publicar por cima deixaria duas autoridades vivas"
    );
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        erro.contains("pink-0.1.0-abcabcabcabc.json"),
        "o bloqueio precisa NOMEAR o que retirar: {erro}"
    );
    assert!(erro.contains("retire os manifests"), "{erro}");
    // Bloquear é antes de ativar: o estado anterior continua intacto.
    cenario.exigir_estado_anterior(&sha, &decoy, "supersessão");
    assert!(
        legado.is_file(),
        "o publicador não escolhe destino para arquivo alheio"
    );
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn manifest_ilegivel_na_colecao_tambem_bloqueia() {
    let cenario = cenario("ilegivel");
    // O manifest que a F1 publicou não é JSON válido — não dá para ler o `name`
    // dele. Quem não consegue provar que o arquivo NÃO é seu não pode publicar
    // por cima: fail closed.
    let quebrado = cenario
        .raiz
        .join("pinker/manifests/pink-0.1.0-quebrado.json");
    fs::create_dir_all(quebrado.parent().unwrap()).expect("criar coleção");
    fs::write(&quebrado, "{ \"name\": \"Pinker CLI\"\n").expect("manifest ilegível");

    let saida = cenario.publicar(&[]);
    assert!(!saida.status.success());
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(erro.contains("ILEGIVEL"), "{erro}");
    assert!(erro.contains("pink-0.1.0-quebrado.json"), "{erro}");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn manifest_de_outro_software_nao_e_confundido_com_autoridade_pinker() {
    let cenario = cenario("alheio");
    let alheio = cenario.raiz.join("pinker/manifests/outro-1.0.0.json");
    fs::create_dir_all(alheio.parent().unwrap()).expect("criar coleção");
    fs::write(&alheio, "{\n  \"name\": \"Outro Software\"\n}\n").expect("manifest alheio");

    let saida = cenario.publicar(&[]);
    assert!(
        saida.status.success(),
        "manifest de outro software não é autoridade Pinker concorrente: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert!(alheio.is_file(), "e não pode ser tocado");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn o_relato_declara_o_estado_da_colecao_e_nao_so_do_arquivo() {
    // O contrato é "a autoridade canônica aceita o estado vivo", não "o arquivo
    // novo é válido isolado". O relato precisa carregar essa diferença.
    let cenario = cenario("estado_da_colecao");
    let saida = cenario.publicar(&[]);
    assert!(saida.status.success());
    let relato = stdout(&saida);
    assert!(
        relato.contains("collection_state="),
        "o relato precisa declarar o veredito da coleção: {relato}"
    );
    // Com a coleção sadia, ACCEPTED.
    assert!(relato.contains("collection_state=ACCEPTED"), "{relato}");
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn colecao_quebrada_por_software_alheio_e_declarada_sem_bloquear() {
    // Uma coleção reprovada por manifest de OUTRO software não é falha desta
    // publicação — acoplar as duas coisas travaria o Pinker por causa de terceiro.
    // Mas também não pode ser relatada como saúde, então vira estado declarado.
    let cenario = cenario("colecao_degradada");
    let saida = cenario.publicar(&[("PINK_TESTE_COLECAO_RUIM", "1")]);
    assert!(
        saida.status.success(),
        "manifest alheio quebrado não pode bloquear a publicação: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let relato = stdout(&saida);
    assert!(
        relato.contains("collection_state=DEGRADED_BY_FOREIGN_MANIFEST"),
        "o relato precisa distinguir arquivo válido de estado vivo aceito: {relato}"
    );
    // E o manifest próprio continua sendo exigido válido.
    assert!(relato.contains("status=PUBLISHED"));
    cenario.limpar();
}

#[cfg(unix)]
#[test]
fn a_seam_de_isolamento_nunca_alcanca_a_instalacao_real() {
    let dir = temp("seam");
    let bundle_dir = bundle(&dir, SHA256);
    // Sem a variável, o default é literalmente /opt: a seam não pode ter mudado
    // o comportamento de produção.
    let padrao = baseline(&["manifest", "--bundle", bundle_dir.to_str().unwrap()]);
    assert!(stdout(&padrao).contains("\"/opt/pinker/bin/pink\""));

    for alvo in ["/opt/pinker", "/", "relativo"] {
        let saida = Command::new(root().join("scripts/pink-baseline"))
            .args(["manifest", "--bundle", bundle_dir.to_str().unwrap()])
            .current_dir(root())
            .env("PINK_BASELINE_ROOT", alvo)
            .output()
            .expect("executar pink-baseline");
        assert!(
            !saida.status.success(),
            "a seam aceitou uma raiz proibida: {alvo}"
        );
        assert!(String::from_utf8_lossy(&saida.stderr).contains("PINK_BASELINE_ROOT"));
    }
    fs::remove_dir_all(&dir).ok();
}

// @pinker-nav:end evidencia.tooling.f2.registros-derivados
