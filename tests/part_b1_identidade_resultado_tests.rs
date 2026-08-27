mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::generic_identity::{specialization_name, GenericKind, GenericOrigin};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.erros.parte-b1-identidade-resultado
// @pinker-nav:domain erros
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da Parte B1: a identidade de `Resultado<T,E>` cujos discriminantes o runtime produz não pode ser reinterpretada por declaração do usuário. A matriz de rejeição cobre reordenação, renomeação, carga incompatível, aridade genérica diferente, forma estruturalmente idêntica, leque não genérico, alias, ninho, eterno e carinho, cada uma nos dois pontos de entrada (interpretador e build nativo) e nas duas ordens de texto, exigindo a mesma mensagem da autoridade e o span real da declaração. Casos próprios cobrem a identidade reivindicada em outro módulo, onde `trazer` é resolvido depois do parse, e o nome monomórfico forjado por um leque genérico de outro nome. Do outro lado, controles positivos provam que o valor builtin continua significando `Ok`/`Erro` com paridade interpretador × nativo, que a política `USER_WINS` da Fase 241 continua valendo para quem não produz valor de runtime e que uma colisão que preserva a taxonomia builtin é aceita — a regra protege o valor, não o nome.

/// Fonte que produz valores de `Resultado` pelo runtime nas duas variantes e os
/// consome por `tentar`, `propagar?` e `encaixe`.
///
/// argv: 0 = caminho existente, 1 = caminho ausente.
const FONTE_BUILTIN_COMPLETA: &str = r#"
pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_resultado;

apelido ResVV = Resultado<verso, verso>;

carinho ler(caminho: verso) -> ResVV {
    propagar? ler_caminho_resultado(caminho) como ResVV.Ok(conteudo);
    mimo ResVV.Ok(conteudo);
}

carinho principal() -> bombom {
    nova existe: verso = argumento_ou(0, "ausente");
    nova ausente: verso = argumento_ou(1, "ausente");

    tentar ler(existe) {
        sucesso ResVV.Ok(c) { falar("tentar-ok"); falar(c); }
        falha ResVV.Erro(m) { falar("tentar-erro"); falar(m); }
    }

    nova r: ResVV = ler(ausente);
    encaixe r {
        caso ResVV.Ok(c) { falar("encaixe-ok"); falar(c); }
        caso ResVV.Erro(m) { falar("encaixe-erro"); }
    }

    falar("fim");
    mimo 0;
}
"#;

/// Compatibilidade legítima: o usuário declara o próprio `Resultado` e **não**
/// produz nenhum valor pelo runtime. É exatamente a Fase 241, e continua válido.
const FONTE_USUARIO_SEM_RUNTIME: &str = r#"
pacote main;

leque Resultado { Ok(bombom), Erro(verso) }

carinho validar(a: bombom, ok: logica) -> Resultado {
    talvez ok {
        mimo Resultado.Ok(a);
    }
    mimo Resultado.Erro("invalido");
}

