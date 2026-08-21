//! U2 / F-05 — identidade de método sobre o tipo semanticamente resolvido.

mod common;

use common::{ControlledCommand as Command, NativeArtifactDir};
use pinker_v0::abstract_machine_validate;
use pinker_v0::cfg_ir;
use pinker_v0::cfg_ir_validate;
use pinker_v0::generic_identity::{specialization_name, GenericKind, GenericOrigin};
use pinker_v0::instr_select;
use pinker_v0::instr_select_validate;
use pinker_v0::interpreter::{self, RuntimeValue};
use pinker_v0::ir;
use pinker_v0::ir_validate;
use pinker_v0::semantic;
use std::fs;

// @pinker-nav:start evidencia.tratos.identidade-resolvida-u2-f05
// @pinker-nav:domain tratos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Matriz U2/F-05 sobre o pipeline real: aliases simples, em cadeia e equivalentes despacham pela mesma identidade resolvida; duplicatas equivalentes falham nas duas ordens sem vazar codecs internos; métodos distintos podem completar blocos equivalentes, tratos e tipos estruturais distintos permanecem independentes, receivers trocados são rejeitados, especializações #476 seguem injetivas, módulos são decididos após montagem, e o caso aceito cobre despacho direto/vtable, paridade entre interpretador e ELF nativo e símbolos locais.

fn run_code(code: &str) -> Result<Option<RuntimeValue>, String> {
    let program = common::parse(code).map_err(|error| error.to_string())?;
    semantic::check_program(&program).map_err(|error| error.to_string())?;
    let program_ir = ir::lower_program(&program).map_err(|error| error.to_string())?;
    ir_validate::validate_program(&program_ir).map_err(|error| error.to_string())?;
    let cfg = cfg_ir::lower_program(&program_ir).map_err(|error| error.to_string())?;
    cfg_ir_validate::validate_program(&cfg).map_err(|error| error.to_string())?;
    let selected = instr_select::lower_program(&cfg).map_err(|error| error.to_string())?;
    instr_select_validate::validate_program(&selected).map_err(|error| error.to_string())?;
    let machine =
        pinker_v0::abstract_machine::lower_program(&selected).map_err(|error| error.to_string())?;
    abstract_machine_validate::validate_program(&machine).map_err(|error| error.to_string())?;
    interpreter::run_program(&machine).map_err(|error| error.to_string())
}

fn rejected(code: &str) -> String {
    common::parse_and_check(code)
        .expect_err("programa deveria ser rejeitado")
        .to_string()
}

fn one_method_program(impl_target: &str, receiver_type: &str, use_type: &str) -> String {
    format!(
        r#"
pacote main;
apelido Numero = bombom;
trato Dobravel {{ carinho dobrar(valor: si) -> bombom; }}
impl Dobravel para {impl_target} {{
    carinho dobrar(valor: {receiver_type}) -> bombom {{ mimo valor + valor; }}
}}
carinho principal() -> bombom {{
    nova valor: {use_type} = 21;
    mimo valor.dobrar();
}}
"#
    )
}

#[test]
fn t1_t2_alias_e_alvo_real_funcionam_nos_dois_sentidos() {
    let alias_impl = one_method_program("Numero", "bombom", "bombom");
    let concrete_impl = one_method_program("bombom", "Numero", "Numero");
    assert_eq!(run_code(&alias_impl), Ok(Some(RuntimeValue::Int(42))));
    assert_eq!(run_code(&concrete_impl), Ok(Some(RuntimeValue::Int(42))));
}

#[test]
fn t3_t4_alias_em_cadeia_e_dois_aliases_equivalentes_compartilham_identidade() {
    let chain = r#"
pacote main;
apelido A = bombom;
apelido B = A;
trato T { carinho valor(item: si) -> bombom; }
impl T para B { carinho valor(item: B) -> bombom { mimo item + 1; } }
carinho principal() -> bombom {
    nova via_a: A = 20;
    nova direto: bombom = 20;
    mimo via_a.valor() + direto.valor();
}
"#;
    let equivalent_aliases = r#"
pacote main;
apelido A = bombom;
apelido B = bombom;
trato T { carinho valor(item: si) -> bombom; }
impl T para A { carinho valor(item: A) -> bombom { mimo item + 1; } }
carinho principal() -> bombom {
    nova via_b: B = 41;
    mimo via_b.valor();
}
"#;
    assert_eq!(run_code(chain), Ok(Some(RuntimeValue::Int(42))));
    assert_eq!(
        run_code(equivalent_aliases),
        Ok(Some(RuntimeValue::Int(42)))
    );
}

