//! D12 — métodos default em `trato`.
//!
//! A matriz fixa uma única seleção de implementação: cada `impl` nominal
//! materializa defaults omitidos como funções `__impl_*`; overrides explícitos
//! ocupam o mesmo slot e vencem sem caminhos especiais no interpretador ou no
//! backend nativo.

mod common;

use common::ControlledCommand as Command;
use pinker_v0::ast::{Item, Type};
use std::time::{SystemTime, UNIX_EPOCH};

const EXEMPLO: &str = "examples/d12_trait_default_methods_valido.pink";
const STDOUT_ESPERADO: &[&str] = &["10", "15", "99", "60", "7", "12", "99", "7"];

fn recusa(code: &str) -> String {
    common::parse_and_check(code)
        .expect_err("o programa deveria ser recusado")
        .to_string()
}

fn interpretado(exemplo: &str) -> (Vec<String>, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", exemplo])
        .output()
        .expect("execução do interpretador");
    assert!(
        output.stderr.is_empty(),
        "stderr interpretado deveria ser vazio: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect(),
        output.status.code(),
    )
}

fn nativo(exemplo: &str) -> Option<(Vec<String>, Option<i32>)> {
    let (_driver, Some(runtime_lib)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)?
    else {
        return None;
    };
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!(
        "pinker_d12_trait_defaults_{}_{nanos}",
        std::process::id()
    ));

    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("build nativo");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let name = std::path::Path::new(exemplo)
        .file_stem()
        .expect("nome do exemplo");
    let run = Command::new(out_dir.join(name))
        .output()
        .expect("execução nativa");
    assert!(
        run.stderr.is_empty(),
        "stderr nativo deveria ser vazio: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let result = (
        String::from_utf8_lossy(&run.stdout)
            .lines()
            .map(str::to_string)
            .collect(),
        run.status.code(),
    );
    let _ = std::fs::remove_dir_all(out_dir);
    Some(result)
}

#[test]
fn parser_distingue_required_default_e_materializa_si_concreto() {
    let code = r#"
        pacote main;
        trato Valor {
            carinho requerido(item: si) -> bombom;
            carinho padrao(item: si) -> bombom { mimo 7; }
        }
        impl Valor para u64 {
            carinho requerido(item: u64) -> bombom { mimo 3; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let program = common::parse(code).expect("parse");
    let trait_decl = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Trait(decl) => Some(decl),
            _ => None,
        })
        .expect("trato");
    assert!(trait_decl.methods[0].body.is_none());
    assert!(trait_decl.methods[1].body.is_some());

    let default = program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name.ends_with("_padrao") => Some(function),
            _ => None,
        })
        .expect("default materializado");
    assert!(matches!(default.params[0].ty, Type::U64(_)));
}

#[test]
fn required_default_override_si_statico_dinamico_e_empty_impl_passam() {
    assert!(common::parse_and_check(include_str!(
        "../examples/d12_trait_default_methods_valido.pink"
    ))
    .is_ok());
}

#[test]
fn receiver_qualificado_objeto_e_vtable_selecionam_o_mesmo_alvo() {
    let ir = common::render_ir(include_str!(
        "../examples/d12_trait_default_methods_valido.pink"
    ))
    .expect("IR");

    assert!(ir.contains(
        "vtable=[__impl_7_Medivel_6_bombom_marcador, __impl_7_Medivel_6_bombom_base, __impl_7_Medivel_6_bombom_dobro, __impl_7_Medivel_6_bombom_triplo]"
    ));
    assert!(ir.contains(
        "vtable=[__impl_7_Medivel_3_u64_marcador, __impl_7_Medivel_3_u64_base, __impl_7_Medivel_3_u64_dobro, __impl_7_Medivel_3_u64_triplo]"
    ));
    assert!(ir.contains("trait_call trato<Medivel>.dobro#2/4"));
    assert!(ir.contains("call __impl_7_Medivel_3_u64_dobro(%u#0) -> bombom"));
    assert!(ir.contains("call __impl_7_Medivel_3_u64_base(%valor#0) -> bombom"));
}

