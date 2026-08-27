mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.ambiente.issue-492-paridade-argumento-nomeado
// @pinker-nav:domain ambiente
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da #492: a família de argumentos nomeados responde a mesma pergunta no interpretador e no ELF nativo. A matriz roda o mesmo programa com o mesmo argv, o mesmo ambiente explícito e o mesmo cwd nos dois backends e compara o observável inteiro — stdout, classe de erro e núcleo da mensagem —, nunca só o exit code. Cobre as sete formas de entrada (ausente, `--chave valor`, `--chave=valor`, `--chave` sem valor, `--chave=` vazio, `--chave --outra` e repetição nas três misturas) contra `pedir_argumento`, `buscar_contexto`, `tem_chave` e `tem_flag`, mais a precedência CLI/ambiente/padrão, os três aliases históricos e as chaves vazias. As invariantes de relação entre consultas afirmam o que é verdade — chave sem valor é `tem_flag` verdadeiro, `tem_chave` falso e erro em `pedir_argumento` ao mesmo tempo — sem forçar consultas diferentes a produzir a mesma resposta.

// ---------------------------------------------------------------------------
// Programas: um por superfície, porque um erro de runtime aborta o processo e
// abafaria as demais leituras se elas dividissem o mesmo programa.
// ---------------------------------------------------------------------------

const FONTE_PEDIR: &str = r#"
pacote main; trazer ambiente.pedir_argumento;

carinho principal() -> bombom {
    falar(pedir_argumento("--chave", "PADRAO"));
    mimo 0;
}
"#;

const FONTE_BUSCAR: &str = r#"
pacote main; trazer ambiente.buscar_contexto;

carinho principal() -> bombom {
    falar(buscar_contexto("--chave", "PINKER_492_ENV", "PADRAO"));
    mimo 0;
}
"#;

const FONTE_TEM_CHAVE: &str = r#"
pacote main; trazer ambiente.tem_chave;

carinho principal() -> bombom {
    falar(tem_chave("--chave"));
    mimo 0;
}
"#;

const FONTE_TEM_FLAG: &str = r#"
pacote main; trazer ambiente.tem_flag;

carinho principal() -> bombom {
    falar(tem_flag("--chave"));
    mimo 0;
}
"#;

/// Aliases históricos das três superfícies que os possuem.
const FONTE_ALIAS_PEDIR: &str = r#"
pacote main; trazer ambiente.pedir_argumento;

carinho principal() -> bombom {
    falar(pedir_argumento("--chave", "PADRAO"));
    mimo 0;
}
"#;

const FONTE_ALIAS_BUSCAR: &str = r#"
pacote main; trazer ambiente.buscar_contexto;

carinho principal() -> bombom {
    falar(buscar_contexto("--chave", "PINKER_492_ENV", "PADRAO"));
    mimo 0;
}
"#;

const FONTE_ALIAS_TEM_CHAVE: &str = r#"
pacote main; trazer ambiente.tem_chave;

carinho principal() -> bombom {
    falar(tem_chave("--chave"));
    mimo 0;
}
"#;

/// Chave de argumento vazia: negativo que já estava em paridade e continua.
const FONTE_CHAVE_VAZIA: &str = r#"
pacote main; trazer ambiente.pedir_argumento;

carinho principal() -> bombom {
    falar(pedir_argumento("", "PADRAO"));
    mimo 0;
}
"#;

/// `ambiente_ou` com chave vazia: uma chave só, e por isso a mensagem genérica.
const FONTE_AMBIENTE_OU_CHAVE_VAZIA: &str = r#"
pacote main; trazer ambiente.variavel_ou;

carinho principal() -> bombom {
    falar(variavel_ou("", "PADRAO"));
    mimo 0;
}
"#;

/// Chave de ambiente vazia: o diagnóstico precisa dizer **qual** chave.
const FONTE_CHAVE_AMBIENTE_VAZIA: &str = r#"
pacote main; trazer ambiente.buscar_contexto;

carinho principal() -> bombom {
    falar(buscar_contexto("--chave", "", "PADRAO"));
    mimo 0;
}
"#;

