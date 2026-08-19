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
use std::process::{Child, Command, Output, Stdio};
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

/// Diretório temporário que se desfaz sozinho.
///
/// A remoção pertence ao término do valor, e não a uma linha escrita depois do
/// último assert: um teste que morre no meio — assert que falha, pânico,
/// handshake que não chega — não pode deixar fixture para trás. E a remoção
/// devolve a permissão de travessia antes de tentar, porque dois testes desta
/// suíte retiram permissão de propósito para provar que o rollback falha, e um
/// diretório em modo 500 não é apenas sujeira: é sujeira IRREMOVÍVEL, que
/// contamina inspeção, limpeza, os testes seguintes, o fresh e o diagnóstico.
///
/// É o mesmo formato de defeito que esta suíte cobra do publicador — efeito
/// material governado por um passo posterior que pode não acontecer — e a
/// resposta é a mesma: a desfeita pertence ao término.
struct DirTemporario {
    caminho: PathBuf,
}

impl std::ops::Deref for DirTemporario {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.caminho
    }
}

impl Drop for DirTemporario {
    fn drop(&mut self) {
        devolver_permissao_de_travessia(&self.caminho);
        fs::remove_dir_all(&self.caminho).ok();
    }
}

/// Devolve `u+rwx` a todo diretório da árvore, sem seguir symlink.
///
/// `remove_dir_all` precisa de travessia e escrita em cada diretório do caminho.
#[cfg(unix)]
fn devolver_permissao_de_travessia(caminho: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = fs::symlink_metadata(caminho) else {
        return;
    };
    if !meta.is_dir() {
        return;
    }
    let modo = meta.permissions().mode();
    if modo & 0o700 != 0o700 {
        fs::set_permissions(caminho, fs::Permissions::from_mode(modo | 0o700)).ok();
    }
    let Ok(entradas) = fs::read_dir(caminho) else {
        return;
    };
    for entrada in entradas.flatten() {
        devolver_permissao_de_travessia(&entrada.path());
    }
}

#[cfg(not(unix))]
fn devolver_permissao_de_travessia(_caminho: &Path) {}

fn temp(name: &str) -> DirTemporario {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let caminho = std::env::temp_dir().join(format!("pinker_f2_{name}_{stamp}"));
    fs::create_dir_all(&caminho).expect("criar diretório temporário");
    DirTemporario { caminho }
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
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
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
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
fi
if [ "$n" = "${PINK_TESTE_REPROVAR:-0}" ]; then
  exit 1
fi
exit 0
"#;

/// `install` que faz o trabalho real e só então bloqueia. Serve a uma janela que
/// nenhum stub de launcher ou validador alcança: o instante entre o manifest já
/// instalado e o link ainda não trocado.
#[cfg(unix)]
const INSTALL_QUE_BLOQUEIA: &str = r#"#!/bin/sh
/usr/bin/install "$@" || exit $?
for arg in "$@"; do
  case "$arg" in
    *manifests/pink-*)
      echo "install:manifest" > "$PINK_TESTE_ALCANCOU"
      timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
      ;;
  esac
done
exit 0
"#;

/// `mv` que faz a troca REAL do launcher, PROVA que ela aconteceu e só então
/// bloqueia. É o único instrumento que alcança a janela do outro lado da troca:
/// depois de `mv -Tf` retornar 0 e antes de a instrução seguinte do publicador
/// rodar. Stub de launcher ou de validador nunca chega ali — quando eles são
/// chamados, o publicador já executou tudo que vinha depois do `mv`.
///
/// Só a troca do launcher é interceptada, reconhecida pelo link candidato
/// `.pink-next-*`. O rename do staging da release e — decisivo — o `mv` do
/// PRÓPRIO rollback passam direto: bloquear o rollback destruiria o
/// experimento em vez de observá-lo.
#[cfg(unix)]
const MV_QUE_BLOQUEIA_APOS_A_TROCA: &str = r#"#!/bin/sh
troca_do_launcher=no
for a in "$@"; do
  case "$a" in
    */.pink-next-*) troca_do_launcher=yes ;;
  esac
done

/usr/bin/mv "$@"
rc=$?

if [ "$troca_do_launcher" = yes ] && [ "$rc" = 0 ] && [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
  # A prova de que a superfície alvo foi atingida é lida DE DENTRO da janela:
  # o comando exposto já é o novo, e o publicador ainda não seguiu adiante.
  printf 'mv:launcher-trocado alvo=%s\n' \
    "$(readlink "$PINK_TESTE_EXPOSTO" 2>/dev/null || echo AUSENTE)" \
    > "$PINK_TESTE_ALCANCOU"
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
fi
exit $rc
"#;

/// `ln` que cria o link candidato de verdade e só então bloqueia. Alcança a
/// janela entre o candidato existir e o `mv` consumi-lo — instante que nem T7
/// (bloqueia na instalação do manifest, antes do `ln`) nem T9 (bloqueia depois
/// do `mv`, com o candidato já consumido) tocam.
///
/// O `ln` do PRÓPRIO rollback (`.pink-restore-*`) não é interceptado: bloquear o
/// rollback observaria o experimento em vez do sujeito.
#[cfg(unix)]
const LN_QUE_BLOQUEIA_APOS_O_CANDIDATO: &str = r#"#!/bin/sh
candidato=no
for a in "$@"; do
  case "$a" in
    */.pink-next-*) candidato=yes ;;
  esac
done

/usr/bin/ln "$@"
rc=$?

if [ "$candidato" = yes ] && [ "$rc" = 0 ] && [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
  # Prova lida DE DENTRO da janela: o candidato existe e o comando exposto ainda
  # é o anterior — a troca viva não aconteceu.
  printf 'ln:candidato-criado exposto=%s\n' \
    "$(readlink "$PINK_TESTE_EXPOSTO" 2>/dev/null || echo AUSENTE)" \
    > "$PINK_TESTE_ALCANCOU"
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
fi
exit $rc
"#;

/// `mv` que instala a release de verdade e só então bloqueia. Intercepta SOMENTE
/// a instalação da release — origem dentro de `.publish-<commit>-<pid>` e
/// terminando em `/release`. A troca do launcher (`-Tf`, `.pink-next-*`) e o
/// rename do próprio rollback (`-Tf`, `.pink-restore-*`) passam direto.
#[cfg(unix)]
const MV_QUE_BLOQUEIA_APOS_INSTALAR_A_RELEASE: &str = r#"#!/bin/sh
origem=""
for a in "$@"; do
  case "$a" in
    -*) ;;
    *) [ -z "$origem" ] && origem="$a" ;;
  esac
done

instalacao_da_release=no
case "$origem" in
  */.publish-*/release) instalacao_da_release=yes ;;
esac

/usr/bin/mv "$@"
rc=$?

