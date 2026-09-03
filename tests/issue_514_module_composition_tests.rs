mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::Type;
use pinker_v0::falha_operacional::span_sintetico;
use std::collections::HashMap;
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
    // Depois da #505 o import INTEIRO deixou de disputar o nome do membro: ele
    // habilita `arquivo.criar(...)`, forma qualificada que não ocupa `criar`
    // no arquivo. Quem ocupa é o import SELETIVO, e é sobre ele que a política
    // da PR #507 continua valendo — na raiz e dentro do módulo igualmente,
    // que é exatamente o que a C3-09 exige.
    const CORPO: &str = concat!(
        "trazer arquivo.criar;\n\n",
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

// @pinker-nav:start evidencia.modulos.revisao-adversarial
// @pinker-nav:domain modulos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Regressões das seis correções vindas da revisão adversarial independente da #514: grafia builtin não é entidade de unidade e por isso não é capturável nos dois sentidos, com guarda de deriva contra a autoridade de intrínsecas; `--check` e o lowering concordam sobre despacho de método em programa composto; a forma qualificada `<módulo>.<entidade>` passa pelo ambiente como qualquer outra referência; a superfície de import é validada em toda unidade e não só na raiz; `impl` duplicado dentro de módulo não é engolido pela deduplicação de identidades geradas; e um arquivo que importa a si mesmo não é rotulado como fonte estrangeira.

/// Revisão adversarial N1 — grafia builtin não pertence a unidade alguma.
///
/// Um módulo que declare `mapa_criar` não pode fazer a RAIZ perder a chamada
/// builtin: era código previamente válido que passava a falhar por causa de um
/// módulo não relacionado.
#[test]
fn revisao_n1_declaracao_de_grafia_builtin_em_modulo_nao_quebra_a_raiz() {
    let c = caso(
        "raiz_n1",
        "pacote main;\ntrazer n1_mod.ua;\ntrazer mapa.criar;\n\ncarinho principal() -> bombom {\n    nova mm: mapa<verso, bombom> = criar();\n    mimo ua();\n}\n",
        &[(
            "n1_mod",
            "pacote n1_mod;\n\ncarinho mapa_criar() -> bombom {\n    mimo 1;\n}\n\ncarinho ua() -> bombom {\n    mimo 2;\n}\n",
        )],
    );
    let saida = executar(&c, "revisao-n1-raiz");
    assert_eq!(codigo(&saida), 2, "{}", stderr(&saida));
}

/// Revisão adversarial N1, sentido simétrico — uma declaração na raiz não
/// alcança o módulo.
///
/// A raiz declara uma grafia de intrínseca que o módulo NÃO usa. O módulo tem
/// de seguir intacto: a não-interferência é sobre alcance, não sobre a mera
/// presença de uma declaração no grafo.
///
/// O caso em que o módulo usa a MESMA grafia que a raiz reivindicou é decidido
/// por `revisao_n1_duplo_estreitamento_e_deliberado_e_tem_diagnostico`, que
/// registra por que ele passou a ser recusado.
#[test]
fn revisao_n1_declaracao_de_grafia_builtin_na_raiz_nao_quebra_o_modulo() {
    let c = caso(
        "raiz_n1b",
        "pacote main;\ntrazer n1b_mod.ua;\n\ncarinho mapa_criar(v: bombom) -> bombom {\n    mimo 0;\n}\n\ncarinho principal() -> bombom {\n    mimo ua();\n}\n",
        &[(
            "n1b_mod",
            "pacote n1b_mod;\ntrazer lista.criar;\n\ncarinho ua() -> bombom {\n    nova l: lista<bombom> = criar();\n    mimo 7;\n}\n",
        )],
    );
    let saida = executar(&c, "revisao-n1-modulo");
    assert_eq!(codigo(&saida), 7, "{}", stderr(&saida));
}

/// Revisão adversarial N1 — guarda de deriva.
///
/// A resolução modular distingue "grafia builtin" de "entidade declarada por
/// alguma unidade" consultando `intrinsic_authority::e_grafia_builtin_chamavel`.
/// Se a autoridade semântica ganhar uma grafia builtin nova sem registrá-la
/// ali, um módulo que a declare volta a capturá-la — silenciosamente. Este
/// teste lê a própria fonte para que a lacuna apareça como falha, não como
/// defeito de composição meses depois.
#[test]
fn revisao_n1_autoridade_de_builtin_cobre_as_grafias_da_semantica() {
    // Não são chamadas: `si` é o marcador de receiver em assinatura de trato e
    // `trato` é palavra-chave. Qualquer outra grafia precisa ser reconhecida.
    const NAO_CHAMAVEIS: &[&str] = &["si", "trato"];

    let fonte =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/semantic.rs"))
            .expect("ler src/semantic.rs");

    let mut ausentes: Vec<String> = Vec::new();
    for pedaco in fonte.split("name == \"").skip(1) {
        let Some(fim) = pedaco.find('"') else {
            continue;
        };
        let grafia = &pedaco[..fim];
        if grafia.starts_with("__") || NAO_CHAMAVEIS.contains(&grafia) {
            continue;
        }
        if !grafia
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        if !pinker_v0::intrinsic_authority::e_grafia_builtin_chamavel(grafia)
            && !ausentes.iter().any(|ja| ja == grafia)
        {
            ausentes.push(grafia.to_string());
        }
    }

    assert!(
        ausentes.is_empty(),
        "grafias builtin de src/semantic.rs que a autoridade de intrínsecas não reconhece: {ausentes:?}. \
         Registre-as em intrinsics::registry, ou um módulo que as declare voltará a capturá-las."
    );
}

/// Revisão adversarial N2 — `--check` e o lowering concordam.
///
/// O filtro de visibilidade de trato vivia só na autoridade semântica. O
/// lowering continuava despachando pela tabela global, então um programa
/// composto podia passar em `--check` e falhar ao ser abaixado: válido para o
/// compilador e inemitível pelo mesmo compilador.
#[test]
fn revisao_n2_check_e_lowering_concordam_sobre_despacho_de_metodo() {
    let c = caso(
        "raiz_n2",
        concat!(
            "pacote main;\n",
            "trazer n2_mod.ua;\n\n",
            "trato Med2 {\n    carinho medir(v: si) -> bombom;\n}\n\n",
            "impl Med2 para bombom {\n",
            "    carinho medir(v: bombom) -> bombom { mimo v + 100; }\n",
            "}\n\n",
            "carinho principal() -> bombom {\n    mimo ua();\n}\n"
        ),
        &[(
            "n2_mod",
            concat!(
                "pacote n2_mod;\n\n",
                "trato Med {\n    carinho medir(v: si) -> bombom;\n}\n\n",
                "impl Med para bombom {\n",
                "    carinho medir(v: bombom) -> bombom { mimo v + 1; }\n",
                "}\n\n",
                "carinho ua() -> bombom {\n    nova b: bombom = 5;\n    mimo b.medir();\n}\n"
            ),
        )],
    );
    let checagem = checar(&c, "revisao-n2-check");
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = executar(&c, "revisao-n2-run");
    assert_eq!(
        codigo(&execucao),
        6,
        "o módulo despachou para o trato da raiz: {}",
        stderr(&execucao)
    );
}

/// Revisão adversarial N3 — a forma qualificada passa pelo ambiente.
///
/// `<módulo>.<entidade>` é escrita direto no texto e não passa por grafia, então
/// escapava inteira do `ModuleEnvironment` e alcançava qualquer unidade
/// carregada.
#[test]
fn revisao_n3_forma_qualificada_exige_autorizacao_do_ambiente() {
    let modulos: &[(&str, &str)] = &[
        (
            "n3_b",
            "pacote n3_b;\n\nninho Segredo {\n    a: bombom;\n    b: bombom;\n    c: bombom;\n}\n\ncarinho ub() -> bombom {\n    mimo 1;\n}\n",
        ),
        (
            "n3_a",
            "pacote n3_a;\ntrazer n3_b.ub;\n\ncarinho ua() -> bombom {\n    mimo ub();\n}\n",
        ),
    ];

    // A raiz importa apenas `n3_a`; `n3_b` está no grafo, mas não no ambiente.
    let sem_autorizacao = caso(
        "raiz_n3",
        "pacote main;\ntrazer n3_a.ua;\n\ncarinho principal() -> bombom {\n    mimo peso(n3_b.Segredo) + ua();\n}\n",
        modulos,
    );
    let saida = checar(&sem_autorizacao, "revisao-n3-sem-autorizacao");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "forma qualificada não autorizada passou: {erro}"
    );
    assert!(erro.contains("n3_b.Segredo"), "{erro}");

    let com_autorizacao = caso(
        "raiz_n3b",
        "pacote main;\ntrazer n3_b.Segredo;\ntrazer n3_a.ua;\n\ncarinho principal() -> bombom {\n    mimo peso(n3_b.Segredo) + ua();\n}\n",
        modulos,
    );
    let saida = executar(&com_autorizacao, "revisao-n3-com-autorizacao");
    assert_eq!(codigo(&saida), 25, "{}", stderr(&saida));
}

