mod common;

use common::{parse, parse_and_check, ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::{ExprKind, Item, Stmt};
use pinker_v0::falha_operacional::OperacaoFalivel;
use pinker_v0::familia_superficie::{self, Exportacao, IdentidadeCanonica, EXPORTACOES};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

// @pinker-nav:start evidencia.importacoes.parte-g-superficie-familia
// @pinker-nav:domain importacoes
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da Parte G: a superfície por família resolve para a identidade executiva que já existe, e nada além do parser aprende o que é uma família. A matriz cobre as 29 superfícies aprovadas nas três grafias — global histórica, qualificada e seletiva — provando por AST que as três canonicalizam para o mesmo chamado, e por execução que interpretador e ELF nativo produzem os mesmos observáveis, incluindo o par FOLLOW/NO_FOLLOW medido no mesmo symlink. Os casos negativos fixam os diagnósticos que distinguem família não importada, membro inexistente, colisão de import e módulo Pinker ausente, e a precedência léxica é exercitada com local, leque, ninho e apelido homônimos, inclusive declarados depois do uso.

// ---------------------------------------------------------------------------
// A superfície aprovada, em forma de chamada. A ligação membro -> identidade
// NÃO é repetida aqui: cada caso cita só a família, o membro e os argumentos,
// e a identidade sai da autoridade. Um caso a mais ou a menos que o registro
// quebra `matriz_exercita_as_29_superficies_aprovadas`.
// ---------------------------------------------------------------------------

/// Uma chamada da matriz, escrita uma vez e emitida em três grafias.
struct Chamada {
    familia: &'static str,
    membro: &'static str,
    args: &'static str,
}

const fn c(familia: &'static str, membro: &'static str, args: &'static str) -> Chamada {
    Chamada {
        familia,
        membro,
        args,
    }
}

/// Argumentos sintáticos usados só pela matriz de canonicalização, que parseia
/// sem checar tipos: o que importa ali é o chamado, não a aridade.
const MATRIZ_CANONICALIZACAO: &[Chamada] = &[
    c("arquivo", "abrir", "\"x\""),
    c("arquivo", "fechar", "h"),
    c("arquivo", "ler_bombom", "h"),
    c("arquivo", "ler_verso", "h"),
    c("arquivo", "ler_caminho_verso", "\"x\""),
    c("arquivo", "ler_caminho_ou", "\"x\", \"p\""),
    c("arquivo", "ler_caminho_resultado", "\"x\""),
    c("arquivo", "escrever_bombom", "h, 7"),
    c("arquivo", "escrever_verso", "h, \"s\""),
    c("arquivo", "criar", "\"x\""),
    c("arquivo", "truncar", "h"),
    c("arquivo", "abrir_anexo", "\"x\""),
    c("arquivo", "anexar_verso", "h, \"s\""),
    c("arquivo", "copiar", "\"a\", \"b\""),
    c("arquivo", "renomear", "\"a\", \"b\""),
    c("integridade", "sha256_arquivo", "\"x\""),
    c("caminho", "existe", "\"x\""),
    c("caminho", "e_arquivo", "\"x\""),
    c("caminho", "e_diretorio", "\"x\""),
    c("caminho", "juntar", "\"a\", \"b\""),
    c("caminho", "tamanho_arquivo", "\"x\""),
    c("caminho", "arquivo_vazio", "\"x\""),
    c("caminho", "criar_diretorio", "\"d\""),
    c("caminho", "remover_arquivo", "\"x\""),
    c("caminho", "remover_diretorio", "\"d\""),
    c("caminho", "diretorio_atual", ""),
    c("caminho", "listar_diretorio", "\"d\""),
    c("caminho", "tipo_de_entrada", "\"x\""),
    c("caminho", "tamanho_de_entrada", "\"x\""),
];

/// Grafia sob a qual um programa da matriz é emitido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grafia {
    /// `trazer familia;` + `familia.membro(...)`.
    Qualificada,
    /// `trazer familia.membro;` + `membro(...)`.
    Seletiva,
}

impl Grafia {
    /// A #505 removeu a terceira: o nome global histórico deixou de ser
    /// chamável. Ele não sumiu deste arquivo — virou oráculo negativo em
    /// `a_grafia_legada_da_matriz_deixou_de_ser_chamavel`, que percorre a
    /// mesma matriz exigindo recusa.
    const TODAS: [Grafia; 2] = [Grafia::Qualificada, Grafia::Seletiva];

    fn nome(self) -> &'static str {
        match self {
            Grafia::Qualificada => "qualificado",
            Grafia::Seletiva => "seletivo",
        }
    }
}

/// Emite uma chamada na grafia pedida. A identidade canônica vem da autoridade.
fn chamada_na_grafia(grafia: Grafia, familia: &str, membro: &str, args: &str) -> String {
    assert!(
        familia_superficie::resolver(familia, membro).is_some(),
        "membro aprovado no registro: {familia}.{membro}"
    );
    match grafia {
        Grafia::Qualificada => format!("{familia}.{membro}({args})"),
        Grafia::Seletiva => format!("{membro}({args})"),
    }
}

/// A chamada na grafia global histórica, que a #505 tornou irrecebível.
fn chamada_legada(familia: &str, membro: &str, args: &str) -> String {
    let canonica =
        familia_superficie::resolver(familia, membro).expect("membro aprovado no registro");
    format!("{canonica}({args})")
}

/// Cabeçalho de imports exigido por cada grafia.
fn cabecalho(grafia: Grafia, usados: &[(&str, &str)]) -> String {
    match grafia {
        Grafia::Qualificada => {
            let familias: BTreeSet<&str> = usados.iter().map(|(familia, _)| *familia).collect();
            let mut texto = String::new();
            for familia in familias {
                let _ = writeln!(texto, "trazer {familia};");
            }
            texto
        }
        Grafia::Seletiva => {
            let membros: BTreeSet<(&str, &str)> = usados.iter().copied().collect();
            let mut texto = String::new();
            for (familia, membro) in membros {
                let _ = writeln!(texto, "trazer {familia}.{membro};");
            }
            texto
        }
    }
}

// ---------------------------------------------------------------------------
// 1. O registro é a autoridade, e não uma segunda semântica
// ---------------------------------------------------------------------------

/// As 29 decisões da #491 continuam vivas depois que a #505 generalizou o
/// mecanismo para a superfície inteira.
///
/// O que mudou de propósito, e está registrado em `MATRIZ_APROVADA`: a
/// taxonomia da #505 levou `sha256_arquivo` para `integridade`, e duas grafias
/// que apenas prefixavam o próprio módulo — `caminho.caminho_existe` e
/// `caminho.juntar_caminho` — perderam o prefixo. Nenhuma identidade mudou.
#[test]
fn as_familias_da_491_continuam_com_a_superficie_aprovada() {
    assert_eq!(
        familia_superficie::membros_da_familia("arquivo").len(),
        15,
        "arquivo perdeu `sha256`, que a taxonomia da #505 levou para `integridade`"
    );
    assert_eq!(familia_superficie::membros_da_familia("caminho").len(), 13);
    assert_eq!(
        familia_superficie::resolver("integridade", "sha256_arquivo"),
        Some("sha256_arquivo")
    );
    assert_eq!(familia_superficie::resolver("arquivo", "sha256"), None);

    // Toda família exportadora é importável. O inverso passou a valer também:
    // depois da #505 não existe módulo declarado sem membro.
    let com_membros: BTreeSet<&str> = EXPORTACOES
        .iter()
        .map(|exportacao| exportacao.familia)
        .collect();
    for familia in &com_membros {
        assert!(
            familia_superficie::familia_conhecida(familia),
            "família exportadora '{familia}' precisa ser importável"
        );
    }
    let importaveis: BTreeSet<&str> = familia_superficie::FAMILIAS.iter().copied().collect();
    assert_eq!(
        com_membros, importaveis,
        "depois da #505 módulo importável e módulo com membros são o mesmo conjunto"
    );
}

/// A matriz aprovada pela Founder, escrita por extenso como oráculo
/// INDEPENDENTE do registro.
///
/// Sem isto o registro seria seu próprio oráculo: um membro repontado para a
/// identidade errada continuaria "consistente" em todas as grafias, porque as
/// três consultariam a mesma tabela errada. Aqui a expectativa vem de fora —
/// do comentário de decisão da Founder de 2026-08-19T22:22:54Z e da matriz
/// recomendada do G0.6 — e mudar o registro sem mudar esta tabela é o que
/// precisa ficar vermelho.
///
/// Os cinco nomes de superfície falível aparecem como texto porque este é um
/// arquivo de `tests/`; o invariante de autoridade única vale para `src/`, e é
/// justamente por a expectativa morar fora de `src/` que ela é independente.
const MATRIZ_APROVADA: &[(&str, &str, &str)] = &[
    ("arquivo", "abrir", "abrir"),
    ("arquivo", "fechar", "fechar"),
    ("arquivo", "ler_bombom", "ler_arquivo"),
    ("arquivo", "ler_verso", "ler_verso_arquivo"),
    ("arquivo", "ler_caminho_verso", "ler_arquivo_verso"),
    ("arquivo", "ler_caminho_ou", "arquivo_ou"),
    ("arquivo", "ler_caminho_resultado", "ler_arquivo_resultado"),
    ("arquivo", "escrever_bombom", "escrever"),
    ("arquivo", "escrever_verso", "escrever_verso"),
    ("arquivo", "criar", "criar_arquivo"),
    ("arquivo", "truncar", "truncar_arquivo"),
    ("arquivo", "abrir_anexo", "abrir_anexo"),
    ("arquivo", "anexar_verso", "anexar_verso"),
    ("arquivo", "copiar", "copiar_arquivo"),
    ("arquivo", "renomear", "renomear_arquivo"),
    ("integridade", "sha256_arquivo", "sha256_arquivo"),
    ("caminho", "existe", "caminho_existe"),
    ("caminho", "e_arquivo", "e_arquivo"),
    ("caminho", "e_diretorio", "e_diretorio"),
    ("caminho", "juntar", "juntar_caminho"),
    ("caminho", "tamanho_arquivo", "tamanho_arquivo"),
    ("caminho", "arquivo_vazio", "e_vazio"),
    ("caminho", "criar_diretorio", "criar_diretorio"),
    ("caminho", "remover_arquivo", "remover_arquivo"),
    ("caminho", "remover_diretorio", "remover_diretorio"),
    ("caminho", "diretorio_atual", "diretorio_atual"),
    ("caminho", "listar_diretorio", "listar_diretorio"),
    ("caminho", "tipo_de_entrada", "tipo_de_entrada"),
    ("caminho", "tamanho_de_entrada", "tamanho_de_entrada"),
];

#[test]
fn o_registro_espelha_a_matriz_aprovada_pela_founder() {
    assert_eq!(MATRIZ_APROVADA.len(), 29);
    for (familia, membro, canonica) in MATRIZ_APROVADA {
        assert_eq!(
            familia_superficie::resolver(familia, membro),
            Some(*canonica),
            "{familia}.{membro} deveria resolver para '{canonica}'"
        );
    }
    // A #505 generalizou o mecanismo, então o registro deixou de ser
    // exatamente esta matriz e passou a contê-la. O que continua exato é o
    // recorte que a #491 decidiu: `arquivo` e `caminho` não podem ganhar nem
    // perder membro por fora da decisão humana. «Nada além disso» na
    // superfície inteira é responsabilidade da tabela dourada da #505, em
    // `issue_505_module_migration_tests`.
    let aprovadas: BTreeSet<(&str, &str)> = MATRIZ_APROVADA
        .iter()
        .map(|(familia, membro, _)| (*familia, *membro))
        .collect();
    let registradas: BTreeSet<(&str, &str)> = EXPORTACOES
        .iter()
        .map(|exportacao| (exportacao.familia, exportacao.membro()))
        .collect();
    assert!(
        aprovadas.is_subset(&registradas),
        "o registro perdeu superfície aprovada pela Founder"
    );
    let recorte_491: BTreeSet<(&str, &str)> = registradas
        .iter()
        .copied()
        .filter(|(familia, _)| *familia == "arquivo" || *familia == "caminho")
        .collect();
    let esperado_491: BTreeSet<(&str, &str)> = aprovadas
        .iter()
        .copied()
        .filter(|(familia, _)| *familia == "arquivo" || *familia == "caminho")
        .collect();
    assert_eq!(
        recorte_491, esperado_491,
        "`arquivo`/`caminho` divergiram da superfície aprovada pela Founder"
    );
}

/// As sete decisões nominais explícitas da Founder, isoladas do resto da
/// matriz para que uma reabertura de naming durante a implementação fique
/// visível como tal.
#[test]
fn as_sete_decisoes_explicitas_da_founder_estao_aplicadas() {
    for (canonica, familia, membro) in [
        ("e_vazio", "caminho", "arquivo_vazio"),
        ("sha256_arquivo", "integridade", "sha256_arquivo"),
        ("arquivo_ou", "arquivo", "ler_caminho_ou"),
        ("criar_arquivo", "arquivo", "criar"),
        ("copiar_arquivo", "arquivo", "copiar"),
        ("renomear_arquivo", "arquivo", "renomear"),
        ("truncar_arquivo", "arquivo", "truncar"),
    ] {
        assert_eq!(
            familia_superficie::resolver(familia, membro),
            Some(canonica),
            "decisão da Founder: {canonica} -> {familia}.{membro}"
        );
    }
    // `caminho.vazio` foi explicitamente proibido como alias adicional.
    assert_eq!(familia_superficie::resolver("caminho", "vazio"), None);
    // E `e_vazio` NÃO foi reexportado por `arquivo` nesta fase.
    assert_eq!(
        familia_superficie::resolver("arquivo", "arquivo_vazio"),
        None
    );
    assert_eq!(familia_superficie::resolver("arquivo", "e_vazio"), None);
}

