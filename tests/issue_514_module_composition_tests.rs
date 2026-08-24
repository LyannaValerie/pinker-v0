mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// @pinker-nav:start evidencia.modulos.composicao-integridade
// @pinker-nav:domain modulos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental dos oito invariantes de aceite da composição modular, cada um com controle positivo, controle adversarial e regressão derivada do conjunto congelado: integridade da localização de diagnóstico, não-interferência do importador, isolamento entre irmãos, preservação do binding de import explícito, ausência de reexport implícito, superfície do import seletivo, preservação da entrada de validação modular e continuidade de identidade de topo ordinária. Inclui paridade interpretador/nativo de um programa composto. Os artefatos congelados não são reexecutados aqui: estas são asserções novas sobre o comportamento corrigido, e o resultado histórico continua sendo evidência do defeito, não expectativa.

/// Um caso é um conjunto de fontes; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, &str)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #514");
    for (modulo, fonte) in modulos {
        escrever(dir.path(), modulo, fonte);
    }
    let raiz = escrever(dir.path(), nome, raiz);
    Caso { dir, raiz }
}

fn escrever(dir: &Path, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.join(format!("{nome}.pink"));
    fs::write(&caminho, fonte)
        .unwrap_or_else(|erro| panic!("gravar {}: {erro}", caminho.display()));
    caminho
}

fn pink(caso_logico: &str, args: &[&str], alvo: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg(alvo)
        .logical_case(caso_logico)
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar pink")
}

fn executar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--run"], &caso.raiz)
}

fn checar(caso: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--check"], &caso.raiz)
}

fn codigo(saida: &std::process::Output) -> i32 {
    saida.status.code().expect("status com código")
}

fn stderr(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

fn stdout(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stdout).into_owned()
}

// ---------------------------------------------------------------------------
// I1 — SOURCE_LOCATION_INTEGRITY
//
// Toda localização de diagnóstico determina a fonte à qual seu span pertence.
// Um span originado em A nunca é interpretado contra a fonte B.
// ---------------------------------------------------------------------------

/// I1 — controle positivo: erro na própria raiz continua desenhado sobre a raiz,
/// sem rótulo de outra fonte.
#[test]
fn i1_positivo_erro_da_raiz_renderiza_a_raiz() {
    let c = caso(
        "raiz_com_erro",
        "pacote main;\n\ncarinho principal() -> bombom {\n    mimo naoexiste;\n}\n",
        &[],
    );
    let saida = checar(&c, "i1-positivo");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("mimo naoexiste;"), "{erro}");
    // Sem composição não há de que se distinguir, e nenhum rótulo é acrescentado.
    assert!(!erro.contains("  em: "), "{erro}");
}

/// I1 — adversarial: a linha do módulo EXISTE na raiz, com outro texto. É o caso
/// em que a renderização errada é plausível e por isso indetectável a olho nu.
#[test]
fn i1_adversarial_linha_existente_na_raiz_nao_captura_o_trecho_do_modulo() {
    let c = caso(
        "raiz_i1",
        "pacote main;\ntrazer m_i1.quebrado;\n\ncarinho principal() -> bombom {\n    mimo quebrado();\n}\n",
        &[(
            "m_i1",
            "pacote m_i1;\n\ncarinho quebrado() -> bombom {\n    mimo naoexiste_no_modulo;\n}\n",
        )],
    );
    let saida = checar(&c, "i1-adversarial");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    // O trecho tem de ser a linha 4 do MÓDULO, não a linha 4 da raiz.
    assert!(erro.contains("mimo naoexiste_no_modulo;"), "{erro}");
    assert!(!erro.contains("mimo quebrado();"), "{erro}");
    assert!(erro.contains("m_i1.pink"), "{erro}");
}

/// I1 — regressão derivada de C1-02: a linha do módulo EXCEDE o tamanho da raiz.
/// O modo congelado omitia o trecho em silêncio.
#[test]
fn i1_regressao_linha_fora_da_faixa_da_raiz_ainda_encontra_a_fonte() {
    let mut modulo = String::from("pacote m_i1b;\n");
    for _ in 0..18 {
        modulo.push('\n');
    }
    modulo.push_str("carinho torto() -> bombom {\n    mimo @@@;\n}\n");
    let c = caso(
        "raiz_i1b",
        "pacote main;\ntrazer m_i1b.torto;\n\ncarinho principal() -> bombom {\n    mimo torto();\n}\n",
        &[("m_i1b", modulo.as_str())],
    );
    let saida = checar(&c, "i1-regressao-fora-de-faixa");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("mimo @@@;"), "trecho omitido: {erro}");
    assert!(erro.contains("m_i1b.pink"), "{erro}");
}