/// Revisão adversarial N3, dentro de um módulo — a mesma pergunta vale para o
/// corpo de um módulo que escreva a forma qualificada de um irmão.
#[test]
fn revisao_n3_forma_qualificada_de_irmao_nao_autorizada_e_recusada() {
    let c = caso(
        "raiz_n3c",
        "pacote main;\ntrazer n3c_a.ua;\ntrazer n3c_b.ub;\n\ncarinho principal() -> bombom {\n    mimo ua() + ub();\n}\n",
        &[
            (
                "n3c_b",
                "pacote n3c_b;\n\nninho Segredo {\n    a: bombom;\n}\n\ncarinho ub() -> bombom {\n    mimo 1;\n}\n",
            ),
            (
                "n3c_a",
                "pacote n3c_a;\n\ncarinho ua() -> bombom {\n    mimo peso(n3c_b.Segredo);\n}\n",
            ),
        ],
    );
    let saida = checar(&c, "revisao-n3-irmao");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("n3c_b.Segredo"), "{erro}");
}

/// Revisão adversarial N4 — a superfície de import é validada em toda unidade.
///
/// A validação de colisão e de import duplicado rodava só sobre os imports da
/// raiz. Dentro de um módulo, o último import vencia em silêncio.
#[test]
fn revisao_n4_superficie_de_import_e_validada_tambem_dentro_do_modulo() {
    let origens: &[(&str, &str)] = &[
        (
            "n4_n",
            "pacote n4_n;\n\ncarinho x() -> bombom {\n    mimo 1;\n}\n",
        ),
        (
            "n4_p",
            "pacote n4_p;\n\ncarinho x() -> bombom {\n    mimo 2;\n}\n",
        ),
    ];

    let mut colisao: Vec<(&str, &str)> = origens.to_vec();
    colisao.push((
        "n4_a",
        "pacote n4_a;\ntrazer n4_n.x;\ntrazer n4_p.x;\n\ncarinho usa() -> bombom {\n    mimo x();\n}\n",
    ));
    let c = caso(
        "raiz_n4",
        "pacote main;\ntrazer n4_a.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &colisao,
    );
    let saida = checar(&c, "revisao-n4-colisao");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "colisão dentro do módulo passou: {erro}");
    assert!(erro.contains("'x' trazido por múltiplos módulos"), "{erro}");

    let mut duplicado: Vec<(&str, &str)> = origens.to_vec();
    duplicado.push((
        "n4_b",
        "pacote n4_b;\ntrazer n4_n.x;\ntrazer n4_n.x;\n\ncarinho usa() -> bombom {\n    mimo x();\n}\n",
    ));
    let c = caso(
        "raiz_n4b",
        "pacote main;\ntrazer n4_b.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &duplicado,
    );
    let saida = checar(&c, "revisao-n4-duplicado");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "import duplicado dentro do módulo passou: {erro}"
    );
    assert!(erro.contains("import duplicado para 'n4_n.x'"), "{erro}");
}

/// Revisão adversarial N5 — `impl` duplicado não é engolido pela deduplicação.
///
/// A projeção deduplicava por prefixo `__`, tratando todo nome do compilador
/// como endereçado por conteúdo. `__impl_*` codifica só `(trato, alvo, método)`:
/// dois corpos distintos colapsavam num, sem diagnóstico.
#[test]
fn revisao_n5_impl_duplicado_em_modulo_continua_recusado() {
    const CORPO: &str = concat!(
        "trato N5T {\n    carinho a(v: si) -> bombom;\n}\n\n",
        "impl N5T para bombom {\n    carinho a(v: bombom) -> bombom { mimo 11; }\n}\n\n",
        "impl N5T para bombom {\n    carinho a(v: bombom) -> bombom { mimo 22; }\n}\n"
    );

    let como_raiz = caso(
        "raiz_n5",
        &format!("pacote main;\n\n{CORPO}\ncarinho principal() -> bombom {{\n    mimo 0;\n}}\n"),
        &[],
    );
    assert_eq!(
        codigo(&checar(&como_raiz, "revisao-n5-controle")),
        1,
        "controle: duplicata já era recusada na raiz"
    );

    let como_modulo = caso(
        "raiz_n5b",
        "pacote main;\ntrazer n5_mod.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[(
            "n5_mod",
            &format!(
                "pacote n5_mod;\n\n{CORPO}\ncarinho usa() -> bombom {{\n    nova b: bombom = 1;\n    mimo b.a();\n}}\n"
            ),
        )],
    );
    let saida = checar(&como_modulo, "revisao-n5-modulo");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "duplicata sobreviveu dentro do módulo: {erro}"
    );
    // #572: a duplicata é recusada pela cardinalidade da relação nominal,
    // não pela colisão de um método explícito.
    assert!(erro.contains("impl do trato"), "{erro}");
    assert!(erro.contains("já declarado"), "{erro}");
}

/// Revisão adversarial N7 — arquivo que importa a si mesmo.
///
/// Ele era registrado uma segunda vez como módulo e ganhava id próprio, então o
/// diagnóstico rotulava o arquivo principal como fonte estrangeira. Continua
/// sendo ciclo e continua sendo recusado; só deixa de mentir sobre a origem.
#[test]
fn revisao_n7_auto_import_nao_rotula_a_raiz_como_fonte_estrangeira() {
    let c = caso(
        "raiz_n7",
        "pacote main;\ntrazer raiz_n7.principal;\n\ncarinho principal() -> bombom {\n    mimo 0;\n}\n",
        &[],
    );
    let saida = checar(&c, "revisao-n7");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("ciclo de módulos detectado"), "{erro}");
    assert!(
        !erro.contains("  em: "),
        "a raiz foi rotulada como estrangeira: {erro}"
    );
}