if [ "$instalacao_da_release" = yes ] && [ "$rc" = 0 ] && [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
  # Prova lida DE DENTRO da janela: a release já está instalada e o publicador
  # ainda não executou a instrução seguinte.
  printf 'mv:release-installed binario=%s exposto=%s\n' \
    "$([ -x "$PINK_TESTE_RELEASE/bin/pink" ] && echo sim || echo nao)" \
    "$(readlink "$PINK_TESTE_EXPOSTO" 2>/dev/null || echo AUSENTE)" \
    > "$PINK_TESTE_ALCANCOU"
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
fi
exit $rc
"#;

/// `mv` que simula OUTRA execução criando `release_root` na janela entre a
/// checagem de ausência e a instalação. Determinístico: não depende de
/// agendamento nem de duas publicações reais concorrentes.
#[cfg(unix)]
const MV_QUE_SIMULA_OUTRA_EXECUCAO: &str = r#"#!/bin/sh
origem=""
for a in "$@"; do
  case "$a" in
    -*) ;;
    *) [ -z "$origem" ] && origem="$a" ;;
  esac
done
case "$origem" in
  */.publish-*/release)
    if [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
      echo "outra-execucao-criou-o-destino" > "$PINK_TESTE_ALCANCOU"
      mkdir -p "$PINK_TESTE_RELEASE/bin"
      printf '#!/bin/sh\nexit 0\n' > "$PINK_TESTE_RELEASE/bin/pink"
      chmod 755 "$PINK_TESTE_RELEASE/bin/pink"
      printf 'alheio\n' > "$PINK_TESTE_RELEASE/PERTENCE-A-OUTRA-EXECUCAO"
    fi
    ;;
esac
exec /usr/bin/mv "$@"
"#;

/// `mv` que deixa a troca de volta do rollback FALHAR, sem executá-la. Serve para
/// alcançar o único caminho em que o link temporário do rollback sobrevive: ele é
/// criado em `pinker/bin`, fora do scratch, e só é consumido se o rename der certo.
#[cfg(unix)]
const MV_QUE_FALHA_NA_VOLTA_DO_ROLLBACK: &str = r#"#!/bin/sh
for a in "$@"; do
  case "$a" in
    */.pink-restore-*) exit 1 ;;
  esac
done
exec /usr/bin/mv "$@"
"#;

/// `install` que faz a volta REAL do manifest e só então bloqueia. É o único
/// instrumento que alcança o meio do próprio rollback: quando ele bloqueia, a
/// restauração do launcher já aconteceu e a remoção da release ainda não.
///
/// Só a volta é interceptada, reconhecida pela origem `manifest.anterior` —
/// o backup capturado antes da primeira mutação. A instalação do manifest NOVO
/// (origem `manifest.json`) e os `install -d` passam direto: bloquear o caminho
/// de ida mediria outra janela, já coberta pelo T7.
#[cfg(unix)]
const INSTALL_QUE_BLOQUEIA_NA_VOLTA_DO_MANIFEST: &str = r#"#!/bin/sh
volta=no
for a in "$@"; do
  case "$a" in
    */manifest.anterior) volta=yes ;;
  esac
done

/usr/bin/install "$@"
rc=$?

if [ "$volta" = yes ] && [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
  # Prova lida DE DENTRO da janela: o rollback ORIGINAL já começou — o comando
  # exposto voltou ao alvo anterior — e ainda não terminou: a release instalada
  # por esta transação continua no lugar.
  printf 'install:volta-do-manifest exposto=%s release=%s\n' \
    "$(readlink "$PINK_TESTE_EXPOSTO" 2>/dev/null || echo AUSENTE)" \
    "$([ -d "$PINK_TESTE_RELEASE" ] && echo presente || echo ausente)" \
    > "$PINK_TESTE_ALCANCOU"
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
fi
exit $rc
"#;

/// `ln` que cria o link temporário do rollback de verdade e só então bloqueia.
/// Alcança a janela mais destrutiva do funil: a volta do launcher ainda não
/// aconteceu, o manifest anterior ainda não voltou e os backups do scratch ainda
/// são necessários.
///
/// É o complemento exato de `LN_QUE_BLOQUEIA_APOS_O_CANDIDATO`, que intercepta
/// `.pink-next-*` e deixa `.pink-restore-*` passar: aqui é o contrário, porque
/// aqui o sujeito observado É o rollback.
#[cfg(unix)]
const LN_QUE_BLOQUEIA_NA_VOLTA_DO_LAUNCHER: &str = r#"#!/bin/sh
volta=no
for a in "$@"; do
  case "$a" in
    */.pink-restore-*) volta=yes ;;
  esac
done

/usr/bin/ln "$@"
rc=$?

