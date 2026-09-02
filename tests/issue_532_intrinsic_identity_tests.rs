//! #532 — identidade intrínseca separada da grafia, e namespace de módulos
//! explícito.
//!
//! ```text
//! SYMBOL_NAME        != SYMBOL_IDENTITY
//! TEXTUAL_SPELLING   != INTRINSIC_IDENTITY
//! CALL_IS_INTRINSIC  <- RESOLVED_IDENTITY, NOT SPELLING
//! ```

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::{Expr, ExprKind, Item, Stmt};
use pinker_v0::familia_superficie;
use pinker_v0::intrinsic_authority;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Apoio
// ---------------------------------------------------------------------------

/// Um caso é um conjunto de fontes vizinhas; a primeira é a raiz.
struct Caso {
    dir: NativeArtifactDir,
    raiz: PathBuf,
}

fn caso(nome: &str, raiz: &str, vizinhos: &[(&str, &str)]) -> Caso {
    let dir = NativeArtifactDir::create().expect("diretório do caso #532");
    for (modulo, fonte) in vizinhos {
        escrever(dir.path(), modulo, fonte);
    }
    let raiz = escrever(dir.path(), nome, raiz);
    Caso { dir, raiz }
}

fn escrever(dir: &Path, nome: &str, fonte: &str) -> PathBuf {
    let caminho = dir.join(format!("{nome}.pink"));
    fs::write(&caminho, fonte).unwrap_or_else(|erro| panic!("gravar {nome}: {erro}"));
    caminho
}

fn pink(caso_logico: &str, args: &[&str], raiz: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(args)
        .arg(raiz)
        .logical_case(caso_logico)
        .timeout(Duration::from_secs(120))
        .output()
        .expect("invocar pink #532")
}

fn executar(c: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--run"], &c.raiz)
}

fn checar(c: &Caso, caso_logico: &str) -> std::process::Output {
    pink(caso_logico, &["--check"], &c.raiz)
}

fn codigo(saida: &std::process::Output) -> i32 {
    saida.status.code().expect("status com código")
}

fn erro(saida: &std::process::Output) -> String {
    String::from_utf8_lossy(&saida.stderr).into_owned()
}

