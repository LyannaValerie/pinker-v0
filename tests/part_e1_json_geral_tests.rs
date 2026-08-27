mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.json.parte-e1-geral
// @pinker-nav:domain dados
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da Parte E1 com paridade interpretador × ELF nativo em cada caso: a matriz numérica diagonal prova que a mesma gramática projeta `i64` no modelo adulto e `u64` no recorte plano histórico, inclusive `i64::MAX + 1` e `u64::MAX`, que o adulto recusa e o legado preserva no parse e na emissão; o nesting recursivo é atravessado pelo mesmo mecanismo em duas árvores de formatos diferentes; `null` é nó JSON de primeira classe sem escalar Pinker; Unicode, escapes e pares surrogate têm contrato próprio; chave duplicada, lixo à direita e acessor com tag errada recusam; JSON externo malformado atravessa `Resultado` por `tentar` e `propagar?`; a serialização de objeto é determinística por ordem de chave e não pela ordem de inserção; e um workflow real read-only lê um schema versionado do próprio repositório. O controle positivo garante que a matriz não passa por falhar em tudo.

/// Modelo adulto: aceita, e o valor sai exato pela serialização.
const FONTE_ADULTO: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.emitir; trazer json.ler_resultado; trazer json.objeto_obter;

apelido ResJson = Resultado<ValorJson, verso>;

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    tentar ler_resultado(ler_caminho_verso(caminho)) {
        sucesso ResJson.Ok(raiz) {
            nova x: ValorJson = objeto_obter(raiz, "x");
            falar(emitir(x));
        }
        falha ResJson.Erro(m) {
            falar("ERRO");
        }
    }
    mimo 0;
}
"#;

/// Modelo adulto em lote: um caminho por argumento, uma linha por caso.
///
/// A matriz tem dez casos e a fonte é a mesma para todos — só o argv muda.
/// Executar dez vezes custaria dez pares fork/supervisor sob envelope, e essa
/// carga derruba asserções de wall-clock de suítes que medem timeout. Um
/// processo, dez casos, a mesma cobertura.
const FONTE_ADULTO_LOTE: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.emitir; trazer json.ler_resultado; trazer json.objeto_obter; trazer texto.tamanho;

apelido ResJson = Resultado<ValorJson, verso>;

carinho principal() -> bombom {
    nova muda i: bombom = 0;
    repetir {
        nova caminho: verso = argumento_ou(i, "");
        talvez tamanho(caminho) > 0 {
            tentar ler_resultado(ler_caminho_verso(caminho)) {
                sucesso ResJson.Ok(raiz) {
                    nova x: ValorJson = objeto_obter(raiz, "x");
                    falar(emitir(x));
                }
                falha ResJson.Erro(m) {
                    falar("ERRO");
                }
            }
        }
        i += 1;
    } ate i >= 16;
    mimo 0;
}
"#;

/// Recorte plano histórico: parse e emissão no domínio `u64`.
const FONTE_LEGADO: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.emitir_plano_bombom; trazer json.ler_plano_bombom;

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    nova dados: mapa<verso,bombom> = ler_plano_bombom(ler_caminho_verso(caminho));
    falar(emitir_plano_bombom(dados));
    mimo 0;
}
"#;

/// Nesting recursivo em duas árvores de formatos materialmente diferentes,
/// `null` em objeto e em lista, e ordem determinística de objeto.
const FONTE_ESTRUTURAL: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.como_logica; trazer json.como_verso; trazer json.emitir; trazer json.ler_resultado; trazer json.lista_obter; trazer json.lista_tamanho; trazer json.objeto_chaves; trazer json.objeto_obter; trazer json.objeto_tamanho; trazer json.objeto_tem; trazer json.tipo; trazer texto.bombom_para_verso;

