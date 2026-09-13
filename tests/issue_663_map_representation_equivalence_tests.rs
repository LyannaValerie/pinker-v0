mod common;

// @pinker-nav:start evidencia.tipos.mapa-representacao-equivalente
// @pinker-nav:domain tipos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência F-04: duas grafias do mesmo mapa resolvido — a literal, dobrada pelo parser numa das quatro variantes históricas, e a que passa por apelido e sobrevive como `Type::Map` — respondem igual. Cobre as quatro classes com apelido na chave, no valor e nos dois, nas duas direções; a invariância metamórfica da compatibilidade sob troca de representação contra terceiros tipos, incluindo a compatibilidade `bombom`/`u64` que só o mapa genérico alcançava; os controles negativos que precisam continuar recusados, o valor de leque entre eles; a concordância entre identidade canônica de união e compatibilidade, com a separação entre identidade exata e compatibilidade; a fronteira preexistente do par numericamente compatível, que para no mesmo lugar em qualquer grafia; e a execução real das operações de mapa no caminho recém-aceito, com paridade interpretador/nativo.

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::union_canon;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Domínio escrito à mão
// ---------------------------------------------------------------------------
//
// Nada aqui é produzido pela autoridade sob prova. As grafias, as classes e a
// noção de "mesma coisa escrita de outro jeito" são literais desta suíte.

/// Prelúdio comum: os apelidos que fazem o parser desistir de dobrar o mapa.
const PRELUDIO: &str = r#"
apelido AA = verso;
apelido BB = bombom;
leque Cor { Rosa, Azul }
"#;

/// Uma classe histórica de mapa e as QUATRO grafias que a denotam.
///
/// A primeira é a literal, que o parser dobra na variante histórica. As outras
/// três passam por apelido — na chave, no valor e nos dois — e sobrevivem à
/// resolução como o mapa genérico adulto.
struct Classe {
    literal: &'static str,
    apelido_na_chave: &'static str,
    apelido_no_valor: &'static str,
    apelidos_nos_dois: &'static str,
}

impl Classe {
    fn grafias(&self) -> [&'static str; 4] {
        [
            self.literal,
            self.apelido_na_chave,
            self.apelido_no_valor,
            self.apelidos_nos_dois,
        ]
    }
}

const CLASSES: [Classe; 4] = [
    Classe {
        literal: "mapa<verso,bombom>",
        apelido_na_chave: "mapa<AA,bombom>",
        apelido_no_valor: "mapa<verso,BB>",
        apelidos_nos_dois: "mapa<AA,BB>",
    },
    Classe {
        literal: "mapa<verso,verso>",
        apelido_na_chave: "mapa<AA,verso>",
        apelido_no_valor: "mapa<verso,AA>",
        apelidos_nos_dois: "mapa<AA,AA>",
    },
    Classe {
        literal: "mapa<bombom,bombom>",
        apelido_na_chave: "mapa<BB,bombom>",
        apelido_no_valor: "mapa<bombom,BB>",
        apelidos_nos_dois: "mapa<BB,BB>",
    },
    Classe {
        literal: "mapa<bombom,verso>",
        apelido_na_chave: "mapa<BB,verso>",
        apelido_no_valor: "mapa<bombom,AA>",
        apelidos_nos_dois: "mapa<BB,AA>",
    },
];

/// Terceiros tipos contra os quais a invariância é medida.
///
/// Alguns são compatíveis com alguma classe por política de componente
/// preexistente (`bombom`/`u64`), outros não são compatíveis com nenhuma. A
/// suíte não declara quais: ela exige que a resposta não dependa da grafia.
const TERCEIROS: [&str; 8] = [
    "mapa<verso,u64>",
    "mapa<bombom,u64>",
    "mapa<verso,logica>",
    "mapa<verso,Cor>",
    "mapa<bombom,Cor>",
    "lista<bombom>",
    "verso",
    "bombom",
];

/// Como se produz um valor de cada tipo do domínio.
fn inicializador(tipo: &str) -> &'static str {
    if tipo.starts_with("mapa<") {
        "criar()"
    } else if tipo == "lista<bombom>" {
        "lista.bombom_criar()"
    } else if tipo == "verso" {
        "\"x\""
    } else if tipo == "bombom" {
        "1"
    } else {
        panic!("tipo fora do domínio desta suíte: {tipo}");
    }
}

