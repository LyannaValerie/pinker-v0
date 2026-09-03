mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// @pinker-nav:start evidencia.modulos.closure-sintetica-de-default-importado
// @pinker-nav:domain modulos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental da #567: um corpo default de trato que contém closure compõe quando o trato é importado, e compõe significando o que a unidade DECLARANTE escreveu. A matriz positiva cobre o par de equivalência local/importado, captura de auxiliar e de local da origem, objeto de trato, override explícito e paridade interpretador/nativo; a adversarial fixa que o homônimo do importador não captura, que duas origens com closures estruturalmente idênticas não colidem, que o `impl` do override não carrega a dependência sintética do default omitido, e que a referência que só existe no importador continua falhando fechada. O oráculo é o valor observado — o auxiliar da origem e o homônimo do importador devolvem números diferentes — e a identidade sintética renderizada na IR, que carrega a proveniência da closure e a da materialização, nunca a grafia do trato.

/// Um caso é um conjunto de fontes; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, modulos: &[(&str, String)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #567");
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

fn ir(caso: &Caso, caso_logico: &str) -> String {
    let saida = pink(caso_logico, &["--ir"], &caso.raiz);
    assert_eq!(codigo(&saida), 0, "{}", stderr(&saida));
    stdout(&saida)
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

/// Símbolo do método de `impl`, sob o codec prefixado por comprimento: a
/// identidade do trato aparece inteira, e uma resolução que perdesse a origem
/// produziria `__impl_5_Marca_...`.
fn simbolo_de_metodo(trato_canonico: &str, alvo: &str, metodo: &str) -> String {
    format!(
        "__impl_{}_{}_{}_{}_{}",
        trato_canonico.len(),
        trato_canonico,
        alvo.len(),
        alvo,
        metodo
    )
}

/// Símbolo da checagem do corpo default vencido por override, mesmo codec.
fn simbolo_de_checagem(trato_canonico: &str, alvo: &str, metodo: &str) -> String {
    format!(
        "__trait_default_check_{}_{}_{}_{}_{}",
        trato_canonico.len(),
        trato_canonico,
        alvo.len(),
        alvo,
        metodo
    )
}

/// Um módulo que declara `Marca`, cujo default soma o resultado de uma closure
/// que chama o auxiliar do PRÓPRIO módulo.
///
/// `apoio_567` é o discriminante do oráculo: a origem devolve `retorno_do_apoio`
/// e um importador que capturasse o homônimo dele devolveria outro número.
fn modulo_com_closure(pacote: &str, trato: &str, metodo: &str, retorno_do_apoio: i32) -> String {
    format!(
        "pacote {pacote};\n\n\
         carinho apoio_567() -> bombom {{ mimo {retorno_do_apoio}; }}\n\n\
         trato {trato} {{\n    \
             carinho {metodo}(valor: si) -> bombom {{\n        \
                 nova base: bombom = 5;\n        \
                 nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom \
                     {{ mimo apoio_567() + base + v; }};\n        \
                 mimo f(2);\n    \
             }}\n\
         }}\n"
    )
}

/// A mesma decisão escrita inteira na raiz. O par local/importado só troca a
/// localização do trato, e é essa igualdade que a #567 fixa.
fn raiz_local(retorno_do_apoio: i32) -> String {
    format!(
        "pacote main;\n\n\
         carinho apoio_567() -> bombom {{ mimo {retorno_do_apoio}; }}\n\n\
         trato Marca {{\n    \
             carinho marcar(valor: si) -> bombom {{\n        \
                 nova base: bombom = 5;\n        \
                 nova f: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom \
                     {{ mimo apoio_567() + base + v; }};\n        \
                 mimo f(2);\n    \
             }}\n\
         }}\n\n\
         impl Marca para bombom {{}}\n\n\
         carinho principal() -> bombom {{\n    \
             nova x: bombom = 10;\n    \
             mimo x.marcar();\n\
         }}\n"
    )
}

// ---------------------------------------------------------------------------
// P1/P2 — o par de equivalência: a mesma decisão, local e importada
// ---------------------------------------------------------------------------

/// P1 — controle: o default com closure já funcionava quando trato e `impl`
/// moravam no mesmo arquivo. `3 + 5 + 2 = 10`.
#[test]
fn p1_default_local_com_closure_continua_valido() {
    let c = caso("p1_567", &raiz_local(3), &[]);
    let saida = executar(&c, "p1-567-local");
    assert_eq!(codigo(&saida), 10, "{}", stderr(&saida));
}

/// P2 — a MESMA decisão com o trato em outra unidade. Antes da #567 o parser
/// recusava aqui, porque o corpo default chegava sem a closure que ele cita.
#[test]
fn p2_default_de_trato_importado_com_closure_compoe() {
    let c = caso(
        "p2_567",
        "pacote main;\ntrazer m567p2.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p2",
            modulo_com_closure("m567p2", "Marca", "marcar", 3),
        )],
    );
    let saida = executar(&c, "p2-567-importado");
    assert_eq!(
        codigo(&saida),
        10,
        "o default importado tem de compor exatamente como o local: {}",
        stderr(&saida)
    );
}

