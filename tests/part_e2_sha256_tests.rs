mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.integridade.parte-e2-sha256
// @pinker-nav:domain integridade
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da Parte E2: SHA-256 geral sobre `verso` e sobre arquivo atravessa interpretador e ELF nativo com digest idêntico byte a byte. A matriz fixa os vetores oficiais de FIPS 180-4 (vazio, `abc`, multibloco), prova que o domínio é BYTE e não codepoint por UTF-8 multibyte e por duas sequências Unicode distintas, prova que newline não é normalizado, e cobre no arquivo os casos que a leitura textual histórica não alcança — UTF-8 inválido, NUL, CRLF preservado e arquivo grande de múltiplos blocos —, além de vazio, ausente, diretório, permissão e symlink seguido. As falhas recuperáveis atravessam `Resultado<verso,verso>` como valor, e a forma canônica do digest (64 caracteres hexadecimais minúsculos, sem prefixo) é asserida em vez de presumida.

/// Digest de `verso`: superfície pura, sem `Resultado`.
///
/// Os dois últimos são os vetores oficiais de 448 e 896 bits de FIPS 180-4:
/// 56 bytes, que já atravessa dois blocos por causa do padding, e 112 bytes,
/// que atravessa múltiplos blocos por conteúdo.
const FONTE_VERSO: &str = r#"
pacote main; trazer integridade.sha256_verso;

carinho principal() -> bombom {
    falar(sha256_verso(""));
    falar(sha256_verso("abc"));
    falar(sha256_verso("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"));
    falar(sha256_verso("abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"));
    mimo 0;
}
"#;

/// Domínio é BYTE, não codepoint: multibyte, newline e duas sequências Unicode
/// cujos bytes UTF-8 diferem.
///
/// As duas últimas linhas são `é` **pré-composto** (U+00E9, dois bytes) e `é`
/// **decomposto** (U+0065 U+0301, três bytes). Renderizam igual e têm o mesmo
/// significado para um leitor humano; os bytes diferem. Escritos por escape
/// explícito porque um literal digitado não deixaria claro qual é qual — e
/// porque é justamente a diferença de bytes que o teste precisa preservar.
fn fonte_bytes() -> String {
    let precomposto = "\u{00e9}";
    let decomposto = "e\u{0301}";
    format!(
        r#"
pacote main; trazer integridade.sha256_verso;

carinho principal() -> bombom {{
    falar(sha256_verso("olá mundo"));
    falar(sha256_verso("linha\n"));
    falar(sha256_verso("{precomposto}"));
    falar(sha256_verso("{decomposto}"));
    mimo 0;
}}
"#
    )
}

/// Digest de arquivo: superfície falível da Parte B.
const FONTE_ARQUIVO: &str = r#"
pacote main; trazer ambiente.argumento_ou; trazer integridade.sha256_arquivo;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    tentar sha256_arquivo(alvo) {
        sucesso ResVV.Ok(digest) { falar(digest); }
        falha ResVV.Erro(causa) { falar("ERRO"); falar(causa); }
    }
    mimo 0;
}
"#;

/// Workflow real de integridade: comparar o digest observado de um artefato
/// contra o digest esperado, exatamente como um manifesto faz.
const FONTE_VERIFICACAO: &str = r#"
pacote main; trazer ambiente.argumento_ou; trazer integridade.sha256_arquivo; trazer texto.igual;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    nova esperado: verso = argumento_ou(1, "ausente");
    tentar sha256_arquivo(alvo) {
        sucesso ResVV.Ok(digest) {
            talvez igual(digest, esperado) {
                falar("INTEGRO");
                mimo 0;
            }
            falar("CORROMPIDO");
            mimo 1;
        }
        falha ResVV.Erro(causa) {
            falar("ILEGIVEL");
            mimo 2;
        }
    }
    mimo 3;
}
"#;

// Digests esperados. TODOS são independentes desta implementação: os quatro
// primeiros são vetores publicados de FIPS 180-4 e os demais foram derivados
// por um oráculo externo (hashlib) durante o desenvolvimento e congelados aqui.
//
// ```text
// EXTERNAL_ORACLE_MAY_VALIDATE BUT MUST_NOT_IMPLEMENT_THE_FEATURE
// ```
//
// Congelar o valor em vez de recalculá-lo em tempo de teste é o que mantém a
// suíte independente de ferramenta externa no CI — e é também o que impede o
// erro clássico de comparar a implementação com ela mesma, que passaria mesmo
// se ela estivesse inteiramente errada.
const VAZIO: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const MULTIBLOCO: &str = "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1";
const MULTIBLOCO_LONGO: &str = "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1";

