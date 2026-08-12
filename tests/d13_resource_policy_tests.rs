mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.arquivos.d13-politica-de-handle
// @pinker-nav:domain arquivos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência D13 da política de recurso da família arquivo: identidade de handle, release explícito, uso após release, double release, stale alias que não ressuscita depois de uma nova abertura e veredicto distinto para handle nunca aberto. Cada caso compara interpretador e ELF nativo sob envelope, exigindo mesmo stdout ordenado até a falha e mesmo exit; o controle positivo garante que a matriz não passa por falhar em tudo.

/// Controle positivo: sem o release, o handle permanece utilizável e o
/// programa termina normalmente. Sem este caso, toda a matriz abaixo poderia
/// passar por um erro que nada tem a ver com a política de recurso.
const FONTE_CICLO_VALIDO: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "d13_ciclo.txt");
    nova h: bombom = criar_arquivo(alvo);
    escrever_verso(h, "d13");
    falar(ler_verso_arquivo(h));
    fechar(h);
    falar("fechou");
    mimo 0;
}
"#;

/// Uso após release: o handle deixa de designar o recurso.
const FONTE_USO_APOS_RELEASE: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "d13_uso.txt");
    nova h: bombom = criar_arquivo(alvo);
    escrever_verso(h, "d13");
    fechar(h);
    falar("fechou");
    falar(ler_verso_arquivo(h));
    falar("nao alcancavel");
    mimo 0;
}
"#;

/// Double release falha em vez de ser idempotente.
const FONTE_DOUBLE_RELEASE: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "d13_double.txt");
    nova h: bombom = criar_arquivo(alvo);
    escrever_verso(h, "d13");
    fechar(h);
    falar("fechou");
    fechar(h);
    falar("nao alcancavel");
    mimo 0;
}
"#;

/// Stale alias não ressuscita: abrir outro arquivo depois do release não faz
/// o handle morto voltar a designar um recurso vivo.
const FONTE_STALE_NAO_RESSUSCITA: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova primeiro: verso = argumento_ou(0, "d13_um.txt");
    nova segundo: verso = argumento_ou(1, "d13_dois.txt");

    nova h1: bombom = criar_arquivo(primeiro);
    escrever_verso(h1, "um");
    fechar(h1);
    falar("fechou h1");

    nova h2: bombom = criar_arquivo(segundo);
    escrever_verso(h2, "dois");
    falar("abriu h2");

    falar(ler_verso_arquivo(h1));
    falar("nao alcancavel");
    mimo 0;
}
"#;

/// Handle nunca aberto é um veredicto diferente de handle já liberado.
const FONTE_HANDLE_NUNCA_ABERTO: &str = r#"
pacote main;

carinho principal() -> bombom {
    falar("antes");
    fechar(4242);
    falar("nao alcancavel");
    mimo 0;
}
"#;

fn escrever_caso(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("gravar fonte D13 temporária");
    caminho
}

fn rodar_interpretador(caminho: &Path, caso: &str, args: &[String]) -> Output {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
    comando.arg("--run").arg(caminho).arg("--");
    for arg in args {
        comando.arg(arg);
    }
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador D13 sob envelope")
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
        .timeout(Duration::from_secs(60))
        .output()
        .expect("compilar D13 sob envelope")
}

fn rodar_nativo(caminho: &Path, caso: &str, args: &[String]) -> Output {
    let mut comando = Command::new(caminho);
    for arg in args {
        comando.arg(arg);
    }
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF D13 sob envelope")
}

struct ParidadeRecurso {
    stdout_interpretador: String,
    stdout_nativo: String,
    stderr_interpretador: String,
    stderr_nativo: String,
    exit_interpretador: Option<i32>,
    exit_nativo: Option<i32>,
}