/// R6: Pinker não resolve por tipo, então dois membros da mesma família nunca
/// compartilham nome. O teste também recusa a mesma identidade exportada duas
/// vezes pela mesma família.
#[test]
fn nenhuma_familia_exporta_dois_membros_com_o_mesmo_nome() {
    let mut vistos = BTreeSet::new();
    for exportacao in EXPORTACOES {
        assert!(
            vistos.insert((exportacao.familia, exportacao.membro())),
            "membro duplicado: {}.{}",
            exportacao.familia,
            exportacao.membro()
        );
    }
    assert_eq!(vistos.len(), EXPORTACOES.len());
}

/// A superfície falível é endereçada pela operação, nunca por literal — e o
/// nome público que sai do registro é o mesmo que a autoridade declara.
#[test]
fn superficie_falivel_e_endereçada_pela_operacao() {
    // As cinco da #491, mais as quatro que a #505 trouxe para a superfície
    // modular ao migrar `json`, `processo` e `texto`. Nenhuma delas é
    // endereçada por literal: todas entram pela `OperacaoFalivel`.
    let esperadas = [
        OperacaoFalivel::LerArquivoPorCaminho,
        OperacaoFalivel::HashArquivo,
        OperacaoFalivel::EnumerarDiretorio,
        OperacaoFalivel::ClassificarEntrada,
        OperacaoFalivel::MedirEntrada,
        OperacaoFalivel::InterpretarJson,
        OperacaoFalivel::ExecutarProcesso,
        OperacaoFalivel::ExecutarProcessoEstruturado,
        OperacaoFalivel::ConverterVersoParaBombom,
    ];
    let observadas: Vec<OperacaoFalivel> = EXPORTACOES
        .iter()
        .filter_map(|exportacao| match exportacao.identidade {
            IdentidadeCanonica::Falivel(operacao) => Some(operacao),
            IdentidadeCanonica::PorGrafia(_) => None,
        })
        .collect();
    assert_eq!(
        observadas.len(),
        esperadas.len(),
        "nove identidades falíveis na superfície pública depois da #505"
    );
    for operacao in esperadas {
        assert!(
            observadas.contains(&operacao),
            "a operação {operacao:?} deixou de ser exportada por família"
        );
        let superficie = pinker_v0::falha_operacional::superficie_por_operacao(operacao)
            .expect("operação declarada na autoridade");
        let exportacao = EXPORTACOES
            .iter()
            .find(|exportacao| exportacao.identidade == IdentidadeCanonica::Falivel(operacao))
            .expect("exportação da operação");
        assert_eq!(
            familia_superficie::resolver(exportacao.familia, exportacao.membro()),
            Some(superficie.intrinseca),
            "o registro precisa resolver para o nome que a autoridade declara"
        );
        assert!(familia_superficie::membro_e_falivel(
            exportacao.familia,
            exportacao.membro()
        ));
    }
}

/// `FAMILY_REGISTRY_MUST_NOT_OWN_RUNTIME_SEMANTICS`.
///
/// O registro liga grafia a identidade e nada mais. Assinatura, aridade,
/// modelo de falha, política de follow e símbolo de runtime continuam sendo
/// ditos pelas camadas que já os diziam — e um símbolo `pinker_*` aparecendo
/// aqui seria a primeira prova de que o registro começou a decidir execução.
/// A proibição de repetir o nome público de superfície falível já é imposta,
/// para toda a crate, por `nome_publico_de_superficie_falivel_existe_so_na_autoridade`.
#[test]
fn registro_nao_declara_semantica_de_runtime() {
    let fonte = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/familia_superficie.rs"),
    )
    .expect("registro legível");
    assert!(
        !fonte.contains("pinker_"),
        "o registro passou a nomear símbolo de runtime"
    );
    for palavra in ["simbolo_runtime", "assinatura_ir", "CargaResultado"] {
        assert!(
            !fonte.contains(palavra),
            "o registro passou a declarar '{palavra}', que pertence a outra autoridade"
        );
    }
}

#[test]
fn familia_desconhecida_nao_resolve_nem_lista_membros() {
    assert!(!familia_superficie::familia_conhecida("colecao"));
    assert_eq!(familia_superficie::resolver("colecao", "abrir"), None);
    assert!(familia_superficie::membros_da_familia("colecao").is_empty());
    // Membro real de outra família não vaza entre famílias.
    assert_eq!(familia_superficie::resolver("caminho", "abrir"), None);
    assert_eq!(familia_superficie::resolver("arquivo", "e_arquivo"), None);
}

// ---------------------------------------------------------------------------
// 2. Matriz de canonicalização: 29 × 3 grafias -> mesma identidade
// ---------------------------------------------------------------------------

/// Extrai o nome chamado no primeiro comando do corpo de `principal`.
fn chamado_do_programa(fonte: &str) -> String {
    let programa = parse(fonte).unwrap_or_else(|erro| panic!("parse falhou: {erro}\n{fonte}"));
    let Item::Function(funcao) = programa
        .items
        .iter()
        .find(|item| matches!(item, Item::Function(f) if f.name == "principal"))
        .expect("função principal")
    else {
        unreachable!()
    };
    let Stmt::Expr(expr) = &funcao.body.stmts[0] else {
        panic!("primeiro comando deveria ser uma chamada");
    };
    let ExprKind::Call(callee, _) = &expr.kind else {
        panic!(
            "expressão deveria ser uma chamada, encontrado {:?}",
            expr.kind
        );
    };
    match &callee.kind {
        ExprKind::Ident(nome) => nome.clone(),
        outro => panic!("chamado deveria ser um identificador simples: {outro:?}"),
    }
}

fn fonte_de_uma_chamada(grafia: Grafia, chamada: &Chamada) -> String {
    format!(
        "pacote main;\n{}carinho principal() -> bombom {{\n    {};\n    mimo 0;\n}}\n",
        cabecalho(grafia, &[(chamada.familia, chamada.membro)]),
        chamada_na_grafia(grafia, chamada.familia, chamada.membro, chamada.args)
    )
}

/// A matriz precisa exercitar exatamente a superfície que a #491 aprovou —
/// nem mais, nem menos. Sem isto, um membro aprovado poderia entrar no
/// registro e sair sem teste.
///
/// Depois que a #505 generalizou o mecanismo, o registro deixou de ser esta
/// matriz e passou a contê-la; quem responde por «nem mais, nem menos» na
/// superfície inteira é a tabela dourada de `issue_505_module_migration_tests`.
/// O que continua exato aqui é o recorte da #491.
#[test]
fn matriz_exercita_as_29_superficies_aprovadas() {
    let da_matriz: BTreeSet<(&str, &str)> = MATRIZ_CANONICALIZACAO
        .iter()
        .map(|chamada| (chamada.familia, chamada.membro))
        .collect();
    let aprovadas: BTreeSet<(&str, &str)> = MATRIZ_APROVADA
        .iter()
        .map(|(familia, membro, _)| (*familia, *membro))
        .collect();
    assert_eq!(
        da_matriz, aprovadas,
        "a matriz de testes divergiu da superfície aprovada pela Founder"
    );
    let do_registro: BTreeSet<(&str, &str)> = EXPORTACOES
        .iter()
        .map(|exportacao| (exportacao.familia, exportacao.membro()))
        .collect();
    assert!(
        da_matriz.is_subset(&do_registro),
        "a matriz exercita par que o registro não declara"
    );
    assert_eq!(da_matriz.len(), 29);
}

#[test]
fn as_tres_grafias_canonicalizam_para_a_mesma_identidade() {
    for chamada in MATRIZ_CANONICALIZACAO {
        let canonica =
            familia_superficie::resolver(chamada.familia, chamada.membro).expect("membro aprovado");
        for grafia in Grafia::TODAS {
            let fonte = fonte_de_uma_chamada(grafia, chamada);
            let chamado = chamado_do_programa(&fonte);
            assert_eq!(
                chamado,
                canonica,
                "{}.{} na grafia {} chamou '{chamado}' em vez de '{canonica}'",
                chamada.familia,
                chamada.membro,
                grafia.nome()
            );
        }
    }
}

/// Identificadores inteiros de um texto, para que um nome não seja dado por
/// presente só porque é prefixo de outro (`ler_verso` dentro de
/// `ler_verso_arquivo`).
fn palavras(texto: &str) -> BTreeSet<&str> {
    texto
        .split(|caractere: char| !caractere.is_alphanumeric() && caractere != '_')
        .filter(|palavra| !palavra.is_empty())
        .collect()
}

/// `FAMILY_MEMBER_SURVIVES_DOWNSTREAM = FALSE`, já na AST.
///
/// A grafia de família some no parser. O único lugar em que ela sobrevive é a
/// própria declaração de import — que existe para ser validada pela autoridade
/// semântica e não é transportada para nenhuma camada de execução. Fora dela,
/// nem o nome do membro nem a forma qualificada existem na árvore.
#[test]
fn familia_e_membro_nao_sobrevivem_a_ast() {
    for chamada in MATRIZ_CANONICALIZACAO {
        let canonica =
            familia_superficie::resolver(chamada.familia, chamada.membro).expect("membro aprovado");
        for grafia in [Grafia::Qualificada, Grafia::Seletiva] {
            let fonte = fonte_de_uma_chamada(grafia, chamada);
            let programa = parse(&fonte).expect("programa válido");
            let arvore = pinker_v0::printer::render_program(&programa);
            let corpo: String = arvore
                .lines()
                .filter(|linha| !linha.trim_start().starts_with("Import "))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                !corpo.contains(&format!("{}.{}", chamada.familia, chamada.membro)),
                "a forma qualificada sobreviveu à AST em {}",
                grafia.nome()
            );
            assert!(
                !corpo.contains("FieldAccess"),
                "a chamada de família virou acesso a campo em {}",
                grafia.nome()
            );
            if chamada.membro != canonica {
                assert!(
                    !palavras(&corpo).contains(chamada.membro),
                    "a grafia do membro '{}' sobreviveu à AST em {}",
                    chamada.membro,
                    grafia.nome()
                );
            }
            assert!(
                corpo.contains(canonica),
                "a identidade canônica '{canonica}' não aparece na AST"
            );
        }
    }
}

/// `DOWNSTREAM_ALIAS_TABLES = 0`.
///
/// A grafia de membro é consumida no parser. Nenhuma camada a jusante — IR,
/// CFG, os quatro validadores, seleção de instrução, máquina abstrata,
/// interpretador, os dois backends — nem o runtime nativo pode DECIDIR por
/// ela. A forma que uma tabela de alias teria é um literal com a grafia do
/// membro; é exatamente isso que este censo recusa.
#[test]
fn nenhuma_camada_a_jusante_decide_pela_grafia_de_membro() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let jusante = [
        "src/ir.rs",
        "src/cfg_ir.rs",
        "src/ir_validate.rs",
        "src/cfg_ir_validate.rs",
        "src/instr_select.rs",
        "src/instr_select_validate.rs",
        "src/abstract_machine.rs",
        "src/abstract_machine_validate.rs",
        "src/interpreter.rs",
        "src/backend_s.rs",
        "src/backend_text.rs",
        "runtime/pinker_rt/src/lib.rs",
    ];
    let proprias: Vec<&str> = EXPORTACOES
        .iter()
        .map(Exportacao::membro)
        .filter(|membro| {
            EXPORTACOES
                .iter()
                .all(|exportacao| exportacao.identidade.nome_publico() != *membro)
        })
        .collect();
    assert!(
        proprias.len() >= 10,
        "controle positivo: a superfície precisa ter grafias próprias a procurar"
    );

    let mut ofensores = Vec::new();
    for relativo in jusante {
        let caminho = raiz.join(relativo);
        if !caminho.is_file() {
            continue;
        }
        let texto = fs::read_to_string(&caminho).expect("fonte a jusante legível");
        // `DESPACHO_DE_EXECUCAO != HARNESS_DE_TESTE`. O que este censo procura
        // é a camada de execução decidindo pela grafia de membro; o módulo de
        // testes do próprio arquivo não decide execução nenhuma, e depois da
        // #505 há membro cuja grafia é palavra comum — `tamanho`, `criar`,
        // `obter` —, que aparece ali por coincidência. Cortar no marcador
        // mantém a produção inteira sob o censo e tira o falso positivo.
        let texto = match texto.find("#[cfg(test)]") {
            Some(corte) => texto[..corte].to_string(),
            None => texto,
        };
        for membro in &proprias {
            // As formas em que um literal DECIDE: braço de `match`, comparação
            // e alternativa de padrão. A mesma palavra numa mensagem de
            // diagnóstico não decide nada, e não é o que se procura aqui.
            for forma in [
                format!("\"{membro}\" =>"),
                format!("== \"{membro}\""),
                format!("\"{membro}\" =="),
                format!("\"{membro}\" |"),
                format!("| \"{membro}\""),
            ] {
                if texto.contains(&forma) {
                    ofensores.push(format!("{relativo} decide pela grafia '{membro}'"));
                }
            }
        }
    }
    assert!(
        ofensores.is_empty(),
        "a grafia de família vazou para camadas de execução:\n  {}",
        ofensores.join("\n  ")
    );
}