/// Revisão adversarial N1' — a grafia builtin registrada não vira ponte.
///
/// Reconhecer `mapa_criar` como superfície global fez a resolução deixá-la
/// passar crua, e a raiz preserva a grafia: a chamada do módulo aterrissava na
/// função homônima da raiz quando a aridade não servia ao builtin. Depois da
/// resolução canônica, toda referência legítima de um módulo a entidade de
/// usuário está qualificada — grafia crua vinda de módulo é builtin ou é
/// tentativa de alcançar a raiz.
#[test]
fn revisao_n1_linha_grafia_builtin_nao_vira_ponte_para_a_raiz() {
    let c = caso(
        "raiz_n1l",
        "pacote main;\ntrazer n1l_mod.ua;\n\ncarinho mapa_criar(v: bombom) -> bombom {\n    mimo 77;\n}\n\ncarinho principal() -> bombom {\n    mimo ua();\n}\n",
        &[(
            "n1l_mod",
            "pacote n1l_mod;\n\ncarinho ua() -> bombom {\n    mimo mapa_criar(1);\n}\n",
        )],
    );
    let saida = checar(&c, "revisao-n1-linha");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "o módulo alcançou a função da raiz: {erro}"
    );
    assert!(erro.contains("mapa_criar"), "{erro}");

    // Controle: sem reivindicação alguma da grafia, o módulo alcança o builtin
    // normalmente. É o par que prova que a recusa acima vem da reivindicação, e
    // não de o módulo ter perdido acesso a builtins.
    let controle = caso(
        "raiz_n1m",
        "pacote main;\ntrazer n1m_mod.ub;\n\ncarinho principal() -> bombom {\n    mimo ub();\n}\n",
        &[(
            "n1m_mod",
            "pacote n1m_mod;\ntrazer mapa.criar;\n\ncarinho ub() -> bombom {\n    nova mm: mapa<verso, bombom> = criar();\n    mimo 5;\n}\n",
        )],
    );
    let saida = executar(&controle, "revisao-n1-linha-controle");
    assert_eq!(codigo(&saida), 5, "{}", stderr(&saida));
}

/// Revisão adversarial N3' — módulo real com nome de família built-in.
///
/// A autorização da forma qualificada vinha depois da superfície global, então
/// `<familia>.<membro>` escapava sempre que existisse um módulo real com o nome
/// da família e uma entidade com a grafia de um membro aprovado. É exatamente a
/// combinação que `REAL_MODULE_X > BUILTIN_FAMILY_X` torna alcançável.
#[test]
fn revisao_n3_linha_modulo_com_nome_de_familia_nao_dispensa_autorizacao() {
    let modulos: &[(&str, &str)] = &[
        (
            "arquivo",
            "pacote arquivo;\n\nninho abrir {\n    a: bombom;\n    b: bombom;\n    c: bombom;\n    d: bombom;\n}\n\ncarinho outracoisa() -> bombom {\n    mimo 1;\n}\n",
        ),
        (
            "n3l_mod",
            "pacote n3l_mod;\ntrazer arquivo.abrir;\n\ncarinho um() -> bombom {\n    mimo peso(abrir);\n}\n",
        ),
    ];

    let sem_autorizacao = caso(
        "raiz_n3l",
        "pacote main;\ntrazer arquivo.outracoisa;\ntrazer n3l_mod.um;\n\ncarinho principal() -> bombom {\n    mimo peso(arquivo.abrir) + outracoisa();\n}\n",
        modulos,
    );
    let saida = checar(&sem_autorizacao, "revisao-n3-linha");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "a forma qualificada dispensou autorização: {erro}"
    );
    assert!(erro.contains("arquivo.abrir"), "{erro}");

    let com_autorizacao = caso(
        "raiz_n3m",
        "pacote main;\ntrazer arquivo.abrir;\ntrazer arquivo.outracoisa;\ntrazer n3l_mod.um;\n\ncarinho principal() -> bombom {\n    mimo peso(arquivo.abrir) + outracoisa();\n}\n",
        modulos,
    );
    let saida = executar(&com_autorizacao, "revisao-n3-linha-controle");
    assert_eq!(codigo(&saida), 33, "{}", stderr(&saida));
}

/// Revisão adversarial N4' — import de família também declara superfície.
///
/// O ramo de família saía da função antes da regra de superfície, então dentro
/// de um módulo `trazer arquivo.criar;` e `trazer n.criar;` conviviam e o
/// último vencia em silêncio — a mesma assimetria que a raiz nunca teve.
#[test]
fn revisao_n4_linha_import_de_familia_disputa_a_superficie_da_unidade() {
    let c = caso(
        "raiz_n4l",
        "pacote main;\ntrazer n4l_a.ua;\n\ncarinho principal() -> bombom {\n    mimo ua();\n}\n",
        &[
            ("n4l_n", "pacote n4l_n;\n\ncarinho criar(v: bombom) -> bombom {\n    mimo 77;\n}\n"),
            (
                "n4l_a",
                "pacote n4l_a;\ntrazer arquivo.criar;\ntrazer n4l_n.criar;\n\ncarinho ua() -> bombom {\n    mimo criar(1);\n}\n",
            ),
        ],
    );
    let saida = checar(&c, "revisao-n4-linha");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "a família e o módulo conviveram na mesma grafia: {erro}"
    );
    assert!(
        erro.contains("'criar' trazido por múltiplos módulos"),
        "{erro}"
    );
}

/// Revisão adversarial N1'' — a captura módulo -> raiz por grafia de intrínseca
/// não sobrevive em NENHUM caminho de referência.
///
/// A PR #507 protege `Item::Function`, então a raiz pode declarar legalmente um
/// `eterno`, `ninho`, `apelido` ou `leque` com grafia de intrínseca — e cada um
/// desses era um caminho de captura distinto, porque a resolução curto-circuitava
/// por grafia sem olhar quem a declarava. Interceptar caminho consumidor por
/// caminho consumidor foi o erro: a pergunta certa é feita uma vez, no
/// curto-circuito, e é "é builtin E ninguém a declarou".
#[test]
fn revisao_n1_duplo_grafia_intrinseca_declarada_nao_captura_em_caminho_algum() {
    // (rótulo, declaração na raiz, corpo do módulo)
    let casos: &[(&str, &str, &str)] = &[
        (
            "constante",
            "eterno lista_criar: bombom = 77;",
            "    mimo lista_criar;",
        ),
        (
            "ninho",
            "ninho abrir {\n    a: bombom;\n    b: bombom;\n    c: bombom;\n}",
            "    mimo peso(abrir);",
        ),
        (
            "apelido",
            "apelido tamanho_verso = bombom;",
            "    nova v: tamanho_verso = 9;\n    mimo v;",
        ),
        (
            "leque",
            "leque lista_obter {\n    A,\n    B,\n}",
            "    nova c: lista_obter = lista_obter.A;\n    mimo 6;",
        ),
        (
            "funcao como valor",
            "carinho mapa_criar(v: bombom) -> bombom {\n    mimo 77;\n}",
            "    nova f: carinho(bombom) -> bombom = mapa_criar;\n    mimo f(1);",
        ),
        (
            "chamada direta",
            "carinho mapa_criar(v: bombom) -> bombom {\n    mimo 77;\n}",
            "    mimo mapa_criar(1);",
        ),
    ];

    for (rotulo, declaracao_raiz, corpo_modulo) in casos {
        let c = caso(
            "raiz_n1d",
            &format!(
                "pacote main;\ntrazer n1d_mod.ua;\n\n{declaracao_raiz}\n\ncarinho principal() -> bombom {{\n    mimo ua();\n}}\n"
            ),
            &[(
                "n1d_mod",
                &format!("pacote n1d_mod;\n\ncarinho ua() -> bombom {{\n{corpo_modulo}\n}}\n"),
            )],
        );
        let saida = checar(&c, "revisao-n1-duplo");
        assert_eq!(
            codigo(&saida),
            1,
            "captura por {rotulo}: {}",
            stderr(&saida)
        );
    }
}