if [ "$volta" = yes ] && [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
  # Prova lida DE DENTRO da janela: o rollback comecou, e nada do estado vivo
  # voltou ainda — o comando exposto e o da release nova.
  printf 'ln:volta-do-launcher exposto=%s release=%s\n' \
    "$(readlink "$PINK_TESTE_EXPOSTO" 2>/dev/null || echo AUSENTE)" \
    "$([ -d "$PINK_TESTE_RELEASE" ] && echo presente || echo ausente)" \
    > "$PINK_TESTE_ALCANCOU"
  timeout 30 cat "$PINK_TESTE_PORTA" > /dev/null || true
fi
exit $rc
"#;

/// python3 que falha. Serve a um caso só: `render_manifest` é chamado DEPOIS de
/// a release entrar por rename e usa `fail`, que é `exit 1` puro — uma saída
/// inesperada que não passa por `abort_publication`.
#[cfg(unix)]
const PYTHON_QUE_FALHA: &str = "#!/bin/sh\nexit 7\n";

#[cfg(unix)]
fn conjunto_de_manifests(raiz: &Path) -> Vec<String> {
    let mut nomes: Vec<String> = fs::read_dir(raiz.join("pinker/manifests"))
        .map(|entradas| {
            entradas
                .flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    nomes.sort();
    nomes
}

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
    dir: DirTemporario,
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
    /// Base comum de toda invocação do publicador contra a raiz isolada.
    ///
    /// `contador` separa as invocações de duas publicações que convivem no mesmo
    /// cenário: a fase alvo é indexada a partir da primeira invocação DAQUELA
    /// tentativa, não da vida inteira do cenário.
    fn comando(&self, bundle: &Path, contador: &str, release: &Path) -> Command {
        fs::remove_file(self.dir.join(contador)).ok();
        fs::remove_file(self.dir.join(format!("verify-{contador}"))).ok();
        let path = format!(
            "{}:{}",
            self.stubs.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut command = Command::new(root().join("scripts/pink-baseline"));
        command
            .args(["publish", "--bundle", bundle.to_str().unwrap()])
            .current_dir(root())
            .env("PATH", path)
            .env("PINK_BASELINE_ROOT", &self.raiz)
            .env("PINK_TESTE_CONTADOR", self.dir.join(contador))
            .env(
                "PINK_TESTE_VERIFY_CONTADOR",
                self.dir.join(format!("verify-{contador}")),
            )
            .env("PINK_TESTE_EXPOSTO", &self.link)
            .env("PINK_TESTE_RELEASE", release);
        command
    }

    fn publicar(&self, extra: &[(&str, &str)]) -> Output {
        let mut command = self.comando(&self.bundle, "contador", &self.release);
        for (chave, valor) in extra {
            command.env(chave, valor);
        }
        command.output().expect("executar publish")
    }

    /// Segunda publicação LEGÍTIMA, de outro commit, contra a MESMA raiz viva.
    /// Contadores próprios: as duas tentativas não podem compartilhar a
    /// indexação de fases.
    fn publicar_concorrente(&self, bundle: &Path, release: &Path) -> Output {
        self.comando(bundle, "contador-concorrente", release)
            .output()
            .expect("executar publish concorrente")
    }

    /// Sobe uma publicação e espera a fase alvo ser REALMENTE alcançada.
    ///
    /// A sincronização é handshake, não espera cega: o stub da fase alvo escreve
    /// um arquivo e bloqueia lendo um FIFO, e quem chamou só age depois que esse
    /// arquivo aparece. Devolve o processo ainda BLOQUEADO — o que o chamador faz
    /// com essa janela é escolha dele: sinalizar, ou deixar outro ator agir.
    fn publicar_ate_a_fase(&self, extra: &[(&str, &str)]) -> (Child, String, PathBuf) {
        let porta = self.dir.join("porta");
        let alcancou = self.dir.join("alcancou");
        for caminho in [&porta, &alcancou] {
            fs::remove_file(caminho).ok();
        }
        assert!(
            Command::new("mkfifo")
                .arg(&porta)
                .status()
                .expect("mkfifo")
                .success(),
            "criar FIFO de handshake"
        );

        let mut command = self.comando(&self.bundle, "contador", &self.release);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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
            "a fase alvo não foi alcançada — o experimento seria cego"
        );
        (filho, fase, porta)
    }

    /// Destrava o handshake e colhe o término.
    ///
    /// O bash adia o tratamento do trap enquanto há comando em primeiro plano,
    /// então destravar o FIFO logo depois de sinalizar é o que faz o handler
    /// rodar em seguida.
    fn concluir(&self, filho: Child, fase: String, porta: PathBuf) -> (i32, String) {
        Command::new("timeout")
            .args(["10", "sh", "-c", &format!("echo > {}", porta.display())])
            .status()
            .ok();

        let saida = filho.wait_with_output().expect("aguardar publish");
        fs::remove_file(&porta).ok();
        let erro = String::from_utf8_lossy(&saida.stderr).to_string();
        // Quando o handler devolve a semântica do sinal, o filho morre PELO sinal
        // e não tem código de saída. A convenção 128+n é o que prova que a
        // semântica foi preservada.
        use std::os::unix::process::ExitStatusExt;
        let codigo = match (saida.status.code(), saida.status.signal()) {
            (Some(c), _) => c,
            (None, Some(s)) => 128 + s,
            (None, None) => -1,
        };
        (codigo, format!("{fase}\n{erro}"))
    }

    /// Publica, espera a fase alvo e só então sinaliza.
    ///
    /// O sinal vai para o processo do publish e para mais ninguém. Sinalizar o
    /// GRUPO alcançaria quem estiver no mesmo grupo — em CI, o próprio runner.
    fn publicar_e_sinalizar(&self, sinal: &str, extra: &[(&str, &str)]) -> (i32, String) {
        let (filho, fase, porta) = self.publicar_ate_a_fase(extra);
        let alvo = filho.id().to_string();
        Command::new("kill")
            .args([&format!("-{sinal}"), &alvo])
            .status()
            .expect("sinalizar o publish");
        self.concluir(filho, fase, porta)
    }

    /// Publica, espera a fase alvo e deixa OUTRO ator agir sobre o estado vivo
    /// enquanto esta publicação está parada dentro da transação. Só então
    /// destrava. É o instrumento da concorrência: nada de sinal, dois
    /// publicadores reais.
    fn publicar_e_intercalar<T>(
        &self,
        extra: &[(&str, &str)],
        durante: impl FnOnce(&str) -> T,
    ) -> (i32, String, T) {
        let (filho, fase, porta) = self.publicar_ate_a_fase(extra);
        let observado = durante(&fase);
        let (codigo, relato) = self.concluir(filho, fase, porta);
        (codigo, relato, observado)
    }

    /// Bundle de uma segunda publicação legítima: mesmo software, mesma versão,
    /// OUTRO commit. É o caso real de duas publicações concorrentes — duas
    /// execuções de CI publicando builds diferentes —, e é o que faz manifest e
    /// launcher serem objetos disputados.
    fn bundle_de_outro_commit(&self) -> (PathBuf, PathBuf) {
        let dir = self.dir.join("bundle-concorrente");
        fs::create_dir_all(&dir).expect("criar bundle concorrente");
        let launcher = dir.join("pink");
        fs::write(
            &launcher,
            LAUNCHER_STUB
                .replace("@COMMIT@", COMMIT_OUTRO)
                .replace("@OUTRO@", COMMIT_TX),
        )
        .expect("escrever launcher concorrente");
        make_executable(&launcher);
        let sha = sha256_de(&launcher);
        fs::write(
            dir.join("identity.env"),
            format!(
                "schema=forja-pink-bundle-v1\nversion=0.1.0\ncommit={COMMIT_OUTRO}\nsha256={sha}\nsource={}\n",
                root().display()
            ),
        )
        .expect("escrever identity.env concorrente");
        let release = self
            .raiz
            .join(format!("pinker/releases/pink/{COMMIT_OUTRO}"));
        (dir, release)
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
}

/// Meta-prova do guard acima: a fixture volta higienizada mesmo quando o teste
/// morre com a permissão retirada.
///
/// Sem esta prova, o contrato "qualquer saída restaura" seria uma afirmação de
/// comentário. A morte é provocada de verdade, dentro da janela, e o que se mede
/// é o disco depois dela.
#[cfg(unix)]
#[test]
fn a_fixture_e_higienizada_mesmo_quando_o_teste_morre_com_a_permissao_retirada() {
    use std::os::unix::fs::PermissionsExt;

    let cenario = cenario("meta_higiene_unwind");
    let dir = cenario.dir.to_path_buf();
    let sabotado = cenario.raiz.join("pinker");
    // O diretório sabotado precisa ter CONTEÚDO. Um diretório vazio em modo 500
    // ainda é removível pelo pai, e a sabotagem não provaria nada — foi
    // exatamente assim que um mutante que desliga o reparo de permissão
    // sobreviveu a este teste. É também a forma real: no T13 e no T6 o
    // diretório sem permissão guarda a release e o comando exposto.
    fs::write(sabotado.join("conteudo"), "impede a remoção sem reparo\n")
        .expect("conteúdo do diretório sabotado");
    fs::set_permissions(&sabotado, fs::Permissions::from_mode(0o500)).expect("sabotar a fixture");
    assert_eq!(
        fs::metadata(&sabotado)
            .expect("medir o diretório sabotado")
            .permissions()
            .mode()
            & 0o777,
        0o500,
        "sem a sabotagem valendo, o teste não prova nada"
    );

    let morreu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _fixture = cenario;
        panic!("meta-prova de higiene: falha adversarial deliberada dentro da janela");
    }));
    assert!(
        morreu.is_err(),
        "o pânico precisa ter acontecido de verdade"
    );

    assert!(
        !dir.exists(),
        "qualquer saída do teste precisa devolver a fixture higienizada; sobreviveu: {dir:?}"
    );
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
    // A permissão retirada pelo stub é devolvida pelo guard da fixture, em
    // qualquer saída — inclusive se um dos asserts abaixo falhar.
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
}