// ---------------------------------------------------------------------------
// Envelope de paridade
// ---------------------------------------------------------------------------

/// Variável usada pela matriz. Sempre explicitamente posta ou explicitamente
/// removida: herdar o ambiente do runner tornaria a precedência não observável.
const CHAVE_AMBIENTE: &str = "PINKER_492_ENV";

/// O ambiente controlado de um caso.
#[derive(Clone, Copy)]
enum Ambiente {
    Ausente,
    Presente(&'static str),
}

/// O observável de um backend, na granularidade em que a paridade é exigida.
///
/// Comparar `Sucesso` com `Sucesso` compara o stdout inteiro. Comparar `Falha`
/// com `Falha` compara o **núcleo** da mensagem, não o envelope: o
/// interpretador escreve `Erro Runtime:` com stack trace e o nativo escreve
/// `Erro de Execução (pinker_rt):`, e essa diferença é do host, não do
/// contrato. O que não é permitido é um backend ter sucesso onde o outro falha
/// — exatamente a divergência que a #492 encontrou.
#[derive(Debug, PartialEq, Eq)]
enum Observavel {
    Sucesso(String),
    Falha(String),
}

/// Núcleo da mensagem de erro, com o envelope de cada host removido.
fn nucleo_do_erro(stderr: &str) -> String {
    for linha in stderr.lines() {
        if let Some(resto) = linha.split_once("[runtime::erro] ") {
            return resto.1.trim().to_string();
        }
        if let Some(resto) = linha.strip_prefix("Erro de Execução (pinker_rt): ") {
            return resto.trim().to_string();
        }
    }
    format!("SEM_NUCLEO_RECONHECIVEL: {stderr}")
}

fn observavel(saida: &Output) -> Observavel {
    let stderr = String::from_utf8_lossy(&saida.stderr);
    assert!(
        !stderr.contains("panicked"),
        "backend entrou em pânico: {stderr}"
    );
    if saida.status.success() {
        Observavel::Sucesso(String::from_utf8_lossy(&saida.stdout).into_owned())
    } else {
        Observavel::Falha(nucleo_do_erro(&stderr))
    }
}

fn escrever_caso(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte #492");
    caminho
}

fn aplicar_ambiente(comando: &mut Command, ambiente: Ambiente) {
    match ambiente {
        Ambiente::Ausente => {
            comando.env_remove(CHAVE_AMBIENTE);
        }
        Ambiente::Presente(valor) => {
            comando.env(CHAVE_AMBIENTE, valor);
        }
    }
}

fn rodar_interpretador(caminho: &Path, caso: &str, args: &[&str], ambiente: Ambiente) -> Output {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
    comando.arg("--run").arg(caminho);
    if !args.is_empty() {
        comando.arg("--");
        comando.args(args);
    }
    aplicar_ambiente(&mut comando, ambiente);
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar interpretador #492 sob envelope")
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
        .expect("compilar #492 sob envelope")
}

fn rodar_nativo(caminho: &Path, caso: &str, args: &[&str], ambiente: Ambiente) -> Output {
    let mut comando = Command::new(caminho);
    comando.args(args);
    aplicar_ambiente(&mut comando, ambiente);
    comando
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar ELF #492 sob envelope")
}

/// Compila uma fonte uma vez e devolve um executor que roda os dois backends
/// sobre o **mesmo** artefato, para que nenhum caso possa passar por ter
/// compilado outra coisa.
struct Sujeito {
    _dir: NativeArtifactDir,
    fonte: PathBuf,
    binario: PathBuf,
}

impl Sujeito {
    fn novo(nome: &str, fonte_texto: &str, runtime_lib: &Path) -> Self {
        let dir = NativeArtifactDir::create().expect("diretório nativo #492");
        let fonte = escrever_caso(&dir, nome, fonte_texto);
        let compilacao = compilar_nativo(&dir, &fonte, runtime_lib, nome);
        assert!(
            compilacao.status.success(),
            "{nome}: build nativo falhou: {}",
            String::from_utf8_lossy(&compilacao.stderr)
        );
        let binario = dir.path().join(nome);
        Sujeito {
            _dir: dir,
            fonte,
            binario,
        }
    }