apelido ResJson = Resultado<ValorJson, verso>;

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    tentar ler_resultado(ler_caminho_verso(caminho)) {
        sucesso ResJson.Ok(raiz) {
            // objeto -> lista -> objeto
            nova lista: ValorJson = objeto_obter(raiz, "arvore_a");
            nova primeiro: ValorJson = lista_obter(lista, 0);
            nova folha: ValorJson = objeto_obter(primeiro, "folha");
            falar(emitir(folha));

            // lista -> objeto -> lista
            nova outra: ValorJson = objeto_obter(raiz, "arvore_b");
            nova dentro: ValorJson = lista_obter(outra, 0);
            nova interna: ValorJson = objeto_obter(dentro, "itens");
            nova item: ValorJson = lista_obter(interna, 1);
            falar(emitir(item));

            // null é nó JSON de primeira classe, observável só pelo tipo
            nova vazio: ValorJson = objeto_obter(raiz, "vazio");
            talvez tipo(vazio) == TipoJson.Nulo {
                falar("nulo-objeto");
            }
            nova lista_com_nulo: ValorJson = objeto_obter(raiz, "lista_nula");
            nova nulo_em_lista: ValorJson = lista_obter(lista_com_nulo, 1);
            talvez tipo(nulo_em_lista) == TipoJson.Nulo {
                falar("nulo-lista");
            }
            falar(emitir(nulo_em_lista));

            // tipos e acessores escalares
            nova texto: ValorJson = objeto_obter(raiz, "texto");
            falar(como_verso(texto));
            nova bandeira_sim: ValorJson = objeto_obter(raiz, "verdade");
            talvez como_logica(bandeira_sim) {
                falar("verdadeiro");
            }
            nova bandeira_nao: ValorJson = objeto_obter(raiz, "falsidade");
            talvez como_logica(bandeira_nao) {
                falar("nao-deveria");
            }

            // objeto: tamanho, presença, chaves em ordem determinística
            nova ordem: ValorJson = objeto_obter(raiz, "ordem");
            falar(bombom_para_verso(objeto_tamanho(ordem)));
            talvez objeto_tem(ordem, "b") {
                falar("tem-b");
            }
            nova chaves: lista<verso> = objeto_chaves(ordem);
            para cada chave em chaves {
                falar(chave);
            }
            falar(emitir(ordem));

            // lista vazia e objeto vazio
            nova vazia: ValorJson = objeto_obter(raiz, "lista_vazia");
            falar(bombom_para_verso(lista_tamanho(vazia)));
            nova objeto_vazio: ValorJson = objeto_obter(raiz, "objeto_vazio");
            falar(emitir(objeto_vazio));
        }
        falha ResJson.Erro(m) {
            falar("ERRO");
            falar(m);
        }
    }
    mimo 0;
}
"#;

/// Unicode: multibyte cru, escapes curtos e par surrogate.
const FONTE_UNICODE: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.como_verso; trazer json.emitir; trazer json.ler_resultado; trazer json.objeto_obter;

apelido ResJson = Resultado<ValorJson, verso>;

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    tentar ler_resultado(ler_caminho_verso(caminho)) {
        sucesso ResJson.Ok(raiz) {
            nova cru: ValorJson = objeto_obter(raiz, "cru");
            falar(como_verso(cru));
            nova escapado: ValorJson = objeto_obter(raiz, "escapado");
            falar(como_verso(escapado));
            nova par: ValorJson = objeto_obter(raiz, "par");
            falar(como_verso(par));
            falar(emitir(raiz));
        }
        falha ResJson.Erro(m) {
            falar("ERRO");
        }
    }
    mimo 0;
}
"#;

/// Falha externa recuperável consumida por `propagar?`, e sucesso pelo mesmo
/// caminho. A propagação é do mecanismo geral — nada aqui é especial por ser
/// JSON.
const FONTE_PROPAGAR: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.ler_resultado; trazer json.objeto_tamanho; trazer texto.bombom_para_verso;

apelido ResJson = Resultado<ValorJson, verso>;

