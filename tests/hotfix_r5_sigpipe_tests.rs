//! Hotfix pós-PR #411 — item R5: a disposição de `SIGPIPE` do runtime nativo
//! não pode depender da ordem de execução do programa.
//!
//! Antes da correção, `SIG_IGN` só era instalado a partir do primeiro `falar`.
//! Um programa que escrevesse stdin de um filho **sem** ter falado antes morria
//! por sinal (exit 141, stderr vazio) em vez de alcançar a agregação de erro de
//! `executar_com_entrada`; com um `falar` antes, o mesmo programa terminava com
//! exit 1 e diagnóstico. O interpretador sempre deu exit 1 nos dois casos, então
//! a divergência também quebrava paridade.
//!
//! A evidência aqui é a matriz completa exigida pelo contrato: stdout anterior ×
//! comportamento do filho × tamanho do stdin, comparando interpretador e nativo
//! célula a célula.
//!
//! A continuação de portabilidade acrescenta o outro lado do contrato: o filho.
//! A Pinker devolve `SIGPIPE` a `SIG_DFL` no contexto pré-`exec` por conta
//! própria, em todas as famílias de subprocesso dos dois back-ends, sem delegar
//! isso à biblioteca padrão. Como a `std` também restaura hoje, a remoção da
//! nossa configuração não seria observável pelo filho — daí o guardião
//! estrutural, ao lado da matriz observável e da validação da própria sonda.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

const EXEMPLO: &str = "examples/hotfix_r5_sigpipe_matriz_valido.pink";
const EXEMPLO_FAMILIAS: &str = "examples/hotfix_r5_sigpipe_familias_valido.pink";

/// Teto de tempo por célula. Qualquer deadlock (writer órfão, `wait` antes da
/// escrita, pipe nunca fechado) estoura este limite em vez de travar a suíte.
const LIMITE: Duration = Duration::from_secs(30);

/// Comportamentos do filho diante do stdin escrito pelo pai.
const MODOS_FILHO: [&str; 5] = [
    "encerra",    // encerra na hora, sem ler
    "espera",     // nunca lê e permanece vivo antes de encerrar
    "le-tudo",    // lê tudo até EOF
    "le-parcial", // lê um byte e sai
    "fecha-cedo", // fecha o stdin explicitamente e sai
];

/// Tamanhos de stdin, incluindo a capacidade padrão do pipe (65536) e valores
/// acima dela, onde a escrita necessariamente bloqueia antes de falhar.
const TAMANHOS: [u64; 6] = [0, 1, 4096, 65536, 262144, 1048576];

/// Escritas em stdout antes do processo: nenhuma, um `falar`, várias.
const ESCRITAS_ANTERIORES: [u64; 3] = [0, 1, 2];

/// Capacidade padrão do buffer de um pipe no Linux. Uma escrita que caiba aqui
/// pode completar sem consumidor; acima disso, ela necessariamente bloqueia.
const CAPACIDADE_PIPE: u64 = 65536;

/// Modos em que o filho encerra sem drenar o stdin.
const MODOS_SEM_DRENAGEM: [&str; 3] = ["encerra", "fecha-cedo", "le-parcial"];

