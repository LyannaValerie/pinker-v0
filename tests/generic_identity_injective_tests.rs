mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::ast::{Item, Type};
use pinker_v0::generic_identity::{specialization_name, GenericKind, GenericOrigin};
use std::collections::BTreeSet;
use std::fs;
use std::time::Duration;

// @pinker-nav:start evidencia.genericos.identidade-injetiva-476
// @pinker-nav:domain genericos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Regressões da #476 sobre o pipeline real: C1-C5 em leques, fronteiras equivalentes em funções, equivalência explícita/inferida, limite deliberado de aliases não resolvidos, proveniências distintas para builtin/fonte raiz/módulo inclusive no cross-case GI-HR3 de Resultado, C6 no loader/flatten e símbolos nativos distintos com montagem, link, chamadas e execução em paridade.

fn enum_alias_target(program: &pinker_v0::ast::Program, alias_name: &str) -> String {
    program
        .items
        .iter()
        .find_map(|item| match item {
            Item::TypeAlias(alias) if alias.name == alias_name => match &alias.target {
                Type::Enum { name, .. } => Some(name.clone()),
                other => panic!("alias {alias_name} deveria apontar para leque, veio {other:?}"),
            },
            _ => None,
        })
        .unwrap_or_else(|| panic!("alias {alias_name} ausente"))
}

fn function_names(program: &pinker_v0::ast::Program) -> Vec<String> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) if function.name.starts_with("__gen_") => {
                Some(function.name.clone())
            }
            _ => None,
        })
        .collect()
}

const C1: &str = r#"
pacote main;
leque Resultado_verso<T> { Usuario(T) }
apelido Builtin = Resultado<verso, verso>;
apelido Usuario = Resultado_verso<verso>;
carinho principal() -> bombom { mimo 0; }
"#;

const C2: &str = r#"
pacote main;
leque lista_verso { Nominal }
leque G<T> { V(T) }
apelido Nominal = G<lista_verso>;
apelido Estrutural = G<lista<verso>>;
carinho principal() -> bombom { mimo 0; }
"#;

const C3: &str = r#"
pacote main;
leque A_B { AB }
leque A { A }
leque B_C { BC }
leque C { C }
leque G<T, U> { V(T, U) }
apelido Esquerda = G<A_B, C>;
apelido Direita = G<A, B_C>;
carinho principal() -> bombom { mimo 0; }
"#;

const C4: &str = r#"
pacote main;
leque A { A }
leque B { B }
leque C { C }
leque F<T, U> { V(T, U) }
leque F_A<T> { V(T) }
leque G<T, U> { V(T, U) }
apelido Esquerda = G<F<A, B>, C>;
apelido Direita = G<F_A<B>, C>;
carinho principal() -> bombom { mimo 0; }
"#;

const C5: &str = r#"
pacote main;
leque Sabor { Sabor }
leque Doce_Sabor { DoceSabor }
leque Caixa_Doce<T> { Uma(T) }
leque Caixa<T> { Outra(T) }
apelido Esquerda = Caixa_Doce<Sabor>;
apelido Direita = Caixa<Doce_Sabor>;
carinho principal() -> bombom { mimo 0; }
"#;

#[test]
fn c1_a_c5_especializacoes_distintas_permanecem_distintas_no_parser() {
    let cases = [
        ("C1", C1, "Builtin", "Usuario"),
        ("C2", C2, "Nominal", "Estrutural"),
        ("C3", C3, "Esquerda", "Direita"),
        ("C4", C4, "Esquerda", "Direita"),
        ("C5", C5, "Esquerda", "Direita"),
    ];

    for (case, source, left_alias, right_alias) in cases {
        let program = common::parse(source).unwrap_or_else(|err| panic!("{case}: {err}"));
        let left = enum_alias_target(&program, left_alias);
        let right = enum_alias_target(&program, right_alias);
        assert_ne!(left, right, "{case} ainda colidiu");
        pinker_v0::semantic::check_program(&program)
            .unwrap_or_else(|err| panic!("{case} falhou na semântica: {err}"));
    }
}