/// Revisão adversarial N1'' — o estreitamento deliberado, e o que a #532 fez
/// com ele.
///
/// A #514 documentou um canto: quando a RAIZ reivindicava a grafia de uma
/// intrínseca, o módulo que quisesse aquele builtin pela grafia crua passava a
/// ser recusado com diagnóstico, em vez de ser religado em silêncio.
///
/// O canto só era alcançável porque ainda existia grafia builtin chamável a
/// seco: `mapa_criar`, a última. A #532 a moveu para `mapa.criar`, e com
/// `GLOBAL_CALLABLE_BUILTIN_EXCEPTIONS = 0` nenhum módulo endereça mais uma
/// intrínseca por grafia crua — cada unidade a traz por import próprio.
///
/// ```text
/// ROOT_DECLARES_OLD_CANONICAL_SPELLING -> NÃO CUSTA A INTRÍNSECA A NINGUÉM
/// ```
///
/// O que este teste mede agora é o dos dois lados: sem reivindicação, o módulo
/// alcança a intrínseca; COM a reivindicação da raiz, ele continua alcançando.
/// A recusa não sumiu por enfraquecimento — sumiu porque a disputa de nome que
/// a produzia deixou de existir.
#[test]
fn revisao_n1_duplo_estreitamento_e_deliberado_e_tem_diagnostico() {
    // Sem reivindicação: o módulo alcança o builtin.
    let livre = caso(
        "raiz_n1e",
        "pacote main;\ntrazer n1e_mod.ua;\n\ncarinho principal() -> bombom {\n    mimo ua();\n}\n",
        &[(
            "n1e_mod",
            "pacote n1e_mod;\ntrazer mapa.criar;\n\ncarinho ua() -> bombom {\n    nova mm: mapa<verso, bombom> = criar();\n    mimo 42;\n}\n",
        )],
    );
    assert_eq!(
        codigo(&executar(&livre, "revisao-n1-duplo-livre")),
        42,
        "o módulo perdeu o builtin sem que ninguém reivindicasse a grafia"
    );

    // Reivindicada pela raiz: recusa COM diagnóstico, nunca religação silenciosa.
    let reivindicada = caso(
        "raiz_n1f",
        "pacote main;\ntrazer n1f_mod.ua;\n\ncarinho mapa_criar(v: bombom) -> bombom {\n    mimo 77;\n}\n\ncarinho principal() -> bombom {\n    mimo ua();\n}\n",
        &[(
            "n1f_mod",
            "pacote n1f_mod;\ntrazer mapa.criar;\n\ncarinho ua() -> bombom {\n    nova mm: mapa<verso, bombom> = criar();\n    mimo 42;\n}\n",
        )],
    );
    let saida = executar(&reivindicada, "revisao-n1-duplo-reivindicada");
    assert_eq!(
        codigo(&saida),
        42,
        "a declaração homônima da raiz custou ao módulo a intrínseca que ele trouxe: {}",
        stderr(&saida)
    );
}

/// Revisão adversarial N1''' — a reivindicação que importa é a da RAIZ.
///
/// Perguntar "alguma unidade declarou esta grafia?" inverteria a
/// não-interferência: um irmão — ou um módulo a três saltos, que este aqui
/// nunca consultou — tiraria dele o builtin, e o diagnóstico ainda apontaria o
/// remédio errado ("importe de lá"), quando o que ele queria era a intrínseca.
///
/// Só a raiz preserva grafia. A entidade de um módulo se chama `M.x` e não pode
/// ser satisfeita por grafia crua, então a declaração de um irmão não captura
/// ninguém e não deve custar nada a ninguém.
#[test]
fn revisao_n1_triplo_reivindicacao_de_irmao_nao_tira_o_builtin_de_ninguem() {
    const USUARIO: &str = "pacote n1t_user;\ntrazer mapa.criar;\ntrazer mapa.definir;\ntrazer mapa.obter;\n\ncarinho uu() -> bombom {\n    nova mm: mapa<verso, bombom> = criar();\n    definir(mm, \"k\", 42);\n    mimo obter(mm, \"k\");\n}\n";

    // Irmão reivindica a grafia, em cada espécie que a #507 permite declarar.
    for (rotulo, declaracao) in [
        ("constante", "eterno mapa_criar: bombom = 1;"),
        (
            "função",
            "carinho mapa_criar(v: bombom) -> bombom {\n    mimo 1;\n}",
        ),
    ] {
        let c = caso(
            "raiz_n1t",
            "pacote main;\ntrazer n1t_user.uu;\ntrazer n1t_claim.uc;\n\ncarinho principal() -> bombom {\n    mimo uu() + uc();\n}\n",
            &[
                ("n1t_user", USUARIO),
                (
                    "n1t_claim",
                    &format!("pacote n1t_claim;\n\n{declaracao}\n\ncarinho uc() -> bombom {{\n    mimo 0;\n}}\n"),
                ),
            ],
        );
        let saida = executar(&c, "revisao-n1-triplo-irmao");
        assert_eq!(
            codigo(&saida),
            42,
            "irmão que declara {rotulo} tirou o builtin de outro módulo: {}",
            stderr(&saida)
        );
    }

    // A três saltos, por um módulo que o usuário do builtin nem consulta para
    // isso: root -> a -> b, e b importa c apenas para outra coisa.
    let transitivo = caso(
        "raiz_n1u",
        "pacote main;\ntrazer n1u_a.ua;\n\ncarinho principal() -> bombom {\n    mimo ua();\n}\n",
        &[
            (
                "n1u_c",
                "pacote n1u_c;\n\neterno mapa_criar: bombom = 1;\n\ncarinho uc() -> bombom {\n    mimo 0;\n}\n",
            ),
            (
                "n1u_b",
                "pacote n1u_b;\ntrazer n1u_c.uc;\ntrazer mapa.criar;\ntrazer mapa.definir;\ntrazer mapa.obter;\n\ncarinho ub() -> bombom {\n    nova mm: mapa<verso, bombom> = criar();\n    definir(mm, \"k\", 42);\n    mimo obter(mm, \"k\") + uc();\n}\n",
            ),
            (
                "n1u_a",
                "pacote n1u_a;\ntrazer n1u_b.ub;\n\ncarinho ua() -> bombom {\n    mimo ub();\n}\n",
            ),
        ],
    );
    let saida = executar(&transitivo, "revisao-n1-triplo-transitivo");
    assert_eq!(
        codigo(&saida),
        42,
        "reivindicação a três saltos tirou o builtin: {}",
        stderr(&saida)
    );

    // #532: a reivindicação da RAIZ também deixou de custar o builtin. O canto
    // documentado pela #514 dependia de existir grafia builtin chamável a seco;
    // com `mapa.criar`, o módulo endereça a intrínseca pelo próprio import e a
    // declaração homônima da raiz é apenas uma função do usuário.
    let raiz_reivindica = caso(
        "raiz_n1v",
        "pacote main;\ntrazer n1t_user.uu;\n\ncarinho mapa_criar(v: bombom) -> bombom {\n    mimo 1;\n}\n\ncarinho principal() -> bombom {\n    mimo uu();\n}\n",
        &[("n1t_user", USUARIO)],
    );
    let saida = executar(&raiz_reivindica, "revisao-n1-triplo-raiz");
    assert_eq!(
        codigo(&saida),
        42,
        "a declaração homônima da raiz tirou o builtin do módulo: {}",
        stderr(&saida)
    );
}