    /// Roda os dois backends com o mesmo argv e o mesmo ambiente, exige que
    /// concordem e devolve o observável comum.
    fn paridade(&self, caso: &str, args: &[&str], ambiente: Ambiente) -> Observavel {
        let (interpretado, nativo) = self.observar(caso, args, ambiente);
        assert_eq!(
            interpretado, nativo,
            "{caso}: interpretador e nativo divergiram para argv {args:?}"
        );
        interpretado
    }

    /// Os dois observáveis, sem exigir igualdade.
    ///
    /// Existe para o único caso da família em que a diferença é conhecida,
    /// classificada e fora do escopo da #492 — a grafia que o diagnóstico
    /// nomeia quando um alias histórico é chamado.
    fn observar(&self, caso: &str, args: &[&str], ambiente: Ambiente) -> (Observavel, Observavel) {
        let interpretado = observavel(&rodar_interpretador(&self.fonte, caso, args, ambiente));
        let nativo = observavel(&rodar_nativo(&self.binario, caso, args, ambiente));
        (interpretado, nativo)
    }
}

fn sucesso(texto: &str) -> Observavel {
    Observavel::Sucesso(format!("{texto}\n"))
}

fn sem_valor(intrinseca: &str) -> Observavel {
    Observavel::Falha(format!(
        "intrínseca '{intrinseca}' encontrou chave '--chave' sem valor na forma '--chave valor'"
    ))
}

// ---------------------------------------------------------------------------
// A matriz de formas, table-driven
// ---------------------------------------------------------------------------

/// As sete formas de entrada que a gramática distingue.
///
/// `F4` é a forma que a #492 encontrou divergente; as outras seis existem para
/// provar que ela foi corrigida **sozinha**.
const FORMAS: &[(&str, &[&str])] = &[
    ("F1_ausente", &[]),
    ("F2_chave_valor", &["--chave", "valor"]),
    ("F3_chave_igual", &["--chave=valor"]),
    ("F4_chave_sem_valor", &["--chave"]),
    ("F5_chave_igual_vazio", &["--chave="]),
    ("F6_chave_proxima_opcao", &["--chave", "--outra"]),
    (
        "F7_repeticao_separada",
        &["--chave", "primeiro", "--chave", "segundo"],
    ),
    (
        "F8_repeticao_igual",
        &["--chave=primeiro", "--chave=segundo"],
    ),
    (
        "F9_repeticao_mista",
        &["--chave", "primeiro", "--chave=segundo"],
    ),
];

#[test]
fn pedir_argumento_tem_paridade_em_todas_as_formas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let sujeito = Sujeito::novo("pedir", FONTE_PEDIR, &runtime_lib);

    let esperado: &[(&str, Observavel)] = &[
        ("F1_ausente", sucesso("PADRAO")),
        ("F2_chave_valor", sucesso("valor")),
        ("F3_chave_igual", sucesso("valor")),
        ("F4_chave_sem_valor", sem_valor("pedir_argumento")),
        // `--chave=` é valor vazio explícito, não ausência: o padrão não entra.
        ("F5_chave_igual_vazio", sucesso("")),
        // A forma separada consome o próximo token qualquer que ele seja; a
        // Pinker não implementa a convenção GNU de parar na próxima opção.
        ("F6_chave_proxima_opcao", sucesso("--outra")),
        // Repetição: a primeira ocorrência vence, nas três misturas.
        ("F7_repeticao_separada", sucesso("primeiro")),
        ("F8_repeticao_igual", sucesso("primeiro")),
        ("F9_repeticao_mista", sucesso("primeiro")),
    ];

    for (nome, args) in FORMAS {
        let observado = sujeito.paridade(nome, args, Ambiente::Ausente);
        let (_, alvo) = esperado
            .iter()
            .find(|(chave, _)| chave == nome)
            .expect("forma coberta");
        assert_eq!(&observado, alvo, "pedir_argumento {nome}");
    }
}