/// A identidade canônica do trato continua sendo a autoridade do método
/// materializado, e a identidade sintética da closure carrega a proveniência da
/// unidade que a declarou — `m567p2` aparece inteiro nos bytes do nome.
#[test]
fn p2_identidade_canonica_e_proveniencia_da_closure_aparecem_na_ir() {
    let c = caso(
        "p2id_567",
        "pacote main;\ntrazer m567p2i.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p2i",
            modulo_com_closure("m567p2i", "Marca", "marcar", 3),
        )],
    );
    let texto = ir(&c, "p2id-567-ir");
    assert!(
        texto.contains(&simbolo_de_metodo("m567p2i.Marca", "bombom", "marcar")),
        "{texto}"
    );
    let closure = nome_da_closure_materializada(&texto);
    assert!(
        closure.contains(&hex_de("m567p2i")),
        "a identidade sintética tem de carregar a proveniência da unidade que declarou a closure: {closure}"
    );
}

/// Renderização hexadecimal de um texto, como o codec de identidade a escreve.
fn hex_de(texto: &str) -> String {
    use std::fmt::Write;
    texto.bytes().fold(String::new(), |mut saida, byte| {
        write!(saida, "{byte:02x}").expect("escrever hex em String");
        saida
    })
}

/// O único nome de closure sintética presente na IR.
fn nome_da_closure_materializada(ir: &str) -> String {
    let nomes = nomes_de_closure(ir);
    assert_eq!(nomes.len(), 1, "esperava uma closure sintética: {nomes:?}");
    nomes.into_iter().next().expect("uma closure")
}

/// Todos os nomes de closure sintética presentes na IR, sem repetição.
fn nomes_de_closure(ir: &str) -> Vec<String> {
    let mut nomes: Vec<String> = ir
        .split(|caractere: char| !caractere.is_ascii_alphanumeric() && caractere != '_')
        .filter(|palavra| palavra.starts_with("__anon_carinho_"))
        .map(ToOwned::to_owned)
        .collect();
    nomes.sort();
    nomes.dedup();
    nomes
}

// ---------------------------------------------------------------------------
// P3/P4 — o corpo copiado continua significando o que a origem escreveu
// ---------------------------------------------------------------------------

/// P3 + P4 — a closure do default importado cita `apoio_567` e uma local do
/// corpo default. A raiz declara um homônimo de `apoio_567` com outro valor.
///
/// Este é o oráculo que separa "compôs" de "compôs certo": `3 + 5 + 2 = 10` é a
/// origem; `900 + 5 + 2 = 907` seria a captura pelo importador — sem erro
/// nenhum, e com o programa executando o auxiliar errado.
#[test]
fn p3_p4_a_closure_resolve_contra_a_origem_e_o_homonimo_da_raiz_nao_captura() {
    let c = caso(
        "p34_567",
        "pacote main;\ntrazer m567p34.Marca;\n\ncarinho apoio_567() -> bombom { mimo 900; }\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p34",
            modulo_com_closure("m567p34", "Marca", "marcar", 3),
        )],
    );
    let saida = executar(&c, "p34-567-homonimo");
    assert_eq!(
        codigo(&saida),
        10,
        "a closure do default tem de alcançar m567p34.apoio_567 (3), nunca o homônimo da raiz (900): {}",
        stderr(&saida)
    );
}

