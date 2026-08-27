//! Gates dirigidos da importação seletiva múltipla (Issue #533).
//!
//! A #505 removeu a superfície intrínseca global e passou a exigir `trazer`
//! explícito. A Founder bloqueou a PR #531 pela ergonomia resultante, e a
//! liberação exige que vários membros de UM módulo caibam numa declaração:
//!
//! ```text
//! trazer M.a, b, c;
//! ==
//! trazer M.a;  trazer M.b;  trazer M.c;
//! ```
//!
//! A implementação é desaçúcar na fronteira sintática, então a propriedade que
//! estes gates precisam medir não é «a lista parseia», e sim:
//!
//! ```text
//! GROUPED_FORM  ==  SEPARATE_FORM   (imports, ordem, identidades, diagnóstico)
//! DOWNSTREAM_MULTI_IMPORT_CONCEPT = 0
//! ALL_IMPORT_PREPASSES_SEE_THE_SAME_MEMBERS
//! ```
//!
//! O oráculo de equivalência compara DUAS FONTES DIFERENTES atravessando o
//! mesmo parser, e não o parser contra um registro que ele próprio alimenta:
//! se o desaçucaramento perder, duplicar ou reordenar um membro, as duas
//! sequências divergem. Onde o parser não basta — precedência de módulo real,
//! REPL, execução — o oráculo é a saída observável da CLI, que não consulta
//! estrutura nenhuma do parser.

mod common;