#[test]
fn t5_t9_tipos_e_tratos_realmente_distintos_permanecem_distintos() {
    let code = r#"
pacote main;
trato A { carinho valor(item: si) -> bombom; }
trato B { carinho valor(item: si) -> bombom; }
impl A para bombom { carinho valor(item: bombom) -> bombom { mimo item + 1; } }
impl A para u64 { carinho valor(item: u64) -> bombom { mimo (item virar bombom) + 2; } }
impl B para bombom { carinho valor(item: bombom) -> bombom { mimo item + 3; } }
carinho principal() -> bombom {
    nova outro: u64 = 10;
    mimo A.valor(10) + A.valor(outro) + B.valor(6);
}
"#;
    assert_eq!(run_code(code), Ok(Some(RuntimeValue::Int(32))));
}

fn alias_conflict(reverse: bool) -> String {
    let concrete = r#"impl T para bombom {
    carinho valor(item: bombom) -> bombom { mimo 1; }
}"#;
    let alias = r#"impl T para Numero {
    carinho valor(item: Numero) -> bombom { mimo 2; }
}"#;
    let blocks = if reverse {
        format!("{alias}\n{concrete}")
    } else {
        format!("{concrete}\n{alias}")
    };
    format!(
        r#"
pacote main;
apelido Numero = bombom;
trato T {{ carinho valor(item: si) -> bombom; }}
{blocks}
carinho principal() -> bombom {{ mimo 0; }}
"#
    )
}

#[test]
fn t6_t7_t14_t16_conflito_equivalente_e_explicito_nas_duas_ordens() {
    for reverse in [false, true] {
        let source = alias_conflict(reverse);
        let error = rejected(&source);
        assert!(error.contains("método 'valor' do trato 'T'"), "{error}");
        assert!(error.contains("'Numero'"), "{error}");
        assert!(error.contains("'bombom'"), "{error}");
        assert!(error.contains("resolvem para 'bombom'"), "{error}");
        assert!(error.contains("outra declaração em"), "{error}");
        assert_eq!(
            error.matches("..").count(),
            2,
            "diagnóstico precisa preservar os dois spans do conflito: {error}"
        );
        assert!(!error.contains("__impl_"), "{error}");
        assert!(
            common::render_backend_s(&source).is_err(),
            "impl inválido não pode chegar ao backend"
        );
    }
}

#[test]
fn t8_duplicata_com_mesma_grafia_preserva_recusa_sem_vazar_simbolo() {
    let code = r#"
pacote main;
trato T { carinho valor(item: si) -> bombom; }
impl T para bombom { carinho valor(item: bombom) -> bombom { mimo 1; } }
impl T para bombom { carinho valor(item: bombom) -> bombom { mimo 2; } }
carinho principal() -> bombom { mimo 0; }
"#;
    let error = rejected(code);
    assert!(error.contains("método 'valor' do trato 'T'"), "{error}");
    assert!(error.contains("já implementado"), "{error}");
    assert!(!error.contains("__impl_"), "{error}");

    let structural = r#"
pacote main;
trato T { carinho valor(item: si) -> bombom; }
impl T para mapa<verso, u64> {
    carinho valor(item: mapa<verso, u64>) -> bombom { mimo 1; }
}
impl T para mapa<verso, u64> {
    carinho valor(item: mapa<verso, u64>) -> bombom { mimo 2; }
}
carinho principal() -> bombom { mimo 0; }
"#;
    let structural_error = rejected(structural);
    assert!(
        structural_error.contains("mapa<verso,u64>"),
        "{structural_error}"
    );
    assert!(!structural_error.contains("__type_"), "{structural_error}");
    assert!(!structural_error.contains("__impl_"), "{structural_error}");

    let missing_receiver = r#"
pacote main;
trato T { carinho valor(item: si) -> bombom; }
impl T para mapa<verso, u64> {
    carinho valor() -> bombom { mimo 1; }
}
carinho principal() -> bombom { mimo 0; }
"#;
    let parser_error = common::parse(missing_receiver)
        .expect_err("receiver ausente precisa falhar no parser")
        .to_string();
    assert!(parser_error.contains("mapa<verso,u64>"), "{parser_error}");
    assert!(!parser_error.contains("__type_"), "{parser_error}");
    assert!(!parser_error.contains("__impl_"), "{parser_error}");
}