/// Conteúdo do arquivo grande: determinístico, 300000 bytes, muitos blocos e
/// mais de uma leitura do buffer de 64 KiB.
const GRANDE_BYTES: usize = 300_000;
const GRANDE: &str = "3c65ea93424a9c362fec0e3a69ea36031e8a358441479dd665cc6110eabe7b08";
const BINARIO: &str = "3b176240af7e44f416dc193f9c5c09326139230e230ea2b9ae194d6240a4da7d";
const CRLF: &str = "18745f36a05e29072709042d6062ce54f1b08ff36c27ba80c39f81fb010c8ce2";
const LF: &str = "7e18f737311b2dc3b2f269dd78396b0351f14fb66efa879f768cb23181883c78";

fn grande_conteudo() -> Vec<u8> {
    (0..GRANDE_BYTES).map(|i| (i % 251) as u8).collect()
}

fn escrever_caso(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte Parte E2");
    caminho
}

fn rodar_interpretador(caminho: &Path, caso: &str, args: &[String]) -> Output {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
    comando.arg("--run").arg(caminho);
    if !args.is_empty() {
        comando.arg("--");
        for arg in args {
            comando.arg(arg);
        }
    }
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar interpretador Parte E2 sob envelope")
}

fn compilar_nativo(
    dir: &NativeArtifactDir,
    caminho: &Path,
    runtime_lib: &Path,
    caso: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(caminho)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case(caso)
        .timeout(Duration::from_secs(120))
        .output()
        .expect("compilar Parte E2 sob envelope")
}

fn rodar_nativo(caminho: &Path, caso: &str, args: &[String]) -> Output {
    let mut comando = Command::new(caminho);
    for arg in args {
        comando.arg(arg);
    }
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar ELF Parte E2 sob envelope")
}

struct Paridade {
    stdout_interpretador: String,
    stdout_nativo: String,
    stderr_nativo: String,
    exit_interpretador: Option<i32>,
    exit_nativo: Option<i32>,
}

impl Paridade {
    /// Exige o mesmo stdout nos dois backends.
    ///
    /// A paridade é verificada **antes** do valor esperado: dois backends que
    /// concordassem num digest errado seriam pegos pela asserção de conteúdo, e
    /// dois que divergissem seriam pegos aqui mesmo que um deles acertasse.
    fn exigir(&self, nome: &str, stdout_esperado: &str) {
        assert_eq!(
            self.stdout_interpretador, self.stdout_nativo,
            "{nome}: interpretador e nativo divergiram"
        );
        assert_eq!(
            self.stdout_interpretador, stdout_esperado,
            "{nome}: stdout inesperado"
        );
        assert_eq!(self.exit_interpretador, self.exit_nativo, "{nome}: exit");
        assert!(
            !self.stderr_nativo.contains("panicked"),
            "{nome}: nativo entrou em pânico: {}",
            self.stderr_nativo
        );
    }

    fn stdout_comum(&self, nome: &str) -> &str {
        assert_eq!(
            self.stdout_interpretador, self.stdout_nativo,
            "{nome}: interpretador e nativo divergiram"
        );
        &self.stdout_interpretador
    }
}

fn paridade(nome: &str, fonte: &str, args: &[String], runtime_lib: &Path) -> Paridade {
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E2");
    let fonte_path = escrever_caso(&dir, nome, fonte);
    let interpretado = rodar_interpretador(&fonte_path, nome, args);
    let compilacao = compilar_nativo(&dir, &fonte_path, runtime_lib, nome);
    assert!(
        compilacao.status.success(),
        "{nome}: build nativo falhou: {}",
        String::from_utf8_lossy(&compilacao.stderr)
    );
    let binario = dir.path().join(nome);
    let nativo = rodar_nativo(&binario, nome, args);
    Paridade {
        stdout_interpretador: String::from_utf8_lossy(&interpretado.stdout).into_owned(),
        stdout_nativo: String::from_utf8_lossy(&nativo.stdout).into_owned(),
        stderr_nativo: String::from_utf8_lossy(&nativo.stderr).into_owned(),
        exit_interpretador: interpretado.status.code(),
        exit_nativo: nativo.status.code(),
    }
}

