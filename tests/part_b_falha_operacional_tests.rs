mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.erros.parte-b-falha-operacional
// @pinker-nav:domain erros
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da Parte B: falha operacional recuperável atravessa `Resultado<T,E>` como valor em três domínios independentes (filesystem, processo/spawn e parsing em tempo de execução), compondo com `tentar`, `propagar?` e `encaixe` sem caminho especial. Cada caso compara interpretador e ELF nativo sob envelope exigindo mesmo stdout e mesmo exit; controles positivos impedem que a matriz passe por falhar em tudo; a compatibilidade histórica, a fronteira fatal e a ausência de vazamento de recurso na falha têm casos próprios.

/// Ponto 1/2/3: construção histórica de `Resultado`, `tentar` e `propagar`
/// continuam válidos com leque declarado pelo usuário — sem nenhuma
/// participação das superfícies novas.
const FONTE_HISTORICO_RESULTADO: &str = r#"
pacote main;

leque Resultado { Ok(bombom), Erro(verso) }

carinho validar(a: bombom, ok: logica) -> Resultado {
    talvez ok {
        mimo Resultado.Ok(a);
    }
    mimo Resultado.Erro("falha validada");
}

carinho somar(a: bombom, b: bombom) -> Resultado {
    propagar validar(a, verdade) como Resultado.Ok(va) senao Resultado.Erro(e1);
    propagar? validar(b, verdade) como Resultado.Ok(vb);
    mimo Resultado.Ok(va + vb);
}

carinho principal() -> bombom {
    tentar somar(20, 22) {
        sucesso Resultado.Ok(v) { falar(v); }
        falha Resultado.Erro(m) { falar(m); }
    }
    tentar validar(1, falso) {
        sucesso Resultado.Ok(v) { falar(v); }
        falha Resultado.Erro(m) { falar(m); }
    }
    mimo 0;
}
"#;

/// Ponto 4/16: sucesso das três superfícies migradas.
/// argv: 0 = arquivo existente, 1 = executável verdadeiro.
const FONTE_SUCESSO: &str = r#"
pacote main;

apelido ResVV = Resultado<verso, verso>;
apelido ResBV = Resultado<bombom, verso>;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    nova exe: verso = argumento_ou(1, "ausente");

    tentar ler_arquivo_resultado(alvo) {
        sucesso ResVV.Ok(conteudo) { falar(conteudo); }
        falha ResVV.Erro(causa) { falar(causa); }
    }

    tentar executar_processo_resultado(exe) {
        sucesso ResBV.Ok(codigo) { falar(codigo); }
        falha ResBV.Erro(causa) { falar(causa); }
    }

    tentar verso_para_bombom_resultado("  4242  ") {
        sucesso ResBV.Ok(n) { falar(n); }
        falha ResBV.Erro(causa) { falar(causa); }
    }

    falar("fim");
    mimo 0;
}
"#;

/// Ponto 5/6/7/10/17: falha recuperável nos três domínios vira valor, o payload
/// chega intacto e o programa continua até o fim com exit 0.
/// argv: 0 = caminho ausente, 1 = executável ausente (com '/').
const FONTE_FALHA_TRES_DOMINIOS: &str = r#"
pacote main;

apelido ResVV = Resultado<verso, verso>;
apelido ResBV = Resultado<bombom, verso>;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    nova exe: verso = argumento_ou(1, "ausente");

    tentar ler_arquivo_resultado(alvo) {
        sucesso ResVV.Ok(conteudo) { falar(conteudo); }
        falha ResVV.Erro(causa) { falar(causa); }
    }

    tentar executar_processo_resultado(exe) {
        sucesso ResBV.Ok(codigo) { falar(codigo); }
        falha ResBV.Erro(causa) { falar(causa); }
    }

    tentar verso_para_bombom_resultado("nao-numero") {
        sucesso ResBV.Ok(n) { falar(n); }
        falha ResBV.Erro(causa) { falar(causa); }
    }

    falar("fim");
    mimo 0;
}
"#;