carinho principal() -> bombom {
    tentar validar(42, verdade) {
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

/// Compatibilidade legítima, forma genérica (Fase 240), também sem runtime.
const FONTE_USUARIO_GENERICO_SEM_RUNTIME: &str = r#"
pacote main;

leque Resultado<T, E> { Erro(E), Ok(T) }

apelido RBV = Resultado<bombom, verso>;

carinho principal() -> bombom {
    nova ok: RBV = RBV.Ok(7);
    encaixe ok {
        caso RBV.Ok(v) { falar(v); }
        caso RBV.Erro(m) { falar(m); }
    }
    mimo 0;
}
"#;

/// Corpo comum das fontes de conflito: produz um `Resultado` pelo runtime e não
/// faz mais nada com ele.
///
/// Deliberadamente mínimo. Consumir o valor exigiria nomear o tipo, e nomeá-lo
/// falharia por conta própria em vários dos casos — o programa seria recusado
/// por uma razão que não é a que está sendo medida. Aqui a **única** coisa que
/// pode dar errado é o conflito de identidade. O cenário completo, com consumo,
/// tem teste próprio em [`cenario_do_defeito_original_e_recusado_nas_duas_ordens`].
const USO_MINIMO: &str = r#"trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_resultado; carinho principal() -> bombom {
    ler_caminho_resultado(argumento_ou(0, "ausente"));
    falar("fim");
    mimo 0;
}"#;

/// Uma forma de o usuário reivindicar o nome que o runtime produz.
struct Conflito {
    nome: &'static str,
    declaracao: &'static str,
}

/// A matriz de reinterpretação exigida pela #474.
///
/// Cada linha é uma forma diferente de reivindicar o nome. Nenhuma pode compilar
/// quando o programa produz o valor pelo runtime — e a razão precisa ser a
/// mesma, porque o defeito é um só.
const CONFLITOS: &[Conflito] = &[
    // Variantes reordenadas: a forma que corrompe em silêncio hoje.
    Conflito {
        nome: "reordenadas",
        declaracao: "leque Resultado<T, E> { Erro(E), Ok(T) }",
    },
    // Variantes renomeadas: a taxonomia pública deixa de ser a do runtime.
    Conflito {
        nome: "renomeadas",
        declaracao: "leque Resultado<T, E> { Bom(T), Ruim(E) }",
    },
    // Renomeadas E reordenadas: corrompe hoje sem sequer parecer familiar.
    Conflito {
        nome: "renomeadas_e_reordenadas",
        declaracao: "leque Resultado<T, E> { Ruim(E), Bom(T) }",
    },
    // Cargas trocadas: hoje só falha na máquina, em tempo de execução.
    Conflito {
        nome: "cargas_incompativeis",
        declaracao: "leque Resultado<T, E> { Ok(E), Erro(T) }",
    },
    // Aridade genérica diferente: nem a forma do template se mantém.
    Conflito {
        nome: "aridade_generica_diferente",
        declaracao: "leque Resultado<T> { Ok(T), Erro(T) }",
    },
    // Estruturalmente idêntica ao predeclarado: rejeitada por decisão, não por
    // acidente. Aceitá-la seria congelar ABI por comparação de forma.
    Conflito {
        nome: "estruturalmente_identica",
        declaracao: "leque Resultado<T, E> { Ok(T), Erro(E) }",
    },
    // Leque não genérico (forma da Fase 223) no programa que produz runtime.
    Conflito {
        nome: "nao_generico",
        declaracao: "leque Resultado { Ok(verso), Erro(verso) }",
    },
    // Indireção por alias: o nome vira outra coisa e o discriminante vazaria cru.
    Conflito {
        nome: "apelido",
        declaracao: "apelido Resultado = bombom;",
    },
    // Outras categorias de item que também reivindicam o nome.
    Conflito {
        nome: "ninho",
        declaracao: "ninho Resultado { campo: bombom; }",
    },
    Conflito {
        nome: "eterno",
        declaracao: "eterno Resultado: bombom = 1;",
    },
    Conflito {
        nome: "carinho",
        declaracao: "carinho Resultado() -> bombom { mimo 1; }",
    },
];

impl Conflito {
    /// Declaração **antes** do primeiro uso da superfície falível.
    fn fonte_antes(&self) -> String {
        format!("pacote main;\n\n{}\n\n{USO_MINIMO}\n", self.declaracao)
    }

    /// Declaração **depois** do uso/materialização. Mesmo programa, outra ordem.
    fn fonte_depois(&self) -> String {
        format!("pacote main;\n\n{USO_MINIMO}\n\n{}\n", self.declaracao)
    }
}

/// Controle positivo da matriz de recusa: sem a declaração conflitante, o mesmo
/// programa compila e roda nos dois modos. Sem isto, a matriz poderia passar por
/// recusar tudo.
const FONTE_SEM_DECLARACAO: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_resultado;

carinho principal() -> bombom {
    ler_caminho_resultado(argumento_ou(0, "ausente"));
    falar("fim");
    mimo 0;
}
"#;

/// O cenário exato do defeito preservado pela Parte C: o valor é produzido pelo
/// runtime, consumido por `tentar`, e a declaração do usuário inverte as
/// variantes. Antes desta Task, esta fonte imprimia `erro` seguido do conteúdo
/// do arquivo lido com sucesso.
const DEFEITO_DECLARACAO_ANTES: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_resultado;

leque Resultado<T, E> { Erro(E), Ok(T) }

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler_caminho_resultado(argumento_ou(0, "ausente")) {
        sucesso ResVV.Ok(c) { falar("ok"); falar(c); }
        falha ResVV.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}
"#;

/// O mesmo cenário com a declaração **depois** do uso. Antes desta Task esta
/// ordem se comportava corretamente — a prova de que o significado do valor
/// dependia da posição no texto.
const DEFEITO_DECLARACAO_DEPOIS: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_resultado;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler_caminho_resultado(argumento_ou(0, "ausente")) {
        sucesso ResVV.Ok(c) { falar("ok"); falar(c); }
        falha ResVV.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}

leque Resultado<T, E> { Erro(E), Ok(T) }
"#;

/// A mensagem exigida vem da autoridade, não de uma cópia literal neste teste.
/// Se a política mudar de lugar ou de texto, este arquivo acompanha sozinho.
fn mensagem_esperada() -> String {
    pinker_v0::falha_operacional::conflito_de_identidade(
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
    )
}

/// Mensagem da checagem sobre o programa completo (módulos e nome monomórfico).
/// Só o prefixo estável: o detalhe descreve a divergência de cada caso.
fn mensagem_taxonomia() -> String {
    let superficie = pinker_v0::falha_operacional::superficie("ler_arquivo_resultado")
        .expect("superfície de filesystem registrada");
    let completa =
        pinker_v0::falha_operacional::conflito_de_taxonomia(&superficie.leque_monomorfico(), "");
    completa
        .split_once(": ")
        .expect("mensagem tem detalhe separado por ': '")
        .0
        .to_string()
}

fn escrever_caso(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).expect("escrever fonte Parte B1");
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
        .expect("executar interpretador Parte B1 sob envelope")
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
        .expect("compilar Parte B1 sob envelope")
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
        .expect("executar ELF Parte B1 sob envelope")
}