/// I1 — regressão derivada de C1-03/C1-04: erro do próprio carregador, cujo span
/// nasce dentro do módulo transitivo.
#[test]
fn i1_regressao_erro_do_carregador_aponta_o_modulo_que_o_produziu() {
    let mut intermediario = String::from("pacote i1c_meio;\n");
    for _ in 0..6 {
        intermediario.push('\n');
    }
    intermediario.push_str("trazer i1c_ausente.x;\n\ncarinho meio() -> bombom {\n    mimo 1;\n}\n");
    let c = caso(
        "raiz_i1c",
        "pacote main;\ntrazer i1c_meio.meio;\n\ncarinho principal() -> bombom {\n    mimo meio();\n}\n",
        &[("i1c_meio", intermediario.as_str())],
    );
    let saida = checar(&c, "i1-regressao-carregador");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("trazer i1c_ausente.x;"), "{erro}");
    assert!(erro.contains("i1c_meio.pink"), "{erro}");
}

// ---------------------------------------------------------------------------
// I2 — MODULE_IMPORTER_NON_INTERFERENCE
//
// Adicionar ou remover declarações/imports no importador não muda bindings
// internos de M fora das dependências autorizadas por M.
// ---------------------------------------------------------------------------

const I2_FATOR_TRES: &str = "pacote i2_tres;\n\neterno FATOR: bombom = 3;\n";
const I2_FATOR_CINCO: &str = "pacote i2_cinco;\n\neterno FATOR: bombom = 5;\n";
const I2_MODULO: &str = "pacote i2_mod;\ntrazer i2_tres.FATOR;\n\ncarinho calcular() -> bombom {\n    mimo FATOR * 5;\n}\n";

/// I2 — controle positivo: o módulo compõe pelo binding que ele declarou.
#[test]
fn i2_positivo_modulo_usa_o_binding_que_declarou() {
    let c = caso(
        "raiz_i2",
        "pacote main;\ntrazer i2_mod.calcular;\n\ncarinho principal() -> bombom {\n    mimo calcular();\n}\n",
        &[
            ("i2_tres", I2_FATOR_TRES),
            ("i2_cinco", I2_FATOR_CINCO),
            ("i2_mod", I2_MODULO),
        ],
    );
    let saida = executar(&c, "i2-positivo");
    assert_eq!(codigo(&saida), 15, "{}", stderr(&saida));
}

/// I2 — adversarial: SÓ o import da raiz muda; o módulo é byte-idêntico.
///
/// No conjunto congelado esta mudança trocava o `FATOR` do módulo e o resultado
/// saltava de 15 para 25.
#[test]
fn i2_adversarial_import_da_raiz_nao_altera_o_binding_do_modulo() {
    let c = caso(
        "raiz_i2b",
        "pacote main;\ntrazer i2_mod.calcular;\ntrazer i2_cinco.FATOR;\n\ncarinho principal() -> bombom {\n    mimo calcular();\n}\n",
        &[
            ("i2_tres", I2_FATOR_TRES),
            ("i2_cinco", I2_FATOR_CINCO),
            ("i2_mod", I2_MODULO),
        ],
    );
    let saida = executar(&c, "i2-adversarial");
    assert_eq!(
        codigo(&saida),
        15,
        "o binding interno do módulo mudou por causa do importador: {}",
        stderr(&saida)
    );
}