#[test]
fn buscar_contexto_tem_paridade_em_todas_as_formas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let sujeito = Sujeito::novo("buscar", FONTE_BUSCAR, &runtime_lib);

    let esperado: &[(&str, Observavel)] = &[
        ("F1_ausente", sucesso("PADRAO")),
        ("F2_chave_valor", sucesso("valor")),
        ("F3_chave_igual", sucesso("valor")),
        ("F4_chave_sem_valor", sem_valor("buscar_contexto")),
        ("F5_chave_igual_vazio", sucesso("")),
        ("F6_chave_proxima_opcao", sucesso("--outra")),
        ("F7_repeticao_separada", sucesso("primeiro")),
        ("F8_repeticao_igual", sucesso("primeiro")),
        ("F9_repeticao_mista", sucesso("primeiro")),
    ];

    for (nome, args) in FORMAS {
        let observado = sujeito.paridade(nome, args, Ambiente::Ausente);
        let (_, alvo) = esperado
            .iter()
            .find(|(chave, _)| chave == nome)
            .expect("forma coberta");
        assert_eq!(&observado, alvo, "buscar_contexto {nome}");
    }
}

#[test]
fn tem_chave_tem_paridade_em_todas_as_formas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let sujeito = Sujeito::novo("temchave", FONTE_TEM_CHAVE, &runtime_lib);

    // `tem_chave` pergunta "presente **com valor**". `--chave` sozinha é falso
    // nas duas pontas, e isso não é contradição com `tem_flag`.
    let esperado: &[(&str, &str)] = &[
        ("F1_ausente", "falso"),
        ("F2_chave_valor", "verdade"),
        ("F3_chave_igual", "verdade"),
        ("F4_chave_sem_valor", "falso"),
        ("F5_chave_igual_vazio", "verdade"),
        ("F6_chave_proxima_opcao", "verdade"),
        ("F7_repeticao_separada", "verdade"),
        ("F8_repeticao_igual", "verdade"),
        ("F9_repeticao_mista", "verdade"),
    ];

    for (nome, args) in FORMAS {
        let observado = sujeito.paridade(nome, args, Ambiente::Ausente);
        let (_, alvo) = esperado
            .iter()
            .find(|(chave, _)| chave == nome)
            .expect("forma coberta");
        assert_eq!(observado, sucesso(alvo), "tem_chave {nome}");
    }
}

#[test]
fn tem_flag_tem_paridade_em_todas_as_formas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let sujeito = Sujeito::novo("temflag", FONTE_TEM_FLAG, &runtime_lib);

    // `tem_flag` pergunta pelo token exato. `--chave=valor` é falso de
    // propósito: o token exato não está no argv. Não é assimetria a corrigir.
    let esperado: &[(&str, &str)] = &[
        ("F1_ausente", "falso"),
        ("F2_chave_valor", "verdade"),
        ("F3_chave_igual", "falso"),
        ("F4_chave_sem_valor", "verdade"),
        ("F5_chave_igual_vazio", "falso"),
        ("F6_chave_proxima_opcao", "verdade"),
        ("F7_repeticao_separada", "verdade"),
        ("F8_repeticao_igual", "falso"),
        ("F9_repeticao_mista", "verdade"),
    ];

    for (nome, args) in FORMAS {
        let observado = sujeito.paridade(nome, args, Ambiente::Ausente);
        let (_, alvo) = esperado
            .iter()
            .find(|(chave, _)| chave == nome)
            .expect("forma coberta");
        assert_eq!(observado, sucesso(alvo), "tem_flag {nome}");
    }
}

// ---------------------------------------------------------------------------
// Precedência CLI / ambiente / padrão
// ---------------------------------------------------------------------------