/// Roda a mesma fonte nos dois modos e exige stdout/exit idênticos.
fn paridade(nome: &str, fonte: &str, args: &[String], runtime_lib: &Path, stdout_esperado: &str) {
    let dir = NativeArtifactDir::create().expect("diretório nativo Parte B1");
    paridade_em(&dir, nome, fonte, args, runtime_lib, stdout_esperado);
}

/// Igual, num diretório já preparado pelo chamador — necessário quando o caso
/// tem módulos vizinhos ou fixtures que precisam existir antes da compilação.
fn paridade_em(
    dir: &NativeArtifactDir,
    nome: &str,
    fonte: &str,
    args: &[String],
    runtime_lib: &Path,
    stdout_esperado: &str,
) {
    let fonte_path = escrever_caso(dir, nome, fonte);

    let interpretado =
        rodar_interpretador(&fonte_path, &format!("parte-b1-{nome}-interpretador"), args);
    let build = compilar_nativo(
        dir,
        &fonte_path,
        runtime_lib,
        &format!("parte-b1-{nome}-build"),
    );
    assert!(
        build.status.success(),
        "build nativo Parte B1 falhou em {nome}: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nativo = rodar_nativo(
        &dir.path().join(nome),
        &format!("parte-b1-{nome}-nativo"),
        args,
    );

    let stdout_interpretador = String::from_utf8_lossy(&interpretado.stdout).into_owned();
    let stdout_nativo = String::from_utf8_lossy(&nativo.stdout).into_owned();
    assert_eq!(
        stdout_interpretador, stdout_esperado,
        "{nome}: stdout do interpretador"
    );
    assert_eq!(stdout_nativo, stdout_esperado, "{nome}: stdout do nativo");
    assert_eq!(
        interpretado.status.code(),
        Some(0),
        "{nome}: interpretador deveria terminar com exit 0 (stderr: {})",
        String::from_utf8_lossy(&interpretado.stderr)
    );
    assert_eq!(
        nativo.status.code(),
        Some(0),
        "{nome}: nativo deveria terminar com exit 0 (stderr: {})",
        String::from_utf8_lossy(&nativo.stderr)
    );
}