/// Ponto 8/9/19: a falha atravessa uma função intermediária por `propagar?` e é
/// tratada por `encaixe` no consumidor. Nenhuma das duas superfícies aparece na
/// função que propaga: a propagação não sabe quem produziu o valor.
const FONTE_PROPAGACAO_INTERMEDIARIA: &str = r#"
pacote main;

apelido ResVV = Resultado<verso, verso>;

carinho ler_e_marcar(caminho: verso) -> ResVV {
    propagar? ler_arquivo_resultado(caminho) como ResVV.Ok(conteudo);
    mimo ResVV.Ok(juntar_verso("lido:", conteudo));
}

carinho intermediaria(caminho: verso) -> ResVV {
    propagar? ler_e_marcar(caminho) como ResVV.Ok(marcado);
    mimo ResVV.Ok(juntar_verso("via:", marcado));
}

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    encaixe intermediaria(alvo) {
        caso ResVV.Ok(texto) { falar(texto); }
        caso ResVV.Erro(causa) { falar(causa); }
    }
    falar("fim");
    mimo 0;
}
"#;

/// Ponto 12: uma falha não deixa recurso parcialmente vivo. Muitas leituras
/// falhas seguidas e o ciclo de handle continua funcionando — se cada falha
/// vazasse um descritor, o ciclo final falharia.
/// argv: 0 = caminho ausente, 1 = arquivo de trabalho.
const FONTE_FALHA_NAO_VAZA_RECURSO: &str = r#"
pacote main;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    nova ausente: verso = argumento_ou(0, "ausente");
    nova trabalho: verso = argumento_ou(1, "trabalho.txt");

    nova muda i: bombom = 0;
    nova muda falhas: bombom = 0;
    sempre que i < 300 {
        tentar ler_arquivo_resultado(ausente) {
            sucesso ResVV.Ok(c) { falar("inesperado"); }
            falha ResVV.Erro(e) { falhas = falhas + 1; }
        }
        i = i + 1;
    }
    falar(falhas);

    nova h: bombom = criar_arquivo(trabalho);
    escrever_verso(h, "vivo");
    falar(ler_verso_arquivo(h));
    fechar(h);
    falar("fechou");
    mimo 0;
}
"#;

/// Ponto 15: a superfície histórica continua com o contrato antigo — inclusive
/// o aborto. Compatibilidade não é só assinatura preservada.
const FONTE_HISTORICO_AINDA_ABORTA: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    falar("antes");
    nova conteudo: verso = ler_arquivo_verso(alvo);
    falar(conteudo);
    falar("nao alcancavel");
    mimo 0;
}
"#;

/// Ponto 15: `arquivo_ou` continua devolvendo o padrão, sem virar `Resultado`.
const FONTE_HISTORICO_ARQUIVO_OU: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "ausente");
    falar(arquivo_ou(alvo, "padrao-historico"));
    falar("fim");
    mimo 0;
}
"#;

/// Ponto 14: invariante interna continua fatal. Um handle já liberado não vira
/// `Erro(...)` só porque a Parte B existe.
const FONTE_INVARIANTE_CONTINUA_FATAL: &str = r#"
pacote main;

carinho principal() -> bombom {
    nova alvo: verso = argumento_ou(0, "invariante.txt");
    nova h: bombom = criar_arquivo(alvo);
    escrever_verso(h, "x");
    fechar(h);
    falar("fechou");
    fechar(h);
    falar("nao alcancavel");
    mimo 0;
}
"#;

fn escrever_caso(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte Parte B");
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
        .expect("executar interpretador Parte B sob envelope")
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
        .expect("compilar Parte B sob envelope")
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
        .expect("executar ELF Parte B sob envelope")
}

struct Paridade {
    stdout_interpretador: String,
    stdout_nativo: String,
    stderr_interpretador: String,
    stderr_nativo: String,
    exit_interpretador: Option<i32>,
    exit_nativo: Option<i32>,
}