#[test]
fn buscar_contexto_tem_paridade_na_precedencia_do_ambiente() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let sujeito = Sujeito::novo("precedencia", FONTE_BUSCAR, &runtime_lib);
    const DO_AMBIENTE: Ambiente = Ambiente::Presente("doambiente");

    // F10: sem chave e sem ambiente -> padrão.
    assert_eq!(
        sujeito.paridade("F10", &[], Ambiente::Ausente),
        sucesso("PADRAO")
    );
    // F11: sem chave e com ambiente -> ambiente. O ambiente é fallback de
    // Ausente, e o valor tem de ser provado, não só o exit.
    assert_eq!(
        sujeito.paridade("F11", &[], DO_AMBIENTE),
        sucesso("doambiente")
    );
    // F12: CLI vence o ambiente.
    assert_eq!(
        sujeito.paridade("F12", &["--chave", "valor"], DO_AMBIENTE),
        sucesso("valor")
    );
    // F13: chave sem valor **bloqueia** o fallback. É o caso que a #492 chama
    // de mascaramento: o nativo devolvia o valor do ambiente para uma chave que
    // o usuário escreveu e deixou sem valor.
    assert_eq!(
        sujeito.paridade("F13", &["--chave"], DO_AMBIENTE),
        sem_valor("buscar_contexto")
    );
    // F14: valor vazio explícito também vence o ambiente — é valor.
    assert_eq!(
        sujeito.paridade("F14", &["--chave="], DO_AMBIENTE),
        sucesso("")
    );
}

#[test]
fn pedir_argumento_ignora_o_ambiente_nas_duas_pontas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let sujeito = Sujeito::novo("pedirenv", FONTE_PEDIR, &runtime_lib);
    const DO_AMBIENTE: Ambiente = Ambiente::Presente("doambiente");

    // `pedir_argumento` não tem chave de ambiente: o ambiente não pode
    // aparecer, nem quando existe uma variável com o nome usado pela matriz.
    assert_eq!(
        sujeito.paridade("sem_arg", &[], DO_AMBIENTE),
        sucesso("PADRAO")
    );
    assert_eq!(
        sujeito.paridade("sem_valor", &["--chave"], DO_AMBIENTE),
        sem_valor("pedir_argumento")
    );
}

// ---------------------------------------------------------------------------
// Relação entre consultas: o que é verdade ao mesmo tempo
// ---------------------------------------------------------------------------

#[test]
fn chave_sem_valor_e_lida_de_forma_coerente_pelas_quatro_consultas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let args = &["--chave"];

    // As quatro consultas respondem perguntas diferentes sobre o MESMO argv, e
    // as quatro respostas coexistem sem contradição:
    //   tem_flag        = verdade   (o token exato está lá)
    //   tem_chave       = falso     (mas não carrega valor)
    //   pedir_argumento = erro      (foi pedido um valor que não existe)
    //   buscar_contexto = erro      (e o ambiente não pode inventá-lo)
    let flag = Sujeito::novo("coer_flag", FONTE_TEM_FLAG, &runtime_lib);
    assert_eq!(
        flag.paridade("coer_flag", args, Ambiente::Ausente),
        sucesso("verdade")
    );

    let chave = Sujeito::novo("coer_chave", FONTE_TEM_CHAVE, &runtime_lib);
    assert_eq!(
        chave.paridade("coer_chave", args, Ambiente::Ausente),
        sucesso("falso")
    );

    let pedir = Sujeito::novo("coer_pedir", FONTE_PEDIR, &runtime_lib);
    assert_eq!(
        pedir.paridade("coer_pedir", args, Ambiente::Ausente),
        sem_valor("pedir_argumento")
    );

    let buscar = Sujeito::novo("coer_buscar", FONTE_BUSCAR, &runtime_lib);
    assert_eq!(
        buscar.paridade("coer_buscar", args, Ambiente::Presente("doambiente")),
        sem_valor("buscar_contexto")
    );
}

#[test]
fn chave_com_igual_separa_as_duas_perguntas_de_presenca() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let args = &["--chave=valor"];

    // A assimetria oposta, e igualmente legítima: a chave carrega valor mas o
    // token exato não está no argv. `tem_flag` responder falso aqui é o
    // contrato, não um bug de simetria.
    let flag = Sujeito::novo("igual_flag", FONTE_TEM_FLAG, &runtime_lib);
    assert_eq!(
        flag.paridade("igual_flag", args, Ambiente::Ausente),
        sucesso("falso")
    );

    let chave = Sujeito::novo("igual_chave", FONTE_TEM_CHAVE, &runtime_lib);
    assert_eq!(
        chave.paridade("igual_chave", args, Ambiente::Ausente),
        sucesso("verdade")
    );

    let pedir = Sujeito::novo("igual_pedir", FONTE_PEDIR, &runtime_lib);
    assert_eq!(
        pedir.paridade("igual_pedir", args, Ambiente::Ausente),
        sucesso("valor")
    );
}