/// Exige que a fonte seja recusada nos dois pontos de entrada, com a mensagem da
/// autoridade. Devolve o diagnóstico do interpretador para comparações de ordem.
fn exigir_recusa(nome: &str, fonte: &str, runtime_lib: &Path) -> String {
    exigir_recusa_com(
        nome,
        &[("principal", fonte)],
        runtime_lib,
        &mensagem_esperada(),
    )
}

/// Igual, mas para um programa de vários arquivos e/ou outra mensagem da
/// autoridade. O primeiro módulo da lista é a raiz compilada; os demais são
/// escritos ao lado dela para que `trazer` os encontre.
fn exigir_recusa_com(
    nome: &str,
    modulos: &[(&str, &str)],
    runtime_lib: &Path,
    esperada: &str,
) -> String {
    let dir = NativeArtifactDir::create().expect("diretório recusa Parte B1");
    let (_, raiz_fonte) = modulos[0];
    for (modulo, fonte) in &modulos[1..] {
        fs::write(dir.path().join(format!("{modulo}.pink")), fonte)
            .expect("escrever módulo Parte B1");
    }
    let fonte_path = escrever_caso(&dir, nome, raiz_fonte);

    let interpretado = rodar_interpretador(
        &fonte_path,
        &format!("parte-b1-recusa-{nome}-interpretador"),
        &[],
    );
    let diag_interpretador = String::from_utf8_lossy(&interpretado.stderr).into_owned();
    assert_eq!(
        interpretado.status.code(),
        Some(1),
        "{nome}: o interpretador deveria recusar o programa (stdout: {})",
        String::from_utf8_lossy(&interpretado.stdout)
    );
    assert!(
        diag_interpretador.contains(esperada),
        "{nome}: o interpretador recusou por outra razão:\n{diag_interpretador}"
    );

    let build = compilar_nativo(
        &dir,
        &fonte_path,
        runtime_lib,
        &format!("parte-b1-recusa-{nome}-build"),
    );
    let diag_nativo = String::from_utf8_lossy(&build.stderr).into_owned();
    assert!(
        !build.status.success(),
        "{nome}: o build nativo deveria recusar o programa"
    );
    assert!(
        diag_nativo.contains(esperada),
        "{nome}: o build nativo recusou por outra razão:\n{diag_nativo}"
    );

    // Nenhum ELF pode ter sido produzido: recusar é fechar, não avisar.
    assert!(
        !dir.path().join(nome).exists(),
        "{nome}: build recusado não pode deixar executável"
    );

    // O span precisa ser o da declaração do usuário. O predeclarado usa a
    // posição sintética 0:0, que nunca pode aparecer aqui.
    assert!(
        !diag_interpretador.contains(" em 0:0"),
        "{nome}: diagnóstico apontou o span sintético do predeclarado:\n{diag_interpretador}"
    );

    diag_interpretador
}