#[cfg(unix)]
#[test]
fn t7_sinal_entre_o_manifest_novo_e_a_troca_do_link_restaura_os_dois() {
    // Janela específica: o manifest já mudou, o link ainda não. Aqui
    // publish_activated está vazio mas o estado vivo JÁ está alterado — se o
    // rollback olhasse só para o link, o manifest novo ficaria.
    let cenario = cenario("t7_entre_manifest_e_link");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let install = cenario.stubs.join("install");
    fs::write(&install, INSTALL_QUE_BLOQUEIA).expect("escrever install stub");
    make_executable(&install);

    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[]);
    assert!(
        relato.starts_with("install:manifest"),
        "fase alvo inesperada: {relato}"
    );
    assert_eq!(codigo, 143);
    cenario.exigir_estado_anterior(&sha, &decoy, "T7");
    fs::remove_file(&install).ok();
}

#[cfg(unix)]
#[test]
fn t9_sinal_depois_da_troca_do_link_e_antes_da_proxima_transicao_restaura_os_dois() {
    // O outro lado da janela de T7. Lá o manifest já mudara e o link ainda não;
    // aqui o link JÁ foi trocado e o publicador ainda não executou uma linha
    // sequer depois do `mv`.
    //
    // Era a janela em que uma flag tardia — atribuída DEPOIS da troca — dizia ao
    // rollback que o launcher estava intacto. O término restaurava o manifest,
    // deixava o launcher novo, e o veredito ainda afirmava RESTORED_EXACTLY.
    // OLD_MANIFEST + NEW_LAUNCHER: nem o estado anterior, nem o novo.
    let cenario = cenario("t9_pos_troca_do_link");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let mv = cenario.stubs.join("mv");
    fs::write(&mv, MV_QUE_BLOQUEIA_APOS_A_TROCA).expect("escrever mv stub");
    make_executable(&mv);

    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[]);
    fs::remove_file(&mv).ok();

    // A superfície alvo não é "perto do mv": é o mv REAL concluído, com o comando
    // exposto já apontando para a release nova, observado de dentro da janela.
    assert!(
        relato.starts_with("mv:launcher-trocado"),
        "fase alvo inesperada: {relato}"
    );
    assert!(
        relato.contains(&format!(
            "alvo={}",
            cenario.release.join("bin/pink").display()
        )),
        "a troca precisa ter acontecido de verdade antes do sinal: {relato}"
    );
    assert_eq!(codigo, 143, "TERM precisa preservar a semântica do sinal");

    // O contrato é o filesystem da fixture, não o stderr: um veredito que afirma
    // restauração é exatamente o que este defeito emitia enquanto o launcher novo
    // seguia vivo.
    cenario.exigir_estado_anterior(&sha, &decoy, "T9");
    assert!(
        relato.contains("RESTORED_EXACTLY"),
        "com o launcher de volta, o veredito precisa afirmá-lo: {relato}"
    );
}

#[cfg(unix)]
#[test]
fn t10_sinal_depois_do_link_candidato_e_antes_da_troca_nao_deixa_temporario() {
    // Terceira janela da região de troca, entre T7 e T9: o link candidato JÁ
    // existe e o `mv` ainda não o consumiu.
    //
    // Aqui o estado vivo voltava certo — manifest e launcher anteriores — mas o
    // `.pink-next-<pid>` sobrevivia, porque a limpeza só conhecia o scratch e o
    // candidato vive em `pinker/bin`, fora dele. Término limpo não é só estado
    // vivo correto; é também não deixar temporário para trás. E o `ln -s` que
    // cria o candidato não usa `-f`, então o resto abandonado faz a próxima
    // publicação que receba aquele PID abortar na preparação do link.
    let cenario = cenario("t10_candidato_pre_troca");
    cenario.publicar(&[]);
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let ln = cenario.stubs.join("ln");
    fs::write(&ln, LN_QUE_BLOQUEIA_APOS_O_CANDIDATO).expect("escrever ln stub");
    make_executable(&ln);

    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[]);
    fs::remove_file(&ln).ok();

    assert!(
        relato.starts_with("ln:candidato-criado"),
        "fase alvo inesperada: {relato}"
    );
    // A janela é ANTES da troca viva: o comando exposto ainda precisa ser o decoy.
    assert!(
        relato.contains(&format!("exposto={}", decoy.display())),
        "o sinal precisa cair antes da troca viva do launcher: {relato}"
    );
    assert_eq!(codigo, 143, "TERM precisa preservar a semântica do sinal");

    // Estado vivo anterior de volta, e nenhum temporário sobrevivente.
    cenario.exigir_estado_anterior(&sha, &decoy, "T10");

    // Explícito, para que a regressão nomeie a causa em vez de dizer só "resíduo".
    let candidatos: Vec<PathBuf> = fs::read_dir(cenario.raiz.join("pinker/bin"))
        .expect("ler pinker/bin")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with(".pink-next-"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        candidatos.is_empty(),
        "o link candidato desta transação precisa ser removido pelo término: {candidatos:?}"
    );
}

#[cfg(unix)]
#[test]
fn t11_sinal_depois_de_instalar_a_release_e_antes_do_marcador_nao_deixa_orfa() {
    // A release entra por rename de um diretório montado no scratch. O marcador
    // que dizia ao rollback "fui eu que criei" só era atribuído DEPOIS do rename,
    // então um término nessa janela restaurava manifest e launcher e deixava a
    // release órfã — o mesmo formato de defeito da flag tardia do launcher.
    let cenario = cenario("t11_release_orfa");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();
    assert!(
        !cenario.release.exists(),
        "a janela exige que a release NÃO exista antes da tentativa"
    );

    let mv = cenario.stubs.join("mv");
    fs::write(&mv, MV_QUE_BLOQUEIA_APOS_INSTALAR_A_RELEASE).expect("escrever mv stub");
    make_executable(&mv);

    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[]);
    fs::remove_file(&mv).ok();

    assert!(
        relato.starts_with("mv:release-installed"),
        "fase alvo inesperada: {relato}"
    );
    // Dentro da janela a release já estava completa e o launcher ainda era o antigo.
    assert!(
        relato.contains("binario=sim"),
        "a release precisa estar instalada de verdade antes do sinal: {relato}"
    );
    assert!(
        relato.contains(&format!("exposto={}", decoy.display())),
        "a troca viva do launcher não podia ter acontecido ainda: {relato}"
    );
    assert_eq!(codigo, 143, "TERM precisa preservar a semântica do sinal");

    cenario.exigir_estado_anterior(&sha, &decoy, "T11");
    assert!(
        !cenario.release.exists(),
        "a release instalada por esta tentativa não pode ficar órfã"
    );
}