/// Revisão adversarial N5-1 — `__` não é propriedade da Pinker.
///
/// A pergunta "este nome é identidade gerada?" tem autoridade única em
/// `native_symbol::is_compiler_generated`, cuja documentação diz, com todas as
/// letras, que ela NÃO é `starts_with("__")`: o superprefixo é compartilhado
/// pelas famílias e não pertence à linguagem, então `__usuario` é identificador
/// de usuário legal.
///
/// Ter respondido por prefixo criava uma segunda autoridade, mais fraca, e com
/// ela um sétimo caminho de captura: o nome ficava fora do ambiente, fora da
/// canonicalização e fora da recusa, e alcançava a raiz por grafia crua.
#[test]
fn revisao_n5_um_nome_de_usuario_com_prefixo_duplo_e_entidade_de_unidade() {
    // Não captura a raiz.
    let captura = caso(
        "raiz_n5u",
        "pacote main;\ntrazer n5u_mod.f;\n\ncarinho __segredo() -> bombom {\n    mimo 42;\n}\n\ncarinho principal() -> bombom {\n    mimo f();\n}\n",
        &[(
            "n5u_mod",
            "pacote n5u_mod;\n\ncarinho f() -> bombom {\n    mimo __segredo();\n}\n",
        )],
    );
    let saida = checar(&captura, "revisao-n5-um-captura");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "grafia crua com `__` alcançou a raiz: {erro}"
    );
    assert!(erro.contains("__segredo"), "{erro}");

    // E recebe identidade de unidade como qualquer outra entidade de usuário:
    // dois módulos independentes com o mesmo `__h` compõem, sem colidir.
    let homonimos = caso(
        "raiz_n5v",
        "pacote main;\ntrazer n5v_a.fa;\ntrazer n5v_b.fb;\n\ncarinho principal() -> bombom {\n    mimo fa() + fb();\n}\n",
        &[
            (
                "n5v_a",
                "pacote n5v_a;\n\ncarinho __h() -> bombom {\n    mimo 1;\n}\n\ncarinho fa() -> bombom {\n    mimo __h();\n}\n",
            ),
            (
                "n5v_b",
                "pacote n5v_b;\n\ncarinho __h() -> bombom {\n    mimo 20;\n}\n\ncarinho fb() -> bombom {\n    mimo __h();\n}\n",
            ),
        ],
    );
    let saida = executar(&homonimos, "revisao-n5-um-homonimos");
    assert_eq!(codigo(&saida), 21, "{}", stderr(&saida));

    let ir = pink("revisao-n5-um-ir", &["--ir"], &homonimos.raiz);
    let texto = stdout(&ir);
    assert!(texto.contains("func n5v_a.__h"), "IR:\n{texto}");
    assert!(texto.contains("func n5v_b.__h"), "IR:\n{texto}");
}

/// Revisão adversarial N5-3 — identidade reservada do runtime não é declaração
/// de quem a menciona.
///
/// O parser materializa `TipoEntrada`, `LimiteTempo` e `TipoJson` como
/// `Item::Enum` comum em qualquer unidade que as mencione. Sem prefixo `__`,
/// elas eram tratadas como entidade da unidade: a cópia de um módulo virava
/// `M.LimiteTempo` enquanto a superfície do runtime continuava devolvendo
/// `LimiteTempo`, e a MESMA fonte aceita como raiz passava a ser recusada como
/// módulo — a divergência que a #514 existe para fechar, invertida.
#[test]
fn revisao_n5_tres_identidade_reservada_do_runtime_vale_igual_em_raiz_e_modulo() {
    const CORPO: &str = concat!(
        "carinho usa() -> bombom {\n",
        "    nova t: LimiteTempo = LimiteTempo.SemLimite;\n",
        "    encaixe t {\n",
        "        caso LimiteTempo.SemLimite { mimo 7; }\n",
        "        caso LimiteTempo.Ate(ms) { mimo 1; }\n",
        "    }\n",
        "    mimo 0;\n",
        "}\n"
    );

    let como_raiz = caso(
        "raiz_n5t",
        &format!(
            "pacote main;\n\n{CORPO}\ncarinho principal() -> bombom {{\n    mimo usa();\n}}\n"
        ),
        &[],
    );
    let saida_raiz = executar(&como_raiz, "revisao-n5-tres-raiz");
    assert_eq!(codigo(&saida_raiz), 7, "{}", stderr(&saida_raiz));

    let como_modulo = caso(
        "raiz_n5w",
        "pacote main;\ntrazer n5w_mod.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[("n5w_mod", &format!("pacote n5w_mod;\n\n{CORPO}"))],
    );
    let saida_modulo = executar(&como_modulo, "revisao-n5-tres-modulo");
    assert_eq!(
        codigo(&saida_modulo),
        7,
        "a mesma fonte foi recusada por virar módulo: {}",
        stderr(&saida_modulo)
    );
}

