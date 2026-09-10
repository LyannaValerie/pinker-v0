mod common;

use common::rust_source::codigo_executavel;
use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// Prova comportamental da #649/U-03B: sob `POLICY_B_RELATION_REACHABILITY_ALWAYS_MATTERS`, uma relação canônica de `impl` só participa de uma operação quando existe caminho de import/composição autorizado dela até o contexto responsável — poder NOMEAR o trato não é caminho nenhum. A matriz cobre as três superfícies em que uma relação concreta é selecionada ou consumida (chamada não qualificada, chamada qualificada e formação de objeto de trato), cada negativo pareado com o controle positivo que difere dele por uma única linha de `trazer`, mais os controles que provam que a fixture chega à decisão de alcance em vez de falhar antes por nomeabilidade; preserva a composição transitiva legítima da #577, o arquivo único sem composição e a relação local; prova que o despacho dinâmico posterior consome a vtable já autorizada na formação, sem refazer busca modular; e fecha com as leis unitárias da autoridade de alcance e o guarda estrutural que impede a nomeabilidade do trato de voltar a autorizar relação. O oráculo é o valor executado e a mensagem renderizada, nunca a simples ausência de erro.

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn escrever(dir: &Path, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.join(format!("{nome}.pink"));
    fs::write(&caminho, fonte)
        .unwrap_or_else(|erro| panic!("gravar {}: {erro}", caminho.display()));
    caminho
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, String)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #649");
    for (modulo, fonte) in modulos {
        escrever(dir.path(), modulo, fonte);
    }
    let raiz = escrever(dir.path(), nome, raiz);
    Caso { dir, raiz }
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

fn codigo(saida: &std::process::Output) -> i32 {
    saida.status.code().expect("status com código")
}