/// Pontos 4, 5, 6, 7, 8, 9 e 10: nenhuma forma de reivindicar o nome pode
/// reinterpretar um valor produzido pelo runtime — nem antes, nem depois do uso.
#[test]
fn declaracao_do_usuario_nao_reinterpreta_valor_produzido_pelo_runtime() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    // Controle positivo: sem a declaração conflitante o programa é aceito e
    // executa nos dois modos. A matriz abaixo não passa por recusar tudo.
    paridade(
        "sem_declaracao",
        FONTE_SEM_DECLARACAO,
        &["/etc/hostname".to_string()],
        &runtime_lib,
        "fim\n",
    );

    for conflito in CONFLITOS {
        exigir_recusa(
            &format!("{}_antes", conflito.nome),
            &conflito.fonte_antes(),
            &runtime_lib,
        );
        exigir_recusa(
            &format!("{}_depois", conflito.nome),
            &conflito.fonte_depois(),
            &runtime_lib,
        );
    }
}

/// A identidade reivindicada em **outro módulo**.
///
/// `trazer` é resolvido depois do parse, juntando itens de parsers distintos:
/// o módulo que chama a superfície e o módulo que redeclara o nome nunca são
/// vistos pelo mesmo parser. A conjunção por unidade de compilação não alcança
/// isto — quem alcança é a checagem sobre o programa já montado.
#[test]
fn identidade_reivindicada_em_outro_modulo_e_recusada() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    const MODULO_IO: &str = r#"pacote io; trazer arquivo.ler_caminho_resultado;

apelido ResVV = Resultado<verso, verso>;

carinho ler(c: verso) -> ResVV {
    mimo ler_caminho_resultado(c);
}
"#;

    const RAIZ_CONFLITANTE: &str = r#"pacote main; trazer ambiente.argumento_ou;

trazer io.ler;

leque Resultado<T, E> { Erro(E), Ok(T) }

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler(argumento_ou(0, "ausente")) {
        sucesso ResVV.Ok(c) { falar("ok"); falar(c); }
        falha ResVV.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}
"#;

    // A produção mora em `io`, a redeclaração mora na raiz: nenhum parser vê as
    // duas metades.
    exigir_recusa_com(
        "modulo_conflitante",
        &[("principal", RAIZ_CONFLITANTE), ("io", MODULO_IO)],
        &runtime_lib,
        &mensagem_taxonomia(),
    );

    // Controle positivo: os mesmos dois módulos sem a redeclaração compilam e
    // rodam nos dois modos, com o sucesso chegando como `Ok`.
    const RAIZ_LIMPA: &str = r#"pacote main; trazer ambiente.argumento_ou;

trazer io.ler;

apelido ResVV = Resultado<verso, verso>;

carinho principal() -> bombom {
    tentar ler(argumento_ou(0, "ausente")) {
        sucesso ResVV.Ok(c) { falar("ok"); falar(c); }
        falha ResVV.Erro(m) { falar("erro"); falar(m); }
    }
    mimo 0;
}
"#;

    let dir = NativeArtifactDir::create().expect("diretório módulos Parte B1");
    fs::write(dir.path().join("io.pink"), MODULO_IO).expect("escrever módulo io");
    let alvo = dir.path().join("conteudo.txt");
    fs::write(&alvo, "conteudo-real").expect("escrever alvo");
    paridade_em(
        &dir,
        "modulo_limpo",
        RAIZ_LIMPA,
        &[alvo.to_string_lossy().into_owned()],
        &runtime_lib,
        "ok\nconteudo-real\n",
    );
}

/// A colisão histórica com outro nome de template desaparece, enquanto a
/// defesa de taxonomia continua cobrindo a identidade runtime verdadeira.
#[test]
fn outro_template_antes_colidente_agora_coexiste_sem_receber_a_tag_runtime() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    const COEXISTENCIA: &str = r#"pacote main; trazer ambiente.argumento_ou; trazer arquivo.ler_caminho_resultado;

leque Resultado_verso<T> { Erro(T), Ok(T) }

apelido F = Resultado_verso<verso>;
apelido R = Resultado<verso, verso>;