// ---------------------------------------------------------------------------
// Aliases históricos
// ---------------------------------------------------------------------------

#[test]
fn aliases_historicos_tem_o_mesmo_contrato_nas_duas_pontas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let pedir = Sujeito::novo("alias_pedir", FONTE_ALIAS_PEDIR, &runtime_lib);
    assert_eq!(
        pedir.paridade("alias_pedir_ausente", &[], Ambiente::Ausente),
        sucesso("PADRAO")
    );
    assert_eq!(
        pedir.paridade("alias_pedir_valor", &["--chave", "v"], Ambiente::Ausente),
        sucesso("v")
    );
    assert_eq!(
        pedir.paridade("alias_pedir_igual", &["--chave=v"], Ambiente::Ausente),
        sucesso("v")
    );
    assert_eq!(
        pedir.paridade("alias_pedir_vazio", &["--chave="], Ambiente::Ausente),
        sucesso("")
    );

    let buscar = Sujeito::novo("alias_buscar", FONTE_ALIAS_BUSCAR, &runtime_lib);
    assert_eq!(
        buscar.paridade("alias_buscar_padrao", &[], Ambiente::Ausente),
        sucesso("PADRAO")
    );
    assert_eq!(
        buscar.paridade("alias_buscar_env", &[], Ambiente::Presente("doambiente")),
        sucesso("doambiente")
    );
    assert_eq!(
        buscar.paridade(
            "alias_buscar_cli",
            &["--chave", "v"],
            Ambiente::Presente("doambiente")
        ),
        sucesso("v")
    );

    let tem = Sujeito::novo("alias_tem", FONTE_ALIAS_TEM_CHAVE, &runtime_lib);
    assert_eq!(
        tem.paridade("alias_tem_sem_valor", &["--chave"], Ambiente::Ausente),
        sucesso("falso")
    );
    assert_eq!(
        tem.paridade("alias_tem_valor", &["--chave=v"], Ambiente::Ausente),
        sucesso("verdade")
    );
}