/// Roda a mesma fonte no interpretador e no ELF nativo e exige o mesmo
/// observável. Devolve `None` quando não há evidência nativa disponível — o
/// gate `PINKER_EXIGE_NATIVO=1` converte a ausência em falha na própria
/// autoridade de capacidade.
fn paridade(c: &Caso, nome: &str, caso_logico: &str) -> Option<(i32, String)> {
    let (_driver, runtime_lib) = common::require_native_evidence(caso_logico, true)?;
    let runtime_lib = runtime_lib?;
    let interpretado = executar(c, &format!("{caso_logico}-interpretado"));

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(c.dir.path())
        .arg(&c.raiz)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case(&format!("{caso_logico}-build"))
        .timeout(Duration::from_secs(180))
        .output()
        .expect("build nativo #532");
    assert!(
        build.status.success(),
        "{caso_logico}: build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nativo = Command::new(c.dir.path().join(nome))
        .logical_case(&format!("{caso_logico}-nativo"))
        .timeout(Duration::from_secs(60))
        .output()
        .expect("executar ELF #532");

    assert_eq!(
        interpretado.status.code(),
        nativo.status.code(),
        "{caso_logico}: código de saída divergiu entre interpretador e nativo"
    );
    assert_eq!(
        interpretado.stdout, nativo.stdout,
        "{caso_logico}: stdout divergiu entre interpretador e nativo"
    );
    Some((
        codigo(&interpretado),
        String::from_utf8_lossy(&interpretado.stdout).into_owned(),
    ))
}

/// Atravessa o pipeline inteiro em processo — parser, semantic, IR e os quatro
/// validadores, até a execução da máquina — e devolve o valor de `principal`.
///
/// É o oráculo que o censo de reservas textuais precisa: parar na semântica
/// provaria a ausência da reserva na única camada onde ela já não existia.
fn valor_pelo_pipeline_completo(fonte: &str) -> Result<Option<i64>, String> {
    use pinker_v0::interpreter::RuntimeValue;
    let programa = common::parse(fonte).map_err(|erro| erro.to_string())?;
    pinker_v0::semantic::check_program(&programa).map_err(|erro| erro.to_string())?;
    let ir = pinker_v0::ir::lower_program(&programa).map_err(|erro| erro.to_string())?;
    pinker_v0::ir_validate::validate_program(&ir).map_err(|erro| erro.to_string())?;
    let cfg = pinker_v0::cfg_ir::lower_program(&ir).map_err(|erro| erro.to_string())?;
    pinker_v0::cfg_ir_validate::validate_program(&cfg).map_err(|erro| erro.to_string())?;
    let selecionado = pinker_v0::instr_select::lower_program(&cfg).map_err(|e| e.to_string())?;
    pinker_v0::instr_select_validate::validate_program(&selecionado)
        .map_err(|erro| erro.to_string())?;
    let maquina =
        pinker_v0::abstract_machine::lower_program(&selecionado).map_err(|e| e.to_string())?;
    pinker_v0::abstract_machine_validate::validate_program(&maquina)
        .map_err(|erro| erro.to_string())?;
    match pinker_v0::interpreter::run_program(&maquina).map_err(|erro| erro.to_string())? {
        Some(RuntimeValue::Int(valor)) => Ok(Some(valor as i64)),
        outro => Err(format!("valor inesperado: {outro:?}")),
    }
}

/// Callee do primeiro `mimo` de `principal`, na forma em que o parser o
/// entregou — `Ident` para função do usuário, `Intrinsic` para identidade
/// resolvida.
fn callee_de_principal(fonte: &str) -> ExprKind {
    let programa = common::parse(fonte).unwrap_or_else(|erro| panic!("parse: {erro}\n{fonte}"));
    let Some(Item::Function(funcao)) = programa
        .items
        .iter()
        .find(|item| matches!(item, Item::Function(f) if f.name == "principal"))
    else {
        panic!("função principal ausente");
    };
    for stmt in &funcao.body.stmts {
        let expr: &Expr = match stmt {
            Stmt::Return(retorno) => match retorno.expr.as_ref() {
                Some(expr) => expr,
                None => continue,
            },
            Stmt::Expr(expr) => expr,
            _ => continue,
        };
        if let ExprKind::Call(callee, _) = &expr.kind {
            return callee.kind.clone();
        }
    }
    panic!("nenhuma chamada em principal");
}

// @pinker-nav:start evidencia.identidade.intrinseca-vs-grafia
// @pinker-nav:domain identificadores
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental do contrato central da #532: a decisão "esta chamada é intrínseca" vem de `CalleeIdentity`, produzida só pela canonicalização de um `trazer`, e não da grafia textual. Cobre a matriz I1 — função do usuário com antiga grafia canônica declarada e chamada, referência modular à mesma grafia alcançando a intrínseca, coexistência das duas no mesmo programa, acordo interpretador × nativo — e o censo de reservas textuais, que passou a ser zero. Os oráculos são observáveis semânticos (valor devolvido, saída, forma do callee na AST), nunca "o programa compila".

// ---------------------------------------------------------------------------
// I1 — grafia canônica deixou de ser identidade
// ---------------------------------------------------------------------------

/// I1-P1 e I1-P2: a declaração é aceita e a chamada não qualificada é dela.
#[test]
fn i1_p1_p2_funcao_do_usuario_usa_antiga_grafia_canonica_e_vence_a_chamada() {
    for grafia in ["tamanho_verso", "ler_arquivo", "mapa_verso_verso_criar"] {
        let c = caso(
            "i1_p1",
            &format!(
                "pacote main;\ncarinho {grafia}(valor: bombom) -> bombom {{ mimo valor + 1; }}\ncarinho principal() -> bombom {{ mimo {grafia}(41); }}\n"
            ),
            &[],
        );
        let saida = executar(&c, "issue-532-i1-p1");
        assert_eq!(codigo(&saida), 42, "{grafia}: {}", erro(&saida));
    }
}

/// I1-P3: a referência modular continua alcançando a intrínseca — e o callee
/// que o parser entrega para ela NÃO é um identificador do namespace do
/// usuário.
#[test]
fn i1_p3_referencia_modular_continua_alcancando_a_intrinseca() {
    let seletiva = "pacote main;\ntrazer texto.tamanho;\ncarinho principal() -> bombom { mimo tamanho(\"abc\"); }\n";
    let qualificada = "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo texto.tamanho(\"abc\"); }\n";

    for fonte in [seletiva, qualificada] {
        match callee_de_principal(fonte) {
            ExprKind::Intrinsic(identidade) => assert_eq!(
                identidade.canonical_public_spelling(),
                "tamanho_verso",
                "a identidade resolvida precisa ser a da operação intrínseca correta"
            ),
            outro => panic!("callee modular deveria ser identidade: {outro:?}"),
        }
        let c = caso("i1_p3", fonte, &[]);
        let saida = executar(&c, "issue-532-i1-p3");
        assert_eq!(codigo(&saida), 3, "{}", erro(&saida));
    }
}

/// I1-P4: as duas convivem no mesmo programa, com valores distintos.
///
/// O oráculo separa as duas implementações: a do usuário soma 1 ao `bombom`, a
/// intrínseca mede o `verso`. Um despacho por grafia devolveria o mesmo valor
/// para as duas chamadas ou recusaria o programa.
#[test]
fn i1_p4_usuario_e_intrinseca_coexistem_com_observaveis_distintos() {
    let fonte = "pacote main;\n\
                 trazer texto.tamanho;\n\
                 carinho tamanho_verso(valor: bombom) -> bombom { mimo valor + 1; }\n\
                 carinho principal() -> bombom {\n\
                 \x20   nova a: bombom = tamanho_verso(40);\n\
                 \x20   nova b: bombom = tamanho(\"ab\");\n\
                 \x20   mimo a + b;\n\
                 }\n";
    let c = caso("i1_p4", fonte, &[]);
    let saida = executar(&c, "issue-532-i1-p4");
    assert_eq!(codigo(&saida), 43, "{}", erro(&saida));
}

/// I1-P5' — censo NATIVO das grafias que têm ramo textual próprio no backend.
///
/// O censo do pipeline em processo para na máquina abstrata e não veria um
/// desvio por grafia no emissor SysV. As grafias abaixo são exatamente as que
/// `backend_s` trata fora da rota comum: `formatar_verso`, pelo pack de
/// substituições, e as de despacho por aridade. Uma delas escapando do portão
/// de identidade é miscompilação silenciosa — o interpretador acerta e o ELF
/// erra —, e é isso que este censo mede.
#[test]
fn i1_p5_censo_nativo_das_grafias_com_ramo_proprio_no_backend() {
    const COM_RAMO_PROPRIO: [&str; 8] = [
        "formatar_verso",
        "afirmar",
        "executar_processo",
        "capturar_stdout",
        "capturar_stderr",
        "executar_com_entrada",
        // Controles de rota comum, para o censo não medir só o caminho especial.
        "tamanho_verso",
        "ler_arquivo",
    ];
    for grafia in COM_RAMO_PROPRIO {
        let fonte = format!(
            "pacote main;\ncarinho {grafia}(valor: bombom) -> bombom {{ mimo valor + 1; }}\ncarinho principal() -> bombom {{ mimo {grafia}(41); }}\n"
        );
        let c = caso("censo_nativo", &fonte, &[]);
        let Some((codigo_saida, _)) = paridade(&c, "censo_nativo", "issue-532-censo-nativo") else {
            return;
        };
        assert_eq!(codigo_saida, 42, "{grafia}: a função do usuário não venceu");
    }
}

/// I1-P5: interpretador e nativo concordam no caso de homônimo.
#[test]
fn i1_p5_interpretador_e_nativo_concordam_no_homonimo() {
    let fonte = "pacote main;\n\
                 trazer texto.tamanho;\n\
                 carinho tamanho_verso(valor: bombom) -> bombom { mimo valor + 1; }\n\
                 carinho principal() -> bombom {\n\
                 \x20   falar(tamanho_verso(40));\n\
                 \x20   falar(tamanho(\"ab\"));\n\
                 \x20   mimo 0;\n\
                 }\n";
    let c = caso("i1_p5", fonte, &[]);
    let Some((codigo_saida, stdout)) = paridade(&c, "i1_p5", "issue-532-i1-p5") else {
        return;
    };
    assert_eq!(codigo_saida, 0);
    assert_eq!(stdout, "41\n2\n");
}

/// Censo das reservas textuais: era a superfície canônica inteira, é zero.
///
/// A conta não é sobre keywords nem sobre gramática: percorre a autoridade de
/// intrínsecas e verifica que nenhuma das grafias canônicas continua proibida
/// como nome de função do usuário.
#[test]
fn censo_de_reservas_textuais_para_funcao_do_usuario_e_zero() {
    let grafias = intrinsic_authority::all_canonical_intrinsic_spellings();
    assert!(
        grafias.len() >= 155,
        "a autoridade encolheu inesperadamente: {}",
        grafias.len()
    );
    // O oráculo atravessa TODAS as camadas e observa o valor devolvido: uma
    // grafia ainda reservada — ou ainda capturada — em qualquer uma delas
    // aparece aqui como recusa ou como valor errado, não como "compila".
    let mut reservadas: Vec<(&str, String)> = Vec::new();
    for entrada in &grafias {
        let fonte = format!(
            "pacote main;\ncarinho {}(valor: bombom) -> bombom {{ mimo valor + 1; }}\ncarinho principal() -> bombom {{ mimo {}(41); }}\n",
            entrada.spelling, entrada.spelling
        );
        match valor_pelo_pipeline_completo(&fonte) {
            Ok(Some(42)) => {}
            Ok(outro) => reservadas.push((entrada.spelling, format!("valor {outro:?}"))),
            Err(erro) => reservadas.push((entrada.spelling, erro)),
        }
    }
    assert_eq!(
        reservadas,
        Vec::<(&str, String)>::new(),
        "grafias canônicas ainda reservadas ou capturadas para o usuário"
    );
}
// @pinker-nav:end evidencia.identidade.intrinseca-vs-grafia

// @pinker-nav:start evidencia.identidade.namespace-de-modulos
// @pinker-nav:domain importacoes
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova comportamental do namespace de módulos da #532: a regra única `EXISTING_LEXICAL_IDENTITY > MODULE_NAME` vale igual para família built-in e módulo real e não depende da ordem do texto (I2); a criação genérica de mapa passou a ser `mapa.criar`, fechando a última grafia builtin chamável sem import (I3); e `REAL_MODULE_X > BUILTIN_FAMILY_X` passou a valer nas DUAS formas de `trazer`, preservando G-517-1 quando não existe arquivo homônimo (I4). Inclui a matriz negativa N1..N8 e o censo estrutural dos quinze módulos, que recusa qualquer módulo tratado como caso especial.

// ---------------------------------------------------------------------------
// I2 — nome de módulo, ligação de valor e nome textual
// ---------------------------------------------------------------------------

/// I2-P1: a superfície modular built-in funciona nas duas formas.
#[test]
fn i2_p1_modulo_e_membro_builtin_funcionam() {
    let c = caso(
        "i2_p1",
        "pacote main;\ntrazer texto;\ntrazer lista.criar;\ncarinho principal() -> bombom { nova l: lista<bombom> = criar(); mimo texto.tamanho(\"abc\"); }\n",
        &[],
    );
    let saida = executar(&c, "issue-532-i2-p1");
    assert_eq!(codigo(&saida), 3, "{}", erro(&saida));
}

/// I2-P2, I2-P3 e I2-P4: a regra é uma só, vale para as duas espécies de
/// módulo e não depende da ordem do texto.
///
/// ```text
/// EXISTING_LEXICAL_IDENTITY > MODULE_NAME_IN_QUALIFIED_POSITION
/// SELECTIVE_MEMBER_IN_CALLABLE_NAMESPACE -> COLISÃO DIAGNOSTICADA
/// ```
///
/// A ligação homônima vence o nome do módulo porque `texto.x` também é acesso
/// a campo: a posição sintática NÃO separa os dois namespaces, então quem já
/// declarou o nome fica com ele. A forma seletiva é o caso oposto — ela ocupa o
/// namespace callable do arquivo, e aí a colisão é real e recusada.
#[test]
fn i2_p2_p3_p4_ligacao_homonima_vence_o_nome_do_modulo_em_qualquer_ordem() {
    const MODULO_REAL: &str = "pacote texto;\ncarinho tamanho(x: bombom) -> bombom { mimo 900; }\n";

    // Família built-in: `eterno texto` vence o nome do módulo, e o programa
    // observa o VALOR, não a família.
    for (rotulo, fonte) in [
        (
            "declaracao-antes",
            "pacote main;\ntrazer texto;\neterno texto: bombom = 5;\ncarinho principal() -> bombom { mimo texto; }\n",
        ),
        (
            "declaracao-depois",
            "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo texto; }\neterno texto: bombom = 5;\n",
        ),
    ] {
        let c = caso("i2_p2_familia", fonte, &[]);
        let saida = executar(&c, "issue-532-i2-p2-familia");
        assert_eq!(codigo(&saida), 5, "{rotulo}: {}", erro(&saida));
    }

    // Módulo real com o mesmo nome: mesma regra, mesmo resultado.
    for (rotulo, fonte) in [
        (
            "declaracao-antes",
            "pacote main;\ntrazer texto;\neterno texto: bombom = 5;\ncarinho principal() -> bombom { mimo texto; }\n",
        ),
        (
            "declaracao-depois",
            "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo texto; }\neterno texto: bombom = 5;\n",
        ),
    ] {
        let c = caso("i2_p2_modulo", fonte, &[("texto", MODULO_REAL)]);
        let saida = executar(&c, "issue-532-i2-p2-modulo");
        assert_eq!(codigo(&saida), 5, "{rotulo}: {}", erro(&saida));
    }

    // Posição QUALIFICADA: é aqui que a regra decide de verdade, porque
    // `texto.x` também é acesso a campo/método. A ligação visível vence o nome
    // do módulo, e o oráculo separa as duas leituras: o método do usuário
    // devolve 90, a família devolveria a intrínseca de `verso`.
    const QUALIFICADA: &str = "pacote main;\ntrazer texto;\ntrato Medida { carinho tamanho(valor: si) -> bombom; }\nimpl Medida para bombom {\n    carinho tamanho(valor: bombom) -> bombom { mimo 90; }\n}\ncarinho principal() -> bombom {\n    nova texto: bombom = 1;\n    mimo texto.tamanho();\n}\n";
    for (rotulo, vizinhos) in [
        ("familia", &[][..]),
        ("modulo-real", &[("texto", MODULO_REAL)][..]),
    ] {
        let c = caso("i2_p2_qualificada", QUALIFICADA, vizinhos);
        let saida = executar(&c, "issue-532-i2-p2-qualificada");
        assert_eq!(
            codigo(&saida),
            90,
            "{rotulo}: o nome do módulo venceu a ligação visível: {}",
            erro(&saida)
        );
    }

    // Forma seletiva: colisão real, recusada, e o veredito também não depende
    // da ordem.
    for (rotulo, fonte) in [
        (
            "import-antes",
            "pacote main;\ntrazer texto.tamanho;\ncarinho tamanho(x: verso) -> bombom { mimo 7; }\ncarinho principal() -> bombom { mimo 0; }\n",
        ),
        (
            "declaracao-antes",
            "pacote main;\ntrazer texto.tamanho;\ncarinho principal() -> bombom { mimo 0; }\ncarinho tamanho(x: verso) -> bombom { mimo 7; }\n",
        ),
    ] {
        let c = caso("i2_p3", fonte, &[]);
        let saida = checar(&c, "issue-532-i2-p3");
        assert_eq!(codigo(&saida), 1, "{rotulo} foi aceito");
        assert!(erro(&saida).contains("tamanho"), "{}", erro(&saida));
    }
}

/// I4/NB — o diagnóstico não manda repetir o `trazer` que já foi escrito.
///
/// Com `REAL_MODULE_X > BUILTIN_FAMILY_X` valendo na forma inteira, o `trazer
/// texto;` de um programa com `texto.pink` real é consumido como import de
/// MÓDULO. O nome chega à semântica sem identidade, e a dica histórica de
/// família mandaria escrever exatamente a linha que já está no arquivo. A dica
/// passou a nomear a leitura correta; a histórica continua intacta quando não
/// existe módulo real.
#[test]
fn i4_dica_de_familia_nao_manda_repetir_o_trazer_ja_escrito() {
    let com_modulo = caso(
        "i4_dica",
        "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo texto.tamanho(1); }\n",
        &[("texto", TEXTO_REAL)],
    );
    let saida = checar(&com_modulo, "issue-532-i4-dica");
    let mensagem = erro(&saida);
    assert_eq!(codigo(&saida), 1);
    assert!(
        mensagem.contains("é um módulo Pinker, não uma família built-in"),
        "{mensagem}"
    );
    assert!(
        !mensagem.contains("escreva 'trazer texto;'"),
        "a dica mandou repetir o import que o arquivo já tem: {mensagem}"
    );

    let sem_modulo = caso(
        "i4_dica_controle",
        "pacote main;\ncarinho principal() -> bombom { mimo texto.tamanho(\"abc\"); }\n",
        &[],
    );
    let saida = checar(&sem_modulo, "issue-532-i4-dica-controle");
    assert_eq!(codigo(&saida), 1);
    assert!(
        erro(&saida).contains("não foi importada neste arquivo"),
        "a dica histórica de família se perdeu: {}",
        erro(&saida)
    );
}

/// I2-P5: o diagnóstico da colisão aponta o span da fonte do usuário, e não
/// uma identidade sintética.
#[test]
fn i2_p5_diagnostico_usa_o_span_da_fonte_e_nao_vaza_identidade_interna() {
    let fonte = "pacote main;\ntrazer texto.tamanho;\ncarinho tamanho(x: verso) -> bombom { mimo 7; }\ncarinho principal() -> bombom { mimo 0; }\n";
    let c = caso("i2_p5", fonte, &[]);
    let saida = checar(&c, "issue-532-i2-p5");
    let mensagem = erro(&saida);
    assert!(mensagem.contains("tamanho"), "{mensagem}");
    assert!(
        mensagem.contains("2:1"),
        "span do import ausente: {mensagem}"
    );
    for vazamento in [
        "Intrinsic(",
        "CalleeIdentity",
        "Historical(",
        "pinker_verso_",
    ] {
        assert!(
            !mensagem.contains(vazamento),
            "diagnóstico vazou '{vazamento}': {mensagem}"
        );
    }
}

// ---------------------------------------------------------------------------
// I3 — criação genérica de mapa
// ---------------------------------------------------------------------------

/// I3: `mapa.criar` é membro público, e a grafia canônica não é mais chamável
/// sem import.
///
/// A tradução `<familia>_<membro> -> familia.membro` não é escolha nova: é a
/// mesma que a #505 aplicou aos outros 29 membros de `mapa` e ao `lista_criar`
/// de mesma natureza. Com ela, `GLOBAL_CALLABLE_BUILTIN_EXCEPTIONS = 0`.
#[test]
fn i3_mapa_criar_virou_membro_publico_e_deixou_de_ser_chamavel_sem_import() {
    assert_eq!(
        intrinsic_authority::public_intrinsic_member("mapa", "criar").map(|m| m.identity),
        intrinsic_authority::intrinsic_from_public_spelling("mapa_criar"),
        "mapa.criar precisa endereçar a mesma identidade que a grafia canônica"
    );

    // Simetria com o gêmeo estrutural.
    assert!(intrinsic_authority::public_intrinsic_member("lista", "criar").is_some());

    let sem_import = caso(
        "i3_sem_import",
        "pacote main;\ncarinho principal() -> bombom { nova m: mapa<verso,bombom> = mapa_criar(); mimo 0; }\n",
        &[],
    );
    let saida = checar(&sem_import, "issue-532-i3-sem-import");
    assert_eq!(codigo(&saida), 1, "grafia canônica seguiu chamável a seco");
    assert!(erro(&saida).contains("mapa.criar"), "{}", erro(&saida));
}

/// I3: a semântica de criação genérica não mudou — as duas formas de import
/// produzem o mesmo mapa, e a operação continua sendo decidida pelo tipo
/// anotado.
#[test]
fn i3_criacao_generica_preserva_a_semantica_nas_duas_formas_de_import() {
    for (rotulo, fonte) in [
        (
            "seletiva",
            "pacote main;\ntrazer mapa.criar, definir, obter;\ncarinho principal() -> bombom {\n    nova m: mapa<verso,bombom> = criar();\n    definir(m, \"k\", 42);\n    mimo obter(m, \"k\");\n}\n",
        ),
        (
            "qualificada",
            "pacote main;\ntrazer mapa;\ncarinho principal() -> bombom {\n    nova m: mapa<verso,bombom> = mapa.criar();\n    mapa.definir(m, \"k\", 42);\n    mimo mapa.obter(m, \"k\");\n}\n",
        ),
    ] {
        let c = caso("i3_generica", fonte, &[]);
        let saida = executar(&c, "issue-532-i3-generica");
        assert_eq!(codigo(&saida), 42, "{rotulo}: {}", erro(&saida));
    }
}

/// I3: interpretador e nativo concordam sobre a criação genérica pela nova
/// superfície.
#[test]
fn i3_criacao_generica_tem_paridade_interpretador_nativo() {
    let fonte = "pacote main;\n\
                 trazer mapa.criar, definir, obter, tamanho;\n\
                 carinho principal() -> bombom {\n\
                 \x20   nova m: mapa<verso,bombom> = criar();\n\
                 \x20   definir(m, \"k\", 42);\n\
                 \x20   falar(tamanho(m));\n\
                 \x20   falar(obter(m, \"k\"));\n\
                 \x20   mimo 0;\n\
                 }\n";
    let c = caso("i3_paridade", fonte, &[]);
    let Some((codigo_saida, stdout)) = paridade(&c, "i3_paridade", "issue-532-i3-paridade") else {
        return;
    };
    assert_eq!(codigo_saida, 0);
    assert_eq!(stdout, "1\n42\n");
}

/// I3: a identidade interna da criação genérica continua sendo a do
/// compilador, e o usuário pode declarar a antiga grafia sem alcançá-la.
#[test]
fn i3_identidade_interna_da_criacao_generica_nao_e_alcancavel_por_declaracao() {
    let fonte = "pacote main;\n\
                 trazer mapa.criar, definir, obter;\n\
                 carinho mapa_criar(valor: bombom) -> bombom { mimo 7; }\n\
                 carinho principal() -> bombom {\n\
                 \x20   nova m: mapa<verso,bombom> = criar();\n\
                 \x20   definir(m, \"k\", 35);\n\
                 \x20   mimo obter(m, \"k\") + mapa_criar(0);\n\
                 }\n";
    let c = caso("i3_interna", fonte, &[]);
    let saida = executar(&c, "issue-532-i3-interna");
    assert_eq!(codigo(&saida), 42, "{}", erro(&saida));
}

// ---------------------------------------------------------------------------
// I4 — módulo real × família built-in
// ---------------------------------------------------------------------------

const TEXTO_REAL: &str = "pacote texto;\ncarinho tamanho(x: bombom) -> bombom { mimo 40 + x; }\n";

/// I4-P1 e I4-P2: com `texto.pink` real, as DUAS formas obedecem a mesma
/// precedência de identidade.
///
/// ```text
/// REAL_MODULE_texto > BUILTIN_FAMILY_texto
/// ```
///
/// A forma inteira de um módulo real traz os itens de topo dele — foi sempre
/// assim para módulo real —, e a seletiva liga o membro pedido. O que a #532
/// fechou é a assimetria: antes, só a seletiva enxergava o arquivo.
#[test]
fn i4_p1_p2_modulo_real_vence_a_familia_nas_duas_formas_de_import() {
    for (rotulo, raiz) in [
        (
            "inteira",
            "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo tamanho(1); }\n",
        ),
        (
            "seletiva",
            "pacote main;\ntrazer texto.tamanho;\ncarinho principal() -> bombom { mimo tamanho(1); }\n",
        ),
    ] {
        let c = caso("i4_p1", raiz, &[("texto", TEXTO_REAL)]);
        let saida = executar(&c, "issue-532-i4-p1");
        assert_eq!(codigo(&saida), 41, "{rotulo}: {}", erro(&saida));
    }
}

/// I4-P3 e G-517-1: sem `<familia>.pink`, a família built-in continua válida.
///
/// A ausência de arquivo é o caso COMUM de uma família legítima, não um módulo
/// faltando. Corrigir I4 não pode transformar toda família numa tentativa
/// obrigatória de abrir arquivo.
#[test]
fn i4_p3_sem_modulo_real_a_familia_builtin_continua_valida() {
    for (rotulo, raiz) in [
        (
            "inteira",
            "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo texto.tamanho(\"abc\"); }\n",
        ),
        (
            "seletiva",
            "pacote main;\ntrazer texto.tamanho;\ncarinho principal() -> bombom { mimo tamanho(\"abc\"); }\n",
        ),
    ] {
        let c = caso("i4_p3", raiz, &[]);
        let saida = executar(&c, "issue-532-i4-p3");
        assert_eq!(codigo(&saida), 3, "{rotulo}: {}", erro(&saida));
    }
}

/// I4-P4: membro do módulo real homônimo de membro da família não é capturado
/// pela família — e o oráculo distingue as duas implementações.
#[test]
fn i4_p4_membro_homonimo_do_modulo_real_nao_e_capturado_pela_familia() {
    let c = caso(
        "i4_p4",
        "pacote main;\ntrazer texto.tamanho;\ncarinho principal() -> bombom { mimo tamanho(2); }\n",
        &[("texto", TEXTO_REAL)],
    );
    let saida = executar(&c, "issue-532-i4-p4");
    assert_eq!(
        codigo(&saida),
        42,
        "a família capturou o membro do módulo real: {}",
        erro(&saida)
    );
}

/// I4-P5: o resultado não depende da ordem textual dos imports.
#[test]
fn i4_p5_resultado_independe_da_ordem_textual() {
    const VIZINHO: &str = "pacote apoio;\ncarinho marcador() -> bombom { mimo 0; }\n";
    for (rotulo, raiz) in [
        (
            "texto-antes",
            "pacote main;\ntrazer texto;\ntrazer apoio.marcador;\ncarinho principal() -> bombom { mimo tamanho(1) + marcador(); }\n",
        ),
        (
            "texto-depois",
            "pacote main;\ntrazer apoio.marcador;\ntrazer texto;\ncarinho principal() -> bombom { mimo tamanho(1) + marcador(); }\n",
        ),
    ] {
        let c = caso("i4_p5", raiz, &[("texto", TEXTO_REAL), ("apoio", VIZINHO)]);
        let saida = executar(&c, "issue-532-i4-p5");
        assert_eq!(codigo(&saida), 41, "{rotulo}: {}", erro(&saida));
    }
}

/// I4: a mesma precedência vale para outras famílias — não é exceção para
/// `texto`.
#[test]
fn i4_a_precedencia_nao_e_uma_excecao_para_texto() {
    for familia in ["lista", "caminho", "acaso", "tempo"] {
        let modulo = format!("pacote {familia};\ncarinho marca() -> bombom {{ mimo 41; }}\n");
        let raiz = format!(
            "pacote main;\ntrazer {familia};\ncarinho principal() -> bombom {{ mimo marca() + 1; }}\n"
        );
        let c = caso("i4_familias", &raiz, &[(familia, &modulo)]);
        let saida = executar(&c, "issue-532-i4-familias");
        assert_eq!(codigo(&saida), 42, "{familia}: {}", erro(&saida));
    }
}

// ---------------------------------------------------------------------------
// Matriz negativa
// ---------------------------------------------------------------------------

/// N1: chamada global a intrínseca sem superfície pública permitida continua
/// recusada. N3: membro inexistente de família continua recusado.
#[test]
fn n1_n3_superficie_global_e_membro_inexistente_continuam_recusados() {
    let global = caso(
        "n1",
        "pacote main;\ncarinho principal() -> bombom { mimo tamanho_verso(\"abc\"); }\n",
        &[],
    );
    let saida = checar(&global, "issue-532-n1");
    assert_eq!(codigo(&saida), 1, "superfície global reabriu");
    assert!(
        erro(&saida).contains("não está no escopo"),
        "{}",
        erro(&saida)
    );

    let membro = caso(
        "n3",
        "pacote main;\ntrazer texto.nao_existe;\ncarinho principal() -> bombom { mimo 0; }\n",
        &[],
    );
    let saida = checar(&membro, "issue-532-n3");
    assert_eq!(codigo(&saida), 1, "membro inexistente foi aceito");
}

/// N2: módulo inexistente continua diagnosticado pelo carregador, nas duas
/// formas — inclusive quando o nome NÃO é família.
#[test]
fn n2_modulo_inexistente_continua_diagnosticado_pelo_carregador() {
    for raiz in [
        "pacote main;\ntrazer ausente;\ncarinho principal() -> bombom { mimo 0; }\n",
        "pacote main;\ntrazer ausente.x;\ncarinho principal() -> bombom { mimo 0; }\n",
    ] {
        let c = caso("n2", raiz, &[]);
        let saida = checar(&c, "issue-532-n2");
        assert_eq!(codigo(&saida), 1);
        assert!(erro(&saida).contains("ausente"), "{}", erro(&saida));
    }
}

/// N4: import ambíguo/colidente continua diagnosticado.
#[test]
fn n4_import_colidente_continua_diagnosticado() {
    const A: &str = "pacote ma;\ncarinho comum() -> bombom { mimo 1; }\n";
    const B: &str = "pacote mb;\ncarinho comum() -> bombom { mimo 2; }\n";
    let c = caso(
        "n4",
        "pacote main;\ntrazer ma.comum;\ntrazer mb.comum;\ncarinho principal() -> bombom { mimo comum(); }\n",
        &[("ma", A), ("mb", B)],
    );
    let saida = checar(&c, "issue-532-n4");
    assert_eq!(codigo(&saida), 1, "colisão de import foi aceita");
    assert!(erro(&saida).contains("colisão"), "{}", erro(&saida));
}

/// N5 e N6: nenhum dos dois lados captura o outro.
///
/// O homônimo do usuário não captura a intrínseca resolvida, e a intrínseca não
/// captura a função do usuário. É o par que faz este gate falhar se a
/// identidade for trocada por qualquer um dos lados.
#[test]
fn n5_n6_nenhum_lado_captura_o_outro() {
    let fonte = "pacote main;\n\
                 trazer texto.tamanho;\n\
                 carinho tamanho_verso(valor: bombom) -> bombom { mimo 100; }\n\
                 carinho principal() -> bombom {\n\
                 \x20   falar(tamanho_verso(0));\n\
                 \x20   falar(tamanho(\"abcd\"));\n\
                 \x20   mimo 0;\n\
                 }\n";
    let c = caso("n5_n6", fonte, &[]);
    let saida = executar(&c, "issue-532-n5-n6");
    assert_eq!(codigo(&saida), 0, "{}", erro(&saida));
    assert_eq!(
        String::from_utf8_lossy(&saida.stdout),
        "100\n4\n",
        "identidade trocada entre usuário e intrínseca"
    );
}

/// N7: a identidade do módulo real não é substituída pela família em silêncio.
#[test]
fn n7_identidade_do_modulo_real_nao_e_substituida_pela_familia() {
    let c = caso(
        "n7",
        "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo tamanho(1); }\n",
        &[("texto", TEXTO_REAL)],
    );
    let saida = executar(&c, "issue-532-n7");
    assert_eq!(codigo(&saida), 41, "{}", erro(&saida));

    // O oposto do mesmo fato: a forma qualificada da família deixa de existir
    // quando o módulo real governa o nome.
    let c = caso(
        "n7b",
        "pacote main;\ntrazer texto;\ncarinho principal() -> bombom { mimo texto.tamanho(\"abc\"); }\n",
        &[("texto", TEXTO_REAL)],
    );
    let saida = checar(&c, "issue-532-n7b");
    assert_eq!(codigo(&saida), 1, "a família respondeu por um módulo real");
}

/// N8: `trazer M;` não reexporta o que M importou.
#[test]
fn n8_sem_reexport_implicito() {
    const FOLHA: &str = "pacote folha;\ncarinho profunda() -> bombom { mimo 1; }\n";
    const MEIO: &str =
        "pacote meio;\ntrazer folha.profunda;\ncarinho rasa() -> bombom { mimo profunda(); }\n";
    let c = caso(
        "n8",
        "pacote main;\ntrazer meio;\ncarinho principal() -> bombom { mimo profunda(); }\n",
        &[("folha", FOLHA), ("meio", MEIO)],
    );
    let saida = checar(&c, "issue-532-n8");
    assert_eq!(codigo(&saida), 1, "reexport implícito apareceu");
}

// ---------------------------------------------------------------------------
// Censo estrutural dos módulos built-in
// ---------------------------------------------------------------------------

/// Os quinze módulos continuam quinze, e nenhum recebe tratamento próprio.
///
/// O censo é estrutural: a taxonomia vem da autoridade única, cada módulo
/// exporta pelo menos um membro, todo membro endereça uma identidade que a
/// autoridade de intrínsecas conhece, e nenhum par `(módulo, membro)` se repete.
#[test]
fn censo_estrutural_dos_quinze_modulos_nao_tem_caso_especial() {
    assert_eq!(familia_superficie::FAMILIAS.len(), 15);

    let membros = intrinsic_authority::all_public_intrinsic_members();
    let mut vistos: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut com_membro: BTreeSet<&str> = BTreeSet::new();
    for membro in &membros {
        assert!(
            familia_superficie::familia_conhecida(membro.module),
            "membro fora da taxonomia: {}.{}",
            membro.module,
            membro.member
        );
        assert!(
            intrinsic_authority::intrinsic_from_public_spelling(
                membro.identity.canonical_public_spelling()
            )
            .is_some(),
            "identidade de {}.{} não é endereçável pela autoridade",
            membro.module,
            membro.member
        );
        assert!(
            vistos.insert((membro.module, membro.member)),
            "par repetido: {}.{}",
            membro.module,
            membro.member
        );
        com_membro.insert(membro.module);
    }
    let sem_membro: Vec<&&str> = familia_superficie::FAMILIAS
        .iter()
        .filter(|familia| !com_membro.contains(**familia))
        .collect();
    assert!(sem_membro.is_empty(), "módulos sem membro: {sem_membro:?}");

    // Nenhuma grafia builtin chamável fora da superfície pública: a lista de
    // exceções ficou vazia com a #532.
    for grafia in ["mapa_criar", "lista_criar"] {
        assert!(
            intrinsic_authority::canonical_public_intrinsic_spelling(grafia).is_some(),
            "{grafia} precisa pertencer à superfície pública"
        );
    }
}
// @pinker-nav:end evidencia.identidade.namespace-de-modulos