#[test]
fn funcoes_genericas_preservam_fronteiras_de_argumentos() {
    let source = r#"
        pacote main;
        leque A_B { AB }
        leque A { A }
        leque B_C { BC }
        leque C { C }

        carinho valor<T, U>(x: bombom) -> bombom { mimo x; }

        carinho principal() -> bombom {
            nova esquerda: bombom = valor<A_B, C>(11);
            nova direita: bombom = valor<A, B_C>(31);
            talvez esquerda == 11 && direita == 31 { mimo 0; }
            mimo 1;
        }
    "#;
    let program = common::parse(source).expect("funções genéricas colidentes devem parsear");
    pinker_v0::semantic::check_program(&program).expect("funções distintas devem ser válidas");
    let names = function_names(&program);
    assert_eq!(names.len(), 2, "especializações inesperadas: {names:?}");
    assert_ne!(names[0], names[1]);
}

#[test]
fn gate_a3_explicito_e_inferido_deduplicam_na_mesma_identidade() {
    let source = r#"
        pacote main;
        carinho id<T>(valor: T) -> T { mimo valor; }
        carinho principal() -> bombom {
            nova explicito: bombom = id<bombom>(20);
            nova inferido: bombom = id(22);
            mimo explicito + inferido;
        }
    "#;
    let program = common::parse(source).expect("caminhos explícito e inferido");
    let names = function_names(&program);
    assert_eq!(
        names.len(),
        1,
        "a mesma especialização foi duplicada: {names:?}"
    );
    assert_eq!(
        names[0],
        specialization_name(
            GenericKind::Function,
            &GenericOrigin::Root,
            "id",
            &[Type::Bombom(pinker_v0::falha_operacional::span_sintetico())],
        )
    );
}

#[test]
fn full_alias_canonicalization_fica_deferida_para_a_auditoria_477() {
    let source = r#"
        pacote main;
        leque A { X }
        apelido AA = A;
        leque G<T> { V(T) }
        apelido ViaAlias = G<AA>;
        apelido Direto = G<A>;
        carinho principal() -> bombom { mimo 0; }
    "#;
    let program = common::parse(source).expect("aliases transparentes continuam válidos");
    pinker_v0::semantic::check_program(&program).expect("aliases resolvem na semântica");
    assert_ne!(
        enum_alias_target(&program, "ViaAlias"),
        enum_alias_target(&program, "Direto"),
        "a #476 não transporta resolução semântica de aliases ao parser"
    );
}

#[test]
fn resultado_runtime_e_parser_usam_exatamente_a_mesma_identidade() {
    let superficie = pinker_v0::falha_operacional::superficie("ler_arquivo_resultado")
        .expect("superfície registrada");
    let expected = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Builtin,
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &superficie.argumentos_de_tipo(pinker_v0::falha_operacional::span_sintetico()),
    );
    assert_eq!(superficie.leque_monomorfico(), expected);

    let program = common::parse(
        r#"
        pacote main;
        apelido R = Resultado<verso, verso>;
        carinho principal() -> bombom { mimo 0; }
        "#,
    )
    .expect("Resultado explícito");
    assert_eq!(enum_alias_target(&program, "R"), expected);
}