/// A diferença que restava na família FECHOU, e o gate passou a medir isso.
///
/// Enquanto o alias histórico era chamável, os dois backends recusavam a mesma
/// entrada pelo mesmo motivo nomeando grafias diferentes da mesma identidade:
/// o interpretador conhecia a grafia chamada e o nativo não podia conhecê-la,
/// porque `backend_s` mapeia as duas grafias para o mesmo símbolo de runtime.
/// A diferença era de ENDEREÇAMENTO POR TEXTO — o assunto da #477 —, e não do
/// contrato da #492.
///
/// A #505 removeu a superfície global e, com ela, a grafia alias como forma
/// chamável: hoje só existe `ambiente.pedir_argumento`, e as duas pontas
/// nomeiam a mesma coisa. O caso continua fixado aqui, agora como oráculo de
/// convergência: se alguma ponta voltar a nomear grafia diferente da outra,
/// isto fica vermelho.
#[test]
fn alias_em_falha_deixou_de_divergir_na_grafia_nomeada_pelo_diagnostico() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let pedir = Sujeito::novo("alias_falha_pedir", FONTE_ALIAS_PEDIR, &runtime_lib);
    let (interpretado, nativo) =
        pedir.observar("alias_falha_pedir", &["--chave"], Ambiente::Ausente);
    assert_eq!(interpretado, sem_valor("pedir_argumento"));
    assert_eq!(nativo, sem_valor("pedir_argumento"));
    assert_eq!(interpretado, nativo);

    let buscar = Sujeito::novo("alias_falha_buscar", FONTE_ALIAS_BUSCAR, &runtime_lib);
    let (interpretado, nativo) = buscar.observar(
        "alias_falha_buscar",
        &["--chave"],
        Ambiente::Presente("doambiente"),
    );
    assert_eq!(interpretado, sem_valor("buscar_contexto"));
    assert_eq!(nativo, sem_valor("buscar_contexto"));
    assert_eq!(interpretado, nativo);

    // O que importa para a #492 não mudou: nenhuma das duas pontas devolveu
    // valor, a classe do erro é a mesma e o ambiente não mascarou nada.
    for observado in [interpretado, nativo] {
        match observado {
            Observavel::Falha(mensagem) => assert!(
                mensagem.contains("encontrou chave '--chave' sem valor"),
                "classe do erro mudou: {mensagem}"
            ),
            Observavel::Sucesso(saida) => {
                panic!("a superfície devolveu valor para chave sem valor: {saida:?}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Chaves vazias: o diagnóstico precisa dizer qual chave
// ---------------------------------------------------------------------------

#[test]
fn chaves_vazias_falham_com_o_mesmo_diagnostico_nas_duas_pontas() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let argumento = Sujeito::novo("vazia_arg", FONTE_CHAVE_VAZIA, &runtime_lib);
    assert_eq!(
        argumento.paridade("vazia_arg", &[], Ambiente::Ausente),
        Observavel::Falha("intrínseca 'pedir_argumento' exige chave não vazia".to_string())
    );

    // `buscar_contexto` recebe duas chaves; o diagnóstico tem de distinguir
    // qual delas está vazia, nos dois backends.
    let ambiente = Sujeito::novo("vazia_env", FONTE_CHAVE_AMBIENTE_VAZIA, &runtime_lib);
    assert_eq!(
        ambiente.paridade("vazia_env", &[], Ambiente::Ausente),
        Observavel::Falha(
            "intrínseca 'buscar_contexto' exige chave de ambiente não vazia".to_string()
        )
    );

    // `ambiente_ou` é a superfície de ambiente da mesma família e recusava a
    // chave vazia só no nativo: o interpretador devolvia o padrão e saía 0.
    // Sucesso de um lado e falha do outro é a mesma classe de divergência que a
    // #492 fecha, então a célula entra na matriz em vez de ficar como nota.
    let ambiente_ou = Sujeito::novo("vazia_amb_ou", FONTE_AMBIENTE_OU_CHAVE_VAZIA, &runtime_lib);
    assert_eq!(
        ambiente_ou.paridade("vazia_amb_ou", &[], Ambiente::Ausente),
        Observavel::Falha("intrínseca 'ambiente_ou' exige chave não vazia".to_string())
    );
}

// ---------------------------------------------------------------------------
// Controle positivo
// ---------------------------------------------------------------------------

#[test]
fn o_envelope_de_paridade_distingue_sucesso_de_falha() {
    // Sem este controle, uma matriz inteira poderia passar por comparar dois
    // backends que falham igual, ou por um `Observavel` que colapsasse as duas
    // classes num valor só.
    let sucesso_a = Observavel::Sucesso("valor\n".to_string());
    let sucesso_b = Observavel::Sucesso("outro\n".to_string());
    let falha = Observavel::Falha("intrínseca 'x' exige chave não vazia".to_string());

    assert_ne!(sucesso_a, sucesso_b, "stdout diferente tem de divergir");
    assert_ne!(sucesso_a, falha, "sucesso não pode igualar falha");
    assert_eq!(
        nucleo_do_erro("Erro de Execução (pinker_rt): intrínseca 'x' exige chave não vazia"),
        "intrínseca 'x' exige chave não vazia"
    );
    assert_eq!(
        nucleo_do_erro("Erro Runtime:\n  mensagem: [runtime::erro] intrínseca 'x' exige chave não vazia\nstack trace:"),
        "intrínseca 'x' exige chave não vazia"
    );
    // Um stderr sem núcleo reconhecível não pode virar igualdade silenciosa
    // entre dois backends.
    assert!(nucleo_do_erro("ruído qualquer").starts_with("SEM_NUCLEO_RECONHECIVEL"));
}
// @pinker-nav:end evidencia.ambiente.issue-492-paridade-argumento-nomeado