/// I2 — regressão derivada de C2-05..C2-09: o corpo do módulo não enxerga o que
/// só a raiz declarou, em nenhuma das superfícies de captura exercitadas.
#[test]
fn i2_regressao_corpo_do_modulo_nao_captura_declaracao_da_raiz() {
    for (nome, declaracao_raiz, uso_no_modulo) in [
        (
            "funcao",
            "carinho so_da_raiz() -> bombom {\n    mimo 231;\n}\n",
            "mimo so_da_raiz();",
        ),
        (
            "constante",
            "eterno SO_DA_RAIZ: bombom = 77;\n",
            "mimo SO_DA_RAIZ;",
        ),
        (
            "alias",
            "apelido SoDaRaiz = bombom;\n",
            "nova v: SoDaRaiz = 3;\n    mimo v;",
        ),
    ] {
        let modulo =
            format!("pacote i2r_mod;\n\ncarinho usa() -> bombom {{\n    {uso_no_modulo}\n}}\n");
        let raiz = format!(
            "pacote main;\ntrazer i2r_mod.usa;\n\n{declaracao_raiz}\ncarinho principal() -> bombom {{\n    mimo usa();\n}}\n"
        );
        let c = caso("raiz_i2r", &raiz, &[("i2r_mod", modulo.as_str())]);
        let saida = checar(&c, "i2-regressao-captura");
        let erro = stderr(&saida);
        assert_eq!(
            codigo(&saida),
            1,
            "captura de {nome} declarada só na raiz foi aceita: {erro}"
        );
        assert!(
            erro.contains("não") && erro.contains("i2r_mod"),
            "{nome}: {erro}"
        );
    }
}

/// I2 — regressão derivada de C2-10: método default de um trato da raiz não
/// atende chamada escrita dentro do módulo. Despacho de método não menciona o
/// trato, então esta é a superfície que a resolução nominal não alcança.
#[test]
fn i2_regressao_trato_da_raiz_nao_atende_chamada_do_modulo() {
    let c = caso(
        "raiz_i2t",
        concat!(
            "pacote main;\n",
            "trazer i2t_mod.usa;\n\n",
            "trato Medivel {\n",
            "    carinho marcador(valor: si) -> bombom;\n\n",
            "    carinho dobro(valor: si) -> bombom {\n",
            "        mimo valor * 2;\n",
            "    }\n",
            "}\n\n",
            "impl Medivel para bombom {\n",
            "    carinho marcador(valor: bombom) -> bombom {\n",
            "        mimo valor;\n",
            "    }\n",
            "}\n\n",
            "carinho principal() -> bombom {\n    mimo usa();\n}\n"
        ),
        &[(
            "i2t_mod",
            "pacote i2t_mod;\n\ncarinho usa() -> bombom {\n    nova b: bombom = 5;\n    mimo b.dobro();\n}\n",
        )],
    );
    let saida = checar(&c, "i2-regressao-trato");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "trato da raiz atendeu o módulo: {erro}");
    assert!(erro.contains("dobro"), "{erro}");
}

// ---------------------------------------------------------------------------
// I3 — MODULE_SIBLING_ISOLATION
//
// Sem caminho de visibilidade autorizado M -> N, um símbolo de N não satisfaz
// referência em M.
// ---------------------------------------------------------------------------

/// I3 — controle positivo: com o import declarado, o irmão é alcançável.
#[test]
fn i3_positivo_irmao_importado_e_alcancavel() {
    let c = caso(
        "raiz_i3",
        "pacote main;\ntrazer i3_x.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[
            ("i3_y", "pacote i3_y;\n\ncarinho fornece() -> bombom {\n    mimo 55;\n}\n"),
            (
                "i3_x",
                "pacote i3_x;\ntrazer i3_y.fornece;\n\ncarinho usa() -> bombom {\n    mimo fornece();\n}\n",
            ),
        ],
    );
    let saida = executar(&c, "i3-positivo");
    assert_eq!(codigo(&saida), 55, "{}", stderr(&saida));
}

/// I3 — adversarial e regressão de C2-11: o irmão é carregado pela raiz, mas o
/// módulo NÃO o importou. Estar presente na composição não é estar visível.
#[test]
fn i3_adversarial_irmao_presente_mas_nao_importado_nao_satisfaz() {
    let c = caso(
        "raiz_i3b",
        "pacote main;\ntrazer i3b_x.usa;\ntrazer i3b_y.fornece;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[
            ("i3b_y", "pacote i3b_y;\n\ncarinho fornece() -> bombom {\n    mimo 55;\n}\n"),
            // Sem `trazer i3b_y.fornece;`: a referência não tem caminho autorizado.
            ("i3b_x", "pacote i3b_x;\n\ncarinho usa() -> bombom {\n    mimo fornece();\n}\n"),
        ],
    );
    let saida = checar(&c, "i3-adversarial");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "captura de irmão aceita: {erro}");
    assert!(erro.contains("fornece"), "{erro}");
    assert!(
        erro.contains("i3b_y"),
        "o diagnóstico deve dizer onde ela existe: {erro}"
    );
}