#[test]
fn t10_metodos_distintos_em_blocos_alias_equivalentes_completam_o_contrato() {
    let code = r#"
pacote main;
apelido Numero = bombom;
trato T {
    carinho primeiro(item: si) -> bombom;
    carinho segundo(item: si) -> bombom;
    carinho padrao(item: si) -> bombom { mimo 3; }
}
impl T para bombom {
    carinho primeiro(item: bombom) -> bombom { mimo item + 10; }
}
impl T para Numero {
    carinho segundo(item: Numero) -> bombom { mimo item + 20; }
}
carinho principal() -> bombom { mimo 2.primeiro() + 7.segundo() + 0.padrao(); }
"#;
    assert_eq!(run_code(code), Ok(Some(RuntimeValue::Int(42))));

    let program = common::parse(code).expect("parse T10");
    semantic::check_program(&program).expect("semântica T10");
    let lowered = ir::lower_program(&program).expect("IR T10");
    let defaults = lowered
        .functions
        .iter()
        .filter(|function| {
            function.name.starts_with("__impl_") && function.name.ends_with("_padrao")
        })
        .count();
    assert_eq!(
        defaults, 1,
        "default equivalente não pode gerar símbolo morto"
    );

    let explicit_override = r#"
pacote main;
apelido Numero = bombom;
trato T {
    carinho primeiro(item: si) -> bombom;
    carinho padrao(item: si) -> bombom { mimo 1; }
}
impl T para bombom {
    carinho primeiro(item: bombom) -> bombom { mimo item; }
}
impl T para Numero {
    carinho padrao(item: Numero) -> bombom { mimo 42; }
}
carinho principal() -> bombom { mimo 0.padrao(); }
"#;
    assert_eq!(run_code(explicit_override), Ok(Some(RuntimeValue::Int(42))));
}

#[test]
fn t11_t12_alvos_monomorfizados_e_identidades_476_nao_colapsam() {
    let code = r#"
pacote main;
leque Caixa<T> { Valor(T) }
apelido CaixaNumero = Caixa<bombom>;
apelido CaixaTexto = Caixa<verso>;
trato Marca { carinho marca(item: si) -> bombom; }
impl Marca para CaixaNumero {
    carinho marca(item: CaixaNumero) -> bombom { mimo 1; }
}
impl Marca para CaixaTexto {
    carinho marca(item: CaixaTexto) -> bombom { mimo 2; }
}
carinho principal() -> bombom { mimo 0; }
"#;
    common::parse_and_check(code).expect("alvos monomorfizados distintos");

    let span = pinker_v0::falha_operacional::span_sintetico();
    let left = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Root,
        "Caixa_Doce",
        &[pinker_v0::ast::Type::Bombom(span)],
    );
    let right = specialization_name(
        GenericKind::Enum,
        &GenericOrigin::Root,
        "Caixa",
        &[pinker_v0::ast::Type::Alias {
            name: "Doce".to_string(),
            span,
        }],
    );
    assert_ne!(left, right, "controle injetivo #476");

    let maps = r#"
pacote main;
trato Marca { carinho marca(item: si) -> bombom; }
impl Marca para mapa<verso, u64> {
    carinho marca(item: mapa<verso, u64>) -> bombom { mimo 1; }
}
impl Marca para mapa<verso, u8> {
    carinho marca(item: mapa<verso, u8>) -> bombom { mimo 2; }
}
carinho principal() -> bombom { mimo 0; }
"#;
    common::parse_and_check(maps).expect("mapas estruturalmente distintos não colapsam");
}

#[test]
fn receiver_divergente_do_alvo_declarado_nao_contorna_a_coerencia() {
    let code = r#"
pacote main;
trato T { carinho valor(item: si) -> bombom; }
impl T para bombom {
    carinho valor(item: u64) -> bombom { mimo 1; }
}
impl T para u64 {
    carinho valor(item: bombom) -> bombom { mimo 2; }
}
carinho principal() -> bombom { mimo 0; }
"#;
    let error = rejected(code);
    assert!(error.contains("receiver do método 'valor'"), "{error}");
    assert!(error.contains("impl 'T' para 'bombom'"), "{error}");
}