/// Uma célula é **decidida por corrida** quando o filho encerra sem drenar o
/// stdin e o que o pai escreve cabe no buffer do pipe.
///
/// Nessas células, quem chega primeiro decide o resultado: se a escrita vence,
/// ela completa no buffer e o programa termina em `ok`; se o filho vence, o
/// descritor de leitura já fechou e a escrita colhe `EPIPE`, virando o
/// diagnóstico controlado. As duas saídas são **igualmente corretas** — a
/// segunda é justamente o que o item R5 existe para garantir.
///
/// Medido sob carga (`nproc × 3` processos ocupados), as duas saídas aparecem
/// nos dois back-ends, e já apareciam antes desta continuação: em `8b6f0c4`,
/// `fecha-cedo` com 1 byte deu 7 `ok` em 20 no nativo e 13 em 20 no
/// interpretador. Exigir paridade da classe exata aqui não afere contrato
/// nenhum — afere o escalonador, e o resultado é um teste que pisca sob carga.
///
/// Fora dessas células o resultado é determinístico e a paridade continua
/// exata: sem escrita (`tamanho == 0`) ou com filho que drena (`le-tudo`)
/// sempre `ok`; acima da capacidade do pipe, sempre `EPIPE` quando ninguém
/// consome. `espera` permanece determinístico dos dois lados porque o filho
/// sobrevive ao tempo da escrita.
fn celula_decidida_por_corrida(modo: &str, tamanho: u64) -> bool {
    MODOS_SEM_DRENAGEM.contains(&modo) && tamanho > 0 && tamanho <= CAPACIDADE_PIPE
}

struct Saida {
    codigo: Option<i32>,
    stdout: String,
    stderr: String,
}

impl Saida {
    /// Classe observável do resultado, usada para comparar back-ends sem
    /// depender do código exato do filho.
    fn classe(&self) -> &'static str {
        match self.codigo {
            Some(0) => "ok",
            Some(1) if self.stderr.contains("falha ao escrever stdin") => "epipe-diagnosticado",
            Some(_) => "outro-erro",
            None => "terminado-por-sinal",
        }
    }

    /// Classe comparável entre back-ends.
    ///
    /// Numa célula decidida por corrida, `ok` e `epipe-diagnosticado` são o
    /// mesmo veredicto de contrato — o programa terminou por decisão do
    /// runtime, não por sinal. As classes que denunciam defeito
    /// (`terminado-por-sinal`, `outro-erro`) continuam distintas e continuam
    /// comparadas exatamente, em toda célula.
    fn classe_comparavel(&self, corrida: bool) -> &'static str {
        let classe = self.classe();
        if corrida && matches!(classe, "ok" | "epipe-diagnosticado") {
            "sem-morte-por-sinal"
        } else {
            classe
        }
    }

    /// Stdout comparável entre back-ends.
    ///
    /// A linha final `codigo=` só existe no caminho feliz, então numa célula
    /// decidida por corrida ela varia com o escalonador. Tudo o que vem antes
    /// dela — as escritas anteriores, que são o eixo de ordem de execução da
    /// matriz — é determinístico e continua comparado byte a byte.
    fn stdout_comparavel(&self, corrida: bool) -> &str {
        if corrida {
            match self.stdout.find("codigo=") {
                Some(fim) => &self.stdout[..fim],
                None => &self.stdout,
            }
        } else {
            &self.stdout
        }
    }
}