/// Revisão adversarial N5-2 — colisão de identidade gerada é ruidosa.
///
/// A deduplicação da projeção repousa em "nome igual prova entidade igual". A
/// premissa vale para closure e para genérico declarado pelo usuário, que
/// codificam a origem. Ela FALHA para especialização de origem builtin: o nome
/// é cunhado no parse a partir da GRAFIA do argumento de tipo, e a
/// canonicalização acontece depois — então duas unidades com um `Cor` local
/// cada produzem o mesmo nome para leques diferentes.
///
/// Este programa também é recusado no `main` baseline, por outro motivo
/// (`identificador 'RC' não declarado`); não é código que funcionava. O que
/// esta correção garante é que a colisão deixe de descartar uma das cópias em
/// silêncio, o que faria a outra unidade ser verificada contra a entidade
/// errada.
#[test]
fn revisao_n5_dois_colisao_de_identidade_gerada_nao_e_silenciosa() {
    let c = caso(
        "raiz_n5g",
        "pacote main;\ntrazer n5g_a.fa;\ntrazer n5g_b.fb;\n\ncarinho principal() -> bombom {\n    mimo fa() + fb();\n}\n",
        &[
            (
                "n5g_a",
                "pacote n5g_a;\n\nleque Cor {\n    Vermelho,\n    Verde,\n}\n\napelido RC = Resultado<Cor, verso>;\n\ncarinho fa() -> bombom {\n    nova r: RC = RC.Ok(Cor.Verde);\n    mimo 2;\n}\n",
            ),
            (
                "n5g_b",
                "pacote n5g_b;\n\nleque Cor {\n    Azul,\n    Amarelo,\n    Preto,\n}\n\napelido RC = Resultado<Cor, verso>;\n\ncarinho fb() -> bombom {\n    nova r: RC = RC.Ok(Cor.Preto);\n    mimo 30;\n}\n",
            ),
        ],
    );
    let saida = checar(&c, "revisao-n5-dois");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("identidade gerada") && erro.contains("entidades diferentes"),
        "a colisão precisa se explicar, não descartar em silêncio: {erro}"
    );
    assert!(
        erro.contains("n5g_a") && erro.contains("n5g_b"),
        "o diagnóstico precisa nomear as duas unidades: {erro}"
    );
}

/// Revisão adversarial N6-1 — identidade reservada do runtime entra uma vez.
///
/// Manter a identidade FORA da canonicalização resolveu só metade: o parser
/// materializa um `Item::Enum` novo em CADA unidade que a mencione, e a
/// projeção não tinha caminho de deduplicação para ela. Duas unidades que
/// tocassem `LimiteTempo` produziam duas declarações do mesmo leque, e a
/// composição era recusada — a MESMA fonte aceita como raiz volta a ser
/// recusada como módulo, que é a divergência que esta Issue existe para fechar.
#[test]
fn revisao_n6_um_identidade_reservada_do_runtime_entra_uma_vez_na_projecao() {
    const USA: &str = concat!(
        "carinho usa() -> bombom {\n",
        "    nova t: LimiteTempo = LimiteTempo.SemLimite;\n",
        "    mimo 7;\n",
        "}\n"
    );

    // Raiz e módulo mencionam a MESMA identidade reservada.
    let raiz_e_modulo = caso(
        "raiz_n6a",
        "pacote main;\ntrazer n6a_mod.usa;\n\ncarinho raiz_usa() -> bombom {\n    nova t: LimiteTempo = LimiteTempo.SemLimite;\n    mimo 5;\n}\n\ncarinho principal() -> bombom {\n    mimo usa() + raiz_usa();\n}\n",
        &[("n6a_mod", &format!("pacote n6a_mod;\n\n{USA}"))],
    );
    let saida = executar(&raiz_e_modulo, "revisao-n6-um-raiz-e-modulo");
    assert_eq!(codigo(&saida), 12, "{}", stderr(&saida));

    // Dois módulos, sem a raiz mencionar nada.
    let dois_modulos = caso(
        "raiz_n6b",
        "pacote main;\ntrazer n6b_a.usa_a;\ntrazer n6b_b.usa_b;\n\ncarinho principal() -> bombom {\n    mimo usa_a() + usa_b();\n}\n",
        &[
            (
                "n6b_a",
                "pacote n6b_a;\n\ncarinho usa_a() -> bombom {\n    nova t: LimiteTempo = LimiteTempo.SemLimite;\n    mimo 3;\n}\n",
            ),
            (
                "n6b_b",
                "pacote n6b_b;\n\ncarinho usa_b() -> bombom {\n    nova t: LimiteTempo = LimiteTempo.SemLimite;\n    mimo 4;\n}\n",
            ),
        ],
    );
    let saida = executar(&dois_modulos, "revisao-n6-um-dois-modulos");
    assert_eq!(codigo(&saida), 7, "{}", stderr(&saida));
}

/// Revisão adversarial N6-2 — a impressão estrutural distingue entidade de
/// grafia, e o fecho concorda com a projeção sobre qual cópia sobrevive.
///
/// Dois defeitos numa fixture só. O primeiro: apelido é transparente, então
/// duas unidades com um `apelido Cor = bombom` privado denotam a MESMA
/// especialização, e compará-las pela grafia canonizada (`fa.Cor` vs `fb.Cor`)
/// as declarava diferentes. O segundo, e o mais grave: o índice do fecho era
/// sobrescrito pela última cópia enquanto a projeção emitia a primeira, então a
/// sobrevivente referenciava um apelido que ninguém materializou.
#[test]
fn revisao_n6_dois_apelido_privado_homonimo_nao_e_colisao() {
    let c = caso(
        "raiz_n6c",
        "pacote main;\ntrazer n6c_a.ga;\ntrazer n6c_b.gb;\n\ncarinho principal() -> bombom {\n    mimo ga() + gb();\n}\n",
        &[
            (
                "n6c_a",
                "pacote n6c_a;\n\napelido Cor = bombom;\napelido RC = Resultado<Cor, bombom>;\n\ncarinho ga() -> bombom {\n    nova r: RC = RC.Ok(11);\n    mimo 11;\n}\n",
            ),
            (
                "n6c_b",
                "pacote n6c_b;\n\napelido Cor = bombom;\napelido RC = Resultado<Cor, bombom>;\n\ncarinho gb() -> bombom {\n    nova r: RC = RC.Ok(22);\n    mimo 22;\n}\n",
            ),
        ],
    );
    let saida = executar(&c, "revisao-n6-dois-falso");
    assert_eq!(
        codigo(&saida),
        33,
        "programa correto foi recusado como colisão: {}",
        stderr(&saida)
    );

    // Contraste: leques nominais homônimos SÃO entidades diferentes, e a
    // colisão continua sendo recusada. Sem este par, a correção acima poderia
    // ter simplesmente desligado a recusa.
    let verdadeira = caso(
        "raiz_n6d",
        "pacote main;\ntrazer n6d_a.fa;\ntrazer n6d_b.fb;\n\ncarinho principal() -> bombom {\n    mimo fa() + fb();\n}\n",
        &[
            (
                "n6d_a",
                "pacote n6d_a;\n\nleque Cor {\n    Vermelho,\n    Verde,\n}\n\napelido RC = Resultado<Cor, bombom>;\n\ncarinho fa() -> bombom {\n    nova r: RC = RC.Ok(Cor.Verde);\n    mimo 2;\n}\n",
            ),
            (
                "n6d_b",
                "pacote n6d_b;\n\nleque Cor {\n    Azul,\n    Amarelo,\n    Preto,\n}\n\napelido RC = Resultado<Cor, bombom>;\n\ncarinho fb() -> bombom {\n    nova r: RC = RC.Ok(Cor.Preto);\n    mimo 30;\n}\n",
            ),
        ],
    );
    let saida = checar(&verdadeira, "revisao-n6-dois-verdadeira");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "colisão real deixou de ser recusada: {erro}"
    );
    assert!(erro.contains("identidade gerada"), "{erro}");
}