fn arg(caminho: &Path) -> Vec<String> {
    vec![caminho.to_string_lossy().into_owned()]
}

/// Toda linha de digest tem de estar na forma canônica pública.
fn exigir_forma_canonica(nome: &str, digest: &str) {
    assert_eq!(
        digest.len(),
        pinker_v0::sha256::DIGEST_CARACTERES,
        "{nome}: comprimento do digest"
    );
    assert!(
        digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{nome}: alfabeto fora de 0-9a-f: {digest}"
    );
    assert!(!digest.starts_with("0x"), "{nome}: digest com prefixo");
}

#[test]
fn sha256_verso_bate_vetores_oficiais_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let esperado = format!("{VAZIO}\n{ABC}\n{MULTIBLOCO}\n{MULTIBLOCO_LONGO}\n");
    let vetores = paridade("verso_vetores", FONTE_VERSO, &[], &runtime_lib);
    vetores.exigir("verso_vetores", &esperado);

    for linha in vetores.stdout_comum("verso_vetores").lines() {
        exigir_forma_canonica("verso_vetores", linha);
    }
}

#[test]
fn sha256_verso_opera_sobre_bytes_utf8_e_nao_codepoints() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let bytes = paridade("verso_bytes", &fonte_bytes(), &[], &runtime_lib);
    let linhas: Vec<&str> = bytes.stdout_comum("verso_bytes").lines().collect();
    assert_eq!(linhas.len(), 4, "quatro digests esperados");

    // Multibyte e newline conferidos contra vetor fixo: se o hash percorresse
    // codepoints em vez de bytes, "olá mundo" mudaria e "abc" continuaria certo.
    assert_eq!(
        linhas[0], "093ca12d5b187564caece279d90c60f99c136780127fd0a231995299bbd36934",
        "UTF-8 multibyte: digest deve cobrir os bytes reais"
    );
    // Newline preservado: nenhum trim, nenhuma normalização de fim de linha.
    assert_eq!(
        linhas[1], "f9817253a08ff16e4b2744b1597a39851e2ba285a60c749887e41fa181f32ed2",
        "newline não pode ser normalizado nem aparado"
    );

    // Pré-composto (U+00E9) e decomposto (U+0065 U+0301) renderizam igual mas
    // têm bytes UTF-8 diferentes: normalização Unicode implícita os colapsaria.
    //
    // Ambos conferidos contra valor conhecido, não só entre si: dois digests
    // errados também seriam diferentes um do outro.
    assert_eq!(
        linhas[2], "4a99557e4033c3539de2eb65472017cad5f9557f7a0625a09f1c3f6e2ba69c4c",
        "é pré-composto (U+00E9)"
    );
    assert_eq!(
        linhas[3], "bf12767b0f2a56b2190075bae8169f656e3ce8d6357d4aff184bc6c7ea48f9f6",
        "é decomposto (U+0065 U+0301)"
    );
    assert_ne!(
        linhas[2], linhas[3],
        "sequências Unicode com bytes diferentes não podem colapsar no mesmo digest"
    );

    for linha in linhas {
        exigir_forma_canonica("verso_bytes", linha);
    }
}

#[test]
fn sha256_verso_nul_embutido_no_interpretador() {
    // NUL é representável em `verso` (o lexer aceita `\0` e o layout é
    // length-prefixed, não NUL-terminated).
    //
    // Este caso fica no interpretador de propósito. Um literal com NUL não
    // atravessa o backend nativo porque `escape_gas_string` não escapa
    // caracteres de controle para o GAS — limitação PRÉ-EXISTENTE do backend,
    // reproduzível com `falar("a\0b")` sem SHA-256 nenhum envolvido, e portanto
    // fora do escopo desta Task.
    //
    // ```text
    // BACKEND_REJECTS_SOURCE_SHAPE != SEMANTIC_OPERATION_INVALID
    // ```
    //
    // A cobertura de bytes NUL nos DOIS backends é feita pelo caminho de
    // arquivo, onde os bytes são lidos em runtime e nunca passam por `.rodata`.
    let dir = NativeArtifactDir::create().expect("diretório Parte E2 NUL");
    let fonte = r#"
pacote main; trazer integridade.sha256_verso;

carinho principal() -> bombom {
    falar(sha256_verso("a\0b"));
    falar(sha256_verso("ab"));
    mimo 0;
}
"#;
    let caminho = escrever_caso(&dir, "verso_nul", fonte);
    let saida = rodar_interpretador(&caminho, "verso_nul", &[]);
    let stdout = String::from_utf8_lossy(&saida.stdout);
    let linhas: Vec<&str> = stdout.lines().collect();
    assert_eq!(linhas.len(), 2, "dois digests esperados");
    // Valor conhecido de "a\0b", derivado por oráculo externo: o NUL entra no
    // digest em vez de truncar a entrada.
    assert_eq!(
        linhas[0], "59b271ae1bbcb1d31d41929817f4b16fb439eb4f31520b5ad1d5ce98920a7138",
        "NUL embutido tem de participar do digest"
    );
    assert_ne!(
        linhas[0], linhas[1],
        "\"a\\0b\" não pode colapsar em \"ab\""
    );
    exigir_forma_canonica("verso_nul", linhas[0]);
}