/// Programa mínimo cuja aceitação É a relação de compatibilidade observada:
/// o argumento tem o tipo `actual` e o parâmetro tem o tipo `expected`.
fn programa(expected: &str, actual: &str) -> String {
    format!(
        r#"pacote demo;

trazer mapa.criar;
trazer lista;
{PRELUDIO}
carinho usa(m: {expected}) -> bombom {{
    mimo 0;
}}

carinho principal() -> bombom {{
    nova valor: {actual} = {init};
    mimo usa(valor);
}}
"#,
        init = inicializador(actual)
    )
}

/// `M(expected, actual)` como o produto responde, sem intermediário.
fn compativel(expected: &str, actual: &str) -> bool {
    common::parse_and_check(&programa(expected, actual)).is_ok()
}

/// O mesmo programa, mas exercitando o mapa: sem operação real, o lowering não
/// tem o que materializar e a fronteira de execução nunca é alcançada.
fn programa_com_operacao(expected: &str, actual: &str) -> String {
    format!(
        r#"pacote demo;

trazer mapa.criar, definir, tamanho;
{PRELUDIO}
carinho usa(m: {expected}) -> bombom {{
    mimo tamanho(m);
}}

carinho principal() -> bombom {{
    nova valor: {actual} = criar();
    definir(valor, "a", 1);
    mimo usa(valor);
}}
"#
    )
}

// ---------------------------------------------------------------------------
// Obrigação 1 — equivalência exata
// ---------------------------------------------------------------------------

#[test]
fn as_quatro_grafias_de_cada_classe_sao_compativeis_nas_duas_direcoes() {
    for classe in &CLASSES {
        for esquerda in classe.grafias() {
            for direita in classe.grafias() {
                assert!(
                    compativel(esquerda, direita),
                    "esperado '{esquerda}' deve aceitar '{direita}': é o mesmo mapa resolvido"
                );
                assert!(
                    compativel(direita, esquerda),
                    "esperado '{direita}' deve aceitar '{esquerda}': é o mesmo mapa resolvido"
                );
            }
        }
    }
}