#[test]
fn dois_tratos_homonimos_permanecem_qualificaveis() {
    let code = r#"
        pacote main;
        trato A { carinho valor(item: si) -> bombom { mimo 1; } }
        trato B { carinho valor(item: si) -> bombom { mimo 2; } }
        impl A para bombom {}
        impl B para bombom {}
        carinho principal() -> bombom {
            mimo A.valor(0) + B.valor(0);
        }
    "#;
    assert!(common::parse_and_check(code).is_ok());
}

#[test]
fn homonimo_nao_qualificado_continua_ambiguo() {
    let code = r#"
        pacote main;
        trato A { carinho valor(item: si) -> bombom { mimo 1; } }
        trato B { carinho valor(item: si) -> bombom { mimo 2; } }
        impl A para bombom {}
        impl B para bombom {}
        carinho principal() -> bombom { mimo 0.valor(); }
    "#;
    assert!(recusa(code).contains("ambíguo"));
}

#[test]
fn required_omitido_continua_invalido() {
    let code = r#"
        pacote main;
        trato A {
            carinho requerido(item: si) -> bombom;
            carinho padrao(item: si) -> bombom { mimo 1; }
        }
        impl A para bombom {}
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(recusa(code).contains("não implementa método 'requerido'"));
}

#[test]
fn impl_vazio_com_required_e_sem_default_e_diagnosticado() {
    let code = r#"
        pacote main;
        trato A { carinho requerido(item: si) -> bombom; }
        impl A para bombom {}
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(recusa(code).contains("não implementa método 'requerido'"));
}

/// #572 — dois blocos da mesma relação são duplicata, e não uma relação
/// completada em partes.
///
/// A política anterior aceitava métodos distintos completando a mesma relação
/// e mantinha um único default materializado. A relação nominal passa a
/// existir pela declaração: a segunda é recusada. O que a materialização
/// garantia — um único corpo default por relação — continua verdadeiro na
/// única relação legítima que sobra.
#[test]
fn blocos_impl_da_mesma_relacao_sao_relacao_duplicada() {
    let code = r#"
        pacote main;
        trato A {
            carinho primeiro(item: si) -> bombom;
            carinho segundo(item: si) -> bombom;
            carinho padrao(item: si) -> bombom { mimo 3; }
        }
        impl A para bombom {
            carinho primeiro(item: bombom) -> bombom { mimo item; }
        }
        impl A para bombom {
            carinho segundo(item: bombom) -> bombom { mimo item; }
        }
        carinho principal() -> bombom { mimo 0.padrao(); }
    "#;
    let erro = recusa(code);
    assert!(erro.contains("impl do trato 'A'"), "{erro}");
    assert!(erro.contains("já declarado"), "{erro}");

    let relacao_unica = r#"
        pacote main;
        trato A {
            carinho primeiro(item: si) -> bombom;
            carinho segundo(item: si) -> bombom;
            carinho padrao(item: si) -> bombom { mimo 3; }
        }
        impl A para bombom {
            carinho primeiro(item: bombom) -> bombom { mimo item; }
            carinho segundo(item: bombom) -> bombom { mimo item; }
        }
        carinho principal() -> bombom { mimo 0.padrao(); }
    "#;
    assert!(common::parse_and_check(relacao_unica).is_ok());
    let program = common::parse(relacao_unica).expect("parse");
    let defaults = program
        .items
        .iter()
        .filter(
            |item| matches!(item, Item::Function(function) if function.name.ends_with("_padrao")),
        )
        .count();
    assert_eq!(defaults, 1);
}