#[test]
fn sha256_arquivo_cobre_bytes_exatos_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório de fixture Parte E2");
    let base = dir.path();

    // Arquivo vazio: mesmo digest do verso vazio, por definição.
    let vazio = base.join("vazio.bin");
    fs::write(&vazio, b"").expect("escrever vazio");

    // Arquivo textual: vetor oficial conhecido.
    let textual = base.join("abc.txt");
    fs::write(&textual, b"abc").expect("escrever textual");

    // UTF-8 INVÁLIDO + NUL: é exatamente o conteúdo que `read_to_string`
    // rejeitaria. Se o hash passasse por leitura textual, este caso falharia.
    let binario = base.join("binario.bin");
    fs::write(&binario, [0xffu8, 0xfe, 0x00, 0x01, b'b', b'i', b'n']).expect("escrever binário");

    // CRLF preservado: normalizar fim de linha mudaria o digest.
    let crlf = base.join("crlf.txt");
    fs::write(&crlf, b"a\r\nb").expect("escrever crlf");
    let lf = base.join("lf.txt");
    fs::write(&lf, b"a\nb").expect("escrever lf");

    // Grande o bastante para múltiplas leituras e muitos blocos SHA-256.
    let grande = base.join("grande.bin");
    fs::write(&grande, grande_conteudo()).expect("escrever grande");

    let digest_de = |nome: &str, caminho: &Path| -> String {
        let execucao = paridade(nome, FONTE_ARQUIVO, &arg(caminho), &runtime_lib);
        let saida = execucao.stdout_comum(nome).trim().to_string();
        assert!(
            !saida.starts_with("ERRO"),
            "{nome}: esperado sucesso, veio {saida}"
        );
        exigir_forma_canonica(nome, &saida);
        saida
    };

    assert_eq!(digest_de("arq_vazio", &vazio), VAZIO, "arquivo vazio");
    assert_eq!(digest_de("arq_textual", &textual), ABC, "arquivo textual");

    // Binário com UTF-8 inválido e NUL: valor conhecido, derivado por oráculo
    // externo. `read_to_string` teria falhado antes de chegar ao digest, então
    // este caso é o que separa hash de arquivo de leitura textual.
    assert_eq!(
        digest_de("arq_binario", &binario),
        BINARIO,
        "arquivo binário/UTF-8 inválido"
    );

    // CRLF e LF: valores conhecidos e distintos. Normalizar fim de linha
    // transformaria o primeiro no segundo.
    assert_eq!(digest_de("arq_crlf", &crlf), CRLF, "CRLF preservado");
    assert_eq!(digest_de("arq_lf", &lf), LF, "LF preservado");
    assert_ne!(CRLF, LF, "CRLF e LF são conteúdos diferentes");

    // Arquivo grande: valor conhecido. Bater com ele prova de uma vez que o
    // streaming interno atravessa múltiplas leituras e múltiplos blocos sem
    // alterar o resultado — sem comparar a implementação com ela mesma.
    assert_eq!(
        digest_de("arq_grande", &grande),
        GRANDE,
        "arquivo grande de múltiplos blocos"
    );
}