carinho principal() -> bombom {
    nova usuario: F = F.Erro("usuario");
    encaixe usuario {
        caso F.Ok(c) { falar("usuario-ok"); falar(c); }
        caso F.Erro(m) { falar("usuario-erro"); falar(m); }
    }
    nova runtime: R = ler_caminho_resultado(argumento_ou(0, "ausente"));
    encaixe runtime {
        caso R.Ok(c) { falar("runtime-ok"); falar(c); }
        caso R.Erro(m) { falar("runtime-erro"); falar(m); }
    }
    mimo 0;
}
"#;
    let dir = NativeArtifactDir::create().expect("diretório colisão Parte B1");
    let alvo = dir.path().join("conteudo.txt");
    fs::write(&alvo, "conteudo-real").expect("escrever alvo");
    paridade_em(
        &dir,
        "identidades_agora_distintas",
        COEXISTENCIA,
        &[alvo.to_string_lossy().into_owned()],
        &runtime_lib,
        "usuario-erro\nusuario\nruntime-ok\nconteudo-real\n",
    );
}

/// O cenário completo do defeito — valor de runtime consumido por `tentar`, com
/// as variantes do usuário invertidas — é recusado nas duas ordens, pela mesma
/// razão. É a forma que a revisão humana da Parte C preservou como reprodutor.
#[test]
fn cenario_do_defeito_original_e_recusado_nas_duas_ordens() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    exigir_recusa("defeito_antes", DEFEITO_DECLARACAO_ANTES, &runtime_lib);
    exigir_recusa("defeito_depois", DEFEITO_DECLARACAO_DEPOIS, &runtime_lib);
}

/// A ordem no texto não pode mudar a identidade builtin.
///
/// Antes desta Task, a mesma declaração reordenada valia como corrupção
/// silenciosa quando vinha **antes** do uso e era inofensiva quando vinha
/// **depois**: o significado do valor dependia da posição textual. Agora as duas
/// ordens produzem o mesmo veredito e a mesma razão.
#[test]
fn ordem_no_texto_nao_muda_a_identidade_builtin() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let esperada = mensagem_esperada();
    for conflito in CONFLITOS {
        let antes = exigir_recusa(
            &format!("ordem_{}_antes", conflito.nome),
            &conflito.fonte_antes(),
            &runtime_lib,
        );
        let depois = exigir_recusa(
            &format!("ordem_{}_depois", conflito.nome),
            &conflito.fonte_depois(),
            &runtime_lib,
        );

        // Mesma razão nas duas ordens. O span difere — e deve diferir: ele
        // aponta a declaração do usuário, que mudou de lugar.
        assert!(
            antes.contains(&esperada) && depois.contains(&esperada),
            "{}: as duas ordens deveriam falhar pela mesma razão",
            conflito.nome
        );
    }
}

/// Pontos 1, 2, 3, 11, 12, 13, 14, 15 e 16: o valor builtin continua tendo uma
/// única interpretação, compõe com `tentar`/`propagar?`/`encaixe` e os dois
/// backends concordam.
#[test]
fn valor_builtin_mantem_uma_unica_interpretacao_com_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório fixture Parte B1");
    let existente = dir.path().join("conteudo.txt");
    fs::write(&existente, "conteudo-real").expect("escrever arquivo de sucesso");
    let ausente = dir.path().join("nao-existe.txt");

    let args = vec![
        existente.to_string_lossy().into_owned(),
        ausente.to_string_lossy().into_owned(),
    ];
    // O sucesso chega como `Ok` e a falha como `Erro`. A causa da falha não é
    // impressa: o que se prova aqui é a variante escolhida, não o texto dela —
    // que já tem cobertura própria na Parte B.
    paridade(
        "builtin_completa",
        FONTE_BUILTIN_COMPLETA,
        &args,
        &runtime_lib,
        "tentar-ok\nconteudo-real\nencaixe-erro\nfim\n",
    );
}