/// Controle negativo do censo acima: o parser — a única camada autorizada a
/// conhecer a grafia — de fato a conhece. Sem isto o censo passaria por não
/// haver nada a encontrar em lugar nenhum.
#[test]
fn a_grafia_de_membro_existe_em_exatamente_uma_camada() {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let registro =
        fs::read_to_string(raiz.join("src/familia_superficie.rs")).expect("registro legível");
    let proprias: Vec<&str> = EXPORTACOES
        .iter()
        .map(Exportacao::membro)
        .filter(|membro| {
            EXPORTACOES
                .iter()
                .all(|exportacao| exportacao.identidade.nome_publico() != *membro)
        })
        .collect();
    for membro in proprias {
        assert!(
            registro.contains(&format!("\"{membro}\"")),
            "a grafia '{membro}' deveria ser declarada no registro"
        );
    }
}

// @pinker-nav:end evidencia.importacoes.parte-g-superficie-familia

// @pinker-nav:start evidencia.importacoes.parte-g-diagnosticos-e-precedencia
// @pinker-nav:domain importacoes
// @pinker-nav:layer evidencia
// @pinker-nav:summary Recusas e precedência da superfície por família: cada modo de erro tem mensagem própria — família não importada, membro inexistente em uso qualificado, membro inexistente em import seletivo, família sem membros exportados, colisão com item de topo, colisão entre imports e módulo Pinker ausente —, e nenhum deles procura `<familia>.pink`. A precedência aprovada (toda identidade já existente vence a família) é exercitada com variável local, leque, ninho, apelido e função de topo homônimos, inclusive declarados depois do uso, e o legado global continua resolvendo sem import e sem ser escondido por ele. A metade ESCOPADA da precedência tem oráculo positivo — o chamado canonicalizado na AST, não a ausência de uma palavra numa mensagem: local, parâmetro e campo de `ninho` em um ponto não desabilitam a família em outro; ligação do mesmo escopo, de bloco interno, de braço de `caso`, de braço de `tentar`, de braço de encaixe de união, de `para cada` e de parâmetro de `carinho` anônimo continuam sombreando onde estão visíveis, e param de valer onde o escopo fecha; identidade de topo posterior vence e ligação local posterior não vence, que é a regra histórica de hoisting da Pinker preservada dos dois lados.

fn erro_de(fonte: &str) -> String {
    parse_and_check(fonte)
        .expect_err("o programa deveria ser recusado")
        .to_string()
}

// ----- diagnósticos -----