#[cfg(unix)]
#[test]
fn t12_release_de_outra_execucao_nao_e_reivindicada_nem_destruida() {
    // Janela check-then-act: entre `[[ -e $release_root ]]` e a instalação, outra
    // execução legítima cria o destino.
    //
    // Sem `-T`, `mv` sobre diretório existente ANINHA a origem dentro do destino e
    // retorna 0 — a transação instalaria `release_root/release/bin/pink`, marcaria
    // a release como sua e, no rollback, apagaria a release da outra execução.
    // Release é artefato imutável endereçado pelo commit e compartilhável: remover
    // a de outra execução é destruir recurso alheio, não desfazer o próprio efeito.
    let cenario = cenario("t12_release_alheia");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let mv = cenario.stubs.join("mv");
    fs::write(&mv, MV_QUE_SIMULA_OUTRA_EXECUCAO).expect("escrever mv stub");
    make_executable(&mv);
    // O stub usa PINK_TESTE_ALCANCOU como marca de "já agi"; aqui não há handshake.
    let marca = cenario.dir.join("outro-ator");
    let saida = cenario.publicar(&[("PINK_TESTE_ALCANCOU", marca.to_str().unwrap())]);
    fs::remove_file(&mv).ok();

    assert!(
        marca.exists(),
        "a outra execução precisa ter agido para o caso existir"
    );
    assert!(
        !saida.status.success(),
        "instalar sobre release alheia não pode ser reportado como sucesso"
    );

    let alheio = cenario.release.join("PERTENCE-A-OUTRA-EXECUCAO");
    assert!(
        alheio.is_file(),
        "a release da outra execução jamais pode ser removida por esta transação"
    );
    assert!(
        !cenario.release.join("release").exists(),
        "sem -T o mv aninharia a origem dentro do destino alheio"
    );
    cenario.exigir_estado_anterior(&sha, &decoy, "T12");
}

#[cfg(unix)]
#[test]
fn t13_veredito_observa_a_release_e_nao_so_launcher_e_manifest() {
    // O veredito é a observação do rollback. Se ele mede launcher e manifest mas
    // não a release, uma remoção que falhou passa por restauração completa — a
    // mesma classe de falso positivo já corrigida para o launcher.
    let cenario = cenario("t13_veredito_release");
    let (_sha, _decoy) = cenario.com_estado_anterior_distinto();

    // Sabota a remoção da release: o diretório-pai fica sem permissão de escrita
    // logo depois de a release ser instalada, então o `rm -rf` do rollback não
    // consegue removê-la e o veredito precisa acusar divergência.
    let mv = cenario.stubs.join("mv");
    fs::write(
        &mv,
        MV_QUE_BLOQUEIA_APOS_INSTALAR_A_RELEASE.replace(
            "  timeout 30 cat \"$PINK_TESTE_PORTA\" > /dev/null || true",
            "  chmod 500 \"$PINK_TESTE_RELEASE/..\"\n  timeout 30 cat \"$PINK_TESTE_PORTA\" > /dev/null || true",
        ),
    )
    .expect("escrever mv stub");
    make_executable(&mv);

    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[]);
    // A permissão retirada pelo stub é devolvida pelo guard da fixture, em
    // qualquer saída — inclusive se um dos asserts abaixo falhar.
    fs::remove_file(&mv).ok();

    assert!(
        relato.starts_with("mv:release-installed"),
        "fase alvo inesperada: {relato}"
    );
    assert!(
        relato.contains("RESTORATION_DIVERGED"),
        "release não removida precisa ser acusada pelo veredito: {relato}"
    );
    assert_eq!(
        codigo, 3,
        "divergência de restauração tem terminal próprio, acima do sinal"
    );
}

#[cfg(unix)]
#[test]
fn t14_temporario_do_rollback_nao_sobrevive_quando_a_volta_falha() {
    // O link temporário do rollback vive em `pinker/bin`, fora do scratch, pela
    // mesma razão física que o candidato: a volta também é um rename atômico.
    // No caminho feliz o rename o consome, então ele passava despercebido — mas
    // se a volta falha, ele fica. Um `.pink-restore-<pid>` abandonado quebra o
    // rollback da próxima execução que receba aquele PID, porque `rm -f` não
    // remove diretório e `ln -s` não sobrescreve.
    let cenario = cenario("t14_temp_do_rollback");
    cenario.publicar(&[]);
    let (sha, _decoy) = cenario.com_estado_anterior_distinto();

    let mv = cenario.stubs.join("mv");
    fs::write(&mv, MV_QUE_FALHA_NA_VOLTA_DO_ROLLBACK).expect("escrever mv stub");
    make_executable(&mv);

    // Sinal na conferência pós-ativação: garante que o rollback do launcher é
    // realmente tentado, e é ele que falha.
    let (codigo, relato) =
        cenario.publicar_e_sinalizar("TERM", &[("PINK_TESTE_BLOQUEAR_LAUNCHER", "2")]);
    fs::remove_file(&mv).ok();

    assert!(
        relato.starts_with("launcher:2"),
        "fase alvo inesperada: {relato}"
    );
    // A volta falhou de propósito: o veredito precisa dizer isso.
    assert!(
        relato.contains("RESTORATION_DIVERGED"),
        "volta impossível precisa ser acusada: {relato}"
    );
    assert_eq!(codigo, 3, "divergência tem terminal próprio");

    // E o temporário do rollback não pode ficar para trás mesmo assim.
    let restos: Vec<PathBuf> = fs::read_dir(cenario.raiz.join("pinker/bin"))
        .expect("ler pinker/bin")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with(".pink-restore-"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        restos.is_empty(),
        "o temporário do rollback precisa ser removido pelo término: {restos:?}"
    );
    assert!(cenario.residuos().is_empty(), "{:?}", cenario.residuos());

    // E falhar na PRIMEIRA operação do rollback não pode abortar as seguintes: o
    // manifest anterior ainda tem de voltar byte a byte. Um rollback que desiste
    // no primeiro erro deixaria o host pior do que um que tenta tudo e mede.
    assert_eq!(
        sha256_de(&cenario.manifest),
        sha,
        "a falha na volta do launcher não pode impedir a restauração do manifest"
    );
}