/// Revisão adversarial N6-4 — a colisão de superfície de um módulo não fala do
/// arquivo principal.
///
/// A frase histórica foi conservada de propósito quando a regra valia só para a
/// raiz. Ao estendê-la a toda unidade, ela passou a mandar o leitor procurar no
/// arquivo errado.
#[test]
fn revisao_n6_quatro_colisao_de_superficie_nomeia_a_unidade_certa() {
    let c = caso(
        "raiz_n6e",
        "pacote main;\ntrazer n6e_a.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[
            ("n6e_n", "pacote n6e_n;\n\ncarinho x() -> bombom {\n    mimo 1;\n}\n"),
            (
                "n6e_a",
                "pacote n6e_a;\ntrazer n6e_n.x;\n\ncarinho x() -> bombom {\n    mimo 2;\n}\n\ncarinho usa() -> bombom {\n    mimo x();\n}\n",
            ),
        ],
    );
    let saida = checar(&c, "revisao-n6-quatro");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(erro.contains("já existe no módulo 'n6e_a'"), "{erro}");
    assert!(
        !erro.contains("no arquivo principal"),
        "a colisão é do módulo, não do arquivo principal: {erro}"
    );
}

/// Revisão adversarial P7-1 — a impressão estrutural não depende de quem
/// pergunta.
///
/// A expansão de apelidos tinha teto de profundidade e, passado o teto, caía no
/// nome canônico — que é diferente em cada unidade. Duas unidades
/// BYTE-IDÊNTICAS passavam a divergir a partir de uma cadeia de 17 apelidos, e
/// a mensagem de colisão afirmava "entidades diferentes" sobre texto igual. O
/// teto sumiu: a expansão termina porque o conjunto de apelidos é finito, e
/// ciclo rende um marcador fixo, igual para todas as unidades.
#[test]
fn revisao_p7_um_cadeia_longa_de_apelidos_nao_fabrica_colisao() {
    fn unidade(pacote: &str, elos: usize) -> String {
        let mut fonte = format!("pacote {pacote};\n\napelido A1 = bombom;\n");
        for elo in 2..=elos {
            fonte.push_str(&format!("apelido A{elo} = A{};\n", elo - 1));
        }
        fonte.push_str(&format!("apelido Cor = A{elos};\n"));
        fonte.push_str("apelido X = Resultado<Cor, verso>;\n\n");
        fonte.push_str(&format!(
            "carinho f_{pacote}() -> bombom {{\n    nova r: X = X.Ok(1);\n    mimo 1;\n}}\n"
        ));
        fonte
    }

    // O teto antigo era 16. 17 e 40 precisam se comportar como 3.
    for elos in [3usize, 16, 17, 40] {
        let a = unidade("p7a_ma", elos);
        let b = unidade("p7a_mb", elos);
        let c = caso(
            "raiz_p7a",
            "pacote main;\ntrazer p7a_ma.f_p7a_ma;\ntrazer p7a_mb.f_p7a_mb;\n\ncarinho principal() -> bombom {\n    mimo f_p7a_ma() + f_p7a_mb();\n}\n",
            &[("p7a_ma", a.as_str()), ("p7a_mb", b.as_str())],
        );
        let saida = executar(&c, "revisao-p7-um");
        assert_eq!(
            codigo(&saida),
            2,
            "cadeia de {elos} apelidos fabricou desacordo entre unidades idênticas: {}",
            stderr(&saida)
        );
    }
}

/// Revisão adversarial P7-2 — a impressão distingue builtins entre si.
///
/// A versão anterior colhia apenas nomes NOMINAIS, e todo builtin contribuía
/// zero referências: `bombom` e `verso` imprimiam igual. Duas unidades com
/// apelidos para builtins DIFERENTES passavam pela deduplicação, uma cópia era
/// descartada em silêncio e a outra unidade era verificada contra a entidade
/// errada — exatamente o que a recusa de colisão existe para impedir.
#[test]
fn revisao_p7_dois_apelidos_para_builtins_distintos_colidem() {
    let distintos = caso(
        "raiz_p7b",
        "pacote main;\ntrazer p7b_ma.f_ma;\ntrazer p7b_mb.f_mb;\n\ncarinho principal() -> bombom {\n    mimo f_ma() + f_mb();\n}\n",
        &[
            (
                "p7b_ma",
                "pacote p7b_ma;\n\napelido Cor = bombom;\napelido X = Resultado<Cor, verso>;\n\ncarinho f_ma() -> bombom {\n    nova r: X = X.Ok(5);\n    mimo 1;\n}\n",
            ),
            (
                "p7b_mb",
                "pacote p7b_mb;\n\napelido Cor = verso;\napelido X = Resultado<Cor, verso>;\n\ncarinho f_mb() -> bombom {\n    nova r: X = X.Ok(\"oi\");\n    mimo 2;\n}\n",
            ),
        ],
    );
    let saida = checar(&distintos, "revisao-p7-dois-distintos");
    let erro = stderr(&saida);
    assert_eq!(codigo(&saida), 1, "{erro}");
    assert!(
        erro.contains("identidade gerada") && erro.contains("entidades diferentes"),
        "a colisão real precisa ser recusada pela guarda, não a jusante: {erro}"
    );

    // Contraste: apelidos para o MESMO builtin continuam sendo a mesma
    // entidade e continuam compondo. Sem este par, a correção acima poderia
    // ter simplesmente voltado a comparar grafia.
    let iguais = caso(
        "raiz_p7c",
        "pacote main;\ntrazer p7c_ma.f_ma;\ntrazer p7c_mb.f_mb;\n\ncarinho principal() -> bombom {\n    mimo f_ma() + f_mb();\n}\n",
        &[
            (
                "p7c_ma",
                "pacote p7c_ma;\n\napelido Cor = bombom;\napelido X = Resultado<Cor, verso>;\n\ncarinho f_ma() -> bombom {\n    nova r: X = X.Ok(5);\n    mimo 1;\n}\n",
            ),
            (
                "p7c_mb",
                "pacote p7c_mb;\n\napelido Cor = bombom;\napelido X = Resultado<Cor, verso>;\n\ncarinho f_mb() -> bombom {\n    nova r: X = X.Ok(7);\n    mimo 22;\n}\n",
            ),
        ],
    );
    let saida = executar(&iguais, "revisao-p7-dois-iguais");
    assert_eq!(codigo(&saida), 23, "{}", stderr(&saida));
}