// ---------------------------------------------------------------------------
// I4 — EXPLICIT_IMPORT_BINDING_IS_PRESERVED
//
// Se M declara `trazer N.x`, a resolução introduzida por esse import não pode
// ser satisfeita por P.x.
// ---------------------------------------------------------------------------

fn i4_fontes(especie: &str) -> Vec<(String, String)> {
    match especie {
        "funcao" => vec![
            (
                "i4b".to_string(),
                "pacote i4b;\n\ncarinho fundo() -> bombom {\n    mimo 7;\n}\n".to_string(),
            ),
            (
                "i4c".to_string(),
                "pacote i4c;\n\ncarinho fundo() -> bombom {\n    mimo 9;\n}\n".to_string(),
            ),
            (
                "i4a".to_string(),
                "pacote i4a;\ntrazer i4b.fundo;\n\ncarinho meio() -> bombom {\n    mimo fundo();\n}\n"
                    .to_string(),
            ),
        ],
        "constante" => vec![
            ("i4b".to_string(), "pacote i4b;\n\neterno X: bombom = 7;\n".to_string()),
            ("i4c".to_string(), "pacote i4c;\n\neterno X: bombom = 9;\n".to_string()),
            (
                "i4a".to_string(),
                "pacote i4a;\ntrazer i4b.X;\n\ncarinho meio() -> bombom {\n    mimo X;\n}\n".to_string(),
            ),
        ],
        _ => unreachable!(),
    }
}

/// I4 — controle positivo: a raiz materializa exatamente o que o módulo
/// declarou, e o resultado é o do módulo declarado.
#[test]
fn i4_positivo_binding_declarado_resolve_para_a_origem_declarada() {
    for especie in ["funcao", "constante"] {
        let fontes = i4_fontes(especie);
        let modulos: Vec<(&str, &str)> = fontes
            .iter()
            .map(|(nome, fonte)| (nome.as_str(), fonte.as_str()))
            .collect();
        let simbolo = if especie == "funcao" {
            "i4b.fundo"
        } else {
            "i4b.X"
        };
        let raiz = format!(
            "pacote main;\ntrazer i4a.meio;\ntrazer {simbolo};\n\ncarinho principal() -> bombom {{\n    mimo meio();\n}}\n"
        );
        let c = caso("raiz_i4", &raiz, &modulos);
        let saida = executar(&c, "i4-positivo");
        assert_eq!(codigo(&saida), 7, "{especie}: {}", stderr(&saida));
    }
}

/// I4 — adversarial e regressão de C2-14/C2-16: a raiz materializa um homônimo
/// de OUTRO módulo. No conjunto congelado o binding declarado era substituído e
/// o resultado virava 9.
#[test]
fn i4_adversarial_homonimo_externo_nao_sequestra_o_binding_declarado() {
    for especie in ["funcao", "constante"] {
        let fontes = i4_fontes(especie);
        let modulos: Vec<(&str, &str)> = fontes
            .iter()
            .map(|(nome, fonte)| (nome.as_str(), fonte.as_str()))
            .collect();
        let simbolo = if especie == "funcao" {
            "i4c.fundo"
        } else {
            "i4c.X"
        };
        let raiz = format!(
            "pacote main;\ntrazer i4a.meio;\ntrazer {simbolo};\n\ncarinho principal() -> bombom {{\n    mimo meio();\n}}\n"
        );
        let c = caso("raiz_i4b", &raiz, &modulos);
        let saida = executar(&c, "i4-adversarial");
        assert_eq!(
            codigo(&saida),
            7,
            "{especie}: binding declarado foi sequestrado pelo homônimo: {}",
            stderr(&saida)
        );
    }
}