#[cfg(unix)]
#[test]
fn t15_sinal_durante_o_finalizador_nao_preempta_a_restauracao_em_curso() {
    // O funil tem duas entradas — EXIT e sinal — e, até aqui, quem entrava por
    // uma não fechava a porta da outra.
    //
    // Medido no bash desta máquina: um TERM que chega enquanto o rollback está
    // parado numa operação externa é tratado assim que ela retorna, e o handler
    // REENTRA no funil. `FINALIZED` impede o segundo rollback, mas não impedia o
    // resto: a reentrância executava a limpeza dos temporários — que apaga o
    // scratch onde vivem os backups que a restauração original ainda não
    // terminou de usar —, desarmava os traps e matava o shell. O rollback
    // original ficava pela metade, com a release desta transação órfã e o
    // veredito nunca medido.
    //
    // O término aqui NÃO é causado por sinal: é uma falha comum, roteada pelo
    // `set -e`/abort. O sinal chega depois, no meio da volta.
    let cenario = cenario("t15_sinal_no_finalizador");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let install = cenario.stubs.join("install");
    fs::write(&install, INSTALL_QUE_BLOQUEIA_NA_VOLTA_DO_MANIFEST).expect("escrever install stub");
    make_executable(&install);

    // `PINK_TESTE_MENTIR=2`: o launcher já publicado declara outro commit na
    // conferência pós-ativação. É uma reprovação comum, sem sinal nenhum, que
    // leva ao funil pelo EXIT com o estado em ACTIVATED.
    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[("PINK_TESTE_MENTIR", "2")]);
    fs::remove_file(&install).ok();

    assert!(
        relato.starts_with("install:volta-do-manifest"),
        "fase alvo inesperada: {relato}"
    );
    // Prova de que o rollback ORIGINAL já estava em curso quando o sinal chegou:
    // o launcher já voltou ao alvo anterior e a release ainda não foi removida.
    assert!(
        relato.contains(&format!("exposto={}", decoy.display())),
        "o sinal precisa chegar com a volta do launcher já feita: {relato}"
    );
    assert!(
        relato.contains("release=presente"),
        "o sinal precisa chegar com a remoção da release ainda pendente: {relato}"
    );

    // O terminal é o do término original, não o do sinal que chegou depois: uma
    // finalização em curso já decidiu como este processo termina.
    assert_eq!(
        codigo, 1,
        "o segundo caminho de término não pode roubar o terminal do primeiro: {relato}"
    );
    // E o funil rodou uma vez só.
    assert_eq!(
        relato.matches("estado anterior").count(),
        1,
        "o funil não pode ser reentrado nem rodar duas vezes: {relato}"
    );
    assert!(
        relato.contains("estado anterior RESTORED_EXACTLY"),
        "a restauração precisa ter sido concluída E medida: {relato}"
    );

    // O que a preempção deixava para trás.
    assert!(
        !cenario.release.exists(),
        "a release desta transação não pode ficar órfã porque um sinal cortou o rollback"
    );
    cenario.exigir_estado_anterior(&sha, &decoy, "T15");
}

#[cfg(unix)]
#[test]
fn t16_sinal_no_inicio_da_restauracao_nao_deixa_a_publicacao_reprovada_viva() {
    // Mesma causa do T15, na janela mais destrutiva: o sinal chega quando o
    // rollback mal começou — o launcher ainda aponta para a release nova, o
    // manifest anterior ainda não voltou e os backups do scratch ainda são
    // necessários.
    //
    // Com o funil desprotegido, a reentrância apagava o scratch e matava o
    // shell aqui: o resultado era uma publicação REPROVADA deixada inteiramente
    // viva — manifest novo, launcher novo, release no lugar — com a saída
    // dizendo apenas "publicação abortada". É o desfecho que esta transação
    // inteira existe para tornar impossível.
    let cenario = cenario("t16_sinal_no_inicio_da_volta");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let ln = cenario.stubs.join("ln");
    fs::write(&ln, LN_QUE_BLOQUEIA_NA_VOLTA_DO_LAUNCHER).expect("escrever ln stub");
    make_executable(&ln);

    let (codigo, relato) = cenario.publicar_e_sinalizar("TERM", &[("PINK_TESTE_MENTIR", "2")]);
    fs::remove_file(&ln).ok();

    assert!(
        relato.starts_with("ln:volta-do-launcher"),
        "fase alvo inesperada: {relato}"
    );
    // Nada do estado vivo tinha voltado quando o sinal chegou.
    assert!(
        relato.contains(&format!(
            "exposto={}",
            cenario.release.join("bin/pink").display()
        )),
        "o sinal precisa chegar antes de o launcher voltar: {relato}"
    );
    assert!(
        relato.contains("release=presente"),
        "o sinal precisa chegar com a release ainda instalada: {relato}"
    );

    assert_eq!(
        codigo, 1,
        "o segundo caminho de término não pode roubar o terminal do primeiro: {relato}"
    );
    assert_eq!(
        relato.matches("estado anterior").count(),
        1,
        "o funil não pode ser reentrado nem rodar duas vezes: {relato}"
    );
    assert!(
        relato.contains("estado anterior RESTORED_EXACTLY"),
        "a restauração precisa ter sido concluída E medida: {relato}"
    );

    // O que a preempção deixava vivo.
    assert!(
        !cenario.release.exists(),
        "a release de uma publicação reprovada não pode sobreviver ao sinal"
    );
    cenario.exigir_estado_anterior(&sha, &decoy, "T16");
}