/// Executa com teto de tempo. Devolve `None` quando o limite estoura — e, nesse
/// caso, **mata** o processo antes de voltar, para que o próprio detector de
/// deadlock não deixe processo órfão atrás de si.
fn executar_com_limite(mut comando: Command) -> Option<Saida> {
    let output = match comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .timeout(LIMITE)
        .logical_case("hotfix-r5-sigpipe")
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => return None,
        Err(error) => panic!("disparar processo da célula: {error}"),
    };

    Some(Saida {
        codigo: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn helper_filho() -> &'static str {
    env!("CARGO_BIN_EXE_pinker_hf412_filho_stdin")
}

fn celula_interpretada(modo: &str, tamanho: u64, antes: u64) -> Option<Saida> {
    let mut comando = Command::new(env!("CARGO_BIN_EXE_pink"));
    comando.args([
        "--run",
        EXEMPLO,
        "--",
        helper_filho(),
        modo,
        &tamanho.to_string(),
        &antes.to_string(),
    ]);
    executar_com_limite(comando)
}

fn celula_nativa(binario: &Path, modo: &str, tamanho: u64, antes: u64) -> Option<Saida> {
    let mut comando = Command::new(binario);
    comando.args([
        helper_filho(),
        modo,
        &tamanho.to_string(),
        &antes.to_string(),
    ]);
    executar_com_limite(comando)
}

/// Diretório temporário exclusivo desta execução.
/// Compila um exemplo uma única vez; a matriz inteira roda sobre o mesmo ELF.
fn compilar_exemplo(
    runtime_lib: &Path,
    exemplo: &str,
    _rotulo: &str,
) -> (NativeArtifactDir, PathBuf) {
    let artifacts = NativeArtifactDir::create().expect("diretório nativo marcado");
    let out_dir = artifacts.path();
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", runtime_lib)
        .output()
        .expect("invocar pink build --nativo");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nome = Path::new(exemplo)
        .file_stem()
        .expect("nome do exemplo")
        .to_str()
        .expect("nome utf-8");
    let executable = out_dir.join(nome);
    (artifacts, executable)
}

fn compilar_nativo(runtime_lib: &Path) -> (NativeArtifactDir, PathBuf) {
    compilar_exemplo(runtime_lib, EXEMPLO, "r5")
}

/// Invariantes exigidas de qualquer célula, em qualquer back-end.
fn exigir_invariantes(rotulo: &str, saida: &Saida) {
    assert!(
        saida.codigo.is_some(),
        "{rotulo}: processo terminou por sinal (SIGPIPE não pode escapar do runtime)"
    );
    assert_ne!(
        saida.codigo,
        Some(141),
        "{rotulo}: exit 141 indica morte por SIGPIPE mascarada pelo shell"
    );
    match saida.classe() {
        "ok" => assert!(
            saida.stdout.contains("codigo="),
            "{rotulo}: sucesso sem a linha final do programa\nstdout: {}",
            saida.stdout
        ),
        "epipe-diagnosticado" => assert!(
            !saida.stderr.is_empty(),
            "{rotulo}: EPIPE precisa virar diagnóstico visível"
        ),
        outra => panic!(
            "{rotulo}: classe inesperada {outra} (exit {:?})\nstderr: {}",
            saida.codigo, saida.stderr
        ),
    }
}

// @pinker-nav:start evidencia.hotfix.r5-sigpipe-ordem
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Matriz R5 de SIGPIPE: stdout anterior (nenhum/um falar/várias escritas) × comportamento do filho (encerra, espera sem ler, lê tudo, lê parcial, fecha stdin cedo) × tamanho de stdin (0, 1, 4096, 65536, 262144, acima da capacidade do pipe), exigindo em cada célula ausência de término por sinal, ausência de exit 141, ausência de deadlock sob teto de tempo, EPIPE convertido em diagnóstico e paridade entre interpretador e nativo; nas 27 células em que o filho encerra sem drenar e a escrita cabe no buffer do pipe, ok e epipe-diagnosticado são o mesmo veredicto de contrato e a paridade compara a classe estável mais o stdout anterior à linha codigo=, porque exigir a classe exata aferiria o escalonador; cobre ainda a disposição de SIGPIPE herdada pelo filho após exec, medida antes da inicialização da std.
#[test]
fn r5_matriz_sigpipe_independe_da_ordem_e_mantem_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let (_artifacts, binario) = compilar_nativo(&runtime_lib);

    for modo in MODOS_FILHO {
        for tamanho in TAMANHOS {
            for antes in ESCRITAS_ANTERIORES {
                let rotulo = format!("modo={modo} tamanho={tamanho} escritas_antes={antes}");

                let interpretado = celula_interpretada(modo, tamanho, antes).unwrap_or_else(|| {
                    panic!("{rotulo}: interpretador não terminou dentro do limite (deadlock)")
                });
                let nativo = celula_nativa(&binario, modo, tamanho, antes).unwrap_or_else(|| {
                    panic!("{rotulo}: nativo não terminou dentro do limite (deadlock)")
                });

                exigir_invariantes(&format!("interpretador {rotulo}"), &interpretado);
                exigir_invariantes(&format!("nativo {rotulo}"), &nativo);

                let corrida = celula_decidida_por_corrida(modo, tamanho);
                assert_eq!(
                    interpretado.classe_comparavel(corrida),
                    nativo.classe_comparavel(corrida),
                    "{rotulo}: back-ends divergem\ninterpretador: exit {:?} / {}\nnativo: exit {:?} / {}",
                    interpretado.codigo,
                    interpretado.stderr,
                    nativo.codigo,
                    nativo.stderr
                );
                assert_eq!(
                    interpretado.stdout_comparavel(corrida),
                    nativo.stdout_comparavel(corrida),
                    "{rotulo}: stdout divergente entre back-ends"
                );
            }
        }
    }
}