/// Revisão adversarial P8-1 — a conferência de colisão custa linear, não
/// exponencial.
///
/// `An = mapa<An-1, An-1>` é um grafo em diamante. Uma representação EXPANDIDA
/// do tipo tem 2^n folhas, e memoizar a expansão não resolve: a própria string
/// memoizada já é exponencial. Trinta e cinco linhas de módulo chegavam a
/// minutos e gigabytes — n=24 levava 47 s e 650 MB, n=26 passava de 200 s e
/// 2,5 GB — enquanto o MESMO texto como raiz saía em 0,00 s, porque a raiz não
/// atravessa a projeção. Era a inversão raiz/módulo mais uma vez, agora medida
/// em tempo e memória em vez de aceitação.
///
/// A conferência passa a comparar um DIGEST de tamanho fixo, memoizado por
/// apelido: o digest de `An` sai do de `An-1` em tempo constante.
///
/// O teste roda sob o prazo do harness, então uma regressão de custo falha por
/// tempo esgotado em vez de pendurar a suíte.
#[test]
fn revisao_p8_um_grafo_de_apelidos_em_diamante_nao_explode() {
    const ELOS: usize = 40;

    let mut modulo = String::from("pacote p8a_mod;\n\napelido A0 = bombom;\n");
    for elo in 1..=ELOS {
        modulo.push_str(&format!(
            "apelido A{elo} = mapa<A{}, A{}>;\n",
            elo - 1,
            elo - 1
        ));
    }
    modulo.push_str(&format!("apelido Cor = A{ELOS};\n"));
    modulo.push_str("apelido X = Resultado<Cor, verso>;\n\n");
    modulo.push_str("carinho usa() -> bombom {\n    nova r: X = X.Erro(\"e\");\n    mimo 4;\n}\n");

    let c = caso(
        "raiz_p8a",
        "pacote main;\ntrazer p8a_mod.usa;\n\ncarinho principal() -> bombom {\n    mimo usa();\n}\n",
        &[("p8a_mod", modulo.as_str())],
    );
    // `mapa` aninhado não é chave válida, então o programa É recusado — e é
    // isso que se afirma. O ponto do teste não é a aceitação: é que a recusa
    // chegue, pela autoridade semântica, em vez de a conferência de colisão
    // ficar expandindo 2^40 folhas antes de qualquer diagnóstico.
    let saida = checar(&c, "revisao-p8-um");
    let erro = stderr(&saida);
    assert_eq!(
        codigo(&saida),
        1,
        "grafo de {ELOS} apelidos em diamante não terminou dentro do prazo: {erro}"
    );
    assert!(
        erro.contains("tipo de chave de mapa incompatível"),
        "a recusa devia vir da autoridade semântica: {erro}"
    );
}

/// Revisão adversarial P9-1 — a forma da coleção é uma só, soletrada de dois
/// jeitos.
///
/// `lista<bombom>` tem DUAS representações no AST: a nominal legada
/// (`ListBombom`, quando o elemento é soletrado direto) e a genérica
/// (`ListEnum`, quando vem por apelido). Elas denotam o mesmo tipo. A versão em
/// texto do fingerprint as unificava por acidente — as duas rendiam a mesma
/// string — e a troca por digest com uma tag por variante desfez isso, passando
/// a recusar duas unidades que denotam a MESMA lista só porque uma soletrou o
/// elemento direto e a outra por apelido.
///
/// A unificação agora é deliberada: uma tag por FORMA, com o digest do conteúdo
/// dentro.
#[test]
fn revisao_p9_um_lista_soletrada_de_dois_jeitos_e_a_mesma_entidade() {
    fn unidade(pacote: &str, declaracoes: &str, retorno: u32) -> String {
        format!(
            "pacote {pacote};\n\n{declaracoes}\napelido RC = Resultado<C, verso>;\n\ncarinho f_{pacote}() -> bombom {{\n    nova r: RC = RC.Erro(\"e\");\n    mimo {retorno};\n}}\n"
        )
    }

    // (rótulo, declarações de p9a, declarações de p9b, compõe?)
    let casos: &[(&str, &str, &str, bool)] = &[
        (
            "lista<bombom> direto vs por apelido",
            "apelido C = lista<bombom>;",
            "apelido B = bombom;\napelido C = lista<B>;",
            true,
        ),
        (
            "lista<verso> direto vs por apelido",
            "apelido C = lista<verso>;",
            "apelido V = verso;\napelido C = lista<V>;",
            true,
        ),
        // A unificação não pode custar fidelidade: elementos diferentes
        // continuam sendo entidades diferentes.
        (
            "lista<bombom> vs lista<verso>",
            "apelido C = lista<bombom>;",
            "apelido C = lista<verso>;",
            false,
        ),
    ];

    for (rotulo, a, b, compoe) in casos {
        let c = caso(
            "raiz_p9a",
            "pacote main;\ntrazer p9a.f_p9a;\ntrazer p9b.f_p9b;\n\ncarinho principal() -> bombom {\n    mimo f_p9a() + f_p9b();\n}\n",
            &[
                ("p9a", unidade("p9a", a, 16).as_str()),
                ("p9b", unidade("p9b", b, 16).as_str()),
            ],
        );
        let saida = checar(&c, "revisao-p9-um");
        let erro = stderr(&saida);
        if *compoe {
            assert_eq!(
                codigo(&saida),
                0,
                "{rotulo}: composição correta foi recusada: {erro}"
            );
        } else {
            assert_eq!(codigo(&saida), 1, "{rotulo}: {erro}");
            assert!(
                erro.contains("identidade gerada"),
                "{rotulo}: a recusa devia vir da guarda de colisão: {erro}"
            );
        }
    }
}

/// Revisão adversarial P10-1/P10-2 — a guarda de identidade gerada consome uma
/// chave exata da mesma autoridade normativa de uniões.
///
/// Ordem, aninhamento e duplicatas não participam da identidade de uma união.
/// Diferença real de membro continua participando. União não é carga admissível
/// de `Resultado` hoje, então o contraexemplo por composição é recusado antes
/// da projeção; o oracle correto prova a autoridade diretamente e prova que a
/// projeção a consome, sem inventar alcançabilidade.
#[test]
fn revisao_p10_uniao_canonica_tem_identidade_exata_na_guarda() {
    let span = span_sintetico();
    let bombom = || Type::Bombom(span);
    let verso = || Type::Verso(span);
    let logica = || Type::Logica(span);

    let direta = Type::Union {
        members: vec![bombom(), verso()],
        span,
    };
    let invertida = Type::Union {
        members: vec![verso(), bombom()],
        span,
    };
    let aninhada_com_duplicata = Type::Union {
        members: vec![
            bombom(),
            Type::Union {
                members: vec![verso(), bombom()],
                span,
            },
        ],
        span,
    };
    let diferente = Type::Union {
        members: vec![bombom(), logica()],
        span,
    };

    let aliases = HashMap::new();
    let chave = |ty| pinker_v0::union_canon::canonical_type_graph_key(ty, &aliases);
    assert_eq!(chave(&direta), chave(&invertida));
    assert_eq!(chave(&direta), chave(&aninhada_com_duplicata));
    assert_ne!(chave(&direta), chave(&diferente));

    let mut aliases = HashMap::new();
    aliases.insert("Parte".to_string(), invertida);
    let via_alias = Type::Union {
        members: vec![
            Type::Alias {
                name: "Parte".to_string(),
                span,
            },
            bombom(),
        ],
        span,
    };
    assert_eq!(
        pinker_v0::union_canon::canonical_type_graph_key(&direta, &HashMap::new()),
        pinker_v0::union_canon::canonical_type_graph_key(&via_alias, &aliases),
        "apelido, aninhamento e duplicata devem desaparecer juntos"
    );

    let comparador = include_str!("../src/module_resolve.rs");
    assert!(
        !comparador.contains("digest_de_tipo"),
        "a igualdade estrutural não pode voltar a uma autoridade probabilística"
    );
    assert!(
        comparador.contains("canonical_type_graph_key"),
        "a projeção deve consumir a chave exata da autoridade canônica"
    );
}
// @pinker-nav:end evidencia.modulos.revisao-adversarial