/// I4 — regressão de C2-12: o binding declarado vale mesmo quando a raiz não
/// materializa homônimo algum. Antes, ele dependia da raiz para existir.
#[test]
fn i4_regressao_binding_declarado_independe_do_importador() {
    let fontes = i4_fontes("funcao");
    let modulos: Vec<(&str, &str)> = fontes
        .iter()
        .map(|(nome, fonte)| (nome.as_str(), fonte.as_str()))
        .collect();
    let c = caso(
        "raiz_i4c",
        "pacote main;\ntrazer i4a.meio;\n\ncarinho principal() -> bombom {\n    mimo meio();\n}\n",
        &modulos,
    );
    let saida = executar(&c, "i4-regressao");
    assert_eq!(codigo(&saida), 7, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// I5 — NO_IMPLICIT_REEXPORT
//
// Dependências disponíveis a M não se tornam automaticamente visíveis aos
// importadores de M.
// ---------------------------------------------------------------------------

const I5_FUNDO: &str = "pacote i5_fundo;\n\ncarinho base() -> bombom {\n    mimo 4;\n}\n";
const I5_MEIO: &str =
    "pacote i5_meio;\ntrazer i5_fundo.base;\n\ncarinho meio() -> bombom {\n    mimo base() * 10;\n}\n";

/// I5 — controle positivo: o importador enxerga o que pediu, e o que ele pediu
/// funciona porque a dependência interna continua existindo.
#[test]
fn i5_positivo_importador_ve_o_que_pediu_e_funciona() {
    let c = caso(
        "raiz_i5",
        "pacote main;\ntrazer i5_meio.meio;\n\ncarinho principal() -> bombom {\n    mimo meio();\n}\n",
        &[("i5_fundo", I5_FUNDO), ("i5_meio", I5_MEIO)],
    );
    let saida = executar(&c, "i5-positivo");
    assert_eq!(codigo(&saida), 40, "{}", stderr(&saida));
}

/// I5 — adversarial: o importador tenta usar a dependência interna do módulo
/// importado. Ela existe na composição; não está na superfície dele.
#[test]
fn i5_adversarial_dependencia_interna_nao_vaza_para_o_importador() {
    let c = caso(
        "raiz_i5b",
        "pacote main;\ntrazer i5_meio.meio;\n\ncarinho principal() -> bombom {\n    mimo base();\n}\n",
        &[("i5_fundo", I5_FUNDO), ("i5_meio", I5_MEIO)],
    );
    let saida = checar(&c, "i5-adversarial");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "reexport implícito aceito: {erro}");
    assert!(erro.contains("base"), "{erro}");
}

/// I5 — adversarial com import inteiro: trazer o módulo todo também não traz o
/// que ELE importou.
#[test]
fn i5_adversarial_import_inteiro_tambem_nao_reexporta() {
    let c = caso(
        "raiz_i5c",
        "pacote main;\ntrazer i5_meio;\n\ncarinho principal() -> bombom {\n    mimo base();\n}\n",
        &[("i5_fundo", I5_FUNDO), ("i5_meio", I5_MEIO)],
    );
    let saida = checar(&c, "i5-adversarial-inteiro");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "import inteiro reexportou: {erro}");
    assert!(erro.contains("base"), "{erro}");
}

// ---------------------------------------------------------------------------
// I6 — SELECTIVE_IMPORT_SURFACE
//
// Import seletivo controla o que se torna visível ao importador; não apaga
// dependências de implementação da entidade em seu módulo de origem.
// ---------------------------------------------------------------------------

const I6_MODULO: &str = concat!(
    "pacote i6_mod;\n\n",
    "carinho auxiliar() -> bombom {\n    mimo 42;\n}\n\n",
    "carinho publica() -> bombom {\n    mimo auxiliar();\n}\n"
);

/// I6 — controle positivo e regressão de C2-03: o import seletivo preserva a
/// dependência ordinária interna da entidade pedida.
#[test]
fn i6_positivo_seletivo_preserva_dependencia_interna_ordinaria() {
    let c = caso(
        "raiz_i6",
        "pacote main;\ntrazer i6_mod.publica;\n\ncarinho principal() -> bombom {\n    mimo publica();\n}\n",
        &[("i6_mod", I6_MODULO)],
    );
    let saida = executar(&c, "i6-positivo");
    assert_eq!(codigo(&saida), 42, "{}", stderr(&saida));
}

/// I6 — adversarial: preservar a dependência interna não é torná-la visível.
#[test]
fn i6_adversarial_dependencia_preservada_nao_entra_na_superficie() {
    let c = caso(
        "raiz_i6b",
        "pacote main;\ntrazer i6_mod.publica;\n\ncarinho principal() -> bombom {\n    mimo auxiliar();\n}\n",
        &[("i6_mod", I6_MODULO)],
    );
    let saida = checar(&c, "i6-adversarial");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "a dependência interna virou superfície: {erro}"
    );
    assert!(erro.contains("auxiliar"), "{erro}");
}