/// A disposição instalada pelo runtime é confinada ao processo Pinker.
///
/// `SIG_IGN` sobrevive a `exec`, então esta célula existe para provar que o
/// filho ainda observa `SIG_DFL`. A leitura acontece num construtor de
/// `.init_array` do binário auxiliar, antes do `lang_start` da std — que
/// instalaria `SIG_IGN` e mascararia a medida. `codigo=0` significa `SIG_DFL`
/// herdado; `codigo=1` significaria `SIG_IGN` vazando para o filho.
#[test]
fn r5_filho_herda_disposicao_padrao_de_sigpipe() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let (_artifacts, binario) = compilar_nativo(&runtime_lib);

    for antes in ESCRITAS_ANTERIORES {
        let interpretado = celula_interpretada("sigpipe-disposicao", 16, antes)
            .expect("interpretador dentro do limite");
        let nativo = celula_nativa(&binario, "sigpipe-disposicao", 16, antes)
            .expect("nativo dentro do limite");
        for (rotulo, saida) in [("interpretador", &interpretado), ("nativo", &nativo)] {
            assert!(
                saida.stdout.contains("codigo=0"),
                "{rotulo} (escritas_antes={antes}): filho não herdou SIG_DFL para SIGPIPE\nstdout: {}",
                saida.stdout
            );
        }
    }
}
// @pinker-nav:end evidencia.hotfix.r5-sigpipe-ordem

// @pinker-nav:start evidencia.hotfix.r5-sigpipe-familias
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência de que a configuração explícita da disposição do filho vale para todas as famílias de subprocesso — executar_processo, capturar_stdout, capturar_stderr, executar_com_entrada e as duas pontas de pipeline_minimo — com paridade entre interpretador e nativo; da portabilidade da própria sonda, provando que o construtor de .init_array do auxiliar precede a inicialização da linguagem e que ele distingue SIG_DFL de SIG_IGN quando a disposição é forçada no pré-exec; e o guardião estrutural que exige o pre_exec dentro do construtor comum dos dois back-ends, já que a std também restaura SIGPIPE hoje e a remoção da configuração da Pinker não seria observável pelo filho.
/// Saída exigida do exemplo das famílias, linha a linha.
///
/// `0` é o código de saída da sonda para `SIG_DFL`; `SIG_DFL` é o rótulo textual
/// da mesma leitura, usado pelas famílias que devolvem texto.
const FAMILIAS_ESPERADAS: &str = "executar_processo=0\n\
     capturar_stdout=SIG_DFL\n\
     capturar_stderr=SIG_DFL\n\
     executar_com_entrada=0\n\
     pipeline_minimo=0\n";