#[test]
fn sha256_arquivo_falha_recuperavel_atravessa_resultado() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório de erro Parte E2");
    let base = dir.path();

    // Ausente.
    let ausente = base.join("nao-existe.bin");

    // Diretório: nunca pode ser tratado como arquivo por acidente.
    let diretorio = base.join("um-diretorio");
    fs::create_dir(&diretorio).expect("criar diretório");

    for (nome, alvo) in [("erro_ausente", &ausente), ("erro_diretorio", &diretorio)] {
        let execucao = paridade(nome, FONTE_ARQUIVO, &arg(alvo), &runtime_lib);
        // `stdout_comum` já exige paridade da saída INTEIRA, então a mensagem de
        // causa também é comparada entre os dois backends — divergir no texto do
        // erro é divergir na superfície pública.
        let saida = execucao.stdout_comum(nome);
        let mut linhas = saida.lines();
        assert_eq!(
            linhas.next(),
            Some("ERRO"),
            "{nome}: falha recuperável tem de atravessar Resultado como valor"
        );
        let causa = linhas.next().unwrap_or_default();
        assert!(
            causa.contains("falha ao hashear arquivo"),
            "{nome}: causa deve nomear a operação: {causa}"
        );
    }

    // Permissão negada, quando economicamente testável (não sob root, que
    // ignora o bit de leitura).
    if !executando_como_root() {
        let sem_permissao = base.join("sem-permissao.bin");
        fs::write(&sem_permissao, b"segredo").expect("escrever protegido");
        fs::set_permissions(&sem_permissao, fs::Permissions::from_mode(0o000))
            .expect("remover permissões");
        let execucao = paridade(
            "erro_permissao",
            FONTE_ARQUIVO,
            &arg(&sem_permissao),
            &runtime_lib,
        );
        assert!(
            execucao.stdout_comum("erro_permissao").starts_with("ERRO"),
            "permissão negada tem de atravessar Resultado como valor"
        );
    }
}

#[test]
fn sha256_arquivo_segue_symlink_como_open_read() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório symlink Parte E2");
    let base = dir.path();

    let alvo = base.join("alvo.txt");
    fs::write(&alvo, b"abc").expect("escrever alvo");
    let link = base.join("link.txt");
    std::os::unix::fs::symlink(&alvo, &link).expect("criar symlink");

    // Contrato executivo: hash de arquivo é open/read e SEGUE symlink, ao
    // contrário de `tipo_de_entrada`/`tamanho_de_entrada`, que usam
    // `symlink_metadata` e NÃO seguem. O digest é o do ALVO.
    let execucao = paridade("symlink", FONTE_ARQUIVO, &arg(&link), &runtime_lib);
    assert_eq!(
        execucao.stdout_comum("symlink").trim(),
        ABC,
        "symlink para arquivo regular tem de render o digest do alvo"
    );

    // Symlink quebrado é falha recuperável, não sucesso.
    let quebrado = base.join("quebrado.txt");
    std::os::unix::fs::symlink(base.join("nada-aqui.txt"), &quebrado).expect("symlink quebrado");
    let execucao = paridade(
        "symlink_quebrado",
        FONTE_ARQUIVO,
        &arg(&quebrado),
        &runtime_lib,
    );
    assert!(
        execucao
            .stdout_comum("symlink_quebrado")
            .starts_with("ERRO"),
        "symlink quebrado tem de falhar como valor"
    );
}