// ---------------------------------------------------------------------------
// I7 — MODULE_VALIDATION_INPUT_PRESERVATION
//
// Regra aplicável a M que dependa de informação presente na unidade-fonte M
// roda antes de qualquer transformação que descarte essa informação.
// ---------------------------------------------------------------------------

/// I7 — regressão de C3-02: `impl` incoerente é recusado como raiz E como
/// módulo. Mover o mesmo conteúdo para um módulo não o torna válido.
#[test]
fn i7_impl_incoerente_e_recusado_como_raiz_e_como_modulo() {
    const CORPO: &str = concat!(
        "trato I7Trato {\n",
        "    carinho exigido(valor: si) -> bombom;\n",
        "}\n\n",
        "impl I7Trato para bombom {\n",
        "}\n"
    );

    let como_raiz = caso(
        "raiz_i7",
        &format!("pacote main;\n\n{CORPO}\ncarinho principal() -> bombom {{\n    mimo 42;\n}}\n"),
        &[],
    );
    let saida_raiz = checar(&como_raiz, "i7-controle-raiz");
    assert_eq!(
        codigo(&saida_raiz),
        1,
        "controle: o conteúdo já era inválido como raiz: {}",
        stderr(&saida_raiz)
    );

    let como_modulo = caso(
        "raiz_i7b",
        "pacote main;\ntrazer i7_mod;\n\ncarinho principal() -> bombom {\n    mimo 42;\n}\n",
        &[("i7_mod", &format!("pacote i7_mod;\n\n{CORPO}"))],
    );
    let saida_modulo = checar(&como_modulo, "i7-modulo");
    assert_eq!(
        codigo(&saida_modulo),
        1,
        "o mesmo conteúdo passou a ser válido só por virar módulo: {}",
        stderr(&saida_modulo)
    );
}

/// I7 — regressão de C3-04: import de família inválido dentro de módulo é
/// validado, como já era na raiz.
#[test]
fn i7_import_de_familia_invalido_e_validado_dentro_do_modulo() {
    let como_raiz = caso(
        "raiz_i7c",
        "pacote main;\ntrazer arquivo.nao_existe;\n\ncarinho principal() -> bombom {\n    mimo 42;\n}\n",
        &[],
    );
    assert_eq!(
        codigo(&checar(&como_raiz, "i7-familia-controle")),
        1,
        "controle: import de família inválido já era recusado na raiz"
    );

    let como_modulo = caso(
        "raiz_i7d",
        "pacote main;\ntrazer i7d_mod.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[(
            "i7d_mod",
            "pacote i7d_mod;\ntrazer arquivo.nao_existe;\n\ncarinho usa() -> bombom {\n    mimo 42;\n}\n",
        )],
    );
    let saida = checar(&como_modulo, "i7-familia-modulo");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "import inválido escapou no módulo: {erro}"
    );
    assert!(erro.contains("nao_existe"), "{erro}");
}

/// I7 — regressão de C3-09: a política de propriedade de grafia da PR #507 não
/// é contornável movendo o mesmo código para dentro de um módulo.
#[test]
fn i7_politica_de_grafia_da_pr_507_vale_dentro_do_modulo() {
    const CORPO: &str = concat!(
        "trazer arquivo;\n\n",
        "carinho criar(x: bombom) -> bombom {\n    mimo x;\n}\n"
    );

    let como_raiz = caso(
        "raiz_i7e",
        &format!("pacote main;\n{CORPO}\ncarinho principal() -> bombom {{\n    mimo 42;\n}}\n"),
        &[],
    );
    let saida_raiz = checar(&como_raiz, "i7-507-controle");
    assert_eq!(
        codigo(&saida_raiz),
        1,
        "controle: a política já recusava isto na raiz: {}",
        stderr(&saida_raiz)
    );

    let como_modulo = caso(
        "raiz_i7f",
        "pacote main;\ntrazer i7f_mod.criar;\n\ncarinho principal() -> bombom {\n    mimo criar(42);\n}\n",
        &[("i7f_mod", &format!("pacote i7f_mod;\n{CORPO}"))],
    );
    let saida_modulo = checar(&como_modulo, "i7-507-modulo");
    assert_eq!(
        codigo(&saida_modulo),
        1,
        "a política da #507 foi contornada por módulo: {}",
        stderr(&saida_modulo)
    );
}