/// Cria as duas cópias do auxiliar usadas como pontas de `pipeline_minimo`.
///
/// `pipeline_minimo(produtor, consumidor)` não aceita argumentos por processo,
/// então o papel é escolhido pelo nome do executável.
fn preparar_pontas_do_pipeline() -> (NativeArtifactDir, PathBuf, PathBuf) {
    let artifacts = NativeArtifactDir::create().expect("diretório marcado das pontas");
    let dir = artifacts.path();
    let produtor = dir.join("pinker_hf412_pipeline_produtor");
    let consumidor = dir.join("pinker_hf412_pipeline_consumidor");
    for destino in [&produtor, &consumidor] {
        let staging = destino.with_extension("staging");
        std::fs::copy(helper_filho(), &staging).expect("copiar auxiliar para staging");
        std::fs::rename(&staging, destino).expect("publicar auxiliar atomicamente");
    }
    (artifacts, produtor, consumidor)
}

#[test]
fn r5_todas_as_familias_configuram_a_disposicao_do_filho() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let (_build_artifacts, binario) = compilar_exemplo(&runtime_lib, EXEMPLO_FAMILIAS, "familias");
    let (_pipeline_artifacts, produtor, consumidor) = preparar_pontas_do_pipeline();

    let argumentos = [
        helper_filho().to_string(),
        produtor.display().to_string(),
        consumidor.display().to_string(),
    ];

    let mut interpretado = Command::new(env!("CARGO_BIN_EXE_pink"));
    interpretado.args(["--run", EXEMPLO_FAMILIAS, "--"]);
    interpretado.args(&argumentos);
    let interpretado = executar_com_limite(interpretado).expect("interpretador dentro do limite");

    let mut nativo = Command::new(binario.as_os_str());
    nativo.args(&argumentos);
    let nativo = executar_com_limite(nativo).expect("nativo dentro do limite");

    for (rotulo, saida) in [("interpretador", &interpretado), ("nativo", &nativo)] {
        assert_eq!(
            saida.stdout, FAMILIAS_ESPERADAS,
            "{rotulo}: alguma família não configurou a disposição do filho\nstderr: {}",
            saida.stderr
        );
        assert_eq!(saida.codigo, Some(0), "{rotulo}: saída inesperada");
    }
}

extern "C" {
    fn signal(signal: i32, handler: usize) -> usize;
}

const SIGPIPE: i32 = 13;
const SIG_DFL: usize = 0;
const SIG_IGN: usize = 1;
const SIG_ERR: usize = usize::MAX;