#[test]
fn workflow_real_de_verificacao_de_integridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    // Aceitação read-only sobre CONTEÚDO REAL E VERSIONADO deste repositório,
    // não sobre um arquivo inventado para o teste:
    //
    // - `LICENSE`, estável desde a Fase 34;
    // - `.pinker/changes/pr-378.yaml`, registro histórico de mudança que a
    //   política forward-only trata como imutável.
    //
    // Os digests esperados vêm de oráculo externo, como o esperado de um
    // manifesto de verdade vem de quem publicou o artefato — nunca recalculado
    // pela implementação sob teste.
    //
    // O formato é o mesmo `sha256=<hex>` que `scripts/pink-baseline` já usa
    // para conferir um bundle publicado. É o recorte de VERIFICAÇÃO desse
    // workflow, não a ferramenta inteira: nada de build, publish, root ou
    // instalação atômica.
    //
    // ```text
    // REAL_INTEGRITY_WORKFLOW != FULL_TOOL_PORT
    // ```
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let reais: [(&str, &str); 2] = [
        (
            "LICENSE",
            "5d4e64d9e1af36bb5a50289d272e5fa5c645c9f3e6be52d54e37f1826981c8fa",
        ),
        (
            ".pinker/changes/pr-378.yaml",
            "cedec2ce8bfc9e76952b3a01a108162a1ecec6fba12108d0b642368d3f37992e",
        ),
    ];

    for (relativo, esperado) in reais {
        let alvo = raiz.join(relativo);
        assert!(alvo.is_file(), "conteúdo real ausente: {relativo}");

        // REAL_CONTENT -> SHA256 -> COMPARE_WITH_EXPECTED -> MATERIAL_DECISION
        //
        // Se este caso falhar, a leitura correta é que um arquivo versionado
        // mudou — que é exatamente o que uma verificação de integridade existe
        // para dizer, e não um defeito do SHA-256.
        let caso = format!("integro_{}", relativo.replace(['/', '.'], "_"));
        let integro = paridade(
            &caso,
            FONTE_VERIFICACAO,
            &[alvo.to_string_lossy().into_owned(), esperado.to_string()],
            &runtime_lib,
        );
        integro.exigir(&caso, "INTEGRO\n");
        assert_eq!(
            integro.exit_interpretador,
            Some(0),
            "{relativo}: exit íntegro"
        );
    }

    // Adulteração de um único byte do conteúdo REAL tem de ser detectada.
    // A cópia é adulterada num diretório temporário: a aceitação permanece
    // read-only sobre o repositório.
    let dir = NativeArtifactDir::create().expect("diretório workflow Parte E2");
    let original = fs::read(raiz.join("LICENSE")).expect("ler LICENSE real");
    let mut adulterado = original.clone();
    let meio = adulterado.len() / 2;
    adulterado[meio] ^= 0x01;
    assert_ne!(adulterado, original, "a adulteração precisa mudar os bytes");
    let copia = dir.path().join("LICENSE.adulterado");
    fs::write(&copia, &adulterado).expect("escrever cópia adulterada");

    let detectado = paridade(
        "verificacao_adulterada",
        FONTE_VERIFICACAO,
        &[copia.to_string_lossy().into_owned(), reais[0].1.to_string()],
        &runtime_lib,
    );
    detectado.exigir("verificacao_adulterada", "CORROMPIDO\n");
    assert_eq!(
        detectado.exit_interpretador,
        Some(1),
        "um bit trocado no conteúdo real tem de virar decisão material"
    );

    // Artefato ausente é decisão distinta de artefato corrompido: um manifesto
    // que confundisse as duas mandaria investigar a coisa errada.
    let ausente = dir.path().join("nao-publicado.bin");
    let ilegivel = paridade(
        "verificacao_ilegivel",
        FONTE_VERIFICACAO,
        &[
            ausente.to_string_lossy().into_owned(),
            reais[0].1.to_string(),
        ],
        &runtime_lib,
    );
    ilegivel.exigir("verificacao_ilegivel", "ILEGIVEL\n");
    assert_eq!(ilegivel.exit_interpretador, Some(2));
}

#[test]
fn superficie_publica_declarada_numa_autoridade_so() {
    // Os nomes e o símbolo vivem em `sha256`; nenhuma camada pode manter cópia.
    assert!(pinker_v0::sha256::e_acessor("sha256_verso"));
    // O nome falível NÃO é acessor: ele pertence à autoridade da Parte B.
    assert!(!pinker_v0::sha256::e_acessor(
        pinker_v0::falha_operacional::SHA256_ARQUIVO
    ));
    assert_eq!(
        pinker_v0::sha256::simbolo_runtime("sha256_verso"),
        Some("pinker_sha256_verso")
    );

    // A superfície de arquivo é falível e pertence à autoridade da Parte B.
    let arquivo = pinker_v0::falha_operacional::SUPERFICIES_FALIVEIS
        .iter()
        .find(|s| s.intrinseca == pinker_v0::falha_operacional::SHA256_ARQUIVO)
        .expect("sha256_arquivo tem de estar na lista fechada da Parte B");
    assert_eq!(
        arquivo.simbolo_runtime, "pinker_sha256_arquivo_resultado",
        "símbolo nativo da superfície falível"
    );
    assert!(
        arquivo.historica.is_none(),
        "SHA-256 não tem gêmeo histórico: registrar um inventaria compatibilidade"
    );
}