use common::{parse, ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::lexer::Lexer;
use pinker_v0::parser::Parser;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

const CORPO: &str = "carinho principal() -> bombom { mimo 0; }";

/// Sequência normalizada de imports: módulo, membro e ORDEM. Span fica de
/// fora de propósito — a forma agrupada tem um span só para a declaração
/// inteira, e essa é a única diferença que a equivalência autoriza.
fn imports_de(fonte: &str) -> Vec<(String, Option<String>)> {
    parse(fonte)
        .unwrap_or_else(|erro| panic!("fonte deveria parsear: {fonte}\nerro: {erro:?}"))
        .imports
        .iter()
        .map(|import| (import.module.clone(), import.symbol.clone()))
        .collect()
}

fn programa(declaracoes: &str) -> String {
    format!("pacote main;\n\n{declaracoes}\n\n{CORPO}\n")
}

fn erro_de_parse(fonte: &str) -> String {
    match parse(fonte) {
        Ok(_) => panic!("esta forma NÃO pode ser aceita:\n{fonte}"),
        Err(erro) => format!("{erro:?}"),
    }
}

fn escrever(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(nome);
    fs::write(&caminho, fonte).expect("escrever fonte da #533");
    caminho
}

fn rodar(caminho: &Path, caso: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(caminho)
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar #533 sob envelope")
}

fn checar(caminho: &Path, caso: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(caminho)
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("checar #533 sob envelope")
}

fn stdout_de(saida: &Output) -> String {
    String::from_utf8_lossy(&saida.stdout).into_owned()
}

fn stderr_de(saida: &Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

// ---------------------------------------------------------------- A, B, I, L

/// A — dois membros. O caso mínimo da decisão da Founder.
#[test]
fn a_dois_membros_equivalem_a_dois_imports_separados() {
    assert_eq!(
        imports_de(&programa("trazer texto.tamanho, aparar;")),
        imports_de(&programa("trazer texto.tamanho;\ntrazer texto.aparar;")),
    );
}

/// B — muitos membros. Se o parser parasse na primeira vírgula, ou lesse só
/// um sufixo, a contagem denunciaria.
#[test]
fn b_muitos_membros_produzem_uma_unidade_de_import_por_membro() {
    let agrupado = imports_de(&programa(
        "trazer texto.tamanho, aparar, vazio, contem, igual;",
    ));
    let separado = imports_de(&programa(
        "trazer texto.tamanho;
trazer texto.aparar;
trazer texto.vazio;
trazer texto.contem;
trazer texto.igual;",
    ));
    assert_eq!(
        agrupado.len(),
        5,
        "cinco membros, cinco imports: {agrupado:?}"
    );
    assert_eq!(agrupado, separado);
}

/// I + L — a sequência é ORDENADA, e a ordem é a textual. Trocar a ordem dos
/// membros tem de produzir uma sequência diferente, ou o gate não mede ordem
/// nenhuma e um desaçucaramento por conjunto passaria batido.
#[test]
fn i_ordem_textual_dos_membros_e_preservada_e_observavel() {
    let direta = imports_de(&programa("trazer texto.tamanho, aparar, vazio;"));
    let inversa = imports_de(&programa("trazer texto.vazio, aparar, tamanho;"));
    assert_eq!(
        direta,
        vec![
            ("texto".to_string(), Some("tamanho".to_string())),
            ("texto".to_string(), Some("aparar".to_string())),
            ("texto".to_string(), Some("vazio".to_string())),
        ]
    );
    assert_ne!(direta, inversa, "a ordem textual precisa ser observável");
    assert_eq!(
        inversa,
        imports_de(&programa(
            "trazer texto.vazio;\ntrazer texto.aparar;\ntrazer texto.tamanho;"
        ))
    );
}

/// A forma agrupada e a separada intercaladas com outros módulos continuam
/// gerando a MESMA sequência, incluindo a posição relativa entre módulos.
#[test]
fn l_ordem_entre_declaracoes_de_modulos_diferentes_e_preservada() {
    assert_eq!(
        imports_de(&programa(
            "trazer texto.tamanho, aparar;\ntrazer lista.criar, inserir;"
        )),
        imports_de(&programa(
            "trazer texto.tamanho;
trazer texto.aparar;
trazer lista.criar;
trazer lista.inserir;",
        )),
    );
}

// ------------------------------------------------------------------- N, O

/// N — `trazer M;` não é tocado pela extensão.
#[test]
fn n_import_inteiro_permanece_sem_membro() {
    assert_eq!(
        imports_de(&programa("trazer deposito;")),
        vec![("deposito".to_string(), None)]
    );
}

/// O — a sintaxe separada antiga continua aceita, isolada e repetida.
#[test]
fn o_sintaxe_separada_antiga_continua_aceita() {
    assert_eq!(
        imports_de(&programa("trazer texto.tamanho;")),
        vec![("texto".to_string(), Some("tamanho".to_string()))]
    );
    assert_eq!(
        imports_de(&programa("trazer texto.tamanho;\ntrazer texto.aparar;")).len(),
        2
    );
}

// ------------------------------------------------------------- P, Q, R, vazio

/// P — vírgula final não é autorizada pela gramática e tem de ser RECUSADA,
/// não tolerada em silêncio.
#[test]
fn p_virgula_final_e_recusada() {
    let erro = erro_de_parse(&programa("trazer texto.tamanho, aparar,;"));
    assert!(
        erro.contains("vírgula final"),
        "a recusa precisa nomear a vírgula final: {erro}"
    );
}

/// Q — `trazer M.a, N.b;` não é gramática desta Issue e não pode ser
/// reinterpretada em silêncio como dois módulos.
#[test]
fn q_segundo_modulo_qualificado_na_lista_e_recusado() {
    let erro = erro_de_parse(&programa("trazer texto.tamanho, mapa.obter;"));
    assert!(
        erro.contains("UM módulo"),
        "a recusa precisa dizer que a lista é de um módulo só: {erro}"
    );
}

/// R — vírgula depois de import INTEIRO não vira gramática acidental.
#[test]
fn r_virgula_apos_import_inteiro_e_recusada() {
    let erro = erro_de_parse(&programa("trazer texto, tamanho;"));
    assert!(
        erro.contains("exige `.`"),
        "a recusa precisa apontar o ponto que falta: {erro}"
    );
}

/// Item vazio no meio da lista também é recusa, não membro anônimo.
#[test]
fn item_vazio_no_meio_da_lista_e_recusado() {
    erro_de_parse(&programa("trazer texto.tamanho, , aparar;"));
    erro_de_parse(&programa("trazer texto.;"));
}

// ---------------------------------------------------------------- C, D, E, F

/// C — membro inexistente escapa se o diagnóstico só olhar a primeira
/// posição. Aqui ele é exigido nas TRÊS posições: início, meio e fim.
#[test]
fn c_membro_inexistente_e_recusado_em_qualquer_posicao_da_lista() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    for (caso, decl) in [
        ("inicio", "trazer texto.nao_existe, tamanho, aparar;"),
        ("meio", "trazer texto.tamanho, nao_existe, aparar;"),
        ("fim", "trazer texto.tamanho, aparar, nao_existe;"),
    ] {
        let caminho = escrever(&dir, &format!("c_{caso}.pink"), &programa(decl));
        let saida = checar(&caminho, &format!("533-c-{caso}"));
        assert!(
            !saida.status.success(),
            "membro inexistente em {caso} deveria falhar: {decl}"
        );
        let erro = stderr_de(&saida);
        assert!(
            erro.contains("nao_existe") && erro.contains("não existe na família"),
            "o diagnóstico tem de nomear o membro ausente em {caso}: {erro}"
        );
    }
}

/// D — membro duplicado na mesma declaração precisa ter EXATAMENTE o
/// tratamento dos dois imports separados duplicados. O oráculo é a forma
/// separada, medida no mesmo binário.
#[test]
fn d_membro_duplicado_tem_a_mesma_semantica_dos_imports_separados() {
    let dir = NativeArtifactDir::create().expect("diretório #533");

    let agrupado = escrever(
        &dir,
        "d_agrupado.pink",
        &programa("trazer texto.tamanho, tamanho;"),
    );
    let separado = escrever(
        &dir,
        "d_separado.pink",
        &programa("trazer texto.tamanho;\ntrazer texto.tamanho;"),
    );

    let saida_agrupado = checar(&agrupado, "533-d-agrupado");
    let saida_separado = checar(&separado, "533-d-separado");

    assert_eq!(
        saida_agrupado.status.success(),
        saida_separado.status.success(),
        "duplicado agrupado e duplicado separado têm de ter o MESMO veredito"
    );
    assert!(
        !saida_agrupado.status.success(),
        "duplicado é recusa nas duas formas"
    );

    // A mensagem também: só o span pode diferir. Comparar a linha do
    // diagnóstico sem a coordenada é o que separa «mesma regra» de «alguma
    // recusa qualquer», que `is_err()` genérico não distinguiria.
    let sem_span = |texto: String| {
        texto
            .lines()
            .find(|linha| linha.contains("Erro"))
            .map(|linha| linha.split(" em ").next().unwrap_or(linha).to_string())
            .unwrap_or_default()
    };
    assert_eq!(
        sem_span(stderr_de(&saida_agrupado)),
        sem_span(stderr_de(&saida_separado)),
        "o duplicado tem de produzir o MESMO diagnóstico nas duas formas"
    );
}

/// E — a colisão de UM membro do grupo com item de topo existente não pode
/// escapar por estar na segunda posição.
#[test]
fn e_colisao_de_um_unico_membro_do_grupo_e_detectada() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    let fonte = "pacote main;

trazer texto.aparar, tamanho;

carinho tamanho(x: verso) -> bombom { mimo 1; }
carinho principal() -> bombom { mimo 0; }
";
    let caminho = escrever(&dir, "e_colisao.pink", fonte);
    let saida = checar(&caminho, "533-e-colisao");
    assert!(!saida.status.success(), "colisão deveria falhar");
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("colisão de nome no import") && erro.contains("tamanho"),
        "a colisão do segundo membro tem de ser nomeada: {erro}"
    );
}