impl Paridade {
    /// Sucesso equivalente: mesmo stdout, exit 0 nos dois modos.
    fn exigir_sucesso(&self, nome: &str, stdout_esperado: &str) {
        assert_eq!(
            self.stdout_interpretador, stdout_esperado,
            "{nome}: stdout do interpretador"
        );
        assert_eq!(
            self.stdout_nativo, stdout_esperado,
            "{nome}: stdout do nativo"
        );
        assert_eq!(
            self.exit_interpretador,
            Some(0),
            "{nome}: interpretador deveria terminar com exit 0 (stderr: {})",
            self.stderr_interpretador
        );
        assert_eq!(
            self.exit_nativo,
            Some(0),
            "{nome}: nativo deveria terminar com exit 0 (stderr: {})",
            self.stderr_nativo
        );
        self.exigir_sem_panico(nome);
    }

    /// Falha equivalente: mesmo stdout até o ponto do aborto, exit 1 nos dois.
    fn exigir_aborto(&self, nome: &str, stdout_esperado: &str) {
        assert_eq!(
            self.stdout_interpretador, stdout_esperado,
            "{nome}: stdout do interpretador até o aborto"
        );
        assert_eq!(
            self.stdout_nativo, stdout_esperado,
            "{nome}: stdout do nativo até o aborto"
        );
        assert_eq!(
            self.exit_interpretador,
            Some(1),
            "{nome}: interpretador deveria abortar com exit 1"
        );
        assert_eq!(
            self.exit_nativo,
            Some(1),
            "{nome}: nativo deveria abortar com exit 1"
        );
        self.exigir_sem_panico(nome);
    }

    fn exigir_sem_panico(&self, nome: &str) {
        assert!(
            !self.stderr_interpretador.contains("panicked"),
            "{nome}: interpretador entrou em pânico: {}",
            self.stderr_interpretador
        );
        assert!(
            !self.stderr_nativo.contains("panicked"),
            "{nome}: nativo entrou em pânico: {}",
            self.stderr_nativo
        );
    }
}

/// Executa o mesmo caso nos dois modos. Cada modo recebe os argumentos já
/// resolvidos pelo chamador, porque parte dos casos precisa de caminhos
/// distintos por modo.
fn paridade_do_caso(
    nome: &str,
    fonte: &str,
    args_interpretador: &[String],
    args_nativo: &[String],
    runtime_lib: &Path,
) -> Paridade {
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte B");
    let fonte_path = escrever_caso(&dir, nome, fonte);

    let interpretado = rodar_interpretador(
        &fonte_path,
        &format!("parte-b-{nome}-interpretador"),
        args_interpretador,
    );

    let build = compilar_nativo(
        &dir,
        &fonte_path,
        runtime_lib,
        &format!("parte-b-{nome}-build"),
    );
    assert!(
        build.status.success(),
        "build nativo Parte B falhou em {nome}: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nativo = rodar_nativo(
        &dir.path().join(nome),
        &format!("parte-b-{nome}-nativo"),
        args_nativo,
    );

    Paridade {
        stdout_interpretador: String::from_utf8_lossy(&interpretado.stdout).into_owned(),
        stdout_nativo: String::from_utf8_lossy(&nativo.stdout).into_owned(),
        stderr_interpretador: String::from_utf8_lossy(&interpretado.stderr).into_owned(),
        stderr_nativo: String::from_utf8_lossy(&nativo.stderr).into_owned(),
        exit_interpretador: interpretado.status.code(),
        exit_nativo: nativo.status.code(),
    }
}

/// Caso sem argumentos por modo.
fn paridade_simples(nome: &str, fonte: &str, args: &[String], runtime_lib: &Path) -> Paridade {
    paridade_do_caso(nome, fonte, args, args, runtime_lib)
}