#[test]
fn sem_dependencia_semantica_em_processo_externo() {
    // A campanha proíbe `sha256sum`, `openssl`, shell ou processo externo como
    // dependência SEMÂNTICA da linguagem. Provado por duas metades baratas.

    // Estrutural: o núcleo compartilhado — que é por onde TODO digest passa nos
    // dois backends — não pode sequer mencionar spawn de processo, e é um crate
    // puro sem dependência nenhuma. As duas superfícies públicas não são varridas
    // aqui de propósito: `interpreter.rs` e o runtime spawnam processos por
    // outras features, então procurar `Command` neles acusaria qualquer arquivo
    // e não provaria nada sobre SHA-256.
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let nucleo = fs::read_to_string(raiz.join("runtime/pinker_sha256_contract/src/lib.rs"))
        .expect("ler núcleo compartilhado");
    for proibido in ["std::process", "Command", "sha256sum", "openssl", "popen"] {
        assert!(
            !nucleo.contains(proibido),
            "núcleo do SHA-256 não pode conter '{proibido}'"
        );
    }
    let manifesto = fs::read_to_string(raiz.join("runtime/pinker_sha256_contract/Cargo.toml"))
        .expect("ler manifesto do núcleo");
    let tem_dependencia = manifesto
        .lines()
        .skip_while(|linha| !linha.trim().starts_with("[dependencies]"))
        .skip(1)
        .any(|linha| {
            let linha = linha.trim();
            !linha.is_empty() && !linha.starts_with('#') && !linha.starts_with('[')
        });
    assert!(
        !tem_dependencia,
        "o núcleo do SHA-256 tem de permanecer sem dependência externa"
    );

    // Comportamental: com `PATH` vazio não existe `sha256sum` nem `openssl`
    // alcançável, e a feature continua produzindo o vetor oficial. Se a
    // implementação delegasse a um processo externo, isto falharia — e não foi
    // preciso remover nada do host para descobrir.
    let dir = NativeArtifactDir::create().expect("diretório sem-PATH Parte E2");
    let caminho = escrever_caso(&dir, "sem_path", FONTE_VERSO);
    let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&caminho)
        .env("PATH", "")
        .logical_case("sem_path")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar sem PATH sob envelope");
    let stdout = String::from_utf8_lossy(&saida.stdout);
    assert!(
        stdout.starts_with(VAZIO),
        "SHA-256 tem de funcionar sem nenhum executável alcançável no PATH: {stdout}"
    );
    assert!(
        stdout.contains(ABC),
        "vetor 'abc' ausente com PATH vazio: {stdout}"
    );
}

#[test]
fn uso_invalido_do_programa_e_erro_de_compilacao_nao_resultado() {
    // INVALID_PROGRAM_USE != RECOVERABLE_IO_FAILURE: aridade e tipo errados
    // param no compilador e nunca viram `Resultado`.
    let aridade = r#"
pacote main; trazer integridade.sha256_verso;

carinho principal() -> bombom {
    falar(sha256_verso("a", "b"));
    mimo 0;
}
"#;
    let erro = common::parse_and_check(aridade).expect_err("aridade inválida tem de falhar");
    assert!(
        format!("{erro:?}").contains("aridade"),
        "erro deve nomear a aridade: {erro:?}"
    );

    let tipo = r#"
pacote main; trazer integridade.sha256_verso;

carinho principal() -> bombom {
    falar(sha256_verso(42));
    mimo 0;
}
"#;
    let erro = common::parse_and_check(tipo).expect_err("tipo inválido tem de falhar");
    let texto = format!("{erro:?}");
    // Não basta conter "verso": o próprio nome `sha256_verso` contém essa
    // substring, então a asserção passaria para qualquer erro que citasse a
    // chamada. O que precisa aparecer é o diagnóstico de tipo.
    assert!(
        texto.contains("tipo inválido no argumento 1"),
        "erro deve ser de tipo no argumento 1: {texto}"
    );
    assert!(
        texto.contains("esperado 'verso'"),
        "erro deve nomear o tipo esperado: {texto}"
    );
}

fn executando_como_root() -> bool {
    // Sob root o bit de permissão é ignorado, então o caso de permissão negada
    // deixa de ser testável e é pulado em vez de virar falso negativo.
    unsafe { libc_geteuid() == 0 }
}

extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

// @pinker-nav:end evidencia.integridade.parte-e2-sha256