#[test]
fn apelido_na_chave_no_valor_e_nos_dois_sao_cobertos_separadamente() {
    // O mesmo fato das quatro grafias, escrito posição a posição: uma correção
    // que consertasse só a chave, só o valor ou só a forma sem apelido ficaria
    // vermelha em exatamente uma destas linhas.
    for classe in &CLASSES {
        for variante in [
            classe.apelido_na_chave,
            classe.apelido_no_valor,
            classe.apelidos_nos_dois,
        ] {
            assert!(
                compativel(classe.literal, variante),
                "literal '{}' deve aceitar '{variante}'",
                classe.literal
            );
            assert!(
                compativel(variante, classe.literal),
                "'{variante}' deve aceitar o literal '{}'",
                classe.literal
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Obrigação 2 — independência de representação
// ---------------------------------------------------------------------------

#[test]
fn trocar_so_a_representacao_nao_muda_a_compatibilidade_contra_terceiros() {
    for classe in &CLASSES {
        let grafias = classe.grafias();
        for terceiro in TERCEIROS {
            let referencia_direta = compativel(terceiro, grafias[0]);
            let referencia_inversa = compativel(grafias[0], terceiro);
            for equivalente in grafias.iter().skip(1) {
                assert_eq!(
                    compativel(terceiro, equivalente),
                    referencia_direta,
                    "M('{terceiro}', X) mudou ao trocar '{}' por '{equivalente}'",
                    grafias[0]
                );
                assert_eq!(
                    compativel(equivalente, terceiro),
                    referencia_inversa,
                    "M(X, '{terceiro}') mudou ao trocar '{}' por '{equivalente}'",
                    grafias[0]
                );
            }
        }
    }
}

#[test]
fn a_compatibilidade_bombom_u64_do_mapa_sobrevive_nas_duas_grafias() {
    // A política numérica de componente é preexistente e não muda aqui: o que
    // muda é que ela deixa de depender de qual grafia o operando usou. No
    // baseline, `mapa<verso,u64>` aceitava a forma com apelido e recusava a
    // literal — mesmo mapa resolvido, respostas opostas.
    //
    // Esta asserção é de ACEITAÇÃO SEMÂNTICA, e só disso. Passar um mapa a um
    // parâmetro cujo valor tem outra representação continua sem baixar — o
    // limite é preexistente e está fixado, também de forma independente de
    // grafia, em `o_par_numericamente_compativel_para_no_mesmo_lugar`.
    for grafia in CLASSES[0].grafias() {
        assert!(
            compativel("mapa<verso,u64>", grafia),
            "'mapa<verso,u64>' deve aceitar '{grafia}' pela compatibilidade de componente"
        );
        assert!(
            compativel(grafia, "mapa<verso,u64>"),
            "'{grafia}' deve aceitar 'mapa<verso,u64>' pela compatibilidade de componente"
        );
    }
    for grafia in CLASSES[2].grafias() {
        assert!(
            compativel("mapa<bombom,u64>", grafia),
            "'mapa<bombom,u64>' deve aceitar '{grafia}'"
        );
    }
}

/// Onde o par numericamente compatível para — e que ele para no MESMO lugar,
/// qualquer que seja a grafia.
///
/// Compatibilidade de componente não é identidade de armazenamento: um mapa
/// cujo valor é `bombom` não tem a representação de um cujo valor é `u64`, e
/// nenhuma camada abaixo da semântica converte containers. Esse limite é
/// preexistente e source-reachable no baseline — lá `mapa<verso,u64>` já
/// aceitava `mapa<AA,bombom>` e já parava na IR. A #663 não abre nem fecha esse
/// caminho: ela faz com que as duas grafias do mesmo mapa resolvido cheguem ao
/// mesmo lugar, aceitação e limite incluídos.
///
/// Escolher entre estreitar a compatibilidade de componente e converter no
/// lowering é decisão de política, e esta Task não a toma. O teste existe para
/// que a fronteira seja explícita e para que qualquer mudança nela apareça.
#[test]
fn o_par_numericamente_compativel_para_no_mesmo_lugar() {
    fn baixa(expected: &str, actual: &str) -> Result<(), String> {
        let fonte = programa_com_operacao(expected, actual);
        let programa = common::parse(&fonte).expect("fonte deve analisar");
        pinker_v0::semantic::check_program(&programa)
            .map_err(|erro| format!("SEMANTICA: {erro:?}"))?;
        let ir = pinker_v0::ir::lower_program(&programa).map_err(|erro| format!("IR: {erro:?}"))?;
        pinker_v0::ir_validate::validate_program(&ir)
            .map_err(|erro| format!("IR_VALIDATE: {erro:?}"))?;
        Ok(())
    }

    /// A camada em que o programa parou e a causa, sem span.
    fn camada(resultado: &Result<(), String>) -> String {
        let Err(bruto) = resultado else {
            return "OK".to_string();
        };
        let (camada, resto) = bruto.split_once(':').unwrap_or((bruto.as_str(), ""));
        let causa = resto
            .split_once("msg: \"")
            .and_then(|(_, apos)| apos.split_once('"'))
            .map(|(msg, _)| msg.to_string())
            .unwrap_or_default();
        format!("{camada}|{causa}")
    }

    let referencia_direta = baixa("mapa<verso,u64>", CLASSES[0].literal);
    let referencia_inversa = baixa(CLASSES[0].literal, "mapa<verso,u64>");
    assert!(
        referencia_direta.is_err() && referencia_inversa.is_err(),
        "o par numericamente compatível não baixa: {referencia_direta:?} / {referencia_inversa:?}"
    );

    for grafia in CLASSES[0].grafias() {
        assert!(
            compativel("mapa<verso,u64>", grafia) && compativel(grafia, "mapa<verso,u64>"),
            "a aceitação semântica do par deve valer para '{grafia}' nas duas direções"
        );
        assert_eq!(
            camada(&baixa("mapa<verso,u64>", grafia)),
            camada(&referencia_direta),
            "'{grafia}' deve parar no mesmo lugar que '{}'",
            CLASSES[0].literal
        );
        assert_eq!(
            camada(&baixa(grafia, "mapa<verso,u64>")),
            camada(&referencia_inversa),
            "'{grafia}' deve parar no mesmo lugar que '{}' na direção inversa",
            CLASSES[0].literal
        );
    }
}

// ---------------------------------------------------------------------------
// Controles negativos
// ---------------------------------------------------------------------------

#[test]
fn mapas_realmente_distintos_continuam_recusados() {
    // Pares escritos à mão: nenhuma linha depende de decomposição.
    let recusas = [
        ("mapa<verso,bombom>", "mapa<verso,verso>"),
        ("mapa<verso,verso>", "mapa<verso,bombom>"),
        ("mapa<verso,bombom>", "mapa<bombom,bombom>"),
        ("mapa<bombom,bombom>", "mapa<AA,bombom>"),
        ("mapa<AA,BB>", "mapa<bombom,verso>"),
        ("mapa<bombom,verso>", "mapa<AA,BB>"),
        ("mapa<verso,verso>", "mapa<BB,AA>"),
        // Valor de leque tem identidade nominal própria: ele não é `bombom`
        // só porque a IR o transporta na mesma representação operacional.
        ("mapa<bombom,bombom>", "mapa<bombom,Cor>"),
        ("mapa<bombom,Cor>", "mapa<bombom,bombom>"),
        ("mapa<verso,bombom>", "mapa<verso,Cor>"),
        ("mapa<verso,Cor>", "mapa<verso,BB>"),
        // Mapa contra não-mapa.
        ("mapa<verso,bombom>", "lista<bombom>"),
        ("lista<bombom>", "mapa<verso,bombom>"),
        ("mapa<AA,BB>", "lista<bombom>"),
        ("mapa<verso,verso>", "verso"),
        ("bombom", "mapa<BB,BB>"),
    ];
    for (expected, actual) in recusas {
        assert!(
            !compativel(expected, actual),
            "'{expected}' NÃO deve aceitar '{actual}'"
        );
    }
}

#[test]
fn controles_de_mesma_representacao_continuam_aceitos() {
    for classe in &CLASSES {
        for grafia in classe.grafias() {
            assert!(
                compativel(grafia, grafia),
                "'{grafia}' deve continuar compatível consigo"
            );
        }
    }
    assert!(compativel("mapa<verso,u64>", "mapa<verso,u64>"));
    assert!(compativel("lista<bombom>", "lista<bombom>"));
}

// ---------------------------------------------------------------------------
// Identidade exata × compatibilidade
// ---------------------------------------------------------------------------

/// A identidade semântica canônica que o programa realmente interna para uma
/// variável declarada com esta grafia.
///
/// A chave sai da tabela de identidades resolvidas da IR — a autoridade que
/// `union_canon` alimenta — e não de uma chamada direta sobre AST crua: uma
/// grafia com apelido só tem identidade depois da resolução.
fn chave_canonica(tipo: &str) -> String {
    let fonte = format!(
        r#"pacote demo;
trazer mapa.criar;
trazer lista;
{PRELUDIO}
carinho principal() -> bombom {{
    nova valor: {tipo} = {init};
    mimo 0;
}}
"#,
        init = inicializador(tipo)
    );
    let programa = common::parse(&fonte).expect("fonte de identidade deve analisar");
    pinker_v0::semantic::check_program(&programa).expect("fonte de identidade deve checar");
    let ir = pinker_v0::ir::lower_program(&programa).expect("fonte de identidade deve baixar");
    let principal = ir
        .functions
        .iter()
        .find(|f| f.name == "principal")
        .expect("a função 'principal' deve existir na IR");
    let local = principal
        .locals
        .iter()
        .find(|l| l.source_name == "valor")
        .expect("o local 'valor' deve existir na IR");
    let id = local
        .resolved
        .expect("um local declarado pelo usuário carrega identidade resolvida");
    let chave = ir
        .resolved_types
        .iter()
        .find(|entry| entry.id == id)
        .expect("a identidade do local deve estar na tabela")
        .canonical_key
        .clone();
    assert!(
        !union_canon::is_poisoned_key(&chave),
        "identidade envenenada para '{tipo}': {chave}"
    );
    chave
}

#[test]
fn identidade_canonica_e_compatibilidade_nao_se_contradizem() {
    for classe in &CLASSES {
        let grafias = classe.grafias();
        let referencia = chave_canonica(grafias[0]);
        for grafia in grafias {
            assert_eq!(
                chave_canonica(grafia),
                referencia,
                "'{grafia}' deve ter a mesma identidade canônica de '{}'",
                grafias[0]
            );
            assert!(compativel(grafias[0], grafia));
            assert!(compativel(grafia, grafias[0]));
        }
    }
}

#[test]
fn compativel_mas_distinto_nao_vira_a_mesma_identidade() {
    // `E` e `M` são relações diferentes. Compatibilidade de componente não
    // pode ser confundida com identidade exata, nem o contrário.
    assert!(compativel("mapa<verso,u64>", "mapa<verso,bombom>"));
    assert_ne!(
        chave_canonica("mapa<verso,u64>"),
        chave_canonica("mapa<verso,bombom>"),
        "u64 e bombom são compatíveis, não idênticos"
    );
    assert_ne!(
        chave_canonica("mapa<verso,bombom>"),
        chave_canonica("mapa<verso,verso>")
    );
}

// ---------------------------------------------------------------------------
// Caminho completo: operação real de mapa no programa recém-aceito
// ---------------------------------------------------------------------------

/// Programa que só é aceito depois de F-04 e que EXERCITA o mapa: a variável é
/// declarada pela grafia com apelidos, atravessa uma função que a recebe pela
/// grafia literal, e as operações reais rodam nos dois lados.
const FONTE_EXECUCAO: &str = r#"pacote demo;

trazer mapa.criar, definir, obter, tem, tamanho, remover;

apelido Chave = verso;
apelido Valor = bombom;

carinho soma_pelo_literal(m: mapa<verso,bombom>) -> bombom {
    mimo obter(m, "a") + obter(m, "b") + tamanho(m);
}

carinho principal() -> bombom {
    nova m: mapa<Chave,Valor> = criar();
    definir(m, "a", 40);
    definir(m, "b", 2);
    definir(m, "c", 9);
    remover(m, "c");
    talvez tem(m, "a") {
        falar(soma_pelo_literal(m));
        mimo 0;
    }
    mimo 1;
}
"#;

fn escrever(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let path = dir.path().join(format!("{nome}.pink"));
    fs::write(&path, fonte).expect("gravar fonte F-04 temporária");
    path
}

fn interpretar(path: &Path, caso: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(path)
        .logical_case(caso)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar interpretador F-04")
}

#[test]
fn o_mapa_recem_aceito_executa_de_verdade_no_interpretador() {
    let dir = NativeArtifactDir::create().expect("diretório F-04");
    let fonte = escrever(&dir, "issue_663_execucao", FONTE_EXECUCAO);
    let saida = interpretar(&fonte, "issue-663-interpretador");
    assert_eq!(
        saida.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&saida.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&saida.stdout), "44\n");
}

#[test]
fn interpretador_e_nativo_concordam_no_mapa_recem_aceito() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório nativo F-04");
    let fonte = escrever(&dir, "issue_663_paridade", FONTE_EXECUCAO);
    let interpretado = interpretar(&fonte, "issue-663-paridade-interpretador");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(&fonte)
        .env("PINKER_RT_LIB", &runtime_lib)
        .logical_case("issue-663-paridade-build")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("compilar F-04 nativo");
    assert!(
        build.status.success(),
        "build nativo F-04 falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(dir.path().join("issue_663_paridade"))
        .logical_case("issue-663-paridade-nativo")
        .timeout(Duration::from_secs(20))
        .output()
        .expect("executar ELF F-04");

    assert_eq!(interpretado.status.code(), Some(0));
    assert_eq!(nativo.status.code(), Some(0));
    assert_eq!(interpretado.stdout, nativo.stdout);
    assert_eq!(String::from_utf8_lossy(&nativo.stdout), "44\n");
}

#[test]
fn a_grafia_com_apelido_produz_a_mesma_ir_da_grafia_literal() {
    // A prova de que as duas grafias convergem no consumidor de IR não é a
    // ausência de erro: é o texto da IR ser o MESMO, módulo o nome do apelido
    // que nem chega lá.
    let literal = r#"pacote demo;
trazer mapa.criar, definir, tamanho;
carinho principal() -> bombom {
    nova m: mapa<verso,bombom> = criar();
    definir(m, "a", 1);
    mimo tamanho(m);
}
"#;
    let com_apelido = r#"pacote demo;
trazer mapa.criar, definir, tamanho;
apelido Chave = verso;
apelido Valor = bombom;
carinho principal() -> bombom {
    nova m: mapa<Chave,Valor> = criar();
    definir(m, "a", 1);
    mimo tamanho(m);
}
"#;
    let ir_literal = common::render_ir(literal).expect("IR da grafia literal");
    let ir_apelido = common::render_ir(com_apelido).expect("IR da grafia com apelido");
    assert_eq!(ir_literal, ir_apelido);
    assert!(
        ir_literal.contains("mapa<verso,bombom>"),
        "a IR deve carregar a representação canônica da classe: {ir_literal}"
    );
}

// @pinker-nav:end evidencia.tipos.mapa-representacao-equivalente