/// O auxiliar que só a closure do default cita é materializado sob o nome
/// canônico do módulo de origem, e continua fora da superfície do importador.
#[test]
fn p3_o_auxiliar_alcancado_apenas_pela_closure_e_materializado_pela_origem() {
    let c = caso(
        "p3aux_567",
        "pacote main;\ntrazer m567p3a.Marca;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p3a",
            modulo_com_closure("m567p3a", "Marca", "marcar", 3),
        )],
    );
    let texto = ir(&c, "p3aux-567-ir");
    assert!(
        texto.contains("m567p3a.apoio_567"),
        "o auxiliar citado uma indireção abaixo do método tem de existir no programa: {texto}"
    );
    assert!(
        !texto.contains("func apoio_567"),
        "e não pode existir sob a grafia crua do importador: {texto}"
    );
}

/// A referência que só o IMPORTADOR poderia satisfazer continua falhando
/// fechada. A composição da #567 abriu o caminho da origem, não um caminho novo
/// em que o corpo default passa a enxergar quem o hospeda.
#[test]
fn n1_closure_do_default_nao_enxerga_a_superficie_do_importador() {
    let c = caso(
        "n1_567",
        "pacote main;\ntrazer m567n1.Marca;\n\ncarinho so_da_raiz() -> bombom { mimo 7; }\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567n1",
            "pacote m567n1;\n\ntrato Marca {\n    carinho marcar(valor: si) -> bombom {\n        nova f: carinho() -> bombom = carinho() -> bombom { mimo so_da_raiz(); };\n        mimo f();\n    }\n}\n".to_string(),
        )],
    );
    let saida = checar(&c, "n1-567-fail-closed");
    assert_eq!(
        codigo(&saida),
        1,
        "a closure não pode passar a resolver contra o importador: {}",
        stderr(&saida)
    );
    assert!(
        stderr(&saida).contains("so_da_raiz"),
        "o diagnóstico tem de nomear a referência que não existe na origem: {}",
        stderr(&saida)
    );
}

// ---------------------------------------------------------------------------
// P5 — duas origens, closures estruturalmente idênticas, nenhuma colisão
// ---------------------------------------------------------------------------

/// P5 — `ma` e `mb` declaram closures de default com a MESMA forma, o mesmo
/// nome de auxiliar e o mesmo índice local do parser. As duas são materializadas
/// no mesmo importador, para o mesmo alvo.
///
/// Uma identidade que perdesse a origem faria as duas ocuparem o mesmo nome, e a
/// captura resolvida na primeira valeria para a segunda: `3+5+2` mais `40+5+2` é
/// `57`; uma colisão devolveria `20` ou `94`.
#[test]
fn p5_duas_origens_com_closures_estruturalmente_identicas_nao_colidem() {
    let c = caso(
        "p5_567",
        "pacote main;\ntrazer m567a.MarcaA;\ntrazer m567b.MarcaB;\n\ncarinho apoio_567() -> bombom { mimo 900; }\n\nimpl MarcaA para bombom {}\n\nimpl MarcaB para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar_a() + x.marcar_b();\n}\n",
        &[
            ("m567a", modulo_com_closure("m567a", "MarcaA", "marcar_a", 3)),
            (
                "m567b",
                modulo_com_closure("m567b", "MarcaB", "marcar_b", 40),
            ),
        ],
    );
    let saida = executar(&c, "p5-567-duas-origens");
    assert_eq!(
        codigo(&saida),
        57,
        "cada default tem de alcançar o auxiliar da SUA origem: {}",
        stderr(&saida)
    );
}

/// E as duas identidades sintéticas são distintas, cada uma carregando a
/// proveniência da unidade que declarou a closure.
#[test]
fn p5_as_identidades_sinteticas_das_duas_origens_sao_distintas() {
    let c = caso(
        "p5id_567",
        "pacote main;\ntrazer m567ai.MarcaA;\ntrazer m567bi.MarcaB;\n\nimpl MarcaA para bombom {}\n\nimpl MarcaB para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar_a() + x.marcar_b();\n}\n",
        &[
            (
                "m567ai",
                modulo_com_closure("m567ai", "MarcaA", "marcar_a", 3),
            ),
            (
                "m567bi",
                modulo_com_closure("m567bi", "MarcaB", "marcar_b", 40),
            ),
        ],
    );
    let texto = ir(&c, "p5id-567-ir");
    let nomes = nomes_de_closure(&texto);
    assert_eq!(
        nomes.len(),
        2,
        "duas materializações, dois nomes: {nomes:?}"
    );
    assert!(
        nomes.iter().any(|nome| nome.contains(&hex_de("m567ai"))),
        "{nomes:?}"
    );
    assert!(
        nomes.iter().any(|nome| nome.contains(&hex_de("m567bi"))),
        "{nomes:?}"
    );
}