#[test]
fn resultado_builtin_dentro_de_modulo_conserva_a_identidade_runtime_global() {
    let dir = NativeArtifactDir::create().expect("diretório Resultado builtin em módulo");
    fs::write(
        dir.path().join("mod_builtin.pink"),
        "pacote mod_builtin; apelido R = Resultado<verso, verso>;",
    )
    .expect("módulo builtin");
    let root = dir.path().join("principal.pink");
    fs::write(
        &root,
        r#"pacote main;
trazer mod_builtin;
carinho principal() -> bombom {
    nova r: R = R.Ok("ok");
    encaixe r {
        caso R.Ok(v) { falar(v); }
        caso R.Erro(e) { falar(e); }
    }
    mimo 0;
}
"#,
    )
    .expect("raiz builtin");

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--ir")
        .arg(&root)
        .logical_case("issue-476-resultado-builtin-module-root")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar Resultado builtin em módulo");
    assert!(
        output.status.success(),
        "builtin em módulo falhou: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let span = pinker_v0::falha_operacional::span_sintetico();
    let args = [Type::Verso(span), Type::Verso(span)];
    let expected = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Builtin,
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let forbidden_module_identity = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("mod_builtin"),
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(rendered.contains(&expected));
    assert!(!rendered.contains(&forbidden_module_identity));
}

fn write_builtin_module_root_user_resultado_fixture(
    dir: &NativeArtifactDir,
    declaration_before_alias: bool,
) -> std::path::PathBuf {
    fs::write(
        dir.path().join("mod_builtin.pink"),
        "pacote mod_builtin; apelido BM = Resultado<verso, verso>;",
    )
    .expect("módulo com Resultado builtin");
    let root = dir.path().join("principal.pink");
    let source = if declaration_before_alias {
        r#"pacote main;
trazer mod_builtin;
leque Resultado<T, E> { Usuario(T), Falha(E) }
apelido RU = Resultado<verso, verso>;
carinho principal() -> bombom {
    nova builtin: BM = BM.Ok("builtin");
    nova usuario: RU = RU.Usuario("usuario");
    encaixe builtin {
        caso BM.Ok(v) { falar(v); }
        caso BM.Erro(e) { falar(e); }
    }
    encaixe usuario {
        caso RU.Usuario(v) { falar(v); }
        caso RU.Falha(e) { falar(e); }
    }
    mimo 0;
}
"#
    } else {
        r#"pacote main;
trazer mod_builtin;
apelido RU = Resultado<verso, verso>;
leque Resultado<T, E> { Usuario(T), Falha(E) }
carinho principal() -> bombom {
    nova builtin: BM = BM.Ok("builtin");
    nova usuario: RU = RU.Usuario("usuario");
    encaixe builtin {
        caso BM.Ok(v) { falar(v); }
        caso BM.Erro(e) { falar(e); }
    }
    encaixe usuario {
        caso RU.Usuario(v) { falar(v); }
        caso RU.Falha(e) { falar(e); }
    }
    mimo 0;
}
"#
    };
    fs::write(&root, source).expect("raiz com Resultado de usuário");
    root
}

#[test]
fn gi_hr3_builtin_de_modulo_e_resultado_de_usuario_na_raiz_sao_distintos_em_paridade() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let span = pinker_v0::falha_operacional::span_sintetico();
    let args = [Type::Verso(span), Type::Verso(span)];
    let builtin = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Builtin,
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let root_user = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Root,
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let forbidden_module_builtin = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("mod_builtin"),
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    assert_ne!(builtin, root_user);

    for (declaration_before_alias, logical_suffix) in
        [(true, "declaration-first"), (false, "alias-first")]
    {
        let dir = NativeArtifactDir::create().expect("diretório GI-HR3");
        let root = write_builtin_module_root_user_resultado_fixture(&dir, declaration_before_alias);
        let ir = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--ir")
            .arg(&root)
            .logical_case(&format!("issue-476-gi-hr3-ir-{logical_suffix}"))
            .timeout(Duration::from_secs(30))
            .output()
            .expect("executar IR do cross-case GI-HR3");
        assert!(
            ir.status.success(),
            "builtin importado e Resultado de fonte raiz colidiram ({logical_suffix}): {}",
            String::from_utf8_lossy(&ir.stderr)
        );
        let rendered = String::from_utf8_lossy(&ir.stdout);
        assert!(rendered.contains(&builtin), "builtin ausente: {rendered}");
        assert!(
            rendered.contains(&root_user),
            "template de fonte raiz ausente: {rendered}"
        );
        assert!(
            !rendered.contains(&forbidden_module_builtin),
            "builtin foi projetado como template modular: {rendered}"
        );

        let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--run")
            .arg(&root)
            .logical_case(&format!("issue-476-gi-hr3-interpreter-{logical_suffix}"))
            .timeout(Duration::from_secs(30))
            .output()
            .expect("executar GI-HR3 no interpretador");
        assert!(
            interpreted.status.success(),
            "interpretador GI-HR3 falhou: {}",
            String::from_utf8_lossy(&interpreted.stderr)
        );

        let build = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["build", "--nativo", "--out-dir"])
            .arg(dir.path())
            .arg(&root)
            .env("PINKER_RT_LIB", &runtime_lib)
            .logical_case(&format!("issue-476-gi-hr3-build-{logical_suffix}"))
            .timeout(Duration::from_secs(60))
            .output()
            .expect("compilar GI-HR3 nativo");
        assert!(
            build.status.success(),
            "build nativo GI-HR3 falhou: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        let native = Command::new(dir.path().join("principal"))
            .logical_case(&format!("issue-476-gi-hr3-native-{logical_suffix}"))
            .timeout(Duration::from_secs(30))
            .output()
            .expect("executar GI-HR3 nativo");
        assert_eq!(interpreted.status.code(), Some(0));
        assert_eq!(native.status.code(), Some(0));
        assert_eq!(interpreted.stdout, native.stdout);
        assert_eq!(
            String::from_utf8_lossy(&native.stdout),
            "builtin\nusuario\n"
        );
    }
}