/// Dispara o auxiliar forçando a disposição de `SIGPIPE` no contexto
/// pré-`exec`, sem passar pelas APIs da Pinker.
///
/// É o controle da sonda: permite observar `SIG_DFL` e `SIG_IGN` no filho a
/// partir de uma causa conhecida, em vez de inferir a sensibilidade da medida.
fn auxiliar_com_disposicao_forcada(modo: &str, handler: usize) -> Option<i32> {
    let mut comando = Command::new(helper_filho());
    comando.arg(modo);
    // SAFETY: a closure roda no filho entre `fork` e `exec` e faz uma única
    // chamada a `signal(2)`, async-signal-safe pela POSIX, sem alocação, lock,
    // formatação ou acesso ao ambiente.
    unsafe {
        comando.pre_exec(move || {
            if signal(SIGPIPE, handler) == SIG_ERR {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    executar_com_limite(comando)
        .expect("auxiliar dentro do limite")
        .codigo
}

#[test]
fn r5_sonda_precede_a_inicializacao_da_linguagem() {
    // O modo de ordem compara a leitura do construtor de `.init_array` com uma
    // segunda leitura feita em `main`. A std instala `SIG_IGN` entre as duas,
    // então a divergência só existe se o construtor tiver mesmo rodado antes —
    // é prova positiva da ordem no binário usado pelos demais testes.
    let mut ordem = Command::new(helper_filho());
    ordem.arg("sigpipe-sonda-ordem");
    let ordem = executar_com_limite(ordem).expect("auxiliar dentro do limite");
    assert_eq!(
        ordem.codigo,
        Some(0),
        "a sonda precisa preceder a inicialização da linguagem \
         (1=leituras iguais, 2=combinação inesperada, 3=erro, 4=sonda não rodou)"
    );

    // Sensibilidade: a mesma sonda distingue as duas disposições que podem
    // atravessar `exec`. Handler personalizado não é alcançável por herança —
    // a POSIX exige que `exec` o reinicie para `SIG_DFL`.
    assert_eq!(
        auxiliar_com_disposicao_forcada("sigpipe-disposicao", SIG_DFL),
        Some(0),
        "a sonda precisa reportar SIG_DFL quando o pré-exec restaura a disposição padrão"
    );
    assert_eq!(
        auxiliar_com_disposicao_forcada("sigpipe-disposicao", SIG_IGN),
        Some(1),
        "a sonda precisa reportar SIG_IGN quando o pré-exec deixa a disposição do pai vazar"
    );
}
/// Recorta uma região cartografada de um arquivo, pelos marcadores de nav.
fn regiao_cartografada(arquivo: &str, chave: &str) -> String {
    let fonte = std::fs::read_to_string(arquivo).expect("ler fonte cartografada");
    let inicio = fonte
        .find(&format!("@pinker-nav:start {chave}"))
        .unwrap_or_else(|| panic!("região {chave} ausente em {arquivo}"));
    let fim = fonte
        .find(&format!("@pinker-nav:end {chave}"))
        .unwrap_or_else(|| panic!("fim da região {chave} ausente em {arquivo}"));
    fonte[inicio..fim].to_string()
}

/// Guardião estrutural da configuração explícita do filho.
///
/// A `std` do Rust também devolve `SIGPIPE` a `SIG_DFL` antes do `exec`, tanto
/// no caminho `fork`/`exec` quanto no de `posix_spawn`. Isso torna a
/// configuração da Pinker **indistinguível por observação do filho**: remover a
/// nossa preparação não muda o que o filho mede enquanto a std mantiver a dela.
///
/// É justamente por isso que o contrato não pode ficar só nela — e por isso a
/// evidência de que a Pinker o cumpre por conta própria precisa ser estrutural.
/// Este teste falha se o `pre_exec` sair do construtor comum, se ele passar a
/// instalar `SIG_IGN` em vez de `SIG_DFL`, ou se alguma família voltar a
/// construir `Command` fora do construtor comum.
#[test]
fn r5_configuracao_do_filho_e_explicita_e_centralizada() {
    let runtime = regiao_cartografada("runtime/pinker_rt/src/lib.rs", "runtime.processos.execucao");
    assert_eq!(
        runtime.matches("Command::new").count(),
        1,
        "todas as famílias do runtime precisam construir Command por comando_saneado"
    );
    assert!(
        runtime.contains("processo.pre_exec(|| restaurar_disposicao_padrao(SINAL_SIGPIPE))"),
        "comando_saneado precisa restaurar SIGPIPE no filho antes do exec"
    );

    let interpretador = regiao_cartografada(
        "src/interpreter.rs",
        "interpreter.hospedeiro.servicos-auxiliares",
    );
    assert_eq!(
        interpretador.matches("Command::new").count(),
        1,
        "todas as famílias do interpretador precisam construir Command por comando_de_processo"
    );
    assert!(
        interpretador.contains("command.pre_exec(|| restaurar_disposicao_padrao(SINAL_SIGPIPE))"),
        "comando_de_processo precisa restaurar SIGPIPE no filho antes do exec"
    );

    // A disposição instalada no filho é a padrão, nunca a do pai.
    for arquivo in ["runtime/pinker_rt/src/lib.rs", "src/interpreter.rs"] {
        let fonte = std::fs::read_to_string(arquivo).expect("ler fonte");
        assert!(
            fonte.contains("let anterior = unsafe { signal(sinal, SINAL_HANDLER_PADRAO) };"),
            "{arquivo}: a restauração precisa passar SIG_DFL, não a disposição do pai"
        );
    }
}
// @pinker-nav:end evidencia.hotfix.r5-sigpipe-familias