// ---------------------------------------------------------------------------
// P6/P7 — override explícito vence, e não carrega o default omitido
// ---------------------------------------------------------------------------

/// P6 — o override escrito no `impl` vence a seleção. `10 + 1 = 11`, nunca o
/// `10` do default.
#[test]
fn p6_override_explicito_vence_o_default_com_closure() {
    let c = caso(
        "p6_567",
        "pacote main;\ntrazer m567p6.Marca;\n\ncarinho apoio_567() -> bombom { mimo 900; }\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p6",
            modulo_com_closure("m567p6", "Marca", "marcar", 3),
        )],
    );
    let saida = executar(&c, "p6-567-override");
    assert_eq!(codigo(&saida), 11, "{}", stderr(&saida));
}

/// P7 — a dependência sintética acompanha apenas o corpo que a cita.
///
/// Com override, o corpo default continua devido à checagem — é o contrato da
/// #566 —, e é a checagem que carrega a closure. O método do `impl`, que é o que
/// o despacho alcança, não recebe dependência nenhuma do default omitido.
#[test]
fn p7_o_metodo_do_override_nao_carrega_a_dependencia_do_default_omitido() {
    let c = caso(
        "p7_567",
        "pacote main;\ntrazer m567p7.Marca;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p7",
            modulo_com_closure("m567p7", "Marca", "marcar", 3),
        )],
    );
    let texto = ir(&c, "p7-567-ir");
    let closure = nome_da_closure_materializada(&texto);
    let metodo = corpo_de_funcao(
        &texto,
        &simbolo_de_metodo("m567p7.Marca", "bombom", "marcar"),
    );
    assert!(
        !metodo.contains(&closure),
        "o corpo do override não pode citar a closure do default omitido: {metodo}"
    );
    let checagem = corpo_de_funcao(
        &texto,
        &simbolo_de_checagem("m567p7.Marca", "bombom", "marcar"),
    );
    assert!(
        checagem.contains(&closure),
        "a checagem do default é quem a cita, e continua devida: {checagem}"
    );
}

/// Corpo de uma função na IR, do cabeçalho dela até o próximo cabeçalho.
fn corpo_de_funcao(ir: &str, simbolo: &str) -> String {
    let cabecalho = format!("func {simbolo}");
    let inicio = ir
        .find(&cabecalho)
        .unwrap_or_else(|| panic!("função '{simbolo}' ausente da IR:\n{ir}"));
    let resto = &ir[inicio + cabecalho.len()..];
    match resto.find("\n  func ") {
        Some(fim) => resto[..fim].to_string(),
        None => resto.to_string(),
    }
}

// ---------------------------------------------------------------------------
// P8 — objeto de trato
// ---------------------------------------------------------------------------

/// P8 — o despacho dinâmico alcança o default importado com closure, e continua
/// resolvendo contra a origem. `(3 + 5 + 2)` pelo default mais `64` pelo
/// override é `74`; captura do homônimo da raiz daria `971`.
#[test]
fn p8_objeto_de_trato_alcanca_o_default_importado_com_closure() {
    let c = caso(
        "p8_567",
        "pacote main;\ntrazer m567p8.Medivel;\n\ncarinho apoio_567() -> bombom { mimo 900; }\n\nimpl Medivel para bombom {}\n\nimpl Medivel para u64 {\n    carinho medir(valor: u64) -> bombom { mimo 64; }\n}\n\ncarinho principal() -> bombom {\n    nova a: bombom = 20;\n    nova b: u64 = 5;\n    nova oa: trato<Medivel> = a virar trato<Medivel>;\n    nova ob: trato<Medivel> = b virar trato<Medivel>;\n    mimo oa.medir() + ob.medir();\n}\n",
        &[(
            "m567p8",
            modulo_com_closure("m567p8", "Medivel", "medir", 3),
        )],
    );
    let saida = executar(&c, "p8-567-objeto");
    assert_eq!(codigo(&saida), 74, "{}", stderr(&saida));
}

// ---------------------------------------------------------------------------
// P9/P10/P11 — as autoridades vizinhas continuam decidindo o que decidiam
// ---------------------------------------------------------------------------