#[cfg(unix)]
#[test]
fn t17_publicacao_concorrente_nao_entra_na_transacao_alheia() {
    // O interleaving que a revisão mandou provar ou refutar:
    //
    //   estado inicial M0/L0
    //   A captura M0/L0 e ativa MA/LA
    //   B captura, ativa MB/LB e COMMITA
    //   A falha depois e executa o rollback
    //
    // Sem exclusão mútua isso foi medido: o rollback atrasado de A repunha
    // M0/L0 por cima da publicação COMMITADA por B — e declarava
    // RESTORED_EXACTLY, porque para A o "anterior" continuava sendo o que ela
    // capturou. Uma transação que desfaz o efeito de outra, já concluída, não é
    // rollback: é perda.
    //
    // A duas publicações são LEGÍTIMAS e reais: mesmo software, mesma versão,
    // commits diferentes — duas execuções de CI publicando builds diferentes.
    // São manifest e launcher os objetos disputados; a release de cada uma já
    // era protegida por posse (T12).
    let cenario = cenario("t17_dois_publicadores");
    let (sha0, decoy) = cenario.com_estado_anterior_distinto();
    let (bundle_b, release_b) = cenario.bundle_de_outro_commit();

    // A para na conferência pós-ativação e, ao voltar, declara outro commit:
    // aborta e cai no funil com o estado em ACTIVATED.
    let (codigo, relato, observado) = cenario.publicar_e_intercalar(
        &[
            ("PINK_TESTE_BLOQUEAR_LAUNCHER", "2"),
            ("PINK_TESTE_MENTIR", "2"),
        ],
        |fase| {
            assert!(
                fase.starts_with("launcher:2"),
                "fase alvo inesperada: {fase}"
            );
            // A janela só existe se A tiver mesmo ativado.
            assert_eq!(
                fs::read_link(&cenario.link).expect("ler link"),
                cenario.release.join("bin/pink"),
                "A precisa ter ativado antes de a segunda publicação tentar entrar"
            );
            let antes = (
                sha256_de(&cenario.manifest),
                fs::read_link(&cenario.link).expect("ler link"),
            );
            let saida = cenario.publicar_concorrente(&bundle_b, &release_b);
            let depois = (
                sha256_de(&cenario.manifest),
                fs::read_link(&cenario.link).expect("ler link"),
            );
            (saida, antes, depois)
        },
    );
    let (saida_b, vivo_antes, vivo_depois) = observado;

    // 1. A segunda publicação é recusada, com causa nomeada.
    assert!(
        !saida_b.status.success(),
        "duas publicações não podem estar dentro da região crítica ao mesmo tempo"
    );
    let erro_b = String::from_utf8_lossy(&saida_b.stderr).to_string();
    assert!(
        erro_b.contains("outra publicação está em andamento"),
        "a recusa precisa nomear a causa: {erro_b}"
    );

    // 2. E é recusada ANTES de observar ou mutar o estado vivo: o lock é
    //    adquirido antes da captura, não depois.
    assert_eq!(
        vivo_antes, vivo_depois,
        "a publicação recusada não pode ter tocado o estado vivo"
    );
    // Medir só o estado vivo não bastaria: um lock adquirido tarde deixa a
    // segunda publicação CAPTURAR e até ATIVAR antes de ser barrada, e o
    // rollback dela devolve tudo — o estado vivo final fica igual e o defeito
    // passa. A prova de que a recusa precedeu a captura é que não houve
    // transação nenhuma para desfazer: quem nada capturou nada restaura.
    assert!(
        !erro_b.contains("estado anterior"),
        "a publicação recusada chegou a capturar e restaurar estado — o lock foi \
         adquirido tarde demais: {erro_b}"
    );
    assert!(
        !release_b.exists(),
        "a publicação recusada não pode ter instalado release nenhuma"
    );

    // 3. O término de A é o dela, e o rollback de A é sobre o que A capturou.
    assert_eq!(codigo, 1, "A precisa terminar pela própria falha: {relato}");
    assert_eq!(
        relato.matches("estado anterior").count(),
        1,
        "o funil rodou mais de uma vez: {relato}"
    );
    assert!(
        relato.contains("estado anterior RESTORED_EXACTLY"),
        "a restauração precisa ter sido concluída e medida: {relato}"
    );

    // 4. Estado vivo final: exatamente M0/L0, e a release de A desfeita.
    cenario.exigir_estado_anterior(&sha0, &decoy, "T17");
    assert!(
        !cenario.release.exists(),
        "a release da publicação que falhou não pode ficar órfã"
    );
}

#[cfg(unix)]
#[test]
fn t18_o_lock_de_publicacao_nao_sobrevive_a_quem_o_tomou() {
    // Um lock que não é liberado é pior que lock nenhum: transforma uma queda em
    // negação permanente de publicação. Como o lock é do núcleo e está preso à
    // descrição de arquivo aberta, ele cai com o processo por qualquer via. É o
    // que este teste mede — inclusive em SIGKILL, que é o caso que nenhum lock
    // por arquivo-marcador ou por PID sobrevive.
    fn publicou(saida: &Output) -> bool {
        saida.status.success()
            && String::from_utf8_lossy(&saida.stdout).contains("status=PUBLISHED")
    }

    // Saída bem-sucedida.
    {
        let cenario = cenario("t18_sucesso");
        assert!(publicou(&cenario.publicar(&[])), "primeira publicação");
        assert!(
            publicou(&cenario.publicar(&[])),
            "o lock precisa cair no caminho de sucesso"
        );
    }

    // Falha comum com rollback.
    {
        let cenario = cenario("t18_falha");
        assert!(publicou(&cenario.publicar(&[])), "primeira publicação");
        let (sha, decoy) = cenario.com_estado_anterior_distinto();
        let falha = cenario.publicar(&[("PINK_TESTE_MENTIR", "2")]);
        assert!(
            !falha.status.success(),
            "a segunda tentativa precisa falhar"
        );
        cenario.exigir_estado_anterior(&sha, &decoy, "T18/falha");
        assert!(
            publicou(&cenario.publicar(&[])),
            "o lock precisa cair no caminho de falha"
        );
    }

    // Término por sinal no meio da transação, e morte sem chance de limpar.
    for (sinal, terminal) in [("TERM", 143), ("KILL", 137)] {
        let cenario = cenario(&format!("t18_{}", sinal.to_lowercase()));
        let (codigo, relato) =
            cenario.publicar_e_sinalizar(sinal, &[("PINK_TESTE_BLOQUEAR_LAUNCHER", "2")]);
        assert_eq!(
            codigo, terminal,
            "{sinal} precisa ter mesmo matado o publisher no meio: {relato}"
        );
        assert!(
            publicou(&cenario.publicar(&[])),
            "{sinal} não pode deixar lock permanente: uma publicação seguinte precisa entrar"
        );
    }

    // E o arquivo de lock não é resíduo: ele é deliberadamente permanente.
    // Removê-lo abriria a corrida de dois processos travando inodes diferentes
    // com o mesmo nome.
    let cenario = cenario("t18_lock_permanece");
    assert!(publicou(&cenario.publicar(&[])), "publicação");
    assert!(
        cenario.raiz.join("pinker/.publicacao.lock").is_file(),
        "o lock precisa continuar existindo depois da publicação"
    );
    assert!(
        cenario.residuos().is_empty(),
        "o lock não pode ser confundido com temporário da transação: {:?}",
        cenario.residuos()
    );
}