carinho raiz_de(caminho: verso) -> ResJson {
    propagar? ler_resultado(ler_caminho_verso(caminho)) como ResJson.Ok(raiz);
    mimo ResJson.Ok(raiz);
}

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    tentar raiz_de(caminho) {
        sucesso ResJson.Ok(r) {
            falar("ok");
            falar(bombom_para_verso(objeto_tamanho(r)));
        }
        falha ResJson.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}
"#;

/// Acessor com tag errada é erro de programa, não dado externo malformado.
const FONTE_TAG_ERRADA: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.como_verso; trazer json.ler_resultado; trazer json.objeto_obter;

apelido ResJson = Resultado<ValorJson, verso>;

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    tentar ler_resultado(ler_caminho_verso(caminho)) {
        sucesso ResJson.Ok(raiz) {
            nova x: ValorJson = objeto_obter(raiz, "x");
            falar(como_verso(x));
        }
        falha ResJson.Erro(m) { falar("erro"); }
    }
    mimo 0;
}
"#;

/// Workflow real read-only: lê um schema versionado do próprio repositório e
/// verifica fatos estruturais dele. Não é fixture inventada para a ocasião —
/// é o manifesto que a Trama Pinker já usa.
const FONTE_WORKFLOW_REAL: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_verso; trazer json.como_verso; trazer json.emitir; trazer json.ler_resultado; trazer json.lista_obter; trazer json.lista_tamanho; trazer json.objeto_obter; trazer json.objeto_tem; trazer texto.bombom_para_verso;

apelido ResJson = Resultado<ValorJson, verso>;

carinho principal() -> bombom {
    nova caminho: verso = argumento_ou(0, "ausente");
    tentar ler_resultado(ler_caminho_verso(caminho)) {
        sucesso ResJson.Ok(schema) {
            falar(como_verso(objeto_obter(schema, "title")));
            nova requeridos: ValorJson = objeto_obter(schema, "required");
            nova total: bombom = lista_tamanho(requeridos);
            falar(bombom_para_verso(total));
            talvez total > 0 {
                nova muda i: bombom = 0;
                repetir {
                    falar(como_verso(lista_obter(requeridos, i)));
                    i += 1;
                } ate i >= total;
            }
            nova propriedades: ValorJson = objeto_obter(schema, "properties");
            nova versao: ValorJson = objeto_obter(propriedades, "schema");
            nova constante: ValorJson = objeto_obter(versao, "const");
            falar(emitir(constante));
            talvez objeto_tem(propriedades, "source") {
                falar("tem-source");
            }
        }
        falha ResJson.Erro(m) {
            falar("ERRO");
            falar(m);
        }
    }
    mimo 0;
}
"#;

/// Serializa os testes desta suíte entre si.
///
/// Cada caso compila um ELF nativo (invocando `cc`/`ld`) e executa binários sob
/// envelope. `cargo test` roda os testes de um binário em threads paralelas e
/// os binários de teste entre si também em paralelo, então esta suíte sozinha
/// conseguia saturar a CPU e derrubar asserções de wall-clock de
/// `part_d_native_process_tests`, que mede timeout de processo.
///
/// A cobertura não muda: os mesmos casos rodam, um de cada vez. O custo é
/// tempo de parede desta suíte; o benefício é não roubar CPU de quem mede tempo.
fn em_serie() -> std::sync::MutexGuard<'static, ()> {
    static ORDEM: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ORDEM
        .lock()
        .unwrap_or_else(|envenenado| envenenado.into_inner())
}

fn escrever(dir: &NativeArtifactDir, nome: &str, conteudo: &str) -> PathBuf {
    let caminho = dir.path().join(nome);
    fs::write(&caminho, conteudo).expect("escrever fixture da Parte E1");
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
        .expect("executar interpretador da Parte E1 sob envelope")
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
        .expect("compilar Parte E1 sob envelope")
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
        .expect("executar ELF da Parte E1 sob envelope")
}

