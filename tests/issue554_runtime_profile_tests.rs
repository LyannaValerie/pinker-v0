//! Gate da política de perfil do runtime nativo (#554).
//!
//! A prova não lê o texto do `Cargo.toml`: ela pergunta ao próprio Cargo qual
//! perfil efetivo cada unidade da staticlib recebeu, e depois confere no
//! artefato produzido que otimização, DWARF, `debug_assertions` e
//! `overflow-checks` realmente sobreviveram até `libpinker_rt.a`.

use std::path::{Path, PathBuf};
use std::process::Command;

use pinker_json_contract::{interpretar, NoJson, TabelaJson};

/// Pacotes que compõem `libpinker_rt.a`: o runtime e os contratos locais que
/// ele consome. A política precisa alcançar todos — otimizar só `pinker_rt`
/// produz um artefato diferente do experimento V2 autorizado na #548.
const PACOTES_DO_RUNTIME: [&str; 5] = [
    "pinker_rt",
    "pinker_argv_contract",
    "pinker_memory_contract",
    "pinker_json_contract",
    "pinker_sha256_contract",
];

const OPT_LEVEL_ESPERADO: &str = "3";
const DEBUGINFO_ESPERADO: i64 = 2;

/// Mensagem do `debug_assert!` de `fim_da_regiao` em `runtime/pinker_rt/src/lib.rs`.
/// Ela só é codegenerada quando o runtime é compilado com debug assertions.
const MARCA_DEBUG_ASSERTIONS: &str =
    "invariante da arena pública: base + tamanho precisa ser representável";

/// Mensagem que o rustc emite para a checagem de overflow de soma. Ela some do
/// artefato quando `overflow-checks` é desligado.
const MARCA_OVERFLOW_CHECKS: &str = "attempt to add with overflow";

/// Nome de seção DWARF presente enquanto `debug` não for reduzido.
const MARCA_DWARF: &str = ".debug_info";

fn campo(tabela: &TabelaJson, handle: u64, chave: &str) -> Option<u64> {
    match tabela.obter(handle)? {
        NoJson::Objeto(entradas) => entradas.get(chave).copied(),
        _ => None,
    }
}

fn verso(tabela: &TabelaJson, handle: u64) -> Option<String> {
    match tabela.obter(handle)? {
        NoJson::Verso(texto) => Some(texto.clone()),
        _ => None,
    }
}

fn numero(tabela: &TabelaJson, handle: u64) -> Option<i64> {
    match tabela.obter(handle)? {
        NoJson::Numero(valor) => Some(*valor),
        _ => None,
    }
}

fn logica(tabela: &TabelaJson, handle: u64) -> Option<bool> {
    match tabela.obter(handle)? {
        NoJson::Logica(valor) => Some(*valor),
        _ => None,
    }
}

fn lista(tabela: &TabelaJson, handle: u64) -> Option<Vec<u64>> {
    match tabela.obter(handle)? {
        NoJson::Lista(itens) => Some(itens.clone()),
        _ => None,
    }
}

#[derive(Debug)]
struct UnidadeCompilada {
    pacote: String,
    opt_level: String,
    debuginfo: Option<i64>,
    debug_assertions: bool,
    overflow_checks: bool,
    arquivos: Vec<PathBuf>,
}

/// Pergunta ao Cargo o perfil efetivo das unidades que produzem a staticlib.
///
/// `--message-format=json` reporta o perfil resolvido também para unidades já
/// atualizadas, então o custo é uma consulta, não uma recompilação.
fn unidades_compiladas(argumentos: &[&str]) -> Vec<UnidadeCompilada> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut argumentos_cargo = vec!["build", "--locked"];
    argumentos_cargo.extend_from_slice(argumentos);
    argumentos_cargo.push("--message-format=json");
    let saida = Command::new(env!("CARGO"))
        .current_dir(raiz)
        .args(argumentos_cargo)
        .output()
        .expect("cargo build precisa ser invocável a partir do teste");
    assert!(
        saida.status.success(),
        "cargo build falhou: {}",
        String::from_utf8_lossy(&saida.stderr)
    );

    let texto = String::from_utf8(saida.stdout).expect("saída JSON do Cargo é UTF-8");
    let mut unidades = Vec::new();
    for linha in texto.lines() {
        if !linha.contains("compiler-artifact") {
            continue;
        }
        let mut tabela = TabelaJson::nova();
        let raiz_json = match interpretar(linha, &mut tabela) {
            Ok(handle) => handle,
            Err(erro) => panic!("mensagem do Cargo não é JSON interpretável: {erro}"),
        };
        let razao = campo(&tabela, raiz_json, "reason").and_then(|h| verso(&tabela, h));
        if razao.as_deref() != Some("compiler-artifact") {
            continue;
        }
        let alvo = campo(&tabela, raiz_json, "target").expect("artefato declara target");
        let pacote = campo(&tabela, alvo, "name")
            .and_then(|h| verso(&tabela, h))
            .expect("target declara name");
        let perfil = campo(&tabela, raiz_json, "profile").expect("artefato declara profile");
        let arquivos = campo(&tabela, raiz_json, "filenames")
            .and_then(|h| lista(&tabela, h))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|h| verso(&tabela, h).map(PathBuf::from))
            .collect();
        unidades.push(UnidadeCompilada {
            pacote,
            opt_level: campo(&tabela, perfil, "opt_level")
                .and_then(|h| verso(&tabela, h))
                .expect("profile declara opt_level"),
            debuginfo: campo(&tabela, perfil, "debuginfo").and_then(|h| numero(&tabela, h)),
            debug_assertions: campo(&tabela, perfil, "debug_assertions")
                .and_then(|h| logica(&tabela, h))
                .expect("profile declara debug_assertions"),
            overflow_checks: campo(&tabela, perfil, "overflow_checks")
                .and_then(|h| logica(&tabela, h))
                .expect("profile declara overflow_checks"),
            arquivos,
        });
    }
    assert!(
        !unidades.is_empty(),
        "nenhum artefato de compilação foi reportado por cargo build"
    );
    unidades
}