fn write_module_fixture(dir: &NativeArtifactDir) -> std::path::PathBuf {
    fs::write(
        dir.path().join("mod_a.pink"),
        "pacote mod_a; leque G<T> { A(T) } apelido GA = G<verso>;",
    )
    .expect("mod_a");
    fs::write(
        dir.path().join("mod_b.pink"),
        "pacote mod_b; leque G<T> { B(T) } apelido GB = G<verso>;",
    )
    .expect("mod_b");
    let root = dir.path().join("principal.pink");
    fs::write(
        &root,
        r#"pacote main;
trazer mod_a;
trazer mod_b;
carinho principal() -> bombom {
    nova a: GA = GA.A("a");
    nova b: GB = GB.B("b");
    encaixe a { caso GA.A(v) { falar(v); } }
    encaixe b { caso GB.B(v) { falar(v); } }
    mimo 0;
}"#,
    )
    .expect("raiz C6");
    root
}

#[test]
fn c6_loader_flatten_preserva_origens_distintas() {
    let dir = NativeArtifactDir::create().expect("diretório C6");
    let root = write_module_fixture(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--ir")
        .arg(&root)
        .logical_case("issue-476-c6-module-origin")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar C6 pelo loader real");
    assert!(
        output.status.success(),
        "C6 falhou no loader/semântica: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rendered = String::from_utf8_lossy(&output.stdout);
    let span = pinker_v0::falha_operacional::span_sintetico();
    let a = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("mod_a"),
        "G",
        &[Type::Verso(span)],
    );
    let b = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("mod_b"),
        "G",
        &[Type::Verso(span)],
    );
    assert_ne!(a, b);
    assert!(
        rendered.contains(&a),
        "IR não contém origem mod_a: {rendered}"
    );
    assert!(
        rendered.contains(&b),
        "IR não contém origem mod_b: {rendered}"
    );
}

fn write_user_resultado_module_fixture(dir: &NativeArtifactDir) -> std::path::PathBuf {
    fs::write(
        dir.path().join("mod_a.pink"),
        r#"pacote mod_a;
apelido RA = Resultado<verso, verso>;
leque Resultado<T, E> { A(T), FalhaA(E) }
"#,
    )
    .expect("mod_a Resultado de usuário, uso antes");
    fs::write(
        dir.path().join("mod_b.pink"),
        r#"pacote mod_b;
leque Resultado<T, E> { B(T), FalhaB(E) }
apelido RB = Resultado<verso, verso>;
"#,
    )
    .expect("mod_b Resultado de usuário, declaração antes");
    let root = dir.path().join("principal.pink");
    fs::write(
        &root,
        r#"pacote main;
trazer mod_a;
trazer mod_b;
carinho principal() -> bombom {
    nova a: RA = RA.A("a");
    nova b: RB = RB.B("b");
    encaixe a {
        caso RA.A(v) { falar(v); }
        caso RA.FalhaA(e) { falar(e); }
    }
    encaixe b {
        caso RB.B(v) { falar(v); }
        caso RB.FalhaB(e) { falar(e); }
    }
    mimo 0;
}
"#,
    )
    .expect("raiz C6-Resultado");
    root
}