/// Ponto 17: a política da Fase 241 continua valendo onde ela sempre foi
/// legítima — programas que declaram o próprio `Resultado` e não produzem
/// nenhum valor pelo runtime. Inclusive com as variantes em ordem invertida,
/// que é exatamente a forma proibida quando há valor de runtime.
#[test]
fn user_wins_preservado_quando_nao_ha_valor_de_runtime() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    paridade(
        "usuario_sem_runtime",
        FONTE_USUARIO_SEM_RUNTIME,
        &[],
        &runtime_lib,
        "42\ninvalido\n",
    );
    paridade(
        "usuario_generico_sem_runtime",
        FONTE_USUARIO_GENERICO_SEM_RUNTIME,
        &[],
        &runtime_lib,
        "7\n",
    );
}

/// A identidade produzida pelo runtime é derivada das superfícies, não de uma
/// lista literal paralela: abrir uma superfície nova não pode deixar a política
/// para trás.
#[test]
fn identidade_reservada_e_derivada_das_superficies() {
    let identidades: Vec<&str> =
        pinker_v0::falha_operacional::identidades_produzidas_pelo_runtime().collect();
    assert_eq!(
        identidades.len(),
        pinker_v0::falha_operacional::SUPERFICIES_FALIVEIS.len(),
        "toda superfície falível precisa declarar a identidade que produz"
    );
    for superficie in pinker_v0::falha_operacional::SUPERFICIES_FALIVEIS {
        assert!(
            pinker_v0::falha_operacional::identidade_produzida_pelo_runtime(
                superficie.identidade()
            ),
            "a identidade de '{}' não é reconhecida pela própria autoridade",
            superficie.intrinseca
        );
        assert_eq!(
            superficie.leque_monomorfico(),
            specialization_name(
                GenericKind::Enum,
                &GenericOrigin::Builtin,
                superficie.identidade(),
                &superficie.argumentos_de_tipo(pinker_v0::falha_operacional::span_sintetico()),
            ),
            "a superfície '{}' desviou da autoridade compartilhada",
            superficie.intrinseca
        );
    }

    // Controle negativo: um nome qualquer não é reservado.
    assert!(
        !pinker_v0::falha_operacional::identidade_produzida_pelo_runtime("Resultado2"),
        "a política não pode reservar nomes que o runtime não produz"
    );
    assert!(
        !pinker_v0::falha_operacional::identidade_produzida_pelo_runtime("TipoEntrada"),
        "a política não pode reservar nomes que o runtime não produz"
    );

    // A taxonomia que a autoridade declara precisa ser a que o parser realmente
    // predeclara e monomorfiza. Sem isto, `variantes_canonicas` seria uma opinião
    // isolada: `taxonomia_divergente` compararia contra algo que o compilador não
    // constrói, e a checagem passaria a medir a si mesma.
    let programa = common::parse(
        r#"
        pacote main;
        apelido RVV = Resultado<verso, verso>;
        carinho principal() -> bombom { mimo 0; }
        "#,
    )
    .expect("programa válido");
    let superficie = pinker_v0::falha_operacional::superficie("ler_arquivo_resultado")
        .expect("superfície de filesystem registrada");
    let materializado = programa
        .items
        .iter()
        .find_map(|item| match item {
            pinker_v0::ast::Item::Enum(decl) if decl.name == superficie.leque_monomorfico() => {
                Some(decl)
            }
            _ => None,
        })
        .expect("o parser materializa a especialização do predeclarado");
    assert_eq!(
        superficie.taxonomia_divergente(materializado),
        None,
        "a taxonomia declarada pela autoridade divergiu da que o parser predeclara"
    );
    let nomes: Vec<&str> = materializado
        .variants
        .iter()
        .map(|variante| variante.name.as_str())
        .collect();
    assert_eq!(
        nomes,
        vec![
            pinker_v0::falha_operacional::VARIANTE_OK,
            pinker_v0::falha_operacional::VARIANTE_ERRO
        ],
        "a ordem das variantes é o que define TAG_OK/TAG_ERRO"
    );
    assert_eq!(pinker_v0::falha_operacional::TAG_OK, 0);
    assert_eq!(pinker_v0::falha_operacional::TAG_ERRO, 1);
}