/// Executa o mesmo caso nos dois modos e devolve o par observável.
///
/// Os arquivos de trabalho ficam dentro do diretório do caso, e o
/// interpretador e o nativo recebem caminhos distintos: a política de recurso
/// é observada por handle, não pelo conteúdo compartilhado de um arquivo.
fn paridade_do_caso(
    nome: &str,
    fonte: &str,
    arquivos: &[&str],
    runtime_lib: &Path,
) -> ParidadeRecurso {
    let dir = NativeArtifactDir::create().expect("diretório nativo D13");
    let fonte_path = escrever_caso(&dir, nome, fonte);

    let args_interpretador: Vec<String> = arquivos
        .iter()
        .map(|arquivo| {
            dir.path()
                .join(format!("interp_{arquivo}"))
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let args_nativo: Vec<String> = arquivos
        .iter()
        .map(|arquivo| {
            dir.path()
                .join(format!("nativo_{arquivo}"))
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    let interpretado = rodar_interpretador(
        &fonte_path,
        &format!("d13-{nome}-interpretador"),
        &args_interpretador,
    );

    let build = compilar_nativo(&dir, &fonte_path, runtime_lib, &format!("d13-{nome}-build"));
    assert!(
        build.status.success(),
        "build nativo D13 falhou em {nome}: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nativo = rodar_nativo(
        &dir.path().join(nome),
        &format!("d13-{nome}-nativo"),
        &args_nativo,
    );

    ParidadeRecurso {
        stdout_interpretador: String::from_utf8_lossy(&interpretado.stdout).into_owned(),
        stdout_nativo: String::from_utf8_lossy(&nativo.stdout).into_owned(),
        stderr_interpretador: String::from_utf8_lossy(&interpretado.stderr).into_owned(),
        stderr_nativo: String::from_utf8_lossy(&nativo.stderr).into_owned(),
        exit_interpretador: interpretado.status.code(),
        exit_nativo: nativo.status.code(),
    }
}

/// Toda violação de política de recurso precisa falhar nos dois modos, com o
/// mesmo stdout ordenado até o ponto da falha e o mesmo exit.
fn exigir_falha_equivalente(nome: &str, paridade: &ParidadeRecurso, stdout_esperado: &str) {
    assert_eq!(
        paridade.stdout_interpretador, stdout_esperado,
        "{nome}: stdout do interpretador divergiu do esperado"
    );
    assert_eq!(
        paridade.stdout_nativo, stdout_esperado,
        "{nome}: stdout do nativo divergiu do esperado"
    );
    assert_eq!(
        paridade.exit_interpretador,
        Some(1),
        "{nome}: interpretador deveria falhar com exit 1 (stderr: {})",
        paridade.stderr_interpretador
    );
    assert_eq!(
        paridade.exit_nativo,
        Some(1),
        "{nome}: nativo deveria falhar com exit 1 (stderr: {})",
        paridade.stderr_nativo
    );
    assert!(
        !paridade.stderr_interpretador.contains("panicked"),
        "{nome}: interpretador entrou em pânico: {}",
        paridade.stderr_interpretador
    );
    assert!(
        !paridade.stderr_nativo.contains("panicked"),
        "{nome}: nativo entrou em pânico: {}",
        paridade.stderr_nativo
    );
}

#[test]
fn politica_de_handle_de_arquivo_tem_paridade_entre_interpretador_e_nativo() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    // Controle positivo: o ciclo completo continua válido nos dois modos.
    let ciclo = paridade_do_caso(
        "d13_ciclo_valido",
        FONTE_CICLO_VALIDO,
        &["ciclo.txt"],
        &runtime_lib,
    );
    assert_eq!(
        ciclo.stdout_interpretador, "d13\nfechou\n",
        "ciclo válido: stdout do interpretador"
    );
    assert_eq!(
        ciclo.stdout_nativo, "d13\nfechou\n",
        "ciclo válido: stdout do nativo"
    );
    assert_eq!(ciclo.exit_interpretador, Some(0));
    assert_eq!(ciclo.exit_nativo, Some(0));

    // Release invalida o acesso: o handle deixa de designar o recurso.
    let uso = paridade_do_caso(
        "d13_uso_apos_release",
        FONTE_USO_APOS_RELEASE,
        &["uso.txt"],
        &runtime_lib,
    );
    exigir_falha_equivalente("uso após release", &uso, "fechou\n");
    assert!(
        uso.stderr_interpretador.contains("já fechado"),
        "uso após release: interpretador não classificou handle liberado: {}",
        uso.stderr_interpretador
    );
    assert!(
        uso.stderr_nativo.contains("já fechado"),
        "uso após release: nativo não classificou handle liberado: {}",
        uso.stderr_nativo
    );

    // Double release falha: a liberação não é idempotente.
    let double = paridade_do_caso(
        "d13_double_release",
        FONTE_DOUBLE_RELEASE,
        &["double.txt"],
        &runtime_lib,
    );
    exigir_falha_equivalente("double release", &double, "fechou\n");
    assert!(
        double.stderr_interpretador.contains("já fechado"),
        "double release: interpretador não classificou o segundo release: {}",
        double.stderr_interpretador
    );
    assert!(
        double.stderr_nativo.contains("já fechado"),
        "double release: nativo não classificou o segundo release: {}",
        double.stderr_nativo
    );

    // Stale alias não ressuscita depois de uma abertura posterior.
    let stale = paridade_do_caso(
        "d13_stale_nao_ressuscita",
        FONTE_STALE_NAO_RESSUSCITA,
        &["um.txt", "dois.txt"],
        &runtime_lib,
    );
    exigir_falha_equivalente("stale alias", &stale, "fechou h1\nabriu h2\n");
    assert!(
        stale.stderr_interpretador.contains("já fechado"),
        "stale alias: interpretador deixou o handle morto ressuscitar: {}",
        stale.stderr_interpretador
    );
    assert!(
        stale.stderr_nativo.contains("já fechado"),
        "stale alias: nativo deixou o handle morto ressuscitar: {}",
        stale.stderr_nativo
    );

    // Handle nunca aberto é um veredicto distinto de handle já liberado.
    let nunca_aberto = paridade_do_caso(
        "d13_handle_nunca_aberto",
        FONTE_HANDLE_NUNCA_ABERTO,
        &[],
        &runtime_lib,
    );
    exigir_falha_equivalente("handle nunca aberto", &nunca_aberto, "antes\n");
    assert!(
        nunca_aberto.stderr_interpretador.contains("inválido")
            && !nunca_aberto.stderr_interpretador.contains("já fechado"),
        "handle nunca aberto: interpretador confundiu inválido com já fechado: {}",
        nunca_aberto.stderr_interpretador
    );
    assert!(
        nunca_aberto.stderr_nativo.contains("inválido")
            && !nunca_aberto.stderr_nativo.contains("já fechado"),
        "handle nunca aberto: nativo confundiu inválido com já fechado: {}",
        nunca_aberto.stderr_nativo
    );
}
// @pinker-nav:end evidencia.arquivos.d13-politica-de-handle