#[test]
fn c6_resultado_usuario_preserva_origem_e_independe_da_ordem_textual() {
    let dir = NativeArtifactDir::create().expect("diretório C6-Resultado");
    let root = write_user_resultado_module_fixture(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--ir")
        .arg(&root)
        .logical_case("issue-476-c6-user-resultado-origin")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar C6-Resultado pelo loader real");
    assert!(
        output.status.success(),
        "C6-Resultado falhou no flatten/semântica: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let span = pinker_v0::falha_operacional::span_sintetico();
    let args = [Type::Verso(span), Type::Verso(span)];
    let a = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("mod_a"),
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let b = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("mod_b"),
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let builtin = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Builtin,
        pinker_v0::falha_operacional::LEQUE_RESULTADO,
        &args,
    );
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert_ne!(a, b);
    assert!(rendered.contains(&a), "origem mod_a ausente: {rendered}");
    assert!(rendered.contains(&b), "origem mod_b ausente: {rendered}");
    assert!(
        !rendered.contains(&builtin),
        "template de usuário foi confundido com builtin: {rendered}"
    );
}

const BACKEND_SOURCE: &str = r#"
pacote main;
leque A_B { AB }
leque A { A }
leque B_C { BC }
leque C { C }
leque Folha<T> { Valor(T) }
leque Caixa<T> { Dentro(T) }
apelido FolhaBombom = Folha<bombom>;
apelido CaixaFolha = Caixa<Folha<bombom>>;

carinho valor<T, U>(x: bombom) -> bombom { mimo x; }

carinho principal() -> bombom {
    nova esquerda: bombom = valor<A_B, C>(11);
    nova direita: bombom = valor<A, B_C>(31);
    falar(esquerda);
    falar(direita);
    nova folha: FolhaBombom = FolhaBombom.Valor(42);
    nova caixa: CaixaFolha = CaixaFolha.Dentro(folha);
    encaixe caixa {
        caso CaixaFolha.Dentro(interna) {
            encaixe interna {
                caso FolhaBombom.Valor(numero) { falar(numero); }
            }
        }
    }
    talvez esquerda == 11 && direita == 31 { mimo 0; }
    mimo 1;
}
"#;

fn generated_global_symbols(assembly: &str) -> BTreeSet<String> {
    assembly
        .lines()
        .filter_map(|line| line.trim().strip_prefix(".globl "))
        .filter(|name| name.starts_with("__gen_") && !name.starts_with("__gen_leque_"))
        .map(ToOwned::to_owned)
        .collect()
}

#[test]
fn backend_emite_monta_liga_e_executa_dois_simbolos_distintos() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let dir = NativeArtifactDir::create().expect("diretório backend #476");
    let source = dir.path().join("generic_identity_backend.pink");
    fs::write(&source, BACKEND_SOURCE).expect("fixture backend #476");

    let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("--run")
        .arg(&source)
        .logical_case("issue-476-backend-interpreter")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("interpretador #476");
    assert!(interpreted.status.success());

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(dir.path())
        .arg(&source)
        .env("PINKER_RT_LIB", runtime_lib)
        .logical_case("issue-476-backend-build")
        .timeout(Duration::from_secs(60))
        .output()
        .expect("build nativo #476");
    assert!(
        build.status.success(),
        "montagem/link falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let assembly = fs::read_to_string(dir.path().join("generic_identity_backend.s"))
        .expect("assembly preservado");
    let symbols = generated_global_symbols(&assembly);
    assert_eq!(
        symbols.len(),
        2,
        "globais genéricas: {symbols:?}\n{assembly}"
    );
    for symbol in &symbols {
        assert!(
            assembly.contains(&format!("{symbol}:")),
            "label ausente: {symbol}"
        );
        assert!(
            assembly.contains(&format!("call {symbol}")),
            "call ausente: {symbol}"
        );
    }

    let native = Command::new(dir.path().join("generic_identity_backend"))
        .logical_case("issue-476-backend-native")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("executar ELF #476");
    assert_eq!(interpreted.status.code(), Some(0));
    assert_eq!(native.status.code(), Some(0));
    assert_eq!(interpreted.stdout, native.stdout);
    assert_eq!(String::from_utf8_lossy(&native.stdout), "11\n31\n42\n");
}

#[test]
fn renderer_nao_introduz_digest_probabilistico() {
    let bytes = pinker_v0::generic_identity::monomorphization_specialization_bytes(
        GenericKind::Enum,
        &GenericOrigin::module("módulo"),
        "G_ç",
        &[Type::Verso(pinker_v0::falha_operacional::span_sintetico())],
    );
    let symbol = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::module("módulo"),
        "G_ç",
        &[Type::Verso(pinker_v0::falha_operacional::span_sintetico())],
    );
    let hex = symbol.strip_prefix("__gen_leque_").unwrap();
    assert_eq!(
        hex.len(),
        bytes.len() * 2,
        "renderer deve conter todos os bytes"
    );
    assert!(hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

// @pinker-nav:end evidencia.genericos.identidade-injetiva-476