/// Resultado observável de uma execução, para comparação entre backends.
struct Observado {
    stdout: String,
    sucesso: bool,
}

/// Compila a fonte **uma vez** e devolve o caminho do ELF.
///
/// Compilar de novo por caso custaria uma invocação de `cc`/`ld` por linha da
/// matriz. Isso não aumenta o poder de detecção — a fonte é a mesma, só o argv
/// muda — e satura a CPU o bastante para derrubar asserções de wall-clock de
/// outras suítes que rodam em paralelo. Um build, muitos casos.
fn compilar_uma_vez(
    dir: &NativeArtifactDir,
    nome: &str,
    fonte: &str,
    runtime_lib: &Path,
) -> (PathBuf, PathBuf) {
    let fonte_path = escrever(dir, &format!("{nome}.pink"), fonte);
    let build = compilar_nativo(dir, &fonte_path, runtime_lib, &format!("e1-{nome}-build"));
    assert!(
        build.status.success(),
        "{nome}: build nativo falhou — nenhuma superfície JSON pode ficar sem dono nativo: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    (fonte_path, dir.path().join(nome))
}

/// Roda o par já compilado nos dois modos e exige stdout e desfecho idênticos.
///
/// Aceita casos que falham nos dois lados: paridade em recusa também é
/// paridade, e é o que separa "os dois recusam" de "um aceita e o outro não" —
/// exatamente a divergência histórica que a Parte E1 fecha.
fn paridade_compilada(nome: &str, fonte_path: &Path, elf: &Path, args: &[String]) -> Observado {
    let interpretado = rodar_interpretador(fonte_path, &format!("e1-{nome}-interpretador"), args);
    let nativo = rodar_nativo(elf, &format!("e1-{nome}-nativo"), args);

    let stdout_interpretador = String::from_utf8_lossy(&interpretado.stdout).into_owned();
    let stdout_nativo = String::from_utf8_lossy(&nativo.stdout).into_owned();
    assert_eq!(
        stdout_interpretador, stdout_nativo,
        "{nome}: stdout divergiu entre interpretador e nativo"
    );
    assert_eq!(
        interpretado.status.success(),
        nativo.status.success(),
        "{nome}: desfecho divergiu (interpretador {:?}, nativo {:?})",
        interpretado.status.code(),
        nativo.status.code()
    );
    Observado {
        stdout: stdout_interpretador,
        sucesso: interpretado.status.success(),
    }
}

/// Compila e roda num caso só, para quem tem uma fonte por teste.
fn paridade(
    dir: &NativeArtifactDir,
    nome: &str,
    fonte: &str,
    args: &[String],
    runtime_lib: &Path,
) -> Observado {
    let (fonte_path, elf) = compilar_uma_vez(dir, nome, fonte, runtime_lib);
    paridade_compilada(nome, &fonte_path, &elf, args)
}

/// Matriz numérica: uma gramática, duas projeções.
///
/// O caso decisivo é `i64::MAX + 1`, recusado pelo modelo adulto e preservado
/// pelo recorte plano. Se alguém fizer a gramática decidir o domínio antes da
/// projeção, a diagonal quebra aqui em vez de perder faixa em silêncio.
#[test]
fn matriz_numerica_adulto_i64_versus_legado_u64_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");

    // (nome, json, adulto_esperado, legado_esperado)
    // `None` no esperado significa recusa.
    let casos: &[(&str, &str, Option<&str>, Option<&str>)] = &[
        ("n1_zero", r#"{"x":0}"#, Some("0\n"), Some("{\"x\":0}\n")),
        (
            "n2_i64_max",
            r#"{"x":9223372036854775807}"#,
            Some("9223372036854775807\n"),
            Some("{\"x\":9223372036854775807}\n"),
        ),
        (
            "n3_i64_max_mais_um",
            r#"{"x":9223372036854775808}"#,
            None,
            Some("{\"x\":9223372036854775808}\n"),
        ),
        (
            "n4_u64_max",
            r#"{"x":18446744073709551615}"#,
            None,
            Some("{\"x\":18446744073709551615}\n"),
        ),
        (
            "n5_acima_de_u64",
            r#"{"x":18446744073709551616}"#,
            None,
            None,
        ),
        ("n6_negativo", r#"{"x":-1}"#, Some("-1\n"), None),
        (
            "n7_i64_min",
            r#"{"x":-9223372036854775808}"#,
            Some("-9223372036854775808\n"),
            None,
        ),
        (
            "n8_abaixo_de_i64_min",
            r#"{"x":-9223372036854775809}"#,
            None,
            None,
        ),
        ("n9_fracao", r#"{"x":1.5}"#, None, None),
        ("n10_expoente", r#"{"x":1e3}"#, None, None),
    ];

    for (nome, json, adulto, legado) in casos {
        let fixture = escrever(&dir, &format!("{nome}.json"), json);
        let args = vec![fixture.to_string_lossy().into_owned()];

        // Adulto: recusa aparece como `Resultado.Erro`, não como aborto.
        let observado = paridade(
            &dir,
            &format!("adulto_{nome}"),
            FONTE_ADULTO,
            &args,
            &runtime_lib,
        );
        assert!(
            observado.sucesso,
            "{nome}: o caminho adulto trata dado externo por Resultado, nunca abortando"
        );
        match adulto {
            Some(esperado) => assert_eq!(
                observado.stdout, *esperado,
                "{nome}: valor adulto divergiu da matriz"
            ),
            None => assert_eq!(
                observado.stdout, "ERRO\n",
                "{nome}: o adulto deveria recusar como falha recuperável"
            ),
        }

        // Legado: recusa é aborto, como sempre foi nesta superfície.
        let observado = paridade(
            &dir,
            &format!("legado_{nome}"),
            FONTE_LEGADO,
            &args,
            &runtime_lib,
        );
        match legado {
            Some(esperado) => {
                assert!(observado.sucesso, "{nome}: o recorte plano deveria aceitar");
                assert_eq!(
                    observado.stdout, *esperado,
                    "{nome}: o recorte plano perdeu valor no parse ou na emissão"
                );
            }
            None => assert!(
                !observado.sucesso,
                "{nome}: o recorte plano deveria recusar, e recusou aceitando: {}",
                observado.stdout
            ),
        }
    }
}

/// Nesting recursivo, `null`, escalares, ordem determinística e coleções
/// vazias, tudo com paridade.
#[test]
fn estrutura_recursiva_null_e_ordem_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");

    // Ordem de inserção deliberadamente inversa da ordem de chave: se a
    // serialização herdar iteração de host, a saída muda.
    let fixture = escrever(
        &dir,
        "estrutura.json",
        r#"{
            "arvore_a": [{"folha": 41}],
            "arvore_b": [{"itens": [10, 20]}],
            "vazio": null,
            "lista_nula": [1, null],
            "texto": "olá",
            "verdade": true,
            "falsidade": false,
            "ordem": {"c": 3, "a": 1, "b": 2},
            "lista_vazia": [],
            "objeto_vazio": {}
        }"#,
    );
    let args = vec![fixture.to_string_lossy().into_owned()];
    let observado = paridade(&dir, "estrutural", FONTE_ESTRUTURAL, &args, &runtime_lib);
    assert!(observado.sucesso, "estrutural: {}", observado.stdout);
    assert_eq!(
        observado.stdout,
        concat!(
            "41\n",                        // objeto -> lista -> objeto
            "20\n",                        // lista -> objeto -> lista
            "nulo-objeto\n",               // null em objeto
            "nulo-lista\n",                // null em lista
            "null\n",                      // null serializa como null
            "olá\n",                       // verso multibyte
            "verdadeiro\n",                // logica true; false não imprime
            "3\n",                         // json_objeto_tamanho
            "tem-b\n",                     // json_objeto_tem
            "a\nb\nc\n",                   // chaves em ordem de chave, não de inserção
            "{\"a\":1,\"b\":2,\"c\":3}\n", // serialização determinística
            "0\n",                         // lista vazia
            "{}\n",                        // objeto vazio
        )
    );
}

/// Unicode e escapes: nada é herdado de biblioteca host sem contrato.
#[test]
fn unicode_escapes_e_par_surrogate_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");

    let fixture = escrever(
        &dir,
        "unicode.json",
        r#"{"cru":"olá 😀","escapado":"a\tb\"c\\d\/e","par":"😀"}"#,
    );
    let args = vec![fixture.to_string_lossy().into_owned()];
    let observado = paridade(&dir, "unicode", FONTE_UNICODE, &args, &runtime_lib);
    assert!(observado.sucesso, "unicode: {}", observado.stdout);
    assert_eq!(
        observado.stdout,
        concat!(
            "olá 😀\n",
            "a\tb\"c\\d/e\n",
            "😀\n",
            // Reemissão canônica: `\/` volta como `/`, o par surrogate volta
            // como UTF-8 cru, e as chaves saem em ordem.
            "{\"cru\":\"olá 😀\",\"escapado\":\"a\\tb\\\"c\\\\d/e\",\"par\":\"😀\"}\n",
        )
    );
}

/// Recusas de dado externo: cada uma atravessa `Resultado`, nenhuma aborta.
#[test]
fn json_malformado_atravessa_resultado_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");

    let (fonte_mal, elf_mal) =
        compilar_uma_vez(&dir, "malformados", FONTE_ADULTO_LOTE, &runtime_lib);
    let mut caminhos: Vec<String> = Vec::new();
    for (nome, json) in [
        ("malformado", r#"{"x":}"#),
        ("lixo_a_direita", r#"{"x":1} lixo"#),
        ("chave_duplicada", r#"{"x":1,"x":2}"#),
        ("string_nao_terminada", r#"{"x":"aberta}"#),
        ("surrogate_isolado", r#"{"x":"\ud83d"}"#),
        ("surrogate_baixo_isolado", r#"{"x":"\udc00"}"#),
        ("escape_desconhecido", r#"{"x":"\q"}"#),
        ("controle_nao_escapado", "{\"x\":\"a\nb\"}"),
        ("profundidade", &("[".repeat(200) + &"]".repeat(200))),
    ] {
        caminhos.push(
            escrever(&dir, &format!("{nome}.json"), json)
                .to_string_lossy()
                .into_owned(),
        );
    }
    // Todos recusam, nenhum aborta: cabem no mesmo processo.
    let observado = paridade_compilada("malformados", &fonte_mal, &elf_mal, &caminhos);
    assert!(
        observado.sucesso,
        "dado externo malformado precisa ser valor, não aborto"
    );
    assert_eq!(
        observado.stdout,
        "ERRO\n".repeat(caminhos.len()),
        "todo documento malformado deveria recusar como falha recuperável"
    );
}

/// Controle positivo da matriz de recusa acima: um documento válido pelo mesmo
/// caminho produz sucesso. Sem isto, "tudo recusa" passaria despercebido.
#[test]
fn controle_positivo_do_caminho_recuperavel() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");
    let fixture = escrever(&dir, "valido.json", r#"{"x":7}"#);
    let args = vec![fixture.to_string_lossy().into_owned()];
    let observado = paridade(&dir, "controle_ok", FONTE_ADULTO, &args, &runtime_lib);
    assert!(observado.sucesso);
    assert_eq!(observado.stdout, "7\n");
}

/// `propagar?` e `tentar` funcionam pelo mecanismo geral, sem special-case.
#[test]
fn propagar_e_tentar_pelo_mecanismo_geral_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");

    let bom = escrever(&dir, "propagar_ok.json", r#"{"a":1,"b":2}"#);
    let observado = paridade(
        &dir,
        "propagar_ok",
        FONTE_PROPAGAR,
        &[bom.to_string_lossy().into_owned()],
        &runtime_lib,
    );
    assert!(observado.sucesso);
    assert_eq!(observado.stdout, "ok\n2\n");

    let ruim = escrever(&dir, "propagar_erro.json", r#"{"a":}"#);
    let observado = paridade(
        &dir,
        "propagar_erro",
        FONTE_PROPAGAR,
        &[ruim.to_string_lossy().into_owned()],
        &runtime_lib,
    );
    assert!(
        observado.sucesso,
        "a falha externa é valor: o programa termina normalmente"
    );
    assert!(
        observado.stdout.starts_with("erro\n"),
        "a causa precisa atravessar a propagação: {}",
        observado.stdout
    );
}

/// Tag errada é erro de programa e aborta nos dois backends — categoria
/// distinta de dado externo malformado.
#[test]
fn acessor_com_tag_errada_aborta_nos_dois_backends() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");
    let fixture = escrever(&dir, "tag.json", r#"{"x":1}"#);
    let observado = paridade(
        &dir,
        "tag_errada",
        FONTE_TAG_ERRADA,
        &[fixture.to_string_lossy().into_owned()],
        &runtime_lib,
    );
    assert!(
        !observado.sucesso,
        "ler número como verso é erro de programa, não falha recuperável"
    );
}

/// Workflow real read-only sobre um manifesto versionado do próprio
/// repositório — não uma fixture batizada de real.
#[test]
fn workflow_real_le_schema_versionado_do_repositorio() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let _ordem = em_serie();
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte E1");

    let schema =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".pinker/schemas/change-v1.schema.json");
    assert!(
        schema.exists(),
        "o workflow real depende de um manifesto que existe no repositório"
    );

    let observado = paridade(
        &dir,
        "workflow_real",
        FONTE_WORKFLOW_REAL,
        &[schema.to_string_lossy().into_owned()],
        &runtime_lib,
    );
    assert!(observado.sucesso, "workflow real: {}", observado.stdout);
    assert_eq!(
        observado.stdout,
        concat!(
            "Pinker change manifest (v1)\n",
            "4\n",
            "schema\n",
            "kind\n",
            "title\n",
            "status\n",
            "1\n",
            "tem-source\n",
        ),
        "o workflow real precisa ler o manifesto de verdade"
    );
}

/// Toda intrínseca JSON exposta precisa ter dono nativo.
///
/// Sem esta checagem, uma superfície nova cairia no fallback de "função Pinker
/// do usuário" no backend e reproduziria a divergência histórica: aceita pelo
/// interpretador, recusada pelo build nativo.
#[test]
fn toda_intrinseca_json_possui_dono_nativo() {
    use pinker_v0::valor_json;
    for nome in valor_json::ACESSORES {
        assert!(
            valor_json::simbolo_runtime(nome).is_some(),
            "intrínseca adulta sem símbolo de runtime: {nome}"
        );
    }
    for nome in [valor_json::plano::LER, valor_json::plano::EMITIR] {
        assert!(
            valor_json::simbolo_runtime(nome).is_some(),
            "intrínseca plana histórica sem símbolo de runtime: {nome}"
        );
    }
    // A falível tem dono declarado pela autoridade de superfícies falíveis.
    let superficie =
        pinker_v0::falha_operacional::superficie(pinker_v0::falha_operacional::LER_JSON_RESULTADO)
            .expect("ler_json_resultado é superfície falível registrada");
    assert_eq!(superficie.simbolo_runtime, "pinker_json_ler_resultado");
}
// @pinker-nav:end evidencia.json.parte-e1-geral