fn stderr(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

// ---------------------------------------------------------------------------
// A topologia central
//
// `m649_tr` DECLARA o trato. `m649_impl` declara a ÚNICA relação e devolve 77.
// `m649_c` escreve a chamada, e a única variável dos casos abaixo é o que ELE
// importa. A raiz importa `m649_impl` para que a relação esteja carregada no
// programa: presença no programa não é autorização, e é exatamente isso que
// estes casos medem.
// ---------------------------------------------------------------------------

const TR: &str = "pacote m649_tr;\n\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
const IMPL: &str = "pacote m649_impl;\ntrazer m649_tr.Medida;\n\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 77; }\n}\n";
const RAIZ: &str = "pacote main;\ntrazer m649_impl;\ntrazer m649_c.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n";

/// As três formas em que uma relação concreta é selecionada ou consumida.
const CORPO_NAO_QUALIFICADA: &str = "    mimo x.medir();\n";
const CORPO_QUALIFICADA: &str = "    mimo Medida.medir(x);\n";
const CORPO_OBJETO: &str =
    "    nova o: trato<Medida> = x virar trato<Medida>;\n    mimo o.medir();\n";

fn caso_central(nome: &str, imports: &str, corpo: &str) -> Caso {
    let chamador =
        format!("pacote m649_c;\n{imports}\ncarinho usar(x: bombom) -> bombom {{\n{corpo}}}\n");
    caso(
        nome,
        RAIZ,
        &[
            ("m649_tr", TR.to_string()),
            ("m649_impl", IMPL.to_string()),
            ("m649_c", chamador),
        ],
    )
}

/// Sem o caminho autorizado até `m649_impl`, o chamador apenas NOMEIA o trato.
const SO_O_TRATO: &str = "trazer m649_tr.Medida;\n";
/// Com o caminho autorizado: a relação de `m649_impl` passa a alcançar.
const TRATO_E_IMPLEMENTADOR: &str = "trazer m649_tr.Medida;\ntrazer m649_impl;\n";

// ---------------------------------------------------------------------------
// Forma NÃO qualificada
// ---------------------------------------------------------------------------

/// O caso causal da #579/HD-01, agora decidido: o chamador declara ou importa
/// o trato, a relação irmã está carregada no programa, e não há aresta
/// autorizada entre os dois.
///
/// Antes da #649 isto era aceito e executava 77, pelo nível próprio concedido a
/// quem podia nomear o trato. `POLICY_B` recusa.
#[test]
fn b1_irmao_sem_caminho_autorizado_nao_e_candidato_na_forma_nao_qualificada() {
    let c = caso_central("b1_649", SO_O_TRATO, CORPO_NAO_QUALIFICADA);
    let checagem = pink("649-b1-check", &["--check"], &c.raiz);
    assert_eq!(
        codigo(&checagem),
        1,
        "nomear o trato não pode autorizar a relação de m649_impl: {}",
        stderr(&checagem)
    );
    assert!(
        stderr(&checagem).contains("método 'medir' não implementado para tipo 'bombom'"),
        "a recusa é a do despacho sem candidato ALCANÇÁVEL, com o dono de \
         diagnóstico preservado: {}",
        stderr(&checagem)
    );
    let execucao = pink("649-b1-run", &["--run"], &c.raiz);
    assert_eq!(
        codigo(&execucao),
        1,
        "o lowering tem de recusar o que `--check` recusou: {}",
        stderr(&execucao)
    );
}

/// Controle positivo de B1, pareado: as MESMAS fontes com uma linha de `trazer`
/// a mais no chamador. É o que prova que a recusa acima é da decisão de
/// alcance, e não de nomeabilidade, tipo, aridade ou fixture malformada.
#[test]
fn b2_implementador_importado_restaura_a_forma_nao_qualificada() {
    let c = caso_central("b2_649", TRATO_E_IMPLEMENTADOR, CORPO_NAO_QUALIFICADA);
    let checagem = pink("649-b2-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-b2-run", &["--run"], &c.raiz);
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

// ---------------------------------------------------------------------------
// Forma QUALIFICADA — `Trato.metodo(receiver, ...)`
// ---------------------------------------------------------------------------

/// Escrever o trato não é bypass da política modular. A identidade exata existe
/// — `method_identity` a encontra —, e mesmo assim a relação não é autorizada
/// neste contexto.
///
/// A mensagem tem de distinguir isto de método ausente: o método existe, a
/// relação existe, e o que falta é caminho.
#[test]
fn b3_chamada_qualificada_nao_atravessa_o_isolamento_entre_irmaos() {
    let c = caso_central("b3_649", SO_O_TRATO, CORPO_QUALIFICADA);
    let checagem = pink("649-b3-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 1, "{}", stderr(&checagem));
    assert!(
        stderr(&checagem)
            .contains("impl de 'm649_tr.Medida' para tipo 'bombom' não é alcançável desta unidade"),
        "a recusa tem de acusar ALCANCE da relação, não ausência de método: {}",
        stderr(&checagem)
    );
    let execucao = pink("649-b3-run", &["--run"], &c.raiz);
    assert_eq!(
        codigo(&execucao),
        1,
        "o lowering tem de recusar o que `--check` recusou: {}",
        stderr(&execucao)
    );
}

/// Controle positivo de B3.
#[test]
fn b4_chamada_qualificada_com_relacao_alcancavel_continua_aceita() {
    let c = caso_central("b4_649", TRATO_E_IMPLEMENTADOR, CORPO_QUALIFICADA);
    let checagem = pink("649-b4-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-b4-run", &["--run"], &c.raiz);
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

/// A autoridade de identidade da U-03A continua respondendo primeiro, e por
/// isso método ausente continua sendo método ausente — não vira "inalcançável".
/// Sem este caso, a mensagem nova poderia estar mascarando a antiga.
#[test]
fn metodo_ausente_continua_sendo_metodo_ausente_e_nao_falta_de_alcance() {
    let c = caso_central(
        "bident_649",
        TRATO_E_IMPLEMENTADOR,
        "    mimo Medida.inexistente(x);\n",
    );
    let checagem = pink("649-ident-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 1, "{}", stderr(&checagem));
    assert!(
        stderr(&checagem)
            .contains("método 'm649_tr.Medida.inexistente' não implementado para tipo 'bombom'"),
        "identidade responde ANTES do alcance: {}",
        stderr(&checagem)
    );
}

// ---------------------------------------------------------------------------
// Objeto de trato — a relação concreta entra na representação dinâmica
// ---------------------------------------------------------------------------

/// A conversão `x virar trato<Medida>` escolhe uma relação concreta para
/// construir a vtable. É esse o ponto de formação, e é nele que o alcance é
/// perguntado.
#[test]
fn b5_objeto_de_trato_nao_se_forma_com_relacao_inalcancavel() {
    let c = caso_central("b5_649", SO_O_TRATO, CORPO_OBJETO);
    let checagem = pink("649-b5-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 1, "{}", stderr(&checagem));
    let erro = stderr(&checagem);
    assert!(
        erro.contains("não é alcançável desta unidade e não pode formar 'trato<m649_tr.Medida>'"),
        "a recusa tem de ser da FORMAÇÃO do objeto, pela relação inalcançável: {erro}"
    );
    assert!(
        !erro.contains("não implementa o trato"),
        "o tipo IMPLEMENTA o trato; confundir as duas causas esconderia a política: {erro}"
    );
    let execucao = pink("649-b5-run", &["--run"], &c.raiz);
    assert_eq!(codigo(&execucao), 1, "{}", stderr(&execucao));
}

/// Controle positivo de B5.
#[test]
fn b6_objeto_de_trato_com_relacao_alcancavel_continua_se_formando() {
    let c = caso_central("b6_649", TRATO_E_IMPLEMENTADOR, CORPO_OBJETO);
    let checagem = pink("649-b6-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-b6-run", &["--run"], &c.raiz);
    assert_eq!(codigo(&execucao), 77, "{}", stderr(&execucao));
}

/// `DYNAMIC_DISPATCH_REDOES_GLOBAL_MODULE_SEARCH = FALSE`, provado pelo
/// comportamento e não por leitura de código.
///
/// `m649_form` alcança a relação e forma o objeto; `m649_use` recebe o objeto
/// pronto, NOMEIA o trato para poder escrever o tipo e NÃO importa
/// `m649_impl`. A chamada dinâmica consome a vtable já autorizada e funciona.
/// Se o despacho dinâmico refizesse a busca modular no sítio da chamada, esta
/// mesma topologia seria recusada — é o par exato de B5, com a formação movida
/// para a unidade autorizada.
#[test]
fn chamada_dinamica_consome_a_vtable_autorizada_na_formacao() {
    let form = "pacote m649_form;\ntrazer m649_tr.Medida;\ntrazer m649_impl;\n\ncarinho formar(x: bombom) -> trato<Medida> {\n    mimo x virar trato<Medida>;\n}\n";
    let user = "pacote m649_use;\ntrazer m649_tr.Medida;\ntrazer m649_form.formar;\n\ncarinho usar(x: bombom) -> bombom {\n    nova o: trato<Medida> = formar(x);\n    mimo o.medir();\n}\n";
    let c = caso(
        "bdin_649",
        "pacote main;\ntrazer m649_impl;\ntrazer m649_use.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &[
            ("m649_tr", TR.to_string()),
            ("m649_impl", IMPL.to_string()),
            ("m649_form", form.to_string()),
            ("m649_use", user.to_string()),
        ],
    );
    let checagem = pink("649-dinamica-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-dinamica-run", &["--run"], &c.raiz);
    assert_eq!(
        codigo(&execucao),
        77,
        "a chamada dinâmica não pode refazer busca modular: {}",
        stderr(&execucao)
    );
}

// ---------------------------------------------------------------------------
// O que POLICY_B não pode quebrar
// ---------------------------------------------------------------------------

/// Controle de causa: o chamador não nomeia o trato NEM importa a unidade da
/// relação. Já era recusado antes da #649 e continua. Sem ele, B1 poderia estar
/// medindo uma recusa que a fixture sofreria de qualquer jeito.
#[test]
fn controle_sem_trato_e_sem_unidade_ja_era_recusado() {
    let c = caso_central("bctl_649", "", CORPO_NAO_QUALIFICADA);
    let checagem = pink("649-ctl-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 1, "{}", stderr(&checagem));
}

/// #577 — composição transitiva legítima. A raiz importa APENAS o
/// implementador, nunca nomeia o trato de `m649o`, e continua alcançando a
/// relação. Uma leitura excessivamente local de `POLICY_B` quebraria isto.
#[test]
fn composicao_transitiva_da_577_continua_valida() {
    let origem = "pacote m649o;\n\ntrato Marca {\n    carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n}\n";
    let implementador = "pacote m649i;\ntrazer m649o.Marca;\n\nimpl Marca para bombom {}\n\ncarinho usar() -> bombom {\n    nova x: bombom = 10;\n    mimo x.padrao();\n}\n";
    let c = caso(
        "b7_649",
        "pacote main;\ntrazer m649i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo y.padrao() + usar();\n}\n",
        &[
            ("m649o", origem.to_string()),
            ("m649i", implementador.to_string()),
        ],
    );
    let checagem = pink("649-577-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-577-run", &["--run"], &c.raiz);
    assert_eq!(
        codigo(&execucao),
        32,
        "a dependência semântica transportada da #577 tem de continuar valendo: {}",
        stderr(&execucao)
    );
}

/// A raiz continua sem NOMEAR o trato que nunca importou: dependência semântica
/// transitiva não é reexport público de nomes. É o contrato que separa a #577
/// de `NO_IMPLICIT_REEXPORT`, e a #649 não o afrouxa.
#[test]
fn transitividade_nao_virou_reexport_de_nome() {
    let origem = "pacote m649o;\n\ntrato Marca {\n    carinho padrao(valor: bombom) -> bombom { mimo valor + 1; }\n}\n";
    let implementador = "pacote m649i;\ntrazer m649o.Marca;\n\nimpl Marca para bombom {}\n\ncarinho usar() -> bombom { mimo 11; }\n";
    let c = caso(
        "b7n_649",
        "pacote main;\ntrazer m649i.usar;\n\ncarinho principal() -> bombom {\n    nova y: bombom = 20;\n    mimo Marca.padrao(y) + usar();\n}\n",
        &[
            ("m649o", origem.to_string()),
            ("m649i", implementador.to_string()),
        ],
    );
    let checagem = pink("649-577-nome-check", &["--check"], &c.raiz);
    assert_eq!(codigo(&checagem), 1, "{}", stderr(&checagem));
    assert!(
        stderr(&checagem).contains("identificador 'Marca' não declarado"),
        "a grafia do trato continua fora do ambiente da raiz: {}",
        stderr(&checagem)
    );
}

/// Relação declarada na PRÓPRIA unidade do chamador, sobre trato importado:
/// alcance pelo nível próprio, nas três formas.
#[test]
fn relacao_local_sobre_trato_importado_continua_aceita_nas_tres_formas() {
    for (nome, corpo) in [
        ("nao-qualificada", CORPO_NAO_QUALIFICADA),
        ("qualificada", CORPO_QUALIFICADA),
        ("objeto-de-trato", CORPO_OBJETO),
    ] {
        let chamador = format!(
            "pacote m649_c;\ntrazer m649_tr.Medida;\n\nimpl Medida para bombom {{\n    carinho medir(valor: bombom) -> bombom {{ mimo 77; }}\n}}\n\ncarinho usar(x: bombom) -> bombom {{\n{corpo}}}\n"
        );
        let c = caso(
            &format!("b11_{nome}_649"),
            "pacote main;\ntrazer m649_c.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
            &[("m649_tr", TR.to_string()), ("m649_c", chamador)],
        );
        let logico = format!("649-local-{nome}");
        let checagem = pink(&logico, &["--check"], &c.raiz);
        assert_eq!(codigo(&checagem), 0, "local/{nome}: {}", stderr(&checagem));
        let execucao = pink(&logico, &["--run"], &c.raiz);
        assert_eq!(codigo(&execucao), 77, "local/{nome}: {}", stderr(&execucao));
    }
}

/// Arquivo único: não há composição, não há a quem restringir, e o índice vazio
/// continua concedendo o nível próprio. `POLICY_B` não fecha fluxo legítimo.
#[test]
fn arquivo_unico_sem_composicao_continua_aceito_nas_tres_formas() {
    for (nome, corpo) in [
        ("nao-qualificada", CORPO_NAO_QUALIFICADA),
        ("qualificada", CORPO_QUALIFICADA),
        ("objeto-de-trato", CORPO_OBJETO),
    ] {
        let fonte = format!(
            "pacote main;\n\ntrato Medida {{\n    carinho medir(valor: si) -> bombom;\n}}\n\nimpl Medida para bombom {{\n    carinho medir(valor: bombom) -> bombom {{ mimo 77; }}\n}}\n\ncarinho usar(x: bombom) -> bombom {{\n{corpo}}}\n\ncarinho principal() -> bombom {{ mimo usar(5); }}\n"
        );
        let dir = NativeArtifactDir::create().expect("diretório do caso único #649");
        let raiz = escrever(dir.path(), &format!("unico_{nome}"), &fonte);
        let logico = format!("649-unico-{nome}");
        let checagem = pink(&logico, &["--check"], &raiz);
        assert_eq!(codigo(&checagem), 0, "único/{nome}: {}", stderr(&checagem));
        let execucao = pink(&logico, &["--run"], &raiz);
        assert_eq!(codigo(&execucao), 77, "único/{nome}: {}", stderr(&execucao));
    }
}

/// A ordem dos imports não decide alcance.
#[test]
fn ordem_de_import_nao_muda_o_alcance() {
    for (nome, imports) in [
        ("direta", "trazer m649_tr.Medida;\ntrazer m649_impl;\n"),
        ("invertida", "trazer m649_impl;\ntrazer m649_tr.Medida;\n"),
    ] {
        let c = caso_central(&format!("bord_{nome}_649"), imports, CORPO_QUALIFICADA);
        let logico = format!("649-ordem-{nome}");
        assert_eq!(
            codigo(&pink(&logico, &["--check"], &c.raiz)),
            0,
            "ordem {nome}"
        );
        assert_eq!(
            codigo(&pink(&logico, &["--run"], &c.raiz)),
            77,
            "ordem {nome}"
        );
    }
}

/// Tratos e alvos homônimos em unidades distintas: o alcance segue a unidade
/// que DECLAROU cada relação, e importar um implementador não autoriza o outro.
#[test]
fn homonimos_em_unidades_distintas_nao_compartilham_alcance() {
    let ta = "pacote m649ta;\n\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let tb = "pacote m649tb;\n\ntrato Medida {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let ia = "pacote m649ia;\ntrazer m649ta.Medida;\n\nimpl Medida para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 11; }\n}\n";
    let ib = "pacote m649ib;\ntrazer m649tb.Medida;\n\nimpl Medida para verso {\n    carinho medir(valor: verso) -> bombom { mimo 22; }\n}\n";
    let raiz = "pacote main;\ntrazer m649ia;\ntrazer m649ib;\ntrazer m649_c.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n";
    let modulos = |chamador: String| {
        vec![
            ("m649ta", ta.to_string()),
            ("m649tb", tb.to_string()),
            ("m649ia", ia.to_string()),
            ("m649ib", ib.to_string()),
            ("m649_c", chamador),
        ]
    };

    // Importa o trato de `ta` E o implementador `ia`: alcança, e o valor prova
    // qual das duas relações homônimas foi usada.
    let c = caso(
        "bhom_ok_649",
        raiz,
        &modulos("pacote m649_c;\ntrazer m649ta.Medida;\ntrazer m649ia;\n\ncarinho usar(x: bombom) -> bombom { mimo Medida.medir(x); }\n".to_string()),
    );
    assert_eq!(codigo(&pink("649-hom-ok", &["--check"], &c.raiz)), 0);
    assert_eq!(codigo(&pink("649-hom-ok", &["--run"], &c.raiz)), 11);

    // Mesmo trato nomeado, mas o implementador importado é o do OUTRO trato:
    // a relação de `ia` continua sem caminho autorizado.
    let c = caso(
        "bhom_cruzado_649",
        raiz,
        &modulos("pacote m649_c;\ntrazer m649ta.Medida;\ntrazer m649ib;\n\ncarinho usar(x: bombom) -> bombom { mimo Medida.medir(x); }\n".to_string()),
    );
    let saida = pink("649-hom-cruzado", &["--check"], &c.raiz);
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
    assert!(
        stderr(&saida)
            .contains("impl de 'm649ta.Medida' para tipo 'bombom' não é alcançável desta unidade"),
        "importar o implementador de um homônimo não autoriza a relação do outro: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// Competição entre relações ALCANÇÁVEIS — o nível segue a relação
// ---------------------------------------------------------------------------
//
// A regra de precedência não mudou: vence o nível mais forte, e `Proprio`
// precede `PorUnidadeImportada` (#577). O que POLICY_B mudou é a ENTRADA dessa
// classificação — o nível passou a seguir a unidade que declarou a RELAÇÃO, e
// não o fato de o chamador poder nomear o trato. Quando duas relações
// alcançáveis competem pelo mesmo `(alvo, método)`, isso é observável, e as
// duas transições abaixo são deltas intencionais desta Task.

/// Relação PRÓPRIA contra relação de unidade importada.
///
/// `c` declara o trato `B` e a relação `B para bombom`; a relação `A para
/// bombom` vem de `ia`, que `c` importou. As duas alcançam.
///
/// Antes da #649 as duas recebiam `Proprio` — uma por declaração, a outra
/// porque `c` também nomeia `A` — e a chamada era AMBÍGUA. Sob POLICY_B a
/// relação declarada por `c` é a própria e a transportada é subordinada, então
/// a autoridade da unidade vence sozinha. É a subordinação que a #577 enunciou,
/// agora aplicada à relação em vez de ao nome do trato.
#[test]
fn relacao_propria_vence_relacao_de_unidade_importada() {
    let tr = "pacote tr;\n\ntrato A {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let ia = "pacote ia;\ntrazer tr.A;\n\nimpl A para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 20; }\n}\n";
    let c = "pacote c;\ntrazer tr.A;\ntrazer ia;\n\ntrato B {\n    carinho medir(valor: si) -> bombom;\n}\n\nimpl B para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 3; }\n}\n\ncarinho usar(x: bombom) -> bombom { mimo x.medir(); }\n";
    let caso = caso(
        "prec_propria_649",
        "pacote main;\ntrazer c.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n",
        &[
            ("tr", tr.to_string()),
            ("ia", ia.to_string()),
            ("c", c.to_string()),
        ],
    );
    let checagem = pink("649-prec-propria-check", &["--check"], &caso.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-prec-propria-run", &["--run"], &caso.raiz);
    assert_eq!(
        codigo(&execucao),
        3,
        "a relação declarada pela própria unidade tem de vencer a transportada: {}",
        stderr(&execucao)
    );
}

/// Duas relações TRANSPORTADAS competindo: nomear um dos tratos não desempata.
///
/// `c` importa `ia` e `ib`, e nomeia apenas `tr.A`. As duas relações alcançam
/// `c` pelo mesmo nível subordinado.
///
/// Antes da #649 nomear `A` promovia aquela relação a `Proprio` e ela vencia
/// sozinha, executando 20. Sob POLICY_B a nomeabilidade não decide nada —
/// inclusive não decide precedência —, os dois candidatos ficam no mesmo nível
/// e a chamada é ambígua, com a mensagem que manda qualificar. Qualificar
/// continua funcionando, e é o que o controle abaixo prova.
#[test]
fn nomear_um_dos_tratos_nao_desempata_duas_relacoes_transportadas() {
    let tr = "pacote tr;\n\ntrato A {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let tr2 = "pacote tr2;\n\ntrato B {\n    carinho medir(valor: si) -> bombom;\n}\n";
    let ia = "pacote ia;\ntrazer tr.A;\n\nimpl A para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 20; }\n}\n";
    let ib = "pacote ib;\ntrazer tr2.B;\n\nimpl B para bombom {\n    carinho medir(valor: bombom) -> bombom { mimo 3; }\n}\n";
    let raiz = "pacote main;\ntrazer c.usar;\n\ncarinho principal() -> bombom { mimo usar(5); }\n";
    let modulos = |corpo: &str| {
        vec![
            ("tr", tr.to_string()),
            ("tr2", tr2.to_string()),
            ("ia", ia.to_string()),
            ("ib", ib.to_string()),
            (
                "c",
                format!("pacote c;\ntrazer tr.A;\ntrazer ia;\ntrazer ib;\n\ncarinho usar(x: bombom) -> bombom {{ {corpo} }}\n"),
            ),
        ]
    };

    let ambiguo = caso("prec_transp_649", raiz, &modulos("mimo x.medir();"));
    let checagem = pink("649-prec-transp-check", &["--check"], &ambiguo.raiz);
    assert_eq!(codigo(&checagem), 1, "{}", stderr(&checagem));
    assert!(
        stderr(&checagem).contains("é ambíguo"),
        "a recusa tem de ser de ambiguidade entre dois subordinados, não de alcance: {}",
        stderr(&checagem)
    );

    // Controle: a saída que a própria mensagem indica continua disponível, e
    // resolve para a relação nomeada — a ambiguidade é de seleção, não perda
    // de alcance.
    let qualificado = caso("prec_transp_q_649", raiz, &modulos("mimo A.medir(x);"));
    let checagem = pink("649-prec-transp-q-check", &["--check"], &qualificado.raiz);
    assert_eq!(codigo(&checagem), 0, "{}", stderr(&checagem));
    let execucao = pink("649-prec-transp-q-run", &["--run"], &qualificado.raiz);
    assert_eq!(codigo(&execucao), 20, "{}", stderr(&execucao));
}

// ---------------------------------------------------------------------------
// Paridade interpretador × nativo
// ---------------------------------------------------------------------------

/// A política é uma só: o ELF nativo observa o mesmo resultado do interpretador
/// no caso positivo, e o caso recusado não chega a gerar binário.
#[test]
fn paridade_interpretador_e_nativo_da_politica_de_alcance() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-649-paridade", true)
    else {
        return;
    };
    let positivo = caso_central("paridade_649", TRATO_E_IMPLEMENTADOR, CORPO_QUALIFICADA);
    let interpretado = pink("649-paridade-interpretador", &["--run"], &positivo.raiz);
    assert_eq!(codigo(&interpretado), 77, "{}", stderr(&interpretado));

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(positivo.dir.path())
        .arg(&positivo.raiz)
        .env("PINKER_RT_LIB", &runtime_lib)
        .logical_case("649-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #649");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nativo = Command::new(positivo.dir.path().join("paridade_649"))
        .logical_case("649-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #649");
    assert_eq!(
        nativo.status.code(),
        interpretado.status.code(),
        "interpretador e nativo têm de consumir a mesma decisão de alcance"
    );

    // O negativo não produz artefato: a recusa acontece antes do backend.
    let negativo = caso_central("paridade_neg_649", SO_O_TRATO, CORPO_QUALIFICADA);
    let build_negativo = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(negativo.dir.path())
        .arg(&negativo.raiz)
        .env("PINKER_RT_LIB", &runtime_lib)
        .logical_case("649-paridade-build-negativo")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo negativo #649");
    assert!(
        !build_negativo.status.success(),
        "relação inalcançável não pode gerar binário"
    );
    assert!(
        !negativo.dir.path().join("paridade_neg_649").exists(),
        "nenhum artefato nativo pode sobrar do caso recusado"
    );
}

// ---------------------------------------------------------------------------
// Leis unitárias da autoridade de alcance
// ---------------------------------------------------------------------------

use pinker_v0::lexer::Lexer;
use pinker_v0::module_graph::ModuleGraph;
use pinker_v0::module_resolve::{
    nivel_de_despacho, relacao_alcanca, resolver_grafo, tratos_visiveis_por_fonte, NivelDeDespacho,
};
use pinker_v0::parser::Parser;
use pinker_v0::source_map::{SourceId, SourceMap};
use pinker_v0::token::{Position, Span};

fn programa(fonte: &str, source: SourceId) -> pinker_v0::ast::Program {
    let tokens = Lexer::com_fonte(fonte, source)
        .tokenize()
        .expect("lexar fonte do caso");
    Parser::new(tokens).parse().expect("parsear fonte do caso")
}

const RAIZ_LEI: &str =
    "pacote main;\ntrazer m649_lei.Marca;\n\ncarinho principal() -> bombom { mimo 0; }\n";
const IMPORTADO_LEI: &str =
    "pacote m649_lei;\n\ntrato Marca {\n    carinho marcar(valor: si) -> bombom { mimo 1; }\n}\n";
const IRMAO_LEI: &str = "pacote m649_irmao;\n";

/// Grafo com composição: a raiz importa `m649_lei.Marca` por NOME; `m649_irmao`
/// é unidade do programa e a raiz nunca a pediu.
fn grafo_com_irmao() -> (ModuleGraph, SourceId) {
    let mut sources = SourceMap::new();
    let raiz = sources.register_root("raiz.pink", RAIZ_LEI.to_string());
    let importado = sources.register_module("m649_lei", "m649_lei.pink", IMPORTADO_LEI.to_string());
    let irmao = sources.register_module("m649_irmao", "m649_irmao.pink", IRMAO_LEI.to_string());
    let mut graph = ModuleGraph::new();
    graph.insert_root(raiz, "raiz.pink", programa(RAIZ_LEI, raiz));
    graph.insert_module(
        "m649_lei",
        importado,
        "m649_lei.pink",
        programa(IMPORTADO_LEI, importado),
    );
    graph.insert_module(
        "m649_irmao",
        irmao,
        "m649_irmao.pink",
        programa(IRMAO_LEI, irmao),
    );
    let resolvido = resolver_grafo(&graph).expect("resolver o grafo do caso");
    (resolvido, irmao)
}

fn span_de(source: SourceId) -> Span {
    Span::em(source, Position::new(1, 1), Position::new(1, 2))
}

/// `TRAIT_NAMEABILITY != RELATION_REACHABILITY`, no ponto de decisão.
///
/// A raiz importa `Marca` por nome e continua sem alcançar a relação declarada
/// pelo irmão que ela nunca pediu. O par positivo prova que o índice não está
/// recusando tudo.
#[test]
fn lei_nomear_o_trato_nao_alcanca_relacao_de_irmao_carregado() {
    let (graph, irmao) = grafo_com_irmao();
    let por_fonte = tratos_visiveis_por_fonte(&graph);
    assert!(!por_fonte.is_empty(), "o caso precisa ter composição");
    let raiz = graph.root().source_id;
    let importado = graph
        .module("m649_lei")
        .expect("o módulo importado está no grafo")
        .source_id;

    assert_eq!(
        nivel_de_despacho(&por_fonte, span_de(raiz), Some(raiz)),
        Some(NivelDeDespacho::Proprio),
        "relação da própria unidade"
    );
    assert_eq!(
        nivel_de_despacho(&por_fonte, span_de(raiz), Some(importado)),
        Some(NivelDeDespacho::PorUnidadeImportada),
        "relação da unidade importada (#577)"
    );
    assert_eq!(
        nivel_de_despacho(&por_fonte, span_de(raiz), Some(irmao)),
        None,
        "irmão carregado sem aresta autorizada não alcança, mesmo com o trato nomeável"
    );
}

/// `relacao_alcanca` é a MESMA pergunta sem o vocabulário da precedência.
/// Se as duas divergissem, uma superfície autorizaria o que a outra recusa.
#[test]
fn lei_o_predicado_booleano_concorda_com_o_nivel() {
    let (graph, irmao) = grafo_com_irmao();
    let por_fonte = tratos_visiveis_por_fonte(&graph);
    let raiz = graph.root().source_id;
    let importado = graph.module("m649_lei").expect("módulo no grafo").source_id;
    for fonte in [Some(raiz), Some(importado), Some(irmao), None] {
        assert_eq!(
            relacao_alcanca(&por_fonte, span_de(raiz), fonte),
            nivel_de_despacho(&por_fonte, span_de(raiz), fonte).is_some(),
            "divergência para fonte {fonte:?}"
        );
    }
}

/// Contexto semântico perdido não amplia alcance (contrato U-04/#645), e índice
/// vazio continua sendo ausência legítima de composição.
#[test]
fn lei_contexto_perdido_e_indice_vazio_permanecem_como_a_u04_os_deixou() {
    let (graph, _irmao) = grafo_com_irmao();
    let por_fonte = tratos_visiveis_por_fonte(&graph);
    let raiz = graph.root().source_id;
    assert!(
        !relacao_alcanca(&por_fonte, span_de(SourceId::UNKNOWN), Some(raiz)),
        "MISSING_SEMANTIC_CONTEXT -X-> EXPANDED_REACHABILITY"
    );
    let vazio = std::collections::HashMap::new();
    assert!(
        relacao_alcanca(&vazio, span_de(SourceId::UNKNOWN), None),
        "índice vazio é ausência de composição, não contexto perdido"
    );
}

// ---------------------------------------------------------------------------
// Guarda estrutural — POLICY_A não pode voltar por outro nome
// ---------------------------------------------------------------------------

/// A autoridade de alcance não tem como perguntar pelo nome do trato.
///
/// Não é estética: enquanto a estrutura que decide alcance guardar nomes de
/// trato, `POLICY_A` volta com uma linha. O oráculo é o código executável da
/// região cartografada, sem comentários.
#[test]
fn a_autoridade_de_alcance_nao_conhece_nome_de_trato() {
    let fonte =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/module_resolve.rs"))
            .expect("ler a autoridade de alcance");
    let inicio = fonte
        .find("// @pinker-nav:start modulos.visibilidade.tratos")
        .expect("região de alcance marcada");
    let fim = fonte
        .find("// @pinker-nav:end modulos.visibilidade.tratos")
        .expect("fim da região de alcance");
    let codigo = codigo_executavel(&fonte[inicio..fim]);
    for proibido in ["trait_name", "autorizados", "Item::Trait"] {
        assert!(
            !codigo.contains(proibido),
            "a decisão de alcance voltou a depender de `{proibido}`: \
             nomeabilidade do trato não pode autorizar relação (#579/POLICY_B)"
        );
    }
}