fn unidades_da_staticlib() -> Vec<UnidadeCompilada> {
    unidades_compiladas(&["-p", "pinker_rt"])
}

fn unidades_do_workspace() -> Vec<UnidadeCompilada> {
    unidades_compiladas(&[])
}

fn staticlib() -> PathBuf {
    unidades_da_staticlib()
        .into_iter()
        .find(|unidade| unidade.pacote == "pinker_rt")
        .expect("pinker_rt é compilado por cargo build -p pinker_rt")
        .arquivos
        .into_iter()
        .find(|arquivo| arquivo.file_name().and_then(|n| n.to_str()) == Some("libpinker_rt.a"))
        .expect("pinker_rt produz a staticlib libpinker_rt.a")
}

/// O perfil efetivo de cada unidade da staticlib é o V2 autorizado na #548:
/// otimizado, mas com a semântica de desenvolvimento intacta.
#[test]
fn perfil_efetivo_do_runtime_e_otimizado_com_semantica_dev() {
    let unidades = unidades_da_staticlib();
    for esperado in PACOTES_DO_RUNTIME {
        let unidade = unidades
            .iter()
            .find(|unidade| unidade.pacote == esperado)
            .unwrap_or_else(|| {
                panic!(
                    "{esperado} não aparece entre as unidades da staticlib: {:?}",
                    unidades.iter().map(|u| &u.pacote).collect::<Vec<_>>()
                )
            });
        assert_eq!(
            unidade.opt_level, OPT_LEVEL_ESPERADO,
            "{esperado} precisa ser compilado otimizado"
        );
        assert_eq!(
            unidade.debuginfo,
            Some(DEBUGINFO_ESPERADO),
            "{esperado} precisa preservar DWARF completo; V3/redução de debug não foi autorizada"
        );
        assert!(
            unidade.debug_assertions,
            "{esperado} não pode perder debug assertions em troca de velocidade"
        );
        assert!(
            unidade.overflow_checks,
            "{esperado} não pode perder overflow checks em troca de velocidade"
        );
    }
}

/// A otimização é estritamente do grafo da staticlib: o compilador e seu binário
/// continuam no `dev` normal, mesmo que alguém tente otimizar o workspace todo.
#[test]
fn compilador_permanece_nao_otimizado() {
    let unidades = unidades_do_workspace();
    for esperado in ["pinker_v0", "pink"] {
        let unidade = unidades
            .iter()
            .find(|unidade| unidade.pacote == esperado)
            .unwrap_or_else(|| {
                panic!(
                    "{esperado} não aparece entre as unidades do workspace: {:?}",
                    unidades.iter().map(|u| &u.pacote).collect::<Vec<_>>()
                )
            });
        assert_eq!(
            unidade.opt_level, "0",
            "{esperado} não pode receber a otimização reservada ao runtime"
        );
    }
}

/// O caminho do artefato não muda: a ponte que os testes nativos e
/// `pink build --nativo` conhecem continua sendo `debug/libpinker_rt.a`.
#[test]
fn staticlib_permanece_no_caminho_conhecido_do_perfil_dev() {
    let caminho = staticlib();
    let diretorio = caminho
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .expect("staticlib fica dentro de um diretório de perfil");
    assert_eq!(
        diretorio,
        "debug",
        "a política não pode mover a staticlib para outro perfil: {}",
        caminho.display()
    );
}

/// A configuração efetiva chega ao artefato: a staticlib publicada carrega
/// DWARF, o `debug_assert!` do runtime e a checagem de overflow do rustc.
#[test]
fn staticlib_preserva_dwarf_e_as_checagens_de_desenvolvimento() {
    let caminho = staticlib();
    let bytes =
        std::fs::read(&caminho).unwrap_or_else(|erro| panic!("ler {}: {erro}", caminho.display()));

    for (marca, explicacao) in [
        (
            MARCA_DWARF,
            "DWARF precisa continuar no artefato (debug = 2)",
        ),
        (
            MARCA_DEBUG_ASSERTIONS,
            "o debug_assert! do runtime precisa continuar codegenerado",
        ),
        (
            MARCA_OVERFLOW_CHECKS,
            "a checagem de overflow do rustc precisa continuar codegenerada",
        ),
    ] {
        assert!(
            contem(&bytes, marca.as_bytes()),
            "{explicacao}; marca ausente em {}: {marca:?}",
            caminho.display()
        );
    }
}

fn contem(agulheiro: &[u8], agulha: &[u8]) -> bool {
    agulheiro
        .windows(agulha.len())
        .any(|janela| janela == agulha)
}