/// As tags usadas pelo runtime nativo têm de ser as do leque predeclarado.
/// Reordenar `Ok`/`Erro` no template quebra aqui, e não em silêncio no valor.
#[test]
fn tags_do_resultado_predeclarado_sao_as_esperadas() {
    assert_eq!(
        pinker_v0::falha_operacional::TAG_OK,
        0,
        "Ok é a primeira variante declarada de Resultado<T,E>"
    );
    assert_eq!(
        pinker_v0::falha_operacional::TAG_ERRO,
        1,
        "Erro é a segunda variante declarada de Resultado<T,E>"
    );

    // A autoridade única precisa concordar com o nome monomórfico que o parser
    // compõe para a mesma especialização.
    let superficie = pinker_v0::falha_operacional::superficie("ler_arquivo_resultado")
        .expect("superfície de filesystem registrada");
    assert_eq!(
        superficie.leque_monomorfico(),
        "__gen_leque_Resultado_verso_verso"
    );
    let processo = pinker_v0::falha_operacional::superficie("executar_processo_resultado")
        .expect("superfície de processo registrada");
    assert_eq!(
        processo.leque_monomorfico(),
        "__gen_leque_Resultado_bombom_verso"
    );
}

/// Ponto 13: erro estático continua estático. Passar `bombom` onde a superfície
/// exige `verso` é recusado antes de qualquer execução.
#[test]
fn erro_estatico_continua_estatico() {
    let dir = NativeArtifactDir::create().expect("diretório Parte B");
    let fonte = r#"
pacote main;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler_arquivo_resultado(42) {
        sucesso ResVV.Ok(c) { falar(c); }
        falha ResVV.Erro(e) { falar(e); }
    }
    mimo 0;
}
"#;
    let caminho = escrever_caso(&dir, "estatico_tipo", fonte);
    let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(&caminho)
        .logical_case("parte-b-estatico-tipo")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("checar Parte B sob envelope");
    assert!(
        !saida.status.success(),
        "tipo inválido deveria ser recusado estaticamente"
    );
    let stderr = String::from_utf8_lossy(&saida.stderr);
    assert!(
        stderr.contains("ler_arquivo_resultado") && stderr.contains("verso"),
        "diagnóstico estático não identificou a superfície e o tipo: {stderr}"
    );

    // Aridade inválida também permanece estática.
    let fonte_aridade = r#"
pacote main;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler_arquivo_resultado("a", "b") {
        sucesso ResVV.Ok(c) { falar(c); }
        falha ResVV.Erro(e) { falar(e); }
    }
    mimo 0;
}
"#;
    let caminho_aridade = escrever_caso(&dir, "estatico_aridade", fonte_aridade);
    let saida_aridade = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(&caminho_aridade)
        .logical_case("parte-b-estatico-aridade")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("checar aridade Parte B sob envelope");
    assert!(
        !saida_aridade.status.success(),
        "aridade inválida deveria ser recusada estaticamente"
    );
}

#[test]
fn falha_operacional_atravessa_resultado_com_paridade_entre_interpretador_e_nativo() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    // -- Controle positivo 1: o fluxo histórico de erro continua intacto.
    let historico = paridade_simples(
        "historico_resultado",
        FONTE_HISTORICO_RESULTADO,
        &[],
        &runtime_lib,
    );
    historico.exigir_sucesso(
        "histórico Resultado/tentar/propagar",
        "42\nfalha validada\n",
    );

    // -- Controle positivo 2: sucesso das três superfícies novas.
    let dir_ok = NativeArtifactDir::create().expect("diretório sucesso");
    let arquivo_ok = dir_ok.path().join("conteudo.txt");
    fs::write(&arquivo_ok, "conteudo-real").expect("escrever arquivo de sucesso");
    let args_ok = vec![
        arquivo_ok.to_string_lossy().into_owned(),
        "/usr/bin/true".to_string(),
    ];
    let sucesso = paridade_simples("sucesso", FONTE_SUCESSO, &args_ok, &runtime_lib);
    sucesso.exigir_sucesso(
        "sucesso das três superfícies",
        "conteudo-real\n0\n4242\nfim\n",
    );

    // -- Três domínios independentes falham como valor, com payload idêntico
    //    nos dois backends, e o programa continua.
    let ausente = dir_ok.path().join("nao-existe.txt");
    let exe_ausente = dir_ok.path().join("nao-existe-exe");
    let args_falha = vec![
        ausente.to_string_lossy().into_owned(),
        exe_ausente.to_string_lossy().into_owned(),
    ];
    let falha = paridade_simples(
        "falha_tres_dominios",
        FONTE_FALHA_TRES_DOMINIOS,
        &args_falha,
        &runtime_lib,
    );
    let esperado_falha = format!(
        "falha ao ler arquivo '{}': No such file or directory (os error 2)\n\
         falha ao executar processo '{}': No such file or directory (os error 2)\n\
         falha ao converter 'nao-numero' para bombom\n\
         fim\n",
        ausente.display(),
        exe_ausente.display()
    );
    falha.exigir_sucesso("três domínios como valor", &esperado_falha);

    // -- Propagação por função intermediária + tratamento por encaixe.
    let propagacao_ok = paridade_simples(
        "propagacao_ok",
        FONTE_PROPAGACAO_INTERMEDIARIA,
        &[arquivo_ok.to_string_lossy().into_owned()],
        &runtime_lib,
    );
    propagacao_ok.exigir_sucesso("propagação — sucesso", "via:lido:conteudo-real\nfim\n");

    let propagacao_erro = paridade_simples(
        "propagacao_erro",
        FONTE_PROPAGACAO_INTERMEDIARIA,
        &[ausente.to_string_lossy().into_owned()],
        &runtime_lib,
    );
    let esperado_propagacao = format!(
        "falha ao ler arquivo '{}': No such file or directory (os error 2)\nfim\n",
        ausente.display()
    );
    propagacao_erro.exigir_sucesso("propagação — erro atravessa intacto", &esperado_propagacao);
}