#[test]
fn t13_modulo_com_impl_em_alias_despacha_no_programa_montado() {
    let dir = NativeArtifactDir::create().expect("sandbox de módulo");
    fs::write(
        dir.path().join("metodos.pink"),
        r#"pacote metodos;
apelido Numero = bombom;
trato Dobravel { carinho dobrar(valor: si) -> bombom; }
impl Dobravel para Numero {
    carinho dobrar(valor: Numero) -> bombom { mimo valor + valor; }
}
"#,
    )
    .expect("módulo");
    let root = dir.path().join("principal.pink");
    fs::write(
        &root,
        r#"pacote main;
trazer metodos;
carinho principal() -> bombom { mimo 21.dobrar(); }
"#,
    )
    .expect("raiz");

    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run"])
        .arg(&root)
        .logical_case("u2-f05-t13-module-alias-impl")
        .output()
        .expect("execução modular");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn source_order_e_import_order_nao_mudam_o_veredito() {
    let before = r#"
pacote main;
apelido A = bombom;
apelido B = A;
trato T {
    carinho primeiro(item: si) -> bombom;
    carinho segundo(item: si) -> bombom;
    carinho padrao(item: si) -> bombom { mimo 3; }
}
impl T para A { carinho primeiro(item: A) -> bombom { mimo item + 10; } }
impl T para B { carinho segundo(item: B) -> bombom { mimo item + 20; } }
carinho principal() -> bombom { mimo 2.primeiro() + 7.segundo() + 0.padrao(); }
"#;
    let after_and_inverted = r#"
pacote main;
trato T {
    carinho primeiro(item: si) -> bombom;
    carinho segundo(item: si) -> bombom;
    carinho padrao(item: si) -> bombom { mimo 3; }
}
impl T para B { carinho segundo(item: B) -> bombom { mimo item + 20; } }
impl T para A { carinho primeiro(item: A) -> bombom { mimo item + 10; } }
apelido B = A;
apelido A = bombom;
carinho principal() -> bombom { mimo 2.primeiro() + 7.segundo() + 0.padrao(); }
"#;
    assert_eq!(run_code(before), Ok(Some(RuntimeValue::Int(42))));
    assert_eq!(
        run_code(after_and_inverted),
        Ok(Some(RuntimeValue::Int(42)))
    );

    let dir = NativeArtifactDir::create().expect("sandbox de ordem de imports");
    fs::write(
        dir.path().join("metodos.pink"),
        r#"pacote metodos;
apelido Numero = bombom;
trato Dobravel { carinho dobrar(valor: si) -> bombom; }
impl Dobravel para Numero {
    carinho dobrar(valor: Numero) -> bombom { mimo valor + valor; }
}
"#,
    )
    .expect("módulo de métodos");
    fs::write(
        dir.path().join("neutro.pink"),
        "pacote neutro; carinho identidade(valor: bombom) -> bombom { mimo valor; }",
    )
    .expect("módulo neutro");

    for (name, imports) in [
        ("imports_a", "trazer metodos;\ntrazer neutro;"),
        ("imports_b", "trazer neutro;\ntrazer metodos;"),
    ] {
        let root = dir.path().join(format!("{name}.pink"));
        fs::write(
            &root,
            format!(
                "pacote main;\n{imports}\ncarinho principal() -> bombom {{ mimo 21.dobrar(); }}\n"
            ),
        )
        .expect("raiz de import");
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .args(["--run"])
            .arg(&root)
            .logical_case(name)
            .output()
            .expect("execução com imports");
        assert!(
            output.stderr.is_empty(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.code(), Some(42), "{name}");
    }
}

#[test]
fn t15_interpretador_e_nativo_concordam_e_simbolo_impl_permanece_local() {
    let vtable_alias = r#"
pacote main;
apelido Numero = bombom;
trato Dobravel { carinho dobrar(valor: si) -> bombom; }
impl Dobravel para Numero {
    carinho dobrar(valor: bombom) -> bombom { mimo valor + valor; }
}
carinho dinamico(valor: trato<Dobravel>) -> bombom { mimo valor.dobrar(); }
carinho principal() -> bombom { mimo dinamico(21 virar trato<Dobravel>); }
"#;
    assert_eq!(run_code(vtable_alias), Ok(Some(RuntimeValue::Int(42))));

    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let dir = NativeArtifactDir::create().expect("sandbox nativo");
    let source = dir.path().join("alias_impl.pink");
    fs::write(&source, one_method_program("Numero", "Numero", "bombom")).expect("fonte nativa");

    let interpreted = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run"])
        .arg(&source)
        .logical_case("u2-f05-t15-interpreter")
        .output()
        .expect("interpretador");
    assert_eq!(interpreted.status.code(), Some(42), "{:?}", interpreted);

    let out_dir = dir.path().join("native");
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(&source)
        .env("PINKER_RT_LIB", &runtime_lib)
        .logical_case("u2-f05-t15-native-build")
        .output()
        .expect("build nativo");
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let native = Command::new(out_dir.join("alias_impl"))
        .logical_case("u2-f05-t15-native-run")
        .output()
        .expect("execução nativa");
    assert_eq!(interpreted.status.code(), native.status.code());

    let symbols = Command::new("nm")
        .args(["-a", "--format=posix"])
        .arg(out_dir.join("alias_impl"))
        .logical_case("u2-f05-t15-nm")
        .output()
        .expect("nm");
    assert!(symbols.status.success(), "{:?}", symbols);
    let symbols = String::from_utf8_lossy(&symbols.stdout);
    let impl_line = symbols
        .lines()
        .find(|line| line.contains("__impl_8_Dobravel_6_Numero_dobrar"))
        .unwrap_or_else(|| panic!("símbolo impl ausente em:\n{symbols}"));
    assert!(
        impl_line.split_whitespace().any(|field| field == "t"),
        "{impl_line}"
    );
}

// @pinker-nav:end evidencia.tratos.identidade-resolvida-u2-f05