/// F — dois módulos distintos exportando o mesmo spelling continuam colidindo
/// quando um deles chega pela forma agrupada.
#[test]
fn f_modulos_distintos_com_mesmo_spelling_colidem_na_forma_agrupada() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    let caminho = escrever(
        &dir,
        "f_homonimo.pink",
        &programa("trazer texto.aparar, tamanho;\ntrazer lista.tamanho;"),
    );
    let saida = checar(&caminho, "533-f-homonimo");
    assert!(!saida.status.success(), "spelling homônimo deveria colidir");
    assert!(
        stderr_de(&saida).contains("múltiplos módulos"),
        "o diagnóstico histórico de colisão entre módulos tem de sobreviver: {}",
        stderr_de(&saida)
    );
}

// ------------------------------------------------------------------- G, H, M

/// G — a forma agrupada vale DENTRO de um módulo Pinker real, não só na raiz.
/// O oráculo é a execução: os dois membros importados pelo módulo precisam
/// funcionar de verdade no programa final.
#[test]
fn g_import_agrupado_funciona_dentro_de_modulo_pinker_real() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    escrever(
        &dir,
        "util.pink",
        "pacote util;

trazer texto.tamanho, aparar;

carinho normaliza(s: verso) -> verso { mimo aparar(s); }
carinho medida(s: verso) -> bombom { mimo tamanho(s); }
",
    );
    let caminho = escrever(
        &dir,
        "g_raiz.pink",
        "pacote main;

trazer util;

carinho principal() -> bombom {
    falar(normaliza(\"  x  \"));
    falar(medida(\"abc\"));
    mimo 0;
}
",
    );
    let saida = rodar(&caminho, "533-g-modulo-real");
    assert!(
        saida.status.success(),
        "import agrupado dentro de módulo real deveria executar: {}",
        stderr_de(&saida)
    );
    assert_eq!(
        stdout_de(&saida),
        "x\n3\n",
        "os DOIS membros do grupo têm de estar realmente ligados dentro do módulo"
    );
}

/// H — `REAL_MODULE_X > BUILTIN_FAMILY_X` continua valendo na forma agrupada.
///
/// Este é o caso que quebraria se o prepass que classifica módulo real visse
/// só parte da declaração: a saída `999`/`REAL` só é possível se o módulo do
/// disco venceu a família built-in homônima para AMBOS os membros.
#[test]
fn h_modulo_real_homonimo_de_familia_vence_tambem_na_forma_agrupada() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    escrever(
        &dir,
        "texto.pink",
        "pacote texto;

carinho tamanho(s: verso) -> bombom { mimo 999; }
carinho aparar(s: verso) -> verso { mimo \"REAL\"; }
",
    );
    let corpo = "carinho principal() -> bombom {
    falar(tamanho(\"abc\"));
    falar(aparar(\"  x  \"));
    mimo 0;
}
";
    let agrupado = escrever(
        &dir,
        "h_agrupado.pink",
        &format!("pacote main;\n\ntrazer texto.tamanho, aparar;\n\n{corpo}"),
    );
    let separado = escrever(
        &dir,
        "h_separado.pink",
        &format!("pacote main;\n\ntrazer texto.tamanho;\ntrazer texto.aparar;\n\n{corpo}"),
    );

    let saida_agrupado = rodar(&agrupado, "533-h-agrupado");
    let saida_separado = rodar(&separado, "533-h-separado");
    assert!(
        saida_agrupado.status.success(),
        "módulo real homônimo tem de carregar na forma agrupada: {}",
        stderr_de(&saida_agrupado)
    );
    assert_eq!(
        stdout_de(&saida_agrupado),
        "999\nREAL\n",
        "o módulo REAL tem de vencer a família nos dois membros do grupo"
    );
    assert_eq!(
        stdout_de(&saida_agrupado),
        stdout_de(&saida_separado),
        "agrupado e separado têm de produzir a MESMA execução"
    );
}

/// M — homônimo em escopo local continua sombreando o membro importado no
/// ponto em que está visível, e o membro importado continua respondendo fora
/// dele. A forma agrupada não pode mudar essa fronteira.
#[test]
fn m_homonimo_local_sombreia_igual_nas_duas_formas() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    let corpo = "carinho principal() -> bombom {
    nova tamanho: bombom = 7;
    falar(tamanho);
    falar(vazio(\"\"));
    mimo 0;
}
";
    let agrupado = escrever(
        &dir,
        "m_agrupado.pink",
        &format!("pacote main;\n\ntrazer texto.tamanho, vazio;\n\n{corpo}"),
    );
    let separado = escrever(
        &dir,
        "m_separado.pink",
        &format!("pacote main;\n\ntrazer texto.tamanho;\ntrazer texto.vazio;\n\n{corpo}"),
    );
    let saida_agrupado = rodar(&agrupado, "533-m-agrupado");
    let saida_separado = rodar(&separado, "533-m-separado");
    assert_eq!(
        saida_agrupado.status.success(),
        saida_separado.status.success(),
        "as duas formas têm de ter o mesmo veredito sob homônimo local"
    );
    assert_eq!(
        stdout_de(&saida_agrupado),
        stdout_de(&saida_separado),
        "as duas formas têm de produzir a mesma saída sob homônimo local"
    );
}

// ----------------------------------------------------------------------- K

/// K — o REPL aceita a forma agrupada pelo mesmo mecanismo de import.
///
/// O REPL separa os `trazer` iniciais por `;`, então uma lista com vírgulas
/// atravessa sem tratamento especial — o gate existe para que isso continue
/// verdadeiro, e para que a comparação com a forma separada seja observável.
#[test]
fn k_repl_aceita_a_forma_agrupada_com_a_mesma_saida() {
    let dir = NativeArtifactDir::create().expect("diretório #533");
    let executar = |linha: &str, caso: &str| -> String {
        // O REPL lê de stdin; o envelope controlado aceita um `Stdio`, então a
        // entrada vai por arquivo em vez de pipe escrito à mão.
        let entrada = escrever(&dir, &format!("{caso}.repl"), &format!("{linha}\n"));
        let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("repl")
            .stdin(fs::File::open(entrada).expect("abrir entrada do repl"))
            .logical_case(caso)
            .timeout(Duration::from_secs(60))
            .output()
            .expect("repl #533 sob envelope");
        stdout_de(&saida)
    };
    let agrupado = executar(
        "trazer texto.tamanho, aparar; falar(tamanho(aparar(\"  oi  \")));",
        "533-k-agrupado",
    );
    let separado = executar(
        "trazer texto.tamanho; trazer texto.aparar; falar(tamanho(aparar(\"  oi  \")));",
        "533-k-separado",
    );
    // `2` é `tamanho(aparar("  oi  "))`: só sai se AMBOS os membros do grupo
    // estiverem ligados. O prompt precede o valor, então a âncora inclui ele.
    assert!(
        agrupado.contains("pinker> 2"),
        "o REPL tem de avaliar os dois membros do grupo: {agrupado}"
    );
    assert_eq!(
        agrupado, separado,
        "REPL agrupado e separado têm de coincidir"
    );
}

// ----------------------------------------------------------------------- J

/// J — identidades canônicas finais idênticas.
///
/// O parser canonicaliza a grafia pública para a identidade executiva; se a
/// forma agrupada canonicalizasse diferente, o AST renderizado divergiria. A
/// comparação é feita sobre o programa INTEIRO, não só sobre a lista de
/// imports, porque é no corpo que a canonicalização aparece.
#[test]
fn j_identidades_canonicas_finais_coincidem_nas_duas_formas() {
    let corpo = "carinho principal() -> bombom {
    falar(aparar(\"  x  \"));
    mimo tamanho(\"abc\");
}
";
    let agrupado = format!("pacote main;\n\ntrazer texto.tamanho, aparar;\n\n{corpo}");
    let separado =
        format!("pacote main;\n\ntrazer texto.tamanho;\ntrazer texto.aparar;\n\n{corpo}");
    let render = |fonte: &str| {
        pinker_v0::printer::render_program(
            &parse(fonte).unwrap_or_else(|erro| panic!("deveria parsear: {erro:?}")),
        )
    };
    let a = render(&agrupado);
    let s = render(&separado);

    // A forma agrupada ocupa uma linha a menos, então TODO span do arquivo
    // desloca. Span é justamente a diferença que a Issue autoriza, e compará-lo
    // aqui mediria layout de texto em vez de identidade. O que não pode diferir
    // é a árvore com as identidades já canonicalizadas — e a asserção seguinte
    // impede que apagar spans esvazie o gate, exigindo que as identidades
    // canônicas estejam mesmo lá.
    let sem_span = |texto: &str| {
        let mut saida = String::new();
        let mut profundidade = 0usize;
        for caractere in texto.chars() {
            match caractere {
                '[' => profundidade += 1,
                ']' if profundidade > 0 => profundidade -= 1,
                _ if profundidade == 0 => saida.push(caractere),
                _ => {}
            }
        }
        saida
    };
    assert_eq!(
        sem_span(&a),
        sem_span(&s),
        "a árvore canonicalizada tem de ser idêntica:\n{a}\n---\n{s}"
    );
    for canonica in ["aparar_verso", "tamanho_verso"] {
        assert!(
            a.contains(canonica) && s.contains(canonica),
            "a identidade canônica {canonica} tem de aparecer nas duas formas"
        );
    }
    assert_eq!(
        a.matches("Import ").count(),
        s.matches("Import ").count(),
        "a contagem de unidades Import tem de coincidir:\n{a}\n---\n{s}"
    );
}

// ----------------------------------------------------------- prepasses (§6)

/// Invariante estrutural da #533:
///
/// ```text
/// PARSER_ACCEPTS_MULTI_IMPORT AND ALL_IMPORT_PREPASSES_SEE_THE_SAME_MEMBERS
/// ```
///
/// Os prepasses de import leem o fluxo de tokens ANTES do parse, para
/// responder o que o parser não pode saber sozinho. Se um deles lesse a
/// declaração por conta própria e parasse na primeira vírgula, a forma
/// agrupada deixaria de ser equivalente à separada exatamente nos membros que
/// ele não viu. Este gate mede a leitura compartilhada contra o que o parser
/// realmente produziu, sobre a mesma fonte.
#[test]
fn prepasses_de_import_enxergam_todos_os_membros_da_declaracao() {
    let fontes = [
        "trazer texto.tamanho, aparar, vazio;",
        "trazer texto.tamanho;\ntrazer lista.criar, inserir;",
        "trazer deposito;\ntrazer texto.tamanho, aparar;",
        "trazer texto.tamanho;",
        "trazer deposito;",
    ];
    for declaracoes in fontes {
        let fonte = programa(declaracoes);
        let tokens = Lexer::new(&fonte).tokenize().expect("tokenizar");

        // Leitura pelos prepasses: mesma autoridade sintática que o parser usa.
        let mut vistos: Vec<(String, Option<String>)> = Vec::new();
        for indice in 0..tokens.len() {
            let Some(declaracao) = Parser::ler_declaracao_trazer(&tokens, indice) else {
                continue;
            };
            let modulo = tokens[declaracao.modulo].lexeme.clone();
            if declaracao.membros.is_empty() {
                vistos.push((modulo, None));
            } else {
                for posicao in declaracao.membros {
                    vistos.push((modulo.clone(), Some(tokens[posicao].lexeme.clone())));
                }
            }
        }

        assert_eq!(
            vistos,
            imports_de(&fonte),
            "prepass e parser divergiram em: {declaracoes}"
        );
    }
}

// -------------------------------------------------- censo de ergonomia (§10)

/// Enumera as fontes `.pink` reais (`apps/` e `examples/`) por varredura de
/// diretório, não por lista transcrita: um corpus que encolhe por engano tem
/// de ser visível, não conveniente.
fn fontes_reais() -> Vec<PathBuf> {
    fn varrer(raiz: &Path, saida: &mut Vec<PathBuf>) {
        let Ok(entradas) = fs::read_dir(raiz) else {
            return;
        };
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            if caminho.is_dir() {
                varrer(&caminho, saida);
            } else if caminho.extension().is_some_and(|ext| ext == "pink") {
                saida.push(caminho);
            }
        }
    }
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut fontes = Vec::new();
    varrer(&raiz.join("apps"), &mut fontes);
    varrer(&raiz.join("examples"), &mut fontes);
    fontes.sort();
    fontes
}

/// Declarações seletivas de uma fonte, por módulo, lidas pela MESMA autoridade
/// sintática que o compilador usa. Um censo com gramática própria mediria
/// outra linguagem.
fn seletivas_por_modulo(fonte: &str) -> Vec<(String, usize)> {
    let Ok(tokens) = Lexer::new(fonte).tokenize() else {
        return Vec::new();
    };
    let mut contagem: Vec<(String, usize)> = Vec::new();
    for indice in 0..tokens.len() {
        let Some(declaracao) = Parser::ler_declaracao_trazer(&tokens, indice) else {
            continue;
        };
        if declaracao.membros.is_empty() {
            continue;
        }
        let modulo = tokens[declaracao.modulo].lexeme.clone();
        match contagem.iter_mut().find(|(nome, _)| *nome == modulo) {
            Some((_, quantas)) => *quantas += 1,
            None => contagem.push((modulo, 1)),
        }
    }
    contagem
}

/// S15 — gate de ergonomia da #533.
///
/// A Founder bloqueou a #531 olhando uma fonte real, então a propriedade que
/// libera o bloqueio é sobre fontes reais, não sobre o parser:
///
/// ```text
/// SAME_MODULE_REPEATED_SELECTIVE_IMPORT_LINES = 0   (apps/ e examples/)
/// ```
///
/// Reintroduzir `trazer M.a;` + `trazer M.b;` em qualquer fonte migrada torna
/// este gate vermelho. Fixtures de `tests/` ficam FORA do corpus de propósito:
/// a forma textual delas existe para provar compatibilidade da sintaxe antiga,
/// e normalizá-las apagaria a prova.
#[test]
fn s15_fontes_reais_nao_repetem_import_seletivo_do_mesmo_modulo() {
    let fontes = fontes_reais();
    assert!(
        fontes.len() >= 400,
        "o corpus real encolheu para {} fontes: um censo que perde arquivos mede a coisa errada",
        fontes.len()
    );

    let mut infratores: Vec<String> = Vec::new();
    for caminho in &fontes {
        let Ok(fonte) = fs::read_to_string(caminho) else {
            continue;
        };
        for (modulo, quantas) in seletivas_por_modulo(&fonte) {
            if quantas > 1 {
                infratores.push(format!(
                    "{}: módulo '{modulo}' em {quantas} declarações seletivas",
                    caminho.display()
                ));
            }
        }
    }
    assert!(
        infratores.is_empty(),
        "fonte real com import seletivo repetido do mesmo módulo (agrupe em `trazer M.a, b;`):\n{}",
        infratores.join("\n")
    );
}

/// O outro lado do mesmo gate: a redução tem de ser ESTRUTURAL e mensurável,
/// não estética. O Guardião é a fonte que a Founder citou.
#[test]
fn guardiao_tem_uma_declaracao_por_modulo_importado() {
    let caminho = Path::new(env!("CARGO_MANIFEST_DIR")).join("apps/guardiao_pinker/principal.pink");
    let fonte = fs::read_to_string(caminho).expect("ler o Guardião");
    let por_modulo = seletivas_por_modulo(&fonte);
    assert!(
        !por_modulo.is_empty(),
        "o Guardião precisa continuar importando seletivamente"
    );
    for (modulo, quantas) in &por_modulo {
        assert_eq!(
            *quantas, 1,
            "o Guardião tem de ter UMA declaração seletiva por módulo, e '{modulo}' tem {quantas}"
        );
    }
}

// ------------------------------------------------- paridade nativa (ataque Q)

/// ADV-533-002 — paridade interpretador/nativo da forma AGRUPADA.
///
/// O restante desta suíte prova a equivalência no frontend e pelo `--run`, que
/// é o interpretador. Isso deixava a afirmação `RUNTIME_DELTA = 0` apoiada
/// apenas em `git diff`: correta, mas sem oráculo executável próprio da
/// sintaxe nova. A revisão adversarial apontou a lacuna, construiu o nativo à
/// mão e mediu paridade — este gate transforma aquela sonda manual em
/// evidência permanente e falsificável.
///
/// `require_native_evidence(..., true)` exige a staticlib de verdade: sob
/// `PINKER_EXIGE_NATIVO=1` a ausência vira FALHA, não skip, então uma execução
/// que não linkou o runtime não pode passar por verde.
#[test]
fn q_forma_agrupada_tem_paridade_interpretador_nativo_com_runtime_real() {
    let Some((_driver, Some(runtime_lib))) = common::require_native_evidence(
        "q_forma_agrupada_tem_paridade_interpretador_nativo_com_runtime_real",
        true,
    ) else {
        return;
    };

    let dir = NativeArtifactDir::create().expect("diretório #533");
    let corpo = "carinho principal() -> bombom {
    falar(tamanho(aparar(\"  oi  \")));
    falar(nao_vazio(aparar(\"  x  \")));
    mimo 0;
}
";
    let esperado = "2\nverdade\n";

    // As duas grafias emitem o MESMO programa; só a forma do import difere.
    // Quatro execuções, um único observável: interpretado e nativo, agrupado e
    // separado. Qualquer assimetria entre as quatro derruba o gate.
    for (nome, declaracoes) in [
        ("agrupado", "trazer texto.tamanho, aparar, nao_vazio;"),
        (
            "separado",
            "trazer texto.tamanho;\ntrazer texto.aparar;\ntrazer texto.nao_vazio;",
        ),
    ] {
        let fonte = escrever(
            &dir,
            &format!("q_{nome}.pink"),
            &format!("pacote main;\n\n{declaracoes}\n\n{corpo}"),
        );

        let interpretado = rodar(&fonte, &format!("533-q-int-{nome}"));
        assert!(
            interpretado.status.success(),
            "interpretador falhou em {nome}: {}",
            stderr_de(&interpretado)
        );
        assert_eq!(
            stdout_de(&interpretado),
            esperado,
            "interpretador divergiu do oráculo em {nome}"
        );

        let build = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["build", "--nativo", "--out-dir"])
            .arg(dir.path())
            .arg(&fonte)
            .env("PINKER_RT_LIB", &runtime_lib)
            .logical_case(&format!("533-q-build-{nome}"))
            .timeout(Duration::from_secs(180))
            .output()
            .expect("compilar #533 sob envelope");
        assert!(
            build.status.success(),
            "build nativo falhou em {nome}: {}",
            stderr_de(&build)
        );

        let executavel = dir.path().join(format!("q_{nome}"));
        assert!(
            executavel.is_file(),
            "o build nativo tem de ter produzido o ELF {}",
            executavel.display()
        );
        let nativo = Command::new(&executavel)
            .logical_case(&format!("533-q-nat-{nome}"))
            .timeout(Duration::from_secs(60))
            .output()
            .expect("executar ELF do #533");
        assert!(
            nativo.status.success(),
            "ELF falhou em {nome}: {}",
            stderr_de(&nativo)
        );
        assert_eq!(
            stdout_de(&nativo),
            esperado,
            "nativo divergiu do oráculo em {nome}"
        );
    }
}