/// A política de identidade tem uma autoridade só.
///
/// Provar pelo diff que nenhuma camada redecide "quem possui este nome" é
/// argumento de revisão, não evidência. Este teste falha se a mensagem do
/// conflito for reconstruída fora de `src/falha_operacional.rs`.
#[test]
fn politica_de_identidade_existe_so_na_autoridade() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let autoridade = raiz.join("src/falha_operacional.rs");

    // Controle positivo: a autoridade contém mesmo o que se procura fora dela.
    let fonte_autoridade = fs::read_to_string(&autoridade).expect("autoridade legível");
    assert!(
        fonte_autoridade.contains("identidade de resultado produzida pelo runtime"),
        "a mensagem do conflito não está na autoridade"
    );

    const MARCA: &str = "identidade de resultado produzida pelo runtime";
    let mut duplicatas = Vec::new();
    for raiz_varrida in ["src", "runtime"] {
        for arquivo in arquivos_rust(&raiz.join(raiz_varrida)) {
            if arquivo == autoridade {
                continue;
            }
            let fonte = fs::read_to_string(&arquivo).expect("fonte legível");
            if fonte.contains(MARCA) {
                duplicatas.push(arquivo.display().to_string());
            }
        }
    }
    assert!(
        duplicatas.is_empty(),
        "política de identidade duplicada fora de src/falha_operacional.rs:\n  {}",
        duplicatas.join("\n  ")
    );

    // O parser impõe a política, mas não a redefine: consulta a autoridade.
    let parser = fs::read_to_string(raiz.join("src/parser.rs")).expect("parser legível");
    assert!(
        parser.contains("falha_operacional::identidade_produzida_pelo_runtime")
            && parser.contains("falha_operacional::conflito_de_identidade"),
        "o parser deixou de derivar a política da autoridade"
    );
}

/// A propagação continua cega à origem do valor: a correção da identidade não
/// pode ter introduzido caminho por operação.
#[test]
fn correcao_de_identidade_nao_criou_fluxo_por_operacao() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fonte = fs::read_to_string(raiz.join("src/parser.rs")).expect("src/parser.rs legível");

    const INICIO: &str = "// @pinker-nav:start parser.resultado.tentar-propagar";
    const FIM: &str = "// @pinker-nav:end parser.resultado.tentar-propagar";
    let i = fonte.find(INICIO).expect("região do desugaring presente");
    let f = fonte
        .find(FIM)
        .expect("fim da região do desugaring presente");
    assert!(i < f, "marcadores da região de propagação fora de ordem");
    let regiao = &fonte[i..f];

    for superficie in pinker_v0::falha_operacional::SUPERFICIES_FALIVEIS {
        assert!(
            !regiao.contains(superficie.intrinseca),
            "o desugaring de propagação passou a conhecer '{}'",
            superficie.intrinseca
        );
    }
    assert!(
        !regiao.contains("identidade_produzida_pelo_runtime"),
        "a política de identidade vazou para o desugaring de propagação"
    );
}

fn arquivos_rust(raiz: &Path) -> Vec<PathBuf> {
    let mut encontrados = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(atual) = pilha.pop() {
        let Ok(entradas) = fs::read_dir(&atual) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            if caminho.is_dir() {
                if caminho.file_name().is_some_and(|nome| nome == "target") {
                    continue;
                }
                pilha.push(caminho);
            } else if caminho.extension().is_some_and(|ext| ext == "rs") {
                encontrados.push(caminho);
            }
        }
    }
    encontrados.sort();
    encontrados
}

// @pinker-nav:end evidencia.erros.parte-b1-identidade-resultado