#[test]
fn uso_qualificado_sem_import_nomeia_a_familia_ausente() {
    let erro = erro_de(
        "pacote main;
         carinho principal() -> bombom { falar(arquivo.ler_caminho_verso(\"x\")); mimo 0; }",
    );
    assert!(
        erro.contains("família 'arquivo' não foi importada"),
        "{erro}"
    );
    assert!(erro.contains("trazer arquivo;"), "{erro}");
}

#[test]
fn membro_inexistente_em_uso_qualificado_lista_os_membros() {
    let erro = erro_de(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom { falar(arquivo.nao_existe(\"x\")); mimo 0; }",
    );
    assert!(
        erro.contains("membro 'nao_existe' não existe na família 'arquivo'"),
        "{erro}"
    );
    assert!(erro.contains("'ler_caminho_verso'"), "{erro}");
    // Não pode cair para um global homônimo nem virar acesso a campo.
    assert!(!erro.contains("não declarado"), "{erro}");
}

#[test]
fn membro_inexistente_em_import_seletivo_lista_os_membros() {
    // `criar_arquivo` é o nome GLOBAL; sob a família o membro é `criar`.
    let erro = erro_de(
        "pacote main;
         trazer arquivo.criar_arquivo;
         carinho principal() -> bombom { mimo 0; }",
    );
    assert!(
        erro.contains("membro 'criar_arquivo' não existe na família 'arquivo'"),
        "{erro}"
    );
    assert!(erro.contains("'criar'"), "{erro}");
    // A recusa categórica histórica não pode voltar disfarçada.
    assert!(!erro.contains("não é suportada"), "{erro}");
}

/// Membro inexistente é recusado citando os membros REAIS do módulo.
///
/// A #505 fechou o caso «família sem membros»: depois dela todo módulo
/// importável exporta pelo menos um membro, e a metade do diagnóstico que
/// falava de família vazia deixou de ser alcançável por fonte. O que continua
/// alcançável — e é o que importa ao usuário — é o membro que não existe, com
/// a lista de quem existe. As duas grafias históricas usadas aqui são as
/// mesmas de antes: elas agora são a grafia CANÔNICA, não o membro.
#[test]
fn membro_inexistente_e_recusado_citando_os_membros_reais() {
    for (familia, membro, esperado) in [
        ("texto", "juntar_verso", "'juntar'"),
        ("tempo", "tempo_unix", "'unix'"),
    ] {
        let erro = erro_de(&format!(
            "pacote main;
             trazer {familia}.{membro};
             carinho principal() -> bombom {{ mimo 0; }}"
        ));
        assert!(
            erro.contains(&format!(
                "membro '{membro}' não existe na família '{familia}'"
            )),
            "{erro}"
        );
        assert!(
            erro.contains(esperado),
            "o diagnóstico precisa listar os membros reais: {erro}"
        );
    }
}

#[test]
fn familia_desconhecida_continua_recusada_pela_semantica() {
    let erro = erro_de(
        "pacote main;
         trazer colecao;
         carinho principal() -> bombom { mimo 0; }",
    );
    assert!(
        erro.contains("família 'colecao' não é reconhecida"),
        "{erro}"
    );
}

/// `trazer familia;` continua sendo aceito para as sete famílias importáveis,
/// inclusive as que não exportam membro nenhum nesta fase.
#[test]
fn import_de_familia_inteira_continua_aceito_para_todas() {
    for familia in pinker_v0::familia_superficie::FAMILIAS {
        let fonte = format!(
            "pacote main;
             trazer {familia};
             carinho principal() -> bombom {{ mimo 0; }}"
        );
        assert!(
            parse_and_check(&fonte).is_ok(),
            "'trazer {familia};' deixou de ser aceito"
        );
    }
}

// ----- precedência: toda identidade já existente vence a família -----

#[test]
fn variavel_local_homonima_vence_a_familia() {
    // Com `arquivo` sombreado por um `bombom`, a expressão cai no caminho de
    // método pré-existente — e recebe a mensagem pré-existente.
    let erro = erro_de(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova arquivo: bombom = 7;
             falar(arquivo.ler_caminho_verso(\"x\"));
             mimo 0;
         }",
    );
    assert!(
        erro.contains("método 'ler_caminho_verso' não implementado para tipo 'bombom'"),
        "{erro}"
    );

    // E o valor local continua sendo o valor local.
    assert!(parse_and_check(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova arquivo: bombom = 7;
             falar(arquivo);
             mimo 0;
         }"
    )
    .is_ok());
}

#[test]
fn leque_homonimo_vence_a_familia_mesmo_declarado_depois() {
    let antes = "pacote main;
         trazer arquivo;
         leque arquivo { Aberto, Fechado }
         carinho principal() -> bombom {
             nova estado: arquivo = arquivo.Aberto;
             escolha estado { caso arquivo.Aberto { falar(\"a\"); } caso arquivo.Fechado { falar(\"f\"); } }
             mimo 0;
         }";
    assert!(parse_and_check(antes).is_ok(), "leque antes do uso");

    let depois = "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova estado: arquivo = arquivo.Aberto;
             escolha estado { caso arquivo.Aberto { falar(\"a\"); } caso arquivo.Fechado { falar(\"f\"); } }
             mimo 0;
         }
         leque arquivo { Aberto, Fechado }";
    assert!(
        parse_and_check(depois).is_ok(),
        "um leque homônimo declarado depois do uso precisa vencer a família: {:?}",
        parse_and_check(depois).err().map(|e| e.to_string())
    );
}

#[test]
fn ninho_e_apelido_homonimos_vencem_a_familia() {
    // Um `ninho` homônimo reivindica o nome; a expressão qualificada volta a
    // ser o que era antes da família existir, e a família não a captura.
    let com_ninho = erro_de(
        "pacote main;
         trazer caminho;
         ninho caminho { raiz: verso; }
         carinho principal() -> bombom { falar(caminho.juntar_caminho(\"a\", \"b\")); mimo 0; }",
    );
    assert!(
        !com_ninho.contains("não existe na família"),
        "a família capturou um nome que já pertencia a um ninho: {com_ninho}"
    );

    let com_apelido = erro_de(
        "pacote main;
         trazer caminho;
         apelido caminho = bombom;
         carinho principal() -> bombom { falar(caminho.juntar_caminho(\"a\", \"b\")); mimo 0; }",
    );
    assert!(
        !com_apelido.contains("não existe na família"),
        "a família capturou um nome que já pertencia a um apelido: {com_apelido}"
    );
}

#[test]
fn funcao_de_topo_homonima_vence_a_familia() {
    // O uso qualificado deixa de resolver pela família; a mensagem é a
    // histórica de identificador, não a de membro inexistente.
    let erro = erro_de(
        "pacote main;
         trazer arquivo;
         carinho arquivo() -> bombom { mimo 1; }
         carinho principal() -> bombom { falar(arquivo.ler_caminho_verso(\"x\")); mimo 0; }",
    );
    assert!(
        !erro.contains("não existe na família"),
        "a família capturou um nome que já pertencia a uma função de topo: {erro}"
    );
    // Asserção POSITIVA: não basta a família se calar, o erro tem de ser o
    // histórico de acesso a campo sobre o valor devolvido pela função de topo.
    assert!(
        erro.contains("campo") || erro.contains("método"),
        "o diagnóstico deveria ser o histórico de acesso a campo: {erro}"
    );
    // E a AST não pode ter canonicalizado nada.
    assert!(
        !arvore_de(
            "pacote main;
             trazer arquivo;
             carinho arquivo() -> bombom { mimo 1; }
             carinho principal() -> bombom { falar(arquivo.ler_caminho_verso(\"x\")); mimo 0; }"
        )
        .contains("ler_arquivo_verso"),
        "a função de topo homônima foi atropelada pela canonicalização"
    );
}

/// BLOCKER da revisão adversarial II: uma ligação de VALOR-FUNÇÃO não emite
/// `Stmt::Let` quando o literal é um `carinho` anônimo não-capturante — o alias
/// vai direto para `function_value_scopes` e o bloco não tem o que ler.
///
/// Sem registrá-la, a família reescrevia em silêncio uma ligação que o
/// programador acabara de criar, e a chamada ia parar na intrínseca: no repro
/// original a closure nunca executava e um arquivo aparecia no disco.
#[test]
fn ligacao_de_valor_funcao_vence_o_membro_seletivo() {
    let seletivo = arvore_de(
        "pacote main;
         trazer arquivo.criar;
         carinho principal() -> bombom {
             nova criar: carinho(verso) -> bombom = carinho(p: verso) -> bombom { mimo 42; };
             mimo criar(\"x\");
         }",
    );
    assert!(
        !seletivo.contains("criar_arquivo"),
        "a família capturou uma ligação de valor-função: {seletivo}"
    );

    // Mesma ligação, forma qualificada: o nome é o da família.
    let qualificado = arvore_de(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova arquivo: carinho(verso) -> bombom = carinho(p: verso) -> bombom { mimo 42; };
             mimo arquivo(\"x\");
         }",
    );
    assert!(
        !qualificado.contains("criar_arquivo") && !qualificado.contains("ler_arquivo"),
        "a família capturou o nome de uma ligação de valor-função: {qualificado}"
    );
}

#[test]
fn local_homonimo_vence_membro_importado_seletivamente() {
    let fonte = "pacote main;
         trazer caminho.tamanho_arquivo;
         carinho principal() -> bombom {
             nova tamanho_arquivo: bombom = 5;
             falar(tamanho_arquivo);
             mimo 0;
         }";
    assert!(
        parse_and_check(fonte).is_ok(),
        "{:?}",
        parse_and_check(fonte).err().map(|e| e.to_string())
    );
}

// ----- o legado é independente do import -----

/// `FAMILY_IMPORT controls NEW_FAMILY_SURFACE` · `LEGACY_GLOBAL remains
/// independently available`. As 29 identidades continuam válidas sem `trazer`,
/// e `trazer arquivo;` não esconde nenhuma delas.
#[test]
fn legado_global_e_independente_do_import_de_familia() {
    // Chamar com aridade zero é inválido para quase todas: o que este teste lê
    // é a MENSAGEM. "não declarada" significaria que o nome global sumiu;
    // qualquer outra recusa significa que ele foi reconhecido e checado.
    let reconhecido = |fonte: &str| match parse_and_check(fonte) {
        Ok(()) => None,
        Err(erro) => {
            let texto = erro.to_string();
            assert!(
                !texto.contains("não declarada") && !texto.contains("não declarado"),
                "nome global deixou de ser reconhecido: {texto}"
            );
            Some(texto)
        }
    };

    for exportacao in EXPORTACOES {
        let canonica = exportacao.identidade.nome_publico();
        reconhecido(&format!(
            "pacote main;
             carinho principal() -> bombom {{ {canonica}(); mimo 0; }}"
        ));
        reconhecido(&format!(
            "pacote main;
             trazer arquivo;
             trazer caminho;
             carinho principal() -> bombom {{ {canonica}(); mimo 0; }}"
        ));
    }
}

// ---------------------------------------------------------------------------
// Regressões da revisão adversarial (F1..F6).
//
// Cada uma nasceu de um programa que o baseline `5e37268c` aceitava e a
// primeira versão desta Parte recusava, ou pior, aceitava com outro
// significado. O oráculo de todas é o comportamento HISTÓRICO, não o desejado:
// a Parte G é aditiva, e o que ela não souber resolver tem de continuar
// resolvendo como antes dela existir.
// ---------------------------------------------------------------------------

/// F1 — a família não pode capturar `x.campo` de uma ligação que o parser não
/// tipou. Sem `trazer` no arquivo inteiro, isto compilava no baseline.
#[test]
fn f1_ligacao_sem_anotacao_nao_e_capturada_pela_familia() {
    let fonte = "pacote main;
ninho Estado { abrir: bombom; }
carinho fabricar() -> Estado { nova e: seta<Estado> = 1; mimo *e; }
carinho principal() -> bombom {
    nova arquivo = fabricar();
    mimo arquivo.abrir;
}";
    parse_and_check(fonte).expect("acesso a campo histórico não pode virar erro de família");
}

/// F1 — a mesma coisa em posição de chamada, sobre um nome de membro real.
#[test]
fn f1_chamada_de_campo_homonimo_de_membro_nao_e_capturada() {
    let fonte = "pacote main;
ninho Estado { abrir: bombom; }
carinho fabricar() -> Estado { nova e: seta<Estado> = 1; mimo *e; }
carinho principal() -> bombom {
    nova caminho = fabricar();
    mimo caminho.abrir;
}";
    parse_and_check(fonte).expect("nome de família em ligação local não habilita a família");
}

/// F2 — `eterno` de topo é identidade existente e vence a família. Antes o
/// `trazer` capturava a constante e `arquivo.ler_bombom(1)` virava
/// `ler_arquivo(1)` em silêncio: mudança de significado sem diagnóstico.
#[test]
fn f2_eterno_de_topo_vence_a_familia() {
    let erro = erro_de(
        "pacote main;
         trazer arquivo;
         eterno arquivo: bombom = 7;
         carinho principal() -> bombom { mimo arquivo.ler_bombom(1); }",
    );
    assert!(
        erro.contains("não implementado para tipo 'bombom'"),
        "a constante tem de continuar sendo a base do acesso: {erro}"
    );
    assert!(
        !erro.contains("família"),
        "nenhum diagnóstico de família cabe aqui: {erro}"
    );
}

/// F2 — e a captura silenciosa é o que não pode voltar: o programa acima não
/// pode COMPILAR.
#[test]
fn f2_eterno_homonimo_nunca_compila_como_chamada_de_familia() {
    assert!(
        parse_and_check(
            "pacote main;
             trazer arquivo;
             eterno arquivo: bombom = 7;
             carinho principal() -> bombom { mimo arquivo.ler_bombom(1); }",
        )
        .is_err(),
        "captura silenciosa de `eterno` pela família"
    );
}

/// F3 — com a família importada e um `carinho` de topo homônimo, o
/// diagnóstico não pode ser "família não importada" num arquivo que importa a
/// família. A identidade existente vence e a mensagem é a histórica.
#[test]
fn f3_carinho_de_topo_homonimo_da_diagnostico_historico() {
    let erro = erro_de(
        "pacote main;
         trazer arquivo;
         carinho arquivo(x: bombom) -> bombom { mimo x; }
         carinho principal() -> bombom { mimo arquivo.ler_caminho_verso(\"x\"); }",
    );
    assert!(
        erro.contains("não implementado para tipo"),
        "esperado o erro histórico de método: {erro}"
    );
    assert!(
        !erro.contains("não foi importada"),
        "mensagem autocontraditória num arquivo com `trazer arquivo;`: {erro}"
    );
}

/// F4 — `leque` declarado DEPOIS do uso, com variante homônima de membro
/// aprovado e nenhum import no arquivo. O baseline compila; a família não pode
/// se meter.
#[test]
fn f4_leque_posterior_com_variante_homonima_de_membro() {
    let fonte = "pacote main;
carinho principal() -> bombom {
    nova e: arquivo = arquivo.criar;
    mimo 0;
}
leque arquivo { criar, fechar }";
    parse_and_check(fonte).expect("leque posterior tem precedência sobre a família");
}

/// NARROW_SHADOWING_COMPLETENESS — a ligação de carga de `encaixe`.
///
/// É a única forma de ligação que não é anunciada por palavra-chave nem
/// seguida de `:`, então o censo de tokens não a alcança; ela é registrada no
/// ponto de parse. Com a família importada, a carga homônima tem de vencer, e
/// o erro tem de ser o HISTÓRICO — sobre o tipo da carga, não sobre família.
///
/// A asserção é positiva de propósito: exigir apenas a ausência da palavra
/// "família" deixaria o teste verde num programa que falha por outro motivo
/// qualquer, e foi assim que a primeira versão deste caso passou sem nunca
/// chegar ao sítio (a carga era `ninho`, que esta fase não aceita em variante).
#[test]
fn carga_de_encaixe_homonima_de_familia_vence_a_familia() {
    let erro = erro_de(
        "pacote main;
         trazer arquivo;
         leque Pacote { Cheio(verso), Vazio }
         carinho consumir(p: Pacote) -> bombom {
             nova muda total: bombom = 0;
             encaixe p {
                 caso Pacote.Cheio(arquivo) { total = arquivo.ler_bombom(1); }
                 caso Pacote.Vazio { total = 0; }
             }
             mimo total;
         }
         carinho principal() -> bombom { mimo consumir(Pacote.Cheio(\"x\")); }",
    );
    assert!(
        erro.contains("método 'ler_bombom' não implementado para tipo 'verso'"),
        "a carga de `encaixe` é a base do acesso, e o erro é o histórico: {erro}"
    );
}

/// F5 — a política de ownership intrínseco vale no caminho de biblioteca, não
/// só na CLI. `parse_and_check` é exatamente o caminho que a crate expõe.
/// #504 registra a disposição humana que moveu este caso da colisão histórica
/// de import para a declaração callable conflitante.
#[test]
fn f5_colisao_de_import_seletivo_vale_no_caminho_de_biblioteca() {
    let erro = erro_de(
        "pacote main;
         trazer arquivo.criar;
         carinho criar(x: bombom) -> bombom { mimo x; }
         carinho principal() -> bombom { mimo criar(1); }",
    );
    assert!(
        erro.contains("declaração callable 'criar'"),
        "a declaração callable precisa ser a dona da falha: {erro}"
    );
    // Depois da #505 a causa é nomeada: não é «a grafia é da linguagem», é
    // «este arquivo traz `arquivo.criar`». O diagnóstico ficou mais preciso
    // sem deixar de ser a mesma recusa.
    assert!(erro.contains("colide com o membro 'arquivo.criar'"), "{erro}");
    assert!(
        !erro.contains("colisão de nome no import"),
        "a expectativa histórica foi substituída por #504: {erro}"
    );
}

/// A dica de família continua existindo — mas só quando o nome é órfão, e ela
/// sai da camada CERTA.
///
/// A dica nasceu no parser e foi de lá que quebrou programa legado (F1/F3/F4):
/// o parser não sabe se o nome tem dono. A camada é parte do contrato, não
/// detalhe de apresentação — por isso a asserção lê `Erro Semântico`, e um
/// retorno ao `Erro Sintático` do parser fica vermelho aqui.
#[test]
fn dica_de_familia_so_aparece_quando_o_nome_nao_tem_dono() {
    let erro = erro_de(
        "pacote main;
         carinho principal() -> bombom { falar(arquivo.ler_caminho_verso(\"x\")); mimo 0; }",
    );
    assert!(
        erro.contains("família 'arquivo' não foi importada"),
        "{erro}"
    );
    assert!(
        erro.starts_with("Erro Semântico:"),
        "a dica tem de vir da camada que enxerga o programa inteiro: {erro}"
    );

    // Com qualquer identidade homônima, a dica cede lugar ao erro histórico.
    let com_dono = erro_de(
        "pacote main;
         ninho arquivo { abrir: bombom; }
         carinho principal() -> bombom { falar(arquivo.ler_caminho_verso(\"x\")); mimo 0; }",
    );
    assert!(
        !com_dono.contains("não foi importada"),
        "o nome tem dono; a família não opina: {com_dono}"
    );
}

// ---------------------------------------------------------------------------
// B2 — escopo é escopo: `EXISTE_EM_ALGUM_ESCOPO` != `ESTÁ_VISÍVEL_NESTE_PONTO`
//
// O censo de nomes ligados era um saco de nomes do arquivo inteiro, e por isso
// uma ligação local em QUALQUER função desabilitava a família em TODAS. Os
// casos abaixo têm oráculo POSITIVO — o chamado canonicalizado — e não apenas
// a ausência da palavra "família" numa mensagem de erro.
// ---------------------------------------------------------------------------

/// A AST contém uma chamada à identidade canônica?
///
/// Oráculo POSITIVO que não depende de a chamada ser o primeiro comando do
/// corpo, o que `chamado_do_programa` exige.
fn arvore_de(fonte: &str) -> String {
    pinker_v0::printer::render_program(&parse(fonte).expect("programa válido"))
}

fn canonicalizou(fonte: &str, canonica: &str) -> bool {
    arvore_de(fonte).contains(&format!("callee Ident({canonica})"))
}

/// T2 — local em `f` não pode desabilitar a família em `principal`.
#[test]
fn t2_local_em_uma_funcao_nao_desabilita_a_familia_em_outra() {
    assert_eq!(
        chamado_do_programa(
            "pacote main;
             trazer arquivo;
             carinho f() -> bombom { nova arquivo: bombom = 3; mimo arquivo; }
             carinho principal() -> bombom { arquivo.criar(\"x\"); mimo 0; }"
        ),
        "criar_arquivo"
    );
}

/// T3 — parâmetro em `f` não pode desabilitar a família em `principal`.
///
/// É o repro literal do review: a ligação local vence em `f`, e a família
/// continua disponível em `principal`.
#[test]
fn t3_parametro_em_uma_funcao_nao_desabilita_a_familia_em_outra() {
    let fonte = "pacote main;
         trazer arquivo;
         carinho f(arquivo: bombom) -> bombom { mimo arquivo; }
         carinho principal() -> bombom { arquivo.criar(\"x\"); mimo 0; }";
    assert_eq!(chamado_do_programa(fonte), "criar_arquivo");

    // E o parâmetro continua sendo o parâmetro dentro de `f`: nada nele foi
    // reescrito para a família.
    let arvore = arvore_de(fonte);
    assert!(
        arvore.contains("value Ident(arquivo)"),
        "o parâmetro homônimo sumiu da AST: {arvore}"
    );
}

/// T3 (forma seletiva) — local homônimo do MEMBRO em `f` não pode desabilitar
/// o membro importado em `principal`.
#[test]
fn t3b_local_homonimo_de_membro_nao_desabilita_o_seletivo_em_outra_funcao() {
    assert_eq!(
        chamado_do_programa(
            "pacote main;
             trazer arquivo.criar;
             carinho f() -> bombom { nova criar: bombom = 3; mimo criar; }
             carinho principal() -> bombom { criar(\"x\"); mimo 0; }"
        ),
        "criar_arquivo"
    );
}

/// T4 — campo de `ninho` não é nome no espaço de valores e não pode sombrear a
/// família no arquivo inteiro.
#[test]
fn t4_campo_de_ninho_nao_desabilita_a_familia_no_arquivo() {
    assert_eq!(
        chamado_do_programa(
            "pacote main;
             trazer arquivo;
             ninho Registro { arquivo: bombom; }
             carinho principal() -> bombom { arquivo.criar(\"x\"); mimo 0; }"
        ),
        "criar_arquivo"
    );
}

/// T5 — no MESMO escopo a ligação local continua vencendo, e o acesso a campo
/// histórico continua sendo construído.
#[test]
fn t5_ligacao_no_mesmo_escopo_ainda_sombreia_a_familia() {
    let fonte = "pacote main;
         trazer arquivo;
         ninho Estado { criar: bombom; }
         carinho fabricar() -> Estado { nova e: seta<Estado> = 1; mimo *e; }
         carinho principal() -> bombom {
             nova arquivo = fabricar();
             mimo arquivo.criar;
         }";
    parse_and_check(fonte).expect("o campo histórico tem de continuar resolvendo");
    let arvore = pinker_v0::printer::render_program(&parse(fonte).expect("programa válido"));
    assert!(
        arvore.contains("FieldAccess"),
        "a família capturou uma ligação do próprio escopo: {arvore}"
    );
    assert!(
        !arvore.contains("criar_arquivo"),
        "a família canonicalizou por cima do local: {arvore}"
    );
}

/// T5 (bloco interno) — a ligação de um bloco aninhado não escapa para o bloco
/// externo. Sem pilha de escopos isto era indistinguível de T5.
#[test]
fn t5b_ligacao_de_bloco_interno_nao_escapa_para_o_bloco_externo() {
    assert!(canonicalizou(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             talvez 1 == 1 { nova arquivo: bombom = 3; falar(arquivo); }
             arquivo.criar(\"x\");
             mimo 0;
         }",
        "criar_arquivo"
    ));
}

/// T6 — identidade de TOPO declarada depois do uso continua vencendo, porque é
/// assim que a Pinker resolve o topo: a passagem 1 da semântica coleta todos os
/// itens antes de verificar qualquer corpo.
#[test]
fn t6_identidade_de_topo_posterior_vence_pela_regra_historica() {
    for declaracao in [
        "carinho arquivo(x: bombom) -> bombom { mimo x; }",
        "eterno arquivo: bombom = 3;",
        "ninho arquivo { criar: bombom; }",
        "apelido arquivo = bombom;",
    ] {
        let fonte = format!(
            "pacote main;
             trazer arquivo;
             carinho principal() -> bombom {{ falar(arquivo.criar(\"x\")); mimo 0; }}
             {declaracao}"
        );
        let arvore = pinker_v0::printer::render_program(&parse(&fonte).expect("programa válido"));
        assert!(
            !arvore.contains("criar_arquivo"),
            "a família capturou um nome de topo declarado depois do uso: {declaracao}\n{arvore}"
        );
    }
}

/// T6 (contraprova) — ligação LOCAL declarada depois do uso NÃO é hoisted, e a
/// Parte G não pode inventar hoisting onde a Pinker não tem.
#[test]
fn t6b_ligacao_local_posterior_nao_e_hoisted() {
    assert_eq!(
        chamado_do_programa(
            "pacote main;
             trazer arquivo;
             carinho principal() -> bombom {
                 arquivo.criar(\"x\");
                 nova arquivo: bombom = 3;
                 mimo arquivo;
             }"
        ),
        "criar_arquivo"
    );
}

/// T7 — sem módulo real ao lado, o import seletivo continua canonicalizando no
/// caminho de biblioteca, que é o que a crate expõe.
#[test]
fn t7_seletivo_canonicaliza_no_caminho_de_biblioteca() {
    for (familia, membro, canonica) in MATRIZ_APROVADA {
        let fonte = format!(
            "pacote main;
             trazer {familia}.{membro};
             carinho principal() -> bombom {{ {membro}(); mimo 0; }}"
        );
        assert_eq!(
            &chamado_do_programa(&fonte).as_str(),
            canonica,
            "{familia}.{membro} deixou de canonicalizar na forma seletiva"
        );
    }
}

/// A carga de um braço `caso` liga nome, e liga só naquele braço.
#[test]
fn carga_de_encaixe_fica_no_proprio_braco() {
    let fonte = "pacote main;
         trazer arquivo;
         leque Passo { Um(bombom), Dois(bombom) }
         carinho principal() -> bombom {
             nova p: Passo = Passo.Um(1);
             encaixe p {
                 caso Passo.Um(arquivo) { falar(arquivo); }
                 caso Passo.Dois(outro) { falar(outro); }
             }
             arquivo.criar(\"x\");
             mimo 0;
         }";
    let arvore = pinker_v0::printer::render_program(&parse(fonte).expect("programa válido"));
    assert!(
        arvore.contains("criar_arquivo"),
        "a carga de um braço desabilitou a família fora dele: {arvore}"
    );
}

/// Parâmetro de `carinho` anônimo também liga nome — e o caminho dele não passa
/// por `parse_callable_body`.
#[test]
fn parametro_de_carinho_anonimo_sombreia_dentro_da_closure() {
    // Dentro da closure o parâmetro vence: o acesso a campo histórico continua
    // sendo construído.
    let dentro = "pacote main;
         trazer arquivo;
         ninho Estado { criar: bombom; }
         carinho principal() -> bombom {
             nova f = carinho(arquivo: Estado) -> bombom { mimo arquivo.criar; };
             mimo 0;
         }";
    let arvore = arvore_de(dentro);
    assert!(
        arvore.contains("FieldAccess"),
        "a família capturou o parâmetro do `carinho` anônimo: {arvore}"
    );
    assert!(
        !arvore.contains("criar_arquivo"),
        "a família canonicalizou por cima do parâmetro da closure: {arvore}"
    );

    // E o escopo dele fecha com a closure: fora dela a família responde.
    assert!(canonicalizou(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova f = carinho(arquivo: bombom) -> bombom { mimo arquivo; };
             arquivo.criar(\"x\");
             mimo f(1);
         }",
        "criar_arquivo"
    ));
}

/// A variável de `para cada` vale dentro do corpo do laço, e só lá.
#[test]
fn variavel_de_para_cada_nao_escapa_do_corpo() {
    assert!(canonicalizou(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova xs: lista<bombom> = lista_nova();
             para cada arquivo em xs { falar(arquivo); }
             arquivo.criar(\"x\");
             mimo 0;
         }",
        "criar_arquivo"
    ));
}

/// Método de `trato` não é identidade de topo.
///
/// Ele vive em profundidade 1 e não pode ser chamado como `arquivo(...)`: não
/// disputa o espaço de valores do arquivo e, portanto, não sombreia a família.
/// É o caso que o filtro de profundidade zero do censo existe para separar.
#[test]
fn metodo_de_trato_homonimo_de_familia_nao_e_identidade_de_topo() {
    assert!(canonicalizou(
        "pacote main;
         trazer arquivo;
         trato Guardiao {
             carinho arquivo(item: si) -> bombom;
         }
         carinho principal() -> bombom { arquivo.criar(\"x\"); mimo 0; }",
        "criar_arquivo"
    ));
}

/// Campo de `ninho` idem — mesma profundidade, mesma razão. O par com o teste
/// acima é o que dá ao filtro de profundidade um detector de verdade.
#[test]
fn campo_de_ninho_homonimo_de_familia_nao_e_identidade_de_topo() {
    assert!(canonicalizou(
        "pacote main;
         trazer arquivo;
         ninho Registro { arquivo: bombom; }
         carinho principal() -> bombom { arquivo.criar(\"x\"); mimo 0; }",
        "criar_arquivo"
    ));
}

/// A variável de `para cada` vence dentro do corpo do laço.
///
/// Sem esta ligação o corpo do laço seria o único lugar do arquivo onde a
/// família captura um nome que o programador acabou de ligar.
#[test]
fn variavel_de_para_cada_sombreia_dentro_do_corpo() {
    let arvore = arvore_de(
        "pacote main;
         trazer arquivo;
         carinho principal() -> bombom {
             nova xs: lista<bombom> = lista_nova();
             para cada arquivo em xs { falar(arquivo.criar); }
             mimo 0;
         }",
    );
    assert!(
        arvore.contains("FieldAccess"),
        "a família capturou a variável do laço: {arvore}"
    );
    assert!(
        !arvore.contains("criar_arquivo"),
        "a família canonicalizou por cima da variável do laço: {arvore}"
    );
}

/// A ligação de um braço de `tentar` vence dentro do braço, e só nele.
#[test]
fn ligacao_de_braco_de_tentar_sombreia_dentro_do_braco() {
    let fonte = "pacote main;
         trazer arquivo;
         leque Resultado { Ok(bombom), Erro(verso) }
         carinho principal() -> bombom {
             nova r: Resultado = Resultado.Ok(1);
             tentar r {
                 sucesso Resultado.Ok(arquivo) { falar(arquivo.criar); }
                 falha Resultado.Erro(msg) { falar(msg); }
             }
             mimo 0;
         }";
    let arvore = arvore_de(fonte);
    assert!(
        arvore.contains("FieldAccess"),
        "a família capturou a ligação do braço de `tentar`: {arvore}"
    );
    assert!(
        !arvore.contains("criar_arquivo"),
        "a família canonicalizou por cima da ligação do braço: {arvore}"
    );

    // E o escopo do braço fecha: depois do `tentar` a família responde.
    assert!(canonicalizou(
        "pacote main;
         trazer arquivo;
         leque Resultado { Ok(bombom), Erro(verso) }
         carinho principal() -> bombom {
             nova r: Resultado = Resultado.Ok(1);
             tentar r {
                 sucesso Resultado.Ok(arquivo) { falar(arquivo); }
                 falha Resultado.Erro(msg) { falar(msg); }
             }
             arquivo.criar(\"x\");
             mimo 0;
         }",
        "criar_arquivo"
    ));
}

/// A ligação de um braço de encaixe de UNIÃO vence dentro do braço, e só nele.
#[test]
fn ligacao_de_braco_de_uniao_sombreia_dentro_do_braco() {
    let fonte = "pacote main;
         trazer arquivo;
         apelido aa = u8;
         apelido zz = u64;
         carinho principal() -> bombom {
             nova valor: uniao<aa, zz> = (8 virar zz) virar uniao<aa, zz>;
             encaixe valor {
                 caso aa(arquivo) { falar(arquivo.criar); }
                 caso zz(outro) { falar(outro virar bombom); }
             }
             mimo 0;
         }";
    let arvore = arvore_de(fonte);
    assert!(
        arvore.contains("FieldAccess"),
        "a família capturou a ligação do braço de união: {arvore}"
    );
    assert!(
        !arvore.contains("criar_arquivo"),
        "a família canonicalizou por cima da ligação do braço de união: {arvore}"
    );
}
// @pinker-nav:end evidencia.importacoes.parte-g-diagnosticos-e-precedencia

// @pinker-nav:start evidencia.importacoes.parte-g-carregador-e-paridade
// @pinker-nav:domain importacoes
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência de ponta a ponta da Parte G pela CLI: `trazer familia.membro;` deixa de procurar `<familia>.pink` e passa pela autoridade de import, enquanto módulo Pinker comum — inteiro, seletivo e misturado com família no mesmo arquivo — mantém o comportamento histórico, inclusive as duas colisões. `REAL_MODULE_X > BUILTIN_FAMILY_X` é provado no caso em que o export do módulo COINCIDE com membro aprovado, por execução e por efeito colateral no disco, e o export ausente de um módulo real continua dando o erro do módulo; a identidade homônima trazida por `trazer <modulo>;` é recusada em vez de capturada, nas duas ordens de import, e continua resolvendo historicamente quando a família não é importada. A matriz de paridade emite o MESMO programa nas três grafias sobre uma fixture com symlink e diretório controlados, executa cada um no interpretador e no ELF nativo e exige observáveis idênticos entre as seis execuções — o que fixa junto o par FOLLOW (`tamanho_arquivo` = 4) × NO_FOLLOW (`tamanho_de_entrada` = 6) medido na mesma entrada.

fn escrever(dir: &NativeArtifactDir, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.path().join(nome);
    fs::write(&caminho, fonte).expect("escrever fonte da Parte G");
    caminho
}

fn checar(caminho: &Path, caso: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--check")
        .arg(caminho)
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("checar Parte G sob envelope")
}

fn stderr_de(saida: &Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

/// O defeito corrigido: com símbolo, a família caía no carregador de módulo e
/// virava "módulo 'arquivo' não encontrado". Agora, **quando não existe módulo
/// real**, a família responde.
///
/// A ausência de `<familia>.pink` é o pressuposto deste caso, e não uma
/// promessa de que ninguém olha para o disco: `REAL_MODULE_X > BUILTIN_FAMILY_X`
/// exige perguntar primeiro se o módulo resolve. Quem fixa o outro lado é
/// `f6_modulo_real_homonimo_de_familia_vence_a_familia`.
#[test]
fn import_seletivo_de_familia_resolve_quando_nao_ha_modulo_real() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    let valido = escrever(
        &dir,
        "seletivo_valido.pink",
        "pacote main;
trazer arquivo.ler_caminho_verso;
carinho principal() -> bombom { falar(ler_caminho_verso(\"x\")); mimo 0; }
",
    );
    let saida = checar(&valido, "parte-g-seletivo-valido");
    assert!(
        saida.status.success(),
        "import seletivo de membro aprovado deveria passar: {}",
        stderr_de(&saida)
    );

    let invalido = escrever(
        &dir,
        "seletivo_invalido.pink",
        "pacote main;
trazer arquivo.nao_existe;
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&invalido, "parte-g-seletivo-invalido");
    assert!(!saida.status.success(), "membro inexistente deveria falhar");
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("membro 'nao_existe' não existe na família 'arquivo'"),
        "{erro}"
    );
    assert!(
        !erro.contains("não encontrado"),
        "sem módulo real ao lado, quem recusa é a família, não o carregador: {erro}"
    );
    assert!(!erro.contains(".pink"), "{erro}");
}

/// F6 — `REAL_MODULE_X > BUILTIN_FAMILY_X`.
///
/// Um módulo Pinker chamado como uma família existia antes desta Parte e
/// continua vencendo, com semântica histórica INTEIRA: o export bom resolve, e
/// o export inexistente dá o erro histórico do módulo, não o da família. A
/// primeira versão desta Parte reservava os sete nomes e arrancava esses
/// programas do usuário.
#[test]
fn f6_modulo_real_homonimo_de_familia_vence_a_familia() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "texto.pink",
        "pacote texto;
carinho ajudante(x: bombom) -> bombom { mimo x + 1; }
",
    );

    let bom = escrever(
        &dir,
        "modulo_vence.pink",
        "pacote main;
trazer texto.ajudante;
carinho principal() -> bombom { mimo ajudante(1); }
",
    );
    let saida = checar(&bom, "parte-g-f6-modulo-vence");
    assert!(
        saida.status.success(),
        "módulo real homônimo de família tem de continuar carregando: {}",
        stderr_de(&saida)
    );

    // E o erro de export inexistente é o do MÓDULO, não o da família: a
    // precedência não pode ser decidida perguntando "a família tem esse
    // membro?", senão o módulo perderia justamente os nomes coincidentes.
    let ruim = escrever(
        &dir,
        "modulo_export_ausente.pink",
        "pacote main;
trazer texto.nao_existe;
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&ruim, "parte-g-f6-export-ausente");
    assert!(!saida.status.success(), "export inexistente deveria falhar");
    let erro = stderr_de(&saida);
    assert!(
        !erro.contains("não existe na família"),
        "com módulo real ao lado, a família não responde: {erro}"
    );
}

/// F1, segunda linha de defesa: identidade que o censo NÃO pode enxergar.
///
/// `trazer deposito;` traz todos os itens do módulo, e os nomes deles não
/// aparecem no fluxo de tokens deste arquivo — o censo é cego para eles por
/// construção. Aqui a única coisa que protege o programa é
/// `FAMILY_RESOLUTION_IS_FALLBACK`: sem `trazer arquivo;`, a família não opina
/// e o `leque` importado responde por `arquivo.criar`.
///
/// O nome da variante é `criar` de propósito: é membro aprovado de `arquivo`.
/// Com uma variante qualquer o caso não morde, porque a recusa antiga só
/// disparava quando o campo era grafia de membro — foi assim que a primeira
/// versão deste teste passou sem tocar no sítio.
#[test]
fn item_de_modulo_inteiro_homonimo_de_familia_vence_sem_import_de_familia() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "deposito.pink",
        "pacote deposito;
leque arquivo { criar, fechar }
",
    );

    let fonte = escrever(
        &dir,
        "usa_leque_importado.pink",
        "pacote main;
trazer deposito;
carinho principal() -> bombom {
    nova e: arquivo = arquivo.criar;
    mimo 0;
}
",
    );
    let saida = checar(&fonte, "parte-g-leque-de-modulo-inteiro");
    assert!(
        saida.status.success(),
        "identidade trazida por módulo inteiro é invisível ao censo e só o \
         fallback a protege: {}",
        stderr_de(&saida)
    );
}

/// F6 — sem módulo real, o mesmo `trazer texto.<x>;` volta a ser assunto da
/// família. O par com o teste acima é o que prova que a precedência é
/// decidida pela existência do módulo, e não pelo conteúdo do registro.
#[test]
fn f6_sem_modulo_real_a_familia_responde_pelo_mesmo_texto() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    let fonte = escrever(
        &dir,
        "sem_modulo.pink",
        "pacote main;
trazer texto.nao_existe;
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&fonte, "parte-g-f6-sem-modulo");
    assert!(!saida.status.success(), "membro inexistente deveria falhar");
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("não existe na família 'texto'"),
        "sem módulo real, quem recusa é a família: {erro}"
    );
}

/// Nome de família e nome de módulo compartilham o espaço lexical: um nome que
/// não é família continua sendo caminho de módulo, e a recusa é a histórica.
#[test]
fn modulo_pinker_ausente_continua_com_a_recusa_historica() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    let fonte = escrever(
        &dir,
        "modulo_ausente.pink",
        "pacote main;
trazer zzz_nao_existe.membro;
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&fonte, "parte-g-modulo-ausente");
    assert!(!saida.status.success());
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("módulo 'zzz_nao_existe' não encontrado"),
        "{erro}"
    );
}

/// Import de módulo Pinker comum permanece íntegro — inteiro, seletivo e
/// misturado com import de família no mesmo arquivo.
#[test]
fn modulo_pinker_comum_permanece_integro() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    escrever(
        &dir,
        "biblioteca.pink",
        "pacote biblioteca;
carinho dobro(n: bombom) -> bombom { mimo n * 2; }
carinho triplo(n: bombom) -> bombom { mimo n * 3; }
",
    );

    let inteiro = escrever(
        &dir,
        "usa_inteiro.pink",
        "pacote main;
trazer biblioteca;
carinho principal() -> bombom { falar(dobro(21)); mimo 0; }
",
    );
    let saida = checar(&inteiro, "parte-g-modulo-inteiro");
    assert!(saida.status.success(), "{}", stderr_de(&saida));

    let seletivo = escrever(
        &dir,
        "usa_seletivo.pink",
        "pacote main;
trazer biblioteca.triplo;
carinho principal() -> bombom { falar(triplo(14)); mimo 0; }
",
    );
    let saida = checar(&seletivo, "parte-g-modulo-seletivo");
    assert!(saida.status.success(), "{}", stderr_de(&saida));

    let misto = escrever(
        &dir,
        "usa_misto.pink",
        "pacote main;
trazer biblioteca.dobro;
trazer arquivo;
trazer caminho.arquivo_vazio;
carinho principal() -> bombom {
    falar(dobro(21));
    falar(arquivo.ler_caminho_ou(\"ausente.txt\", \"padrao\"));
    falar(arquivo_vazio(\"ausente.txt\"));
    mimo 0;
}
",
    );
    let saida = checar(&misto, "parte-g-modulo-misto");
    assert!(saida.status.success(), "{}", stderr_de(&saida));
}

/// As duas recusas de colisão são as MESMAS do import de módulo comum, e a
/// segunda só é observável quando família e módulo trazem o mesmo nome.
#[test]
fn colisoes_de_import_seletivo_reusam_a_politica_existente() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    let com_topo = escrever(
        &dir,
        "colisao_topo.pink",
        "pacote main;
trazer arquivo.criar;
carinho criar(caminho: verso) -> bombom { mimo 0; }
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&com_topo, "parte-g-colisao-topo");
    assert!(!saida.status.success());
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("colisão de nome no import: 'criar' já existe no arquivo principal"),
        "{erro}"
    );

    escrever(
        &dir,
        "outra.pink",
        "pacote outra;
carinho criar(caminho: verso) -> bombom { mimo 0; }
",
    );
    let com_modulo = escrever(
        &dir,
        "colisao_modulo.pink",
        "pacote main;
trazer arquivo.criar;
trazer outra.criar;
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&com_modulo, "parte-g-colisao-modulo");
    assert!(!saida.status.success());
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("colisão de nome no import: 'criar' trazido por múltiplos módulos"),
        "{erro}"
    );

    // A ordem inversa é o caso que SÓ o ramo de família decide: com o módulo
    // primeiro, quem tem de recusar é a checagem de `imported_names` dentro do
    // ramo de família. Sem este caso, desligar aquela checagem não muda nada
    // observável — o outro ramo cobre a ordem direta e o mutante sobrevive.
    let invertido = escrever(
        &dir,
        "colisao_modulo_invertido.pink",
        "pacote main;
trazer outra.criar;
trazer arquivo.criar;
carinho principal() -> bombom { mimo 0; }
",
    );
    let saida = checar(&invertido, "parte-g-colisao-modulo-invertido");
    assert!(
        !saida.status.success(),
        "colisão entre imports não pode depender da ordem"
    );
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("colisão de nome no import: 'criar' trazido por múltiplos módulos"),
        "{erro}"
    );
}

// ----- matriz de paridade: 29 superfícies × 3 grafias × 2 modos -----

/// Fixture determinística. O symlink é o que torna FOLLOW × NO_FOLLOW
/// observável: o alvo tem 4 bytes e o nome do alvo tem 6 caracteres.
fn montar_fixture(raiz: &Path, nome: &str) -> PathBuf {
    let base = raiz.join(nome);
    fs::create_dir_all(base.join("lista")).expect("criar fixture");
    fs::write(base.join("alvo.txt"), "conteudo do arquivo\n").expect("alvo");
    fs::write(base.join("numero.txt"), "4242\n").expect("numero");
    fs::write(base.join("sem_conteudo.txt"), "").expect("vazio");
    fs::write(base.join("ok.txt"), "abcd").expect("ok");
    // Bytes que NÃO são UTF-8 válido: um hash de arquivo que recuse metade dos
    // arquivos não é hash de arquivo, e é isso que este byte prova.
    fs::write(base.join("binario.bin"), [0xffu8, 0xfe, 0x00, 0x41]).expect("binario");
    std::os::unix::fs::symlink("ok.txt", base.join("link_arq")).expect("symlink");
    for entrada in ["a.txt", "b.txt", "c.txt"] {
        fs::write(base.join("lista").join(entrada), "x").expect("entrada de lista");
    }
    base
}

/// Emite o MESMO programa em qualquer das três grafias.
///
/// A ligação membro -> identidade continua vindo da autoridade; aqui só se
/// escolhe como a chamada é escrita.
fn fonte_matriz(grafia: Grafia) -> String {
    let a = |membro: &str, args: String| chamada_na_grafia(grafia, "arquivo", membro, &args);
    let p = |membro: &str, args: String| chamada_na_grafia(grafia, "caminho", membro, &args);
    let i = |membro: &str, args: String| chamada_na_grafia(grafia, "integridade", membro, &args);
    let usados: Vec<(&str, &str)> = MATRIZ_CANONICALIZACAO
        .iter()
        .map(|chamada| (chamada.familia, chamada.membro))
        .collect();

    let alvo = p("juntar", "base, \"alvo.txt\"".to_string());
    let vazio = p("juntar", "base, \"sem_conteudo.txt\"".to_string());
    let ausente = p("juntar", "base, \"ausente.txt\"".to_string());
    let numero = p("juntar", "base, \"numero.txt\"".to_string());
    let link = p("juntar", "base, \"link_arq\"".to_string());
    let binario = p("juntar", "base, \"binario.bin\"".to_string());
    let pasta = p("juntar", "base, \"lista\"".to_string());

    let bloco_falivel = format!(
        r#"    tentar {tamanho_entrada_link} {{
        sucesso ResBV.Ok(n) {{ falar(n); }}
        falha ResBV.Erro(e) {{ falar("ERRO_TAMANHO_ENTRADA"); }}
    }}
    tentar {tipo_entrada_link} {{
        sucesso ResTE.Ok(tp) {{
            escolha tp {{
                caso TipoEntrada.Arquivo {{ falar("Arquivo"); }}
                caso TipoEntrada.Diretorio {{ falar("Diretorio"); }}
                caso TipoEntrada.Symlink {{ falar("Symlink"); }}
                caso TipoEntrada.Outro {{ falar("Outro"); }}
            }}
        }}
        falha ResTE.Erro(e) {{ falar("ERRO_TIPO_ENTRADA"); }}
    }}
    tentar {ler_resultado_alvo} {{
        sucesso ResVV.Ok(texto) {{ falar(texto); }}
        falha ResVV.Erro(e) {{ falar("ERRO_LEITURA_OK"); }}
    }}
    tentar {ler_resultado_ausente} {{
        sucesso ResVV.Ok(texto) {{ falar(texto); }}
        falha ResVV.Erro(e) {{ falar("ERRO_LEITURA_AUSENTE"); }}
    }}
    tentar {sha_alvo} {{
        sucesso ResVV.Ok(digest) {{ falar(digest); }}
        falha ResVV.Erro(e) {{ falar("ERRO_SHA"); }}
    }}
    tentar {sha_binario} {{
        sucesso ResVV.Ok(digest) {{ falar(digest); }}
        falha ResVV.Erro(e) {{ falar("ERRO_SHA_BINARIO"); }}
    }}
    tentar {listar_pasta} {{
        sucesso ResLV.Ok(nomes) {{ falar(lista.tamanho(nomes)); }}
        falha ResLV.Erro(e) {{ falar("ERRO_LISTAR"); }}
    }}
"#,
        tamanho_entrada_link = p("tamanho_de_entrada", "link".to_string()),
        tipo_entrada_link = p("tipo_de_entrada", "link".to_string()),
        ler_resultado_alvo = a("ler_caminho_resultado", "alvo".to_string()),
        ler_resultado_ausente = a("ler_caminho_resultado", ausente.clone()),
        sha_alvo = i("sha256_arquivo", "alvo".to_string()),
        sha_binario = i("sha256_arquivo", binario.clone()),
        listar_pasta = p("listar_diretorio", pasta.clone()),
    );

    format!(
        r#"pacote main;
trazer ambiente.argumento_ou;
trazer lista;
{cabecalho}apelido ResVV = Resultado<verso, verso>;
apelido ResBV = Resultado<bombom, verso>;
apelido ResLV = Resultado<lista<verso>, verso>;
apelido ResTE = Resultado<TipoEntrada, verso>;

carinho principal() -> bombom {{
    nova base: verso = argumento_ou(0, "ausente");
    nova alvo: verso = {alvo};

    falar({ler_caminho_verso_alvo});
    falar({existe_alvo});
    falar({e_arquivo_alvo});
    falar({e_diretorio_base});
    falar({tamanho_alvo});
    falar({vazio_do_vazio});
    falar({vazio_do_alvo});
    falar({ou_ausente});
    falar({ou_alvo});

    nova hn: bombom = {abrir_numero};
    falar({ler_bombom_hn});
    {fechar_hn};

    nova hv: bombom = {abrir_alvo};
    falar({ler_verso_hv});
    {fechar_hv};

    nova trab: verso = {trab};
    {criar_dir_trab};
    nova w: verso = {w};
    nova hw: bombom = {criar_w};
    {escrever_verso_hw};
    {fechar_hw};
    falar({ler_w});
    nova ha: bombom = {abrir_anexo_w};
    {anexar_ha};
    {fechar_ha};
    falar({ler_w2});

    nova num: verso = {num};
    nova hnum: bombom = {criar_num};
    {escrever_bombom_hnum};
    {fechar_hnum};
    nova hr: bombom = {abrir_num};
    falar({ler_bombom_hr});
    {fechar_hr};

    nova t: verso = {t};
    nova ht: bombom = {criar_t};
    {escrever_verso_ht};
    {truncar_ht};
    {fechar_ht};
    falar({tamanho_t});

    nova c1: verso = {c1};
    {copiar_w_c1};
    falar({ler_c1});
    nova c2: verso = {c2};
    {renomear_c1_c2};
    falar({existe_c1});
    falar({ler_c2});

    falar({existe_cwd});

    nova link: verso = {link};
    falar({tamanho_link});
    falar({vazio_do_link});
    falar({ou_diretorio});
{bloco_falivel}
    {remover_w};
    {remover_c2};
    {remover_num};
    {remover_t};
    {remover_trab};
    falar({existe_trab});
    mimo 0;
}}
"#,
        cabecalho = cabecalho(grafia, &usados),
        alvo = alvo,
        ler_caminho_verso_alvo = a("ler_caminho_verso", "alvo".to_string()),
        existe_alvo = p("existe", "alvo".to_string()),
        e_arquivo_alvo = p("e_arquivo", "alvo".to_string()),
        e_diretorio_base = p("e_diretorio", "base".to_string()),
        tamanho_alvo = p("tamanho_arquivo", "alvo".to_string()),
        vazio_do_vazio = p("arquivo_vazio", vazio.clone()),
        vazio_do_alvo = p("arquivo_vazio", "alvo".to_string()),
        ou_ausente = a("ler_caminho_ou", format!("{ausente}, \"padrao\"")),
        ou_alvo = a("ler_caminho_ou", "alvo, \"padrao\"".to_string()),
        abrir_numero = a("abrir", numero.clone()),
        ler_bombom_hn = a("ler_bombom", "hn".to_string()),
        fechar_hn = a("fechar", "hn".to_string()),
        abrir_alvo = a("abrir", "alvo".to_string()),
        ler_verso_hv = a("ler_verso", "hv".to_string()),
        fechar_hv = a("fechar", "hv".to_string()),
        trab = p("juntar", "base, \"trabalho\"".to_string()),
        criar_dir_trab = p("criar_diretorio", "trab".to_string()),
        w = p("juntar", "trab, \"w.txt\"".to_string()),
        criar_w = a("criar", "w".to_string()),
        escrever_verso_hw = a("escrever_verso", "hw, \"rosa\"".to_string()),
        fechar_hw = a("fechar", "hw".to_string()),
        ler_w = a("ler_caminho_verso", "w".to_string()),
        abrir_anexo_w = a("abrir_anexo", "w".to_string()),
        anexar_ha = a("anexar_verso", "ha, \" pinker\"".to_string()),
        fechar_ha = a("fechar", "ha".to_string()),
        ler_w2 = a("ler_caminho_verso", "w".to_string()),
        num = p("juntar", "trab, \"num.txt\"".to_string()),
        criar_num = a("criar", "num".to_string()),
        escrever_bombom_hnum = a("escrever_bombom", "hnum, 4242".to_string()),
        fechar_hnum = a("fechar", "hnum".to_string()),
        abrir_num = a("abrir", "num".to_string()),
        ler_bombom_hr = a("ler_bombom", "hr".to_string()),
        fechar_hr = a("fechar", "hr".to_string()),
        t = p("juntar", "trab, \"t.txt\"".to_string()),
        criar_t = a("criar", "t".to_string()),
        escrever_verso_ht = a("escrever_verso", "ht, \"conteudo\"".to_string()),
        truncar_ht = a("truncar", "ht".to_string()),
        fechar_ht = a("fechar", "ht".to_string()),
        tamanho_t = p("tamanho_arquivo", "t".to_string()),
        c1 = p("juntar", "trab, \"c1.txt\"".to_string()),
        copiar_w_c1 = a("copiar", "w, c1".to_string()),
        ler_c1 = a("ler_caminho_verso", "c1".to_string()),
        c2 = p("juntar", "trab, \"c2.txt\"".to_string()),
        renomear_c1_c2 = a("renomear", "c1, c2".to_string()),
        existe_c1 = p("existe", "c1".to_string()),
        ler_c2 = a("ler_caminho_verso", "c2".to_string()),
        existe_cwd = p("existe", p("diretorio_atual", String::new())),
        link = link,
        tamanho_link = p("tamanho_arquivo", "link".to_string()),
        vazio_do_link = p("arquivo_vazio", "link".to_string()),
        ou_diretorio = a("ler_caminho_ou", "base, \"padrao\"".to_string()),
        bloco_falivel = bloco_falivel,
        remover_w = p("remover_arquivo", "w".to_string()),
        remover_c2 = p("remover_arquivo", "c2".to_string()),
        remover_num = p("remover_arquivo", "num".to_string()),
        remover_t = p("remover_arquivo", "t".to_string()),
        remover_trab = p("remover_diretorio", "trab".to_string()),
        existe_trab = p("existe", "trab".to_string()),
    )
}

/// Observáveis esperados da matriz. Escritos por extenso para que o teste
/// tenha oráculo próprio e não passe por comparar erro com erro.
const OBSERVAVEIS_ESPERADOS: &str = "conteudo do arquivo\n\
\n\
verdade\n\
verdade\n\
verdade\n\
20\n\
verdade\n\
falso\n\
padrao\n\
conteudo do arquivo\n\
\n\
4242\n\
conteudo do arquivo\n\
\n\
rosa\n\
rosa pinker\n\
4242\n\
0\n\
rosa pinker\n\
falso\n\
rosa pinker\n\
verdade\n\
4\n\
falso\n\
padrao\n\
6\n\
Symlink\n\
conteudo do arquivo\n\
\n\
ERRO_LEITURA_AUSENTE\n\
75c3cbd32fa48365578e3606ac7c147dfc1e2b05754affb0729a59e054bc44e6\n\
6e153708ea1302ccc480999bda6939c7aef6dd60531b7acfff00e81bde4986ab\n\
3\n\
falso\n";

fn rodar_interpretado(caminho: &Path, caso: &str, base: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(caminho)
        .arg("--")
        .arg(base)
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar Parte G no interpretador")
}

#[test]
fn matriz_29_tem_observaveis_identicos_nas_tres_grafias_no_interpretador() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    for grafia in Grafia::TODAS {
        let base = montar_fixture(dir.path(), &format!("fix-int-{}", grafia.nome()));
        let fonte = escrever(
            &dir,
            &format!("matriz_{}.pink", grafia.nome()),
            &fonte_matriz(grafia),
        );
        let saida = rodar_interpretado(&fonte, &format!("parte-g-int-{}", grafia.nome()), &base);
        assert!(
            saida.status.success(),
            "grafia {} falhou no interpretador: {}",
            grafia.nome(),
            stderr_de(&saida)
        );
        assert_eq!(
            String::from_utf8_lossy(&saida.stdout),
            OBSERVAVEIS_ESPERADOS,
            "observáveis divergentes na grafia {}",
            grafia.nome()
        );
    }
}

#[test]
fn matriz_29_tem_paridade_interpretador_nativo_nas_tres_grafias() {
    let Some((_driver, Some(runtime_lib))) = common::require_native_evidence(
        "matriz_29_tem_paridade_interpretador_nativo_nas_tres_grafias",
        true,
    ) else {
        return;
    };
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    for grafia in Grafia::TODAS {
        let nome = format!("matriz_{}", grafia.nome());
        let fonte = escrever(&dir, &format!("{nome}.pink"), &fonte_matriz(grafia));

        let base_interpretada = montar_fixture(dir.path(), &format!("fix-i-{}", grafia.nome()));
        let interpretado = rodar_interpretado(
            &fonte,
            &format!("parte-g-par-int-{}", grafia.nome()),
            &base_interpretada,
        );
        assert!(
            interpretado.status.success(),
            "interpretador falhou em {}: {}",
            grafia.nome(),
            stderr_de(&interpretado)
        );

        let build = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["build", "--nativo", "--out-dir"])
            .arg(dir.path())
            .arg(&fonte)
            .env("PINKER_RT_LIB", &runtime_lib)
            .logical_case(&format!("parte-g-build-{}", grafia.nome()))
            .timeout(Duration::from_secs(180))
            .output()
            .expect("compilar Parte G sob envelope");
        assert!(
            build.status.success(),
            "build nativo falhou em {}: {}",
            grafia.nome(),
            stderr_de(&build)
        );

        let base_nativa = montar_fixture(dir.path(), &format!("fix-n-{}", grafia.nome()));
        let nativo = Command::new(dir.path().join(&nome))
            .arg(&base_nativa)
            .logical_case(&format!("parte-g-nat-{}", grafia.nome()))
            .timeout(Duration::from_secs(60))
            .output()
            .expect("executar ELF da Parte G");

        assert_eq!(
            String::from_utf8_lossy(&interpretado.stdout),
            OBSERVAVEIS_ESPERADOS,
            "interpretador divergiu do oráculo em {}",
            grafia.nome()
        );
        assert_eq!(
            String::from_utf8_lossy(&nativo.stdout),
            OBSERVAVEIS_ESPERADOS,
            "nativo divergiu do oráculo em {}",
            grafia.nome()
        );
        assert_eq!(
            interpretado.status.code(),
            nativo.status.code(),
            "exit divergente em {}",
            grafia.nome()
        );
    }
}

/// Representação estrutural: as três grafias produzem os MESMOS artefatos em
/// todas as etapas do pipeline. Se a canonicalização deixasse de acontecer no
/// parser, isto veria a diferença antes de qualquer observável de execução.
///
/// Cobre junto o censo de sobrevivência: nenhuma grafia de membro chega a
/// nenhuma dessas etapas.
#[test]
fn as_tres_grafias_produzem_as_mesmas_representacoes() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    let fontes: Vec<(Grafia, PathBuf)> = Grafia::TODAS
        .iter()
        .map(|grafia| {
            (
                *grafia,
                escrever(
                    &dir,
                    &format!("repr_{}.pink", grafia.nome()),
                    &fonte_matriz(*grafia),
                ),
            )
        })
        .collect();

    for etapa in ["--ir", "--cfg-ir", "--selected", "--machine"] {
        let mut artefatos = Vec::new();
        for (grafia, fonte) in &fontes {
            let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
                .arg(etapa)
                .arg(fonte)
                .logical_case(&format!("parte-g-{}-{}", &etapa[2..], grafia.nome()))
                .timeout(Duration::from_secs(120))
                .output()
                .expect("gerar representação da Parte G");
            assert!(
                saida.status.success(),
                "{etapa} falhou em {}: {}",
                grafia.nome(),
                stderr_de(&saida)
            );
            let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
            assert!(!texto.is_empty(), "{etapa} vazio em {}", grafia.nome());
            let vocabulario = palavras(&texto);
            for exportacao in EXPORTACOES {
                let membro = exportacao.membro();
                if membro != exportacao.identidade.nome_publico() {
                    assert!(
                        !vocabulario.contains(membro),
                        "a grafia de membro '{membro}' chegou a {etapa} em {}",
                        grafia.nome()
                    );
                }
            }
            artefatos.push((*grafia, texto));
        }
        let (_, referencia) = &artefatos[0];
        for (grafia, texto) in &artefatos[1..] {
            assert_eq!(
                texto,
                referencia,
                "{etapa} da grafia {} divergiu do legado",
                grafia.nome()
            );
        }
    }
}

/// Recorte que o `.s` textual suporta hoje — sem slot local `verso` — com um
/// representante de cada classe que cabe nele: handle, caminho, predicado
/// FOLLOW e metadado FOLLOW. As classes que ele ainda não alcança
/// (`Resultado`, NO_FOLLOW, SHA-256) têm equivalência provada em
/// `as_tres_grafias_produzem_as_mesmas_representacoes` e paridade provada no
/// ELF nativo.
fn fonte_assembly(grafia: Grafia) -> String {
    let a = |membro: &str, args: &str| chamada_na_grafia(grafia, "arquivo", membro, args);
    let p = |membro: &str, args: &str| chamada_na_grafia(grafia, "caminho", membro, args);
    let usados = [
        ("arquivo", "abrir"),
        ("arquivo", "ler_bombom"),
        ("arquivo", "fechar"),
        ("caminho", "arquivo_vazio"),
        ("caminho", "tamanho_arquivo"),
        ("caminho", "existe"),
    ];
    format!(
        "pacote main;\n{}carinho principal() -> bombom {{\n    \
         nova h: bombom = {};\n    \
         falar({});\n    \
         {};\n    \
         falar({});\n    \
         falar({});\n    \
         falar({});\n    \
         mimo 0;\n}}\n",
        cabecalho(grafia, &usados),
        a("abrir", "\"numero.txt\""),
        a("ler_bombom", "h"),
        a("fechar", "h"),
        p("arquivo_vazio", "\"sem_conteudo.txt\""),
        p("tamanho_arquivo", "\"numero.txt\""),
        p("existe", "\"numero.txt\""),
    )
}

/// O `.s` textual, no recorte que ele suporta: mesmo assembly nas três
/// grafias, byte a byte.
#[test]
fn as_tres_grafias_produzem_o_mesmo_assembly() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    let mut assemblies = Vec::new();
    for grafia in Grafia::TODAS {
        let fonte = escrever(
            &dir,
            &format!("asm_{}.pink", grafia.nome()),
            &fonte_assembly(grafia),
        );
        let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--asm-s")
            .arg(&fonte)
            .logical_case(&format!("parte-g-asm-{}", grafia.nome()))
            .timeout(Duration::from_secs(120))
            .output()
            .expect("gerar assembly da Parte G");
        assert!(
            saida.status.success(),
            "assembly falhou em {}: {}",
            grafia.nome(),
            stderr_de(&saida)
        );
        let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
        assert!(!texto.is_empty(), "assembly vazio em {}", grafia.nome());
        let vocabulario = palavras(&texto);
        for exportacao in EXPORTACOES {
            let membro = exportacao.membro();
            if membro != exportacao.identidade.nome_publico() {
                assert!(
                    !vocabulario.contains(membro),
                    "a grafia de membro '{membro}' chegou ao assembly em {}",
                    grafia.nome()
                );
            }
        }
        assemblies.push((grafia, texto));
    }
    let (_, referencia) = &assemblies[0];
    for (grafia, texto) in &assemblies[1..] {
        assert_eq!(
            texto,
            referencia,
            "o assembly da grafia {} divergiu do legado",
            grafia.nome()
        );
    }
}

/// O limite do `.s` textual com slot `verso` vindo de `tentar` é
/// **pré-existente**: ele recusa a grafia LEGADA exatamente como recusa as
/// outras duas, com a mesma classe de erro. Registrar isso aqui impede que a
/// Parte G leve a culpa por ele — e faz o teste quebrar no dia em que o limite
/// for removido, em vez de deixá-lo esquecido.
///
/// A asserção nomeia a CLASSE do limite e o tipo `verso` que o provoca, mas não
/// qual slot o diagnóstico escolhe: `SelectedFunction::slot_types` é um
/// `HashMap`, e o validador recusa o primeiro slot não suportado que a
/// iteração entregar. Ora sai `verso`, ora sai `lista<verso>`, na mesma fonte.
/// É indeterminismo PRÉ-EXISTENTE do diagnóstico do backend `.s`, alheio a esta
/// Parte e alheio ao próprio limite — registrado aqui, não corrigido aqui.
#[test]
fn limite_do_backend_s_textual_e_pre_existente() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");
    for grafia in Grafia::TODAS {
        let fonte = escrever(
            &dir,
            &format!("limite_{}.pink", grafia.nome()),
            &fonte_matriz(grafia),
        );
        let saida = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--asm-s")
            .arg(&fonte)
            .logical_case(&format!("parte-g-limite-{}", grafia.nome()))
            .timeout(Duration::from_secs(120))
            .output()
            .expect("gerar assembly da Parte G");
        assert!(
            !saida.status.success(),
            "o limite pré-existente do .s textual desapareceu em {}; \
             se foi corrigido, uma o `.s` completo deve entrar na matriz",
            grafia.nome()
        );
        let erro = stderr_de(&saida);
        assert!(
            erro.contains("backend .s textual ainda não suporta slot") && erro.contains("verso"),
            "o .s textual passou a falhar por outro motivo em {}: {erro}",
            grafia.nome()
        );
    }
}

// ---------------------------------------------------------------------------
// B1 — `REAL_MODULE_X > BUILTIN_FAMILY_X` quando o export COINCIDE com membro
//
// O caso que faltava. Com `<familia>.pink` real ao lado exportando um nome que
// TAMBÉM é membro aprovado, o carregador decidia certo e o parser já tinha
// decidido errado: o módulo entrava no programa e o corpo já chamava a
// intrínseca. O oráculo aqui é a EXECUÇÃO, e a diferença entre as duas
// identidades é observável a olho nu.
// ---------------------------------------------------------------------------

fn executar(caminho: &Path, caso: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(caminho)
        .logical_case(caso)
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar Parte G sob envelope")
}

fn stdout_de(saida: &Output) -> String {
    String::from_utf8_lossy(&saida.stdout).into_owned()
}

/// T1 — módulo real chamado como família, exportando um nome que é membro
/// aprovado da família. A semântica histórica do módulo vence INTEGRALMENTE.
#[test]
fn t1_modulo_real_com_membro_coincidente_vence_a_familia() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "arquivo.pink",
        "pacote arquivo;
carinho criar(x: bombom) -> bombom { mimo x + 1000; }
",
    );

    let fonte = escrever(
        &dir,
        "usa_modulo_criar.pink",
        "pacote main;
trazer arquivo.criar;
carinho principal() -> bombom {
    falar(criar(1));
    mimo 0;
}
",
    );

    let saida = executar(&fonte, "parte-g-b1-modulo-membro-coincidente");
    assert!(
        saida.status.success(),
        "o módulo real tem de continuar sendo chamado: {}",
        stderr_de(&saida)
    );
    assert_eq!(
        stdout_de(&saida).trim(),
        "1001",
        "quem respondeu não foi o `criar` do módulo: {}",
        stderr_de(&saida)
    );
}

/// T1 (efeito colateral) — a prova de que a captura não é só de nome.
///
/// Aqui as assinaturas coincidem, então nenhum erro de tipo denuncia a troca:
/// o único sinal é o disco. `criar_arquivo` cria o arquivo; o `criar` do módulo
/// devolve 7 e não toca em nada. Se a família capturar, o arquivo aparece.
#[test]
fn t1b_modulo_real_com_membro_coincidente_nao_dispara_a_intrinseca() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "arquivo.pink",
        "pacote arquivo;
carinho criar(alvo: verso) -> bombom { mimo 7; }
",
    );

    let prova = dir.path().join("prova-de-captura.txt");
    let fonte = escrever(
        &dir,
        "usa_modulo_criar_verso.pink",
        &format!(
            "pacote main;
trazer arquivo.criar;
carinho principal() -> bombom {{
    falar(criar(\"{}\"));
    mimo 0;
}}
",
            prova.display()
        ),
    );

    let saida = executar(&fonte, "parte-g-b1-efeito-colateral");
    assert!(saida.status.success(), "{}", stderr_de(&saida));
    assert_eq!(stdout_de(&saida).trim(), "7", "{}", stderr_de(&saida));
    assert!(
        !prova.exists(),
        "a intrínseca da família rodou no lugar do módulo e criou {}",
        prova.display()
    );
}

/// T9 — com módulo real ao lado, um export ausente continua sendo erro DE
/// MÓDULO, inclusive quando o nome pedido é membro aprovado da família.
#[test]
fn t9_export_ausente_de_modulo_real_usa_o_erro_do_modulo() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "arquivo.pink",
        "pacote arquivo;
carinho outra_coisa(x: bombom) -> bombom { mimo x; }
",
    );

    let fonte = escrever(
        &dir,
        "export_ausente_membro_aprovado.pink",
        "pacote main;
trazer arquivo.criar;
carinho principal() -> bombom { mimo 0; }
",
    );

    let saida = checar(&fonte, "parte-g-t9-export-ausente");
    assert!(!saida.status.success(), "export ausente deveria falhar");
    let erro = stderr_de(&saida);
    assert!(
        erro.contains("símbolo 'criar' não encontrado no módulo 'arquivo'"),
        "o erro tem de ser o do módulo, não o da família: {erro}"
    );
}

/// T10 — identidade de topo trazida por `trazer <modulo>;` é invisível ao fluxo
/// de tokens deste arquivo, e mesmo assim vence a família — em silêncio, como
/// vence qualquer outra identidade de topo.
///
/// `B2b_REPRODUCED`. Antes desta correção o programa abaixo COMPILAVA, com
/// `arquivo.abrir` virando a intrínseca por cima de um `eterno arquivo` que o
/// módulo tinha acabado de trazer.
///
/// A primeira tentativa de fechar isso foi RECUSAR o par de imports — e a
/// revisão adversarial provou que recusar quebra um programa que o baseline
/// ACEITA: o que importa a família e usa só a grafia histórica. A resposta
/// correta não é uma recusa nova, é a família ceder; para isso a autoridade de
/// import entrega ao parser os nomes do módulo ANTES da canonicalização.
#[test]
fn t10_identidade_importada_homonima_vence_a_familia() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "deposito.pink",
        "pacote deposito;
eterno arquivo: bombom = 5;
",
    );

    for (rotulo, imports) in [
        ("modulo-antes", "trazer deposito;\ntrazer arquivo;"),
        ("familia-antes", "trazer arquivo;\ntrazer deposito;"),
    ] {
        // O programa que usa SÓ a grafia histórica continua compilando, que é
        // exatamente o que o baseline faz. Recusar aqui seria regressão.
        let legado = escrever(
            &dir,
            &format!("importado_legado_{rotulo}.pink"),
            &format!(
                "pacote main;
{imports}
trazer caminho.diretorio_atual;
carinho principal() -> bombom {{
    nova d: verso = diretorio_atual();
    mimo arquivo;
}}
"
            ),
        );
        let saida = checar(&legado, &format!("parte-g-t10-legado-{rotulo}"));
        assert!(
            saida.status.success(),
            "a família derrubou um programa que só usa o legado ({rotulo}): {}",
            stderr_de(&saida)
        );

        // E a forma qualificada volta ao diagnóstico HISTÓRICO: o nome tem
        // dono, e o dono é o item importado.
        let qualificado = escrever(
            &dir,
            &format!("importado_qualificado_{rotulo}.pink"),
            &format!(
                "pacote main;
{imports}
carinho principal() -> bombom {{
    nova h: bombom = arquivo.abrir(\"x.txt\");
    mimo 0;
}}
"
            ),
        );
        let saida = checar(&qualificado, &format!("parte-g-t10-qualificado-{rotulo}"));
        assert!(
            !saida.status.success(),
            "a família capturou uma identidade importada ({rotulo})"
        );
        let erro = stderr_de(&saida);
        assert!(
            erro.contains("método 'abrir' não implementado para tipo 'bombom'"),
            "o erro tem de ser o histórico do item importado ({rotulo}): {erro}"
        );
        assert!(
            !erro.contains("família"),
            "a família não tem o que dizer sobre um nome que já tem dono ({rotulo}): {erro}"
        );
    }
}

/// T10 (contraprova) — sem `trazer arquivo;`, a identidade importada continua
/// respondendo pelo comportamento histórico.
#[test]
fn t10b_identidade_importada_sem_import_de_familia_segue_historica() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "deposito.pink",
        "pacote deposito;
eterno arquivo: bombom = 5;
",
    );

    let fonte = escrever(
        &dir,
        "so_o_modulo.pink",
        "pacote main;
trazer deposito;
carinho principal() -> bombom { mimo arquivo; }
",
    );

    let saida = checar(&fonte, "parte-g-t10b-sem-familia");
    assert!(
        saida.status.success(),
        "o item importado tem de continuar resolvendo: {}",
        stderr_de(&saida)
    );
}

/// A superfície aprovada vale DENTRO de um módulo importado, não só no arquivo
/// raiz.
///
/// A recursão do carregador não pulava família built-in: um módulo que
/// escrevesse `trazer arquivo;` levava "módulo 'arquivo' não encontrado". Era
/// idêntico ao baseline — mas no baseline `trazer arquivo;` num módulo não
/// tinha nada a oferecer, e agora tem. Uma superfície que só existe no arquivo
/// raiz é meia superfície.
#[test]
fn superficie_por_familia_vale_dentro_de_modulo_importado() {
    let dir = NativeArtifactDir::create().expect("diretório Parte G");

    escrever(
        &dir,
        "biblioteca.pink",
        "pacote biblioteca;
trazer arquivo;

carinho fabricar(alvo: verso) -> bombom {
    mimo arquivo.criar(alvo);
}
",
    );

    let alvo = dir.path().join("criado-pelo-modulo.txt");
    let fonte = escrever(
        &dir,
        "usa_biblioteca.pink",
        &format!(
            "pacote main;
trazer arquivo;
trazer biblioteca.fabricar;

carinho principal() -> bombom {{
    nova h: bombom = fabricar(\"{}\");
    arquivo.fechar(h);
    mimo 0;
}}
",
            alvo.display()
        ),
    );

    let saida = executar(&fonte, "parte-g-familia-dentro-de-modulo");
    assert!(
        saida.status.success(),
        "a superfície por família tem de valer dentro de um módulo: {}",
        stderr_de(&saida)
    );
    assert!(
        alvo.exists(),
        "o membro qualificado dentro do módulo não executou: {}",
        alvo.display()
    );
}

// @pinker-nav:end evidencia.importacoes.parte-g-carregador-e-paridade