/// I7 — contratos indecisos permanecem indecisos.
///
/// `package` e `freestanding` são preservados como DADO. Esta Task não decide o
/// contrato deles, e o teste existe para provar que ela não o decidiu por
/// acidente: o comportamento observável dos dois continua exatamente o que o
/// conjunto congelado registrou.
#[test]
fn i7_package_e_freestanding_continuam_sem_contrato_novo() {
    // `aa.pink` declara `pacote bb;` e continua sendo importado por `aa`.
    let por_arquivo = caso(
        "raiz_i7g",
        "pacote main;\ntrazer i7g_aa.valor;\n\ncarinho principal() -> bombom {\n    mimo valor();\n}\n",
        &[("i7g_aa", "pacote bb;\n\ncarinho valor() -> bombom {\n    mimo 42;\n}\n")],
    );
    assert_eq!(
        codigo(&executar(&por_arquivo, "i7-package-por-arquivo")),
        42,
        "a chave de import deixou de ser o nome do arquivo"
    );

    // Importar pelo `pacote` declarado continua não resolvendo.
    let por_pacote = caso(
        "raiz_i7h",
        "pacote main;\ntrazer bb.valor;\n\ncarinho principal() -> bombom {\n    mimo valor();\n}\n",
        &[(
            "i7g_aa",
            "pacote bb;\n\ncarinho valor() -> bombom {\n    mimo 42;\n}\n",
        )],
    );
    let saida = checar(&por_pacote, "i7-package-por-pacote");
    assert_eq!(
        codigo(&saida),
        1,
        "o `pacote` declarado passou a participar da identidade do módulo: {}",
        stderr(&saida)
    );

    // Módulo `livre` importado por raiz hospedada: a IR final continua hospedada.
    let livre = caso(
        "raiz_i7i",
        "pacote main;\ntrazer i7i_mod.valor;\n\ncarinho principal() -> bombom {\n    mimo valor();\n}\n",
        &[("i7i_mod", "pacote i7i_mod;\nlivre;\n\ncarinho valor() -> bombom {\n    mimo 42;\n}\n")],
    );
    let ir = pink("i7-freestanding", &["--ir"], &livre.raiz);
    assert_eq!(codigo(&ir), 0, "{}", stderr(&ir));
    assert!(
        stdout(&ir).contains("mode hospedado"),
        "o contrato de `livre` foi decidido por acidente: {}",
        stdout(&ir)
    );
}

// ---------------------------------------------------------------------------
// I8 — ORDINARY_TOP_LEVEL_IDENTITY_CONTINUITY
//
// Entidades top-level ordinárias possuem identidade canônica vinculada ao
// módulo de origem. Grafia igual não implica identidade igual.
// ---------------------------------------------------------------------------

fn i8_modulos() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "i8_ma",
            "pacote i8_ma;\n\ncarinho helper() -> bombom {\n    mimo 10;\n}\n\ncarinho publica_a() -> bombom {\n    mimo helper();\n}\n",
        ),
        (
            "i8_mb",
            "pacote i8_mb;\n\ncarinho helper() -> bombom {\n    mimo 20;\n}\n\ncarinho publica_b() -> bombom {\n    mimo helper();\n}\n",
        ),
    ]
}

/// I8 — controle positivo e regressão de C4-01: dois módulos independentes com
/// um `helper` interno homônimo compõem, e cada um chama o SEU.
#[test]
fn i8_positivo_helpers_homonimos_compoem_com_import_seletivo() {
    let c = caso(
        "raiz_i8",
        "pacote main;\ntrazer i8_ma.publica_a;\ntrazer i8_mb.publica_b;\n\ncarinho principal() -> bombom {\n    mimo publica_a() * 100 + publica_b();\n}\n",
        &i8_modulos(),
    );
    let saida = executar(&c, "i8-positivo");
    // 10 * 100 + 20 = 1020; 1020 & 0xFF = 252.
    assert_eq!(codigo(&saida), 252, "{}", stderr(&saida));
}