#[test]
fn falha_recuperavel_nao_deixa_recurso_parcialmente_vivo() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório recurso");
    let ausente = dir.path().join("nunca-existiu.txt");
    let args_interp = vec![
        ausente.to_string_lossy().into_owned(),
        dir.path()
            .join("interp_trabalho.txt")
            .to_string_lossy()
            .into_owned(),
    ];
    let args_nativo = vec![
        ausente.to_string_lossy().into_owned(),
        dir.path()
            .join("nativo_trabalho.txt")
            .to_string_lossy()
            .into_owned(),
    ];
    let paridade = paridade_do_caso(
        "falha_nao_vaza",
        FONTE_FALHA_NAO_VAZA_RECURSO,
        &args_interp,
        &args_nativo,
        &runtime_lib,
    );
    paridade.exigir_sucesso("300 falhas não vazam recurso", "300\nvivo\nfechou\n");
}

#[test]
fn compatibilidade_historica_e_fronteira_fatal_permanecem() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório compatibilidade");
    let ausente = dir.path().join("historico-ausente.txt");
    let args = vec![ausente.to_string_lossy().into_owned()];

    // A superfície histórica continua abortando: não virou Resultado por baixo.
    let historico = paridade_simples(
        "historico_aborta",
        FONTE_HISTORICO_AINDA_ABORTA,
        &args,
        &runtime_lib,
    );
    historico.exigir_aborto("ler_arquivo_verso continua fatal", "antes\n");

    // arquivo_ou continua entregando o padrão, sem virar Resultado.
    let arquivo_ou = paridade_simples(
        "historico_arquivo_ou",
        FONTE_HISTORICO_ARQUIVO_OU,
        &args,
        &runtime_lib,
    );
    arquivo_ou.exigir_sucesso("arquivo_ou preservado", "padrao-historico\nfim\n");

    // Violação de invariante continua fatal nos dois modos.
    let args_interp = vec![dir
        .path()
        .join("interp_invariante.txt")
        .to_string_lossy()
        .into_owned()];
    let args_nativo = vec![dir
        .path()
        .join("nativo_invariante.txt")
        .to_string_lossy()
        .into_owned()];
    let invariante = paridade_do_caso(
        "invariante_fatal",
        FONTE_INVARIANTE_CONTINUA_FATAL,
        &args_interp,
        &args_nativo,
        &runtime_lib,
    );
    invariante.exigir_aborto("double release continua fatal", "fechou\n");
}
// @pinker-nav:end evidencia.erros.parte-b-falha-operacional