/// `install` que planta um resíduo de uma execução ANTERIOR que morreu sem poder
/// limpar — com o PID DESTA execução, que é o caso de colisão por reuso de PID.
///
/// `$PPID` dentro do stub é o PID do próprio publicador, então o resíduo nasce
/// exatamente com o nome que esta transação vai calcular. É o que torna a
/// colisão determinística sem depender de o núcleo reciclar um PID.
#[cfg(unix)]
const INSTALL_QUE_PLANTA_RESIDUO_DE_MORTO: &str = r#"#!/bin/sh
/usr/bin/install "$@" || exit $?
if [ ! -f "$PINK_TESTE_ALCANCOU" ]; then
  echo "residuo-plantado pid=$PPID" > "$PINK_TESTE_ALCANCOU"
  case "${PINK_TESTE_RESIDUO:-}" in
    staging)
      # O diretório de releases é passado explicitamente: derivá-lo com `..` a
      # partir de PINK_TESTE_RELEASE faria o `mkdir -p` criar a própria release,
      # e o experimento passaria a medir o efeito do stub.
      mkdir -p "$PINK_TESTE_RELEASES/.publish-$PINK_TESTE_COMMIT-$PPID"
      printf 'de uma execucao morta\n' \
        > "$PINK_TESTE_RELEASES/.publish-$PINK_TESTE_COMMIT-$PPID/marca"
      ;;
    next-link)
      mkdir -p "$PINK_TESTE_BIN"
      ln -s /caminho/de/outra/execucao "$PINK_TESTE_BIN/.pink-next-$PPID"
      ;;
  esac
fi
exit 0
"#;

#[cfg(unix)]
#[test]
fn t19_residuo_de_uma_execucao_morta_com_o_mesmo_pid_nao_e_adotado() {
    // Um processo morto por SIGKILL — ou por queda de energia — não tem como
    // limpar os próprios temporários. Eles ficam com o PID de quem morreu, e o
    // núcleo recicla PIDs. A publicação seguinte que receber aquele PID vai
    // calcular exatamente os mesmos nomes.
    //
    // O contrato aqui não é "limpar o que for de outro": é PARAR, com causa
    // nomeada, sem adotar como seu o staging alheio e sem deixar o estado vivo
    // pior do que estava. Este é o único caminho por onde um resíduo antigo
    // encontra uma instância legítima.

    // Caso 1 — staging remanescente: a transação nem começa.
    {
        let cenario = cenario("t19_staging_remanescente");
        let (sha, decoy) = cenario.com_estado_anterior_distinto();
        let install = cenario.stubs.join("install");
        fs::write(&install, INSTALL_QUE_PLANTA_RESIDUO_DE_MORTO).expect("escrever install stub");
        make_executable(&install);

        let marca = cenario.dir.join("residuo-staging");
        let saida = cenario.publicar(&[
            ("PINK_TESTE_RESIDUO", "staging"),
            ("PINK_TESTE_COMMIT", COMMIT_TX),
            (
                "PINK_TESTE_RELEASES",
                cenario.raiz.join("pinker/releases/pink").to_str().unwrap(),
            ),
            ("PINK_TESTE_ALCANCOU", marca.to_str().unwrap()),
        ]);
        fs::remove_file(&install).ok();

        assert!(marca.exists(), "o resíduo precisa ter sido plantado");
        assert!(
            !saida.status.success(),
            "staging remanescente não pode ser adotado como se fosse desta transação"
        );
        let erro = String::from_utf8_lossy(&saida.stderr).to_string();
        assert!(
            erro.contains("staging já existe"),
            "a recusa precisa nomear a causa: {erro}"
        );

        // O resíduo alheio continua onde estava: parar não é limpar o que não é seu.
        let pid = fs::read_to_string(&marca)
            .expect("ler marca")
            .split("pid=")
            .nth(1)
            .expect("pid na marca")
            .trim()
            .to_string();
        let residuo = cenario
            .raiz
            .join(format!("pinker/releases/pink/.publish-{COMMIT_TX}-{pid}"));
        assert!(
            residuo.join("marca").is_file(),
            "o staging da execução morta não pode ser removido nem sobrescrito"
        );

        // E o estado vivo não foi tocado: a recusa acontece antes da captura.
        assert_eq!(
            sha256_de(&cenario.manifest),
            sha,
            "a recusa não pode ter mexido no manifest"
        );
        assert_eq!(
            fs::read_link(&cenario.link).expect("ler link"),
            decoy,
            "a recusa não pode ter mexido no comando exposto"
        );
        assert!(
            !cenario.release.exists(),
            "a recusa não pode ter instalado release"
        );
    }

    // Caso 2 — link candidato remanescente: a transação já está ativada quando
    // esbarra nele, então o que se exige é o rollback exato.
    {
        let cenario = cenario("t19_next_link_remanescente");
        let (sha, decoy) = cenario.com_estado_anterior_distinto();
        let install = cenario.stubs.join("install");
        fs::write(&install, INSTALL_QUE_PLANTA_RESIDUO_DE_MORTO).expect("escrever install stub");
        make_executable(&install);

        let marca = cenario.dir.join("residuo-next-link");
        let saida = cenario.publicar(&[
            ("PINK_TESTE_RESIDUO", "next-link"),
            (
                "PINK_TESTE_BIN",
                cenario.raiz.join("pinker/bin").to_str().unwrap(),
            ),
            ("PINK_TESTE_ALCANCOU", marca.to_str().unwrap()),
        ]);
        fs::remove_file(&install).ok();

        assert!(marca.exists(), "o resíduo precisa ter sido plantado");
        assert!(
            !saida.status.success(),
            "`ln -s` não sobrescreve: a preparação do link precisa falhar"
        );
        let erro = String::from_utf8_lossy(&saida.stderr).to_string();
        assert!(
            erro.contains("link candidato"),
            "a recusa precisa nomear a causa: {erro}"
        );
        assert!(
            erro.contains("estado anterior RESTORED_EXACTLY"),
            "a falha depois da ativação precisa restaurar e medir: {erro}"
        );
        cenario.exigir_estado_anterior(&sha, &decoy, "T19/next-link");
        assert!(
            !cenario.release.exists(),
            "a release desta tentativa não pode ficar órfã"
        );
    }
}

#[cfg(unix)]
#[test]
fn t8_abort_restaura_o_conjunto_de_manifests_e_nao_so_o_arquivo() {
    // Restaurar o manifest certo mas deixar a coleção com outro conjunto seria
    // trocar um defeito por outro: quem valida é a coleção inteira.
    let cenario = cenario("t8_conjunto");
    cenario.publicar(&[]);
    let alheio = cenario.raiz.join("pinker/manifests/outro-1.0.0.json");
    fs::write(&alheio, "{\n  \"name\": \"Outro Software\"\n}\n").expect("manifest alheio");
    let (sha, decoy) = cenario.com_estado_anterior_distinto();

    let antes = conjunto_de_manifests(&cenario.raiz);
    let saida = cenario.publicar(&[("PINK_TESTE_REPROVAR", "2")]);
    assert!(!saida.status.success());
    assert_eq!(
        conjunto_de_manifests(&cenario.raiz),
        antes,
        "o conjunto de manifests precisa voltar exatamente, não só o arquivo próprio"
    );
    assert!(alheio.is_file(), "manifest alheio jamais é tocado");
    cenario.exigir_estado_anterior(&sha, &decoy, "T8");
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
}

// @pinker-nav:end evidencia.tooling.f2.registros-derivados