/// I8 — controle de contraste, derivado de C4-03: a mesma forma com closures
/// sintéticas já compunha antes desta Task, e continua compondo. É o controle
/// que impede confundir "passou a funcionar" com "sempre funcionou".
#[test]
fn i8_contraste_closures_sinteticas_continuam_compondo() {
    let c = caso(
        "raiz_i8b",
        "pacote main;\ntrazer i8c_ca.publica_a;\ntrazer i8c_cb.publica_b;\n\ncarinho principal() -> bombom {\n    mimo publica_a() * 100 + publica_b();\n}\n",
        &[
            (
                "i8c_ca",
                "pacote i8c_ca;\n\ncarinho publica_a() -> bombom {\n    nova f: carinho(bombom) -> bombom = carinho(x: bombom) -> bombom {\n        mimo x + 10;\n    };\n    mimo f(0);\n}\n",
            ),
            (
                "i8c_cb",
                "pacote i8c_cb;\n\ncarinho publica_b() -> bombom {\n    nova f: carinho(bombom) -> bombom = carinho(x: bombom) -> bombom {\n        mimo x + 20;\n    };\n    mimo f(0);\n}\n",
            ),
        ],
    );
    let saida = executar(&c, "i8-contraste");
    assert_eq!(codigo(&saida), 252, "{}", stderr(&saida));
}

/// I8 — adversarial e regressão de C4-02: com import INTEIRO dos dois módulos, o
/// `helper` de cada um disputa a mesma grafia NA SUPERFÍCIE do importador. Isso
/// continua sendo recusado — e agora por ambiguidade de superfície declarada,
/// não por colisão global de símbolo.
#[test]
fn i8_adversarial_import_inteiro_conflita_na_superficie_do_importador() {
    let c = caso(
        "raiz_i8c",
        "pacote main;\ntrazer i8_ma;\ntrazer i8_mb;\n\ncarinho principal() -> bombom {\n    mimo publica_a() * 100 + publica_b();\n}\n",
        &i8_modulos(),
    );
    let saida = checar(&c, "i8-adversarial");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("'helper' trazido por múltiplos módulos"),
        "a recusa deve ser de superfície do importador: {erro}"
    );
}

/// I8 — a identidade separada é observável na IR: dois `helper` distintos, cada
/// um qualificado pelo módulo que o declarou, sem símbolo duplicado.
#[test]
fn i8_identidade_canonica_e_observavel_na_ir_sem_duplicar_simbolo() {
    let c = caso(
        "raiz_i8d",
        "pacote main;\ntrazer i8_ma.publica_a;\ntrazer i8_mb.publica_b;\n\ncarinho principal() -> bombom {\n    mimo publica_a() + publica_b();\n}\n",
        &i8_modulos(),
    );
    let ir = pink("i8-ir", &["--ir"], &c.raiz);
    assert_eq!(codigo(&ir), 0, "{}", stderr(&ir));
    let texto = stdout(&ir);
    assert!(texto.contains("func i8_ma.helper"), "IR:\n{texto}");
    assert!(texto.contains("func i8_mb.helper"), "IR:\n{texto}");
    assert!(
        !texto.contains("func helper "),
        "grafia global sobreviveu à composição: IR:\n{texto}"
    );
    for qualificado in ["func i8_ma.helper", "func i8_mb.helper"] {
        assert_eq!(
            texto.matches(qualificado).count(),
            1,
            "símbolo de runtime duplicado para {qualificado}: IR:\n{texto}"
        );
    }
}

// ---------------------------------------------------------------------------
// Paridade interpretador/nativo
// ---------------------------------------------------------------------------

/// A composição corrigida produz o mesmo observável nos dois backends.
#[test]
fn paridade_interpretador_e_nativo_de_programa_composto() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-514-paridade", true)
    else {
        return;
    };
    let c = caso(
        "paridade_514",
        "pacote main;\ntrazer i8_ma.publica_a;\ntrazer i8_mb.publica_b;\n\ncarinho principal() -> bombom {\n    falar(publica_a() + publica_b());\n    mimo 0;\n}\n",
        &i8_modulos(),
    );
    let interpretado = executar(&c, "issue-514-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 0, "{}", stderr(&interpretado));

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("issue-514-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #514");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_514"))
        .logical_case("issue-514-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #514");
    assert!(nativo.status.success(), "{nativo:?}");
    assert_eq!(interpretado.stdout, nativo.stdout);
    assert_eq!(String::from_utf8_lossy(&nativo.stdout), "30\n");
}
// @pinker-nav:end evidencia.modulos.composicao-integridade