/// P9 (#517) — o import explícito continua sendo a autoridade sobre `impl`. Sem
/// ele, o default com closure não vira porta de entrada para o trato.
#[test]
fn p9_517_sem_import_o_trato_com_default_de_closure_nao_e_alvo_de_impl() {
    let c = caso(
        "p9_567",
        "pacote main;\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p9",
            modulo_com_closure("m567p9", "Marca", "marcar", 3),
        )],
    );
    let saida = checar(&c, "p9-567-sem-import");
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
    assert!(
        stderr(&saida).contains("não declarado antes deste ponto nem trazido por import"),
        "{}",
        stderr(&saida)
    );
}

/// P10 (#566) — dois tratos HOMÔNIMOS de unidades distintas, os dois com default
/// de closure, recebem checagens endereçadas pela identidade canônica de cada
/// um. Uma identidade por grafia faria as duas colidirem em `Marca`.
#[test]
fn p10_566_tratos_homonimos_com_default_de_closure_recebem_checagens_distintas() {
    let c = caso(
        "p10_567",
        "pacote main;\ntrazer m567ha.Marca;\ntrazer m567hb.enfeite_hb;\n\nimpl Marca para bombom {\n    carinho marcar(valor: bombom) -> bombom { mimo valor + 1; }\n}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar() + enfeite_hb();\n}\n",
        &[
            ("m567ha", modulo_com_closure("m567ha", "Marca", "marcar", 3)),
            (
                "m567hb",
                format!(
                    "{}\ncarinho enfeite_hb() -> bombom {{ mimo 0; }}\n",
                    modulo_com_closure("m567hb", "Marca", "marcar", 40)
                ),
            ),
        ],
    );
    let texto = ir(&c, "p10-567-ir");
    assert!(
        texto.contains(&simbolo_de_checagem("m567ha.Marca", "bombom", "marcar")),
        "{texto}"
    );
    assert!(
        !texto.contains("__trait_default_check_5_Marca_"),
        "a checagem nunca pode ser endereçada pela grafia: {texto}"
    );
}

/// P11 (#572) — a relação de `impl` duplicada continua sendo recusada pela
/// autoridade de contratos de trato, e não por choque entre nomes sintéticos das
/// closures que os dois blocos materializariam.
#[test]
fn p11_572_relacao_duplicada_e_recusada_pela_autoridade_de_contratos() {
    let c = caso(
        "p11_567",
        "pacote main;\ntrazer m567p11.Marca;\n\nimpl Marca para bombom {}\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    mimo x.marcar();\n}\n",
        &[(
            "m567p11",
            modulo_com_closure("m567p11", "Marca", "marcar", 3),
        )],
    );
    let saida = checar(&c, "p11-567-duplicada");
    assert_eq!(codigo(&saida), 1, "{}", stderr(&saida));
    let erro = stderr(&saida);
    assert!(
        erro.contains("impl") && erro.contains("m567p11.Marca"),
        "a recusa tem de vir da relação, com a identidade canônica: {erro}"
    );
    assert!(
        !erro.contains("__anon_carinho_"),
        "e nunca de um choque de nome sintético: {erro}"
    );
}

// ---------------------------------------------------------------------------
// Paridade interpretador/nativo
// ---------------------------------------------------------------------------

/// O mesmo programa observa o mesmo resultado no interpretador e no ELF nativo.
#[test]
fn paridade_interpretador_e_nativo_do_default_importado_com_closure() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence("issue-567-paridade", true)
    else {
        return;
    };
    let c = caso(
        "paridade_567",
        "pacote main;\ntrazer m567par.Marca;\n\ncarinho apoio_567() -> bombom { mimo 900; }\n\nimpl Marca para bombom {}\n\ncarinho principal() -> bombom {\n    nova x: bombom = 10;\n    falar(x.marcar());\n    mimo 0;\n}\n",
        &[(
            "m567par",
            modulo_com_closure("m567par", "Marca", "marcar", 3),
        )],
    );
    let interpretado = executar(&c, "567-paridade-interpretador");
    assert_eq!(codigo(&interpretado), 0, "{}", stderr(&interpretado));
    assert_eq!(stdout(&interpretado), "10\n");

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("567-paridade-build")
        .timeout(Duration::from_secs(120))
        .output()
        .expect("build nativo #567");
    assert!(
        build.status.success(),
        "build stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join("paridade_567"))
        .logical_case("567-paridade-nativo")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar nativo #567");
    assert!(nativo.status.success(), "{nativo:?}");
    assert_eq!(interpretado.stdout, nativo.stdout);
}
// @pinker-nav:end evidencia.modulos.closure-sintetica-de-default-importado
