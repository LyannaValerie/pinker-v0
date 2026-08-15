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
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.tooling.f2.registros-derivados
// @pinker-nav:domain tooling
// @pinker-nav:layer evidence
// @pinker-nav:summary Contratos positivos e negativos dos registros derivados pelo publicador do baseline: manifest bem-formado e em forma canônica, cobertura de checksums, identidade de verificação, ficha de catálogo com launcher declarado e idempotente, e recusa de bundle inválido.

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
// O compilador é zero-dependência por contrato, então o teste carrega o próprio
// verificador em vez de importar um parser. Ele é estrito de propósito: é
// exatamente o token nu deixado por uma aspa mal escapada que precisa falhar.
// ---------------------------------------------------------------------------

struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
    top_level_keys: Vec<String>,
    depth: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Scanner<'a> {
        Scanner {
            bytes: text.as_bytes(),
            pos: 0,
            top_level_keys: Vec::new(),
            depth: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.skip_ws();
        if self.pos < self.bytes.len() && self.bytes[self.pos] == byte {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "esperado {:?} na posição {}",
                byte as char, self.pos
            ))
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            if self.pos >= self.bytes.len() {
                return Err("string não terminada".to_string());
            }
            match self.bytes[self.pos] {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    if self.pos >= self.bytes.len() {
                        return Err("escape truncado".to_string());
                    }
                    let escaped = self.bytes[self.pos];
                    if !matches!(
                        escaped,
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' | b'u'
                    ) {
                        return Err(format!("escape inválido: \\{}", escaped as char));
                    }
                    out.push(escaped as char);
                    self.pos += 1;
                }
                other => {
                    out.push(other as char);
                    self.pos += 1;
                }
            }
        }
    }

    fn value(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.pos >= self.bytes.len() {
            return Err("valor ausente".to_string());
        }
        match self.bytes[self.pos] {
            b'"' => {
                self.string()?;
                Ok(())
            }
            b'{' => self.object(),
            b'[' => self.array(),
            b't' => self.literal("true"),
            b'f' => self.literal("false"),
            b'n' => self.literal("null"),
            b'-' | b'0'..=b'9' => self.number(),
            other => Err(format!(
                "token inesperado {:?} na posição {}",
                other as char, self.pos
            )),
        }
    }

    fn literal(&mut self, word: &str) -> Result<(), String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(())
        } else {
            Err(format!("literal inválido na posição {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<(), String> {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        {
            self.pos += 1;
        }
        if self.pos == start {
            Err("número vazio".to_string())
        } else {
            Ok(())
        }
    }

    fn array(&mut self) -> Result<(), String> {
        self.expect(b'[')?;
        self.skip_ws();
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b']' {
            self.pos += 1;
            return Ok(());
        }
        loop {
            self.value()?;
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => return Err(format!("array malformado na posição {}", self.pos)),
            }
        }
    }

    fn object(&mut self) -> Result<(), String> {
        self.expect(b'{')?;
        self.depth += 1;
        self.skip_ws();
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'}' {
            self.pos += 1;
            self.depth -= 1;
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            if self.depth == 1 {
                self.top_level_keys.push(key);
            }
            self.expect(b':')?;
            self.value()?;
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(());
                }
                _ => return Err(format!("objeto malformado na posição {}", self.pos)),
            }
        }
    }
}

/// Devolve as chaves de topo, ou o erro sintático encontrado.
fn scan_json(text: &str) -> Result<Vec<String>, String> {
    let mut scanner = Scanner::new(text);
    scanner.value()?;
    scanner.skip_ws();
    if scanner.pos != scanner.bytes.len() {
        return Err(format!("conteúdo extra na posição {}", scanner.pos));
    }
    Ok(scanner.top_level_keys)
}

/// Todos os valores string associados à chave, em qualquer profundidade.
fn string_values_for(text: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\":");
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(&needle) {
        let trimmed = rest[at + needle.len()..].trim_start();
        if trimmed.starts_with('"') {
            // Ler pelo scanner, e não até a próxima aspa: o valor pode conter
            // aspas escapadas — é justamente o caso de `expected_contains`.
            let mut scanner = Scanner::new(trimmed);
            if let Ok(value) = scanner.string() {
                found.push(value);
            }
        }
        rest = &rest[at + needle.len()..];
    }
    found
}

// ---------------------------------------------------------------------------
// Positivos — manifest
// ---------------------------------------------------------------------------

#[test]
fn manifest_renderizado_e_json_bem_formado() {
    let dir = temp("manifest_bem_formado");
    let release_root = dir.join("release");
    let rendered = render_manifest(&dir, &release_root);
    let keys = scan_json(&rendered).unwrap_or_else(|error| {
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
    let keys = scan_json(&rendered).expect("manifest válido");
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
    assert!(rendered.ends_with("}\n"), "manifest deve terminar em uma \\n");
    assert!(!rendered.contains('\t'), "indentação canônica não usa tab");
    assert!(
        rendered.contains("\n  \"schema\": "),
        "indentação de topo deve ser de dois espaços"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn checksums_cobrem_todos_os_executaveis_e_comandos_declarados() {
    let dir = temp("manifest_checksums");
    let release_root = dir.join("release");
    let rendered = render_manifest(&dir, &release_root);

    let declarados: BTreeSet<String> = string_values_for(&rendered, "path")
        .into_iter()
        .chain(string_values_for(&rendered, "installation_root"))
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
        string_values_for(&rendered, "sha256"),
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
    let contains = string_values_for(&rendered, "expected_contains");
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
    assert!(!output.status.success(), "data não derivável deveria falhar");
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
// @pinker-nav:end evidencia.tooling.f2.registros-derivados