#[test]
fn metodo_estranho_continua_invalido() {
    let code = r#"
        pacote main;
        trato A { carinho padrao(item: si) -> bombom { mimo 1; } }
        impl A para bombom {
            carinho estranho(item: bombom) -> bombom { mimo item; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    assert!(recusa(code).contains("não existe no trato"));
}

#[test]
fn override_incompativel_continua_invalido() {
    let code = r#"
        pacote main;
        trato A {
            carinho valor(item: si, extra: bombom) -> bombom { mimo extra; }
        }
        impl A para bombom {
            carinho valor(item: bombom, extra: logica) -> bombom { mimo item; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(err.contains("espera 'bombom'"), "{err}");
    assert!(err.contains("usa 'logica'"), "{err}");
}

#[test]
fn override_duplicado_continua_invalido() {
    let code = r#"
        pacote main;
        trato A { carinho valor(item: si) -> bombom { mimo 1; } }
        impl A para bombom {
            carinho valor(item: bombom) -> bombom { mimo 2; }
            carinho valor(item: bombom) -> bombom { mimo 3; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(err.contains("já declarada"), "{err}");
}

#[test]
fn corpo_default_semanticamente_invalido_falha_como_funcao_materializada() {
    let code = r#"
        pacote main;
        trato A {
            carinho valor(item: si) -> bombom { mimo inexistente; }
        }
        impl A para bombom {}
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(
        err.contains("identificador 'inexistente' não declarado"),
        "{err}"
    );
}

#[test]
fn corpo_default_invalido_nao_escapa_quando_override_vence() {
    let code = r#"
        pacote main;
        trato A {
            carinho valor(item: si) -> bombom { mimo inexistente; }
        }
        impl A para bombom {
            carinho valor(item: bombom) -> bombom { mimo 1; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(
        err.contains("identificador 'inexistente' não declarado"),
        "{err}"
    );
}

#[test]
fn override_incompativel_nao_e_mascarado_por_outro_impl_compativel() {
    let code = r#"
        pacote main;
        trato A {
            carinho valor(item: bombom, extra: bombom) -> bombom { mimo extra; }
        }
        carinho valor(item: bombom, extra: bombom) -> bombom { mimo extra; }
        impl A para bombom {
            carinho valor(item: bombom, extra: logica) -> bombom { mimo item; }
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let err = recusa(code);
    assert!(err.contains("espera 'bombom'"), "{err}");
    assert!(err.contains("usa 'logica'"), "{err}");
}

#[test]
fn dois_tipos_clonam_closure_do_default_com_capturas_independentes() {
    let code = r#"
        pacote main;
        trato T {
            carinho pega(x: si) -> bombom {
                nova g: carinho(bombom) -> bombom = carinho(y: bombom) -> bombom {
                    mimo x virar bombom;
                };
                mimo g(0);
            }
        }
        impl T para bombom {}
        impl T para u64 {}
        carinho principal() -> bombom {
            nova u: u64 = 9;
            falar(4.pega());
            falar(u.pega());
            mimo 0;
        }
    "#;

    assert!(common::parse_and_check(code).is_ok());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let source = std::env::temp_dir().join(format!(
        "pinker_d12_default_closure_{}_{nanos}.pink",
        std::process::id()
    ));
    std::fs::write(&source, code).expect("gravar probe de closure default");
    let source = source.to_string_lossy().into_owned();
    let interpreted = interpretado(&source);
    let native = nativo(&source);
    let _ = std::fs::remove_file(&source);
    assert_eq!(
        interpreted,
        (vec!["4".to_string(), "9".to_string()], Some(0))
    );
    if let Some(native) = native {
        assert_eq!(native, interpreted);
    }
}

#[test]
fn default_nao_cria_relacao_nominal() {
    let code = r#"
        pacote main;
        trato A { carinho valor(item: si) -> bombom { mimo 1; } }
        carinho principal() -> bombom { mimo 0.valor(); }
    "#;
    let err = recusa(code);
    assert!(
        err.contains("exige ao menos um impl completo")
            || err.contains("não implementado para tipo"),
        "{err}"
    );
}

#[test]
fn trato_nao_objetificavel_continua_recusado() {
    let code = r#"
        pacote main;
        trato A { carinho valor(item: bombom) -> bombom { mimo item; } }
        impl A para bombom {}
        carinho principal() -> bombom {
            nova objeto: trato<A> = 1 virar trato<A>;
            mimo 0;
        }
    "#;
    assert!(recusa(code).contains("não é objetificável"));
}

#[test]
fn interpretador_e_nativo_concordam_em_defaults_overrides_si_e_slots() {
    let (stdout_interpretado, exit_interpretado) = interpretado(EXEMPLO);
    assert_eq!(stdout_interpretado, STDOUT_ESPERADO);

    let Some((stdout_nativo, exit_nativo)) = nativo(EXEMPLO) else {
        return;
    };
    assert_eq!(stdout_nativo, STDOUT_ESPERADO);
    assert_eq!(exit_interpretado, exit_nativo);
}
