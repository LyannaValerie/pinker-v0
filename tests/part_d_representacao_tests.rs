//! Parte D, Step 2 — representação e identidade antes da execução hospedada.

// @pinker-nav:start evidencia.processos.parte-d-representacao
// @pinker-nav:domain processos
// @pinker-nav:layer evidencia
// @pinker-nav:summary Gate representacional da Parte D: prova Resultado<SaidaProcesso, verso> e accessors tipados do parser ao backend, liga OpaqueWordHandle à identidade nominal correta, fixa assinaturas falíveis declarativas e recusa shadowing de SaidaProcesso ou LimiteTempo antes/depois do uso e em módulo importado.
mod common;

use pinker_v0::enum_payload::{self, EnumPayloadClass};
use pinker_v0::ir::{self, TypeIR};
use pinker_v0::{
    cfg_ir, cfg_ir_validate, instr_select, instr_select_validate, ir_validate, semantic,
};
use std::process::Command;

const PROBE_EXATO: &str = r#"
pacote main; trazer arquivo.criar; trazer lista.criar; trazer processo.codigo; trazer processo.erro; trazer processo.executar_estruturado; trazer processo.saida;

apelido Saida = SaidaProcesso;
apelido Res = Resultado<SaidaProcesso, verso>;

carinho atravessar(valor: Res) -> Res { mimo valor; }

carinho observar(valor: Res) -> bombom {
    encaixe valor {
        caso Res.Ok(saida) {
            falar(codigo(saida));
            falar(saida(saida));
            falar(erro(saida));
            mimo 0;
        }
        caso Res.Erro(mensagem) { falar(mensagem); mimo 1; }
    }
    mimo 2;
}

carinho principal() -> bombom {
    nova muda argumentos: lista<verso> = criar();
    nova muda ambiente: mapa<verso,verso> = mapa_criar();
    nova resultado: Resultado<SaidaProcesso, verso> = atravessar(
        executar_estruturado(
            "/bin/true", argumentos, "", "", ambiente, LimiteTempo.SemLimite
        )
    );
    mimo observar(resultado);
}
"#;

#[test]
fn resultado_saida_processo_atravessa_parser_semantica_ir_cfg_selecao_maquina_backend() {
    let program = common::parse(PROBE_EXATO).expect("parser");
    semantic::check_program(&program).expect("semântica");
    let ir = ir::lower_program(&program).expect("IR");
    ir_validate::validate_program(&ir).expect("validação IR");

    let payload = ir
        .enum_variants
        .iter()
        .find(|meta| meta.variant_name == "Ok")
        .and_then(|meta| meta.payloads.first())
        .expect("payload Ok(SaidaProcesso)");
    assert_eq!(payload.class, EnumPayloadClass::OpaqueWordHandle);
    assert_eq!(payload.operational_type, TypeIR::OpaqueWordHandle);
    let identity = &ir.resolved_types[payload.resolved_type_id.0 as usize];
    assert_eq!(identity.canonical_key, "opaque:13:SaidaProcesso");

    let cfg = cfg_ir::lower_program(&ir).expect("CFG");
    cfg_ir_validate::validate_program(&cfg).expect("validação CFG");
    let selected = instr_select::lower_program(&cfg).expect("seleção");
    instr_select_validate::validate_program(&selected).expect("validação seleção");
    let rendered_machine = common::render_machine(PROBE_EXATO).expect("máquina abstrata");
    assert!(
        rendered_machine.contains(enum_payload::CARGA_SAIDA_PROCESSO),
        "{rendered_machine}"
    );

    let asm = common::render_backend_s_external_subset_nativo(PROBE_EXATO)
        .expect("backend nativo montável");
    assert!(asm.contains("call pinker_processo_executar_estruturado"));
    assert!(asm.contains("call pinker_saida_processo_codigo"));
    assert!(asm.contains("call pinker_saida_processo_stdout"));
    assert!(asm.contains("call pinker_saida_processo_stderr"));
    assert!(asm.contains("call pinker_leque_carga"));
}

#[test]
fn remover_saida_processo_da_autoridade_de_carga_quebraria_a_superficie() {
    let superficie = pinker_v0::falha_operacional::superficie("executar_processo_estruturado")
        .expect("superfície estruturada pertence à autoridade");
    assert_eq!(
        superficie.sucesso,
        pinker_v0::falha_operacional::CargaResultado::SaidaProcesso
    );
    let span = pinker_v0::falha_operacional::span_sintetico();
    let ty = superficie.sucesso.tipo(span);
    let shape = enum_payload::classify_enum_payload(
        &ty,
        &Default::default(),
        &Default::default(),
        &Default::default(),
    )
    .expect("autoridade aceita SaidaProcesso");
    assert_eq!(
        shape.anexar_intrinsic(),
        enum_payload::ANEXAR_SAIDA_PROCESSO
    );
    assert_eq!(shape.carga_intrinsic(), enum_payload::CARGA_SAIDA_PROCESSO);
}

#[test]
fn assinaturas_faliveis_preservam_historicas_e_declaram_aridade_seis() {
    for superficie in pinker_v0::falha_operacional::SUPERFICIES_FALIVEIS {
        let esperado = if superficie.intrinseca == "executar_processo_estruturado" {
            6
        } else {
            1
        };
        assert_eq!(superficie.aridade(), esperado, "{}", superficie.intrinseca);
    }
}

#[test]
fn identidades_runtime_reservadas_têm_categoria_real() {
    use pinker_v0::runtime_identity::{runtime_reserved_identity, RuntimeSemanticKind};
    assert_eq!(
        runtime_reserved_identity("SaidaProcesso").map(|id| id.kind),
        Some(RuntimeSemanticKind::OpaqueWordHandle)
    );
    assert_eq!(
        runtime_reserved_identity("LimiteTempo").map(|id| id.kind),
        Some(RuntimeSemanticKind::PlainEnum)
    );
}

#[test]
fn identidade_reservada_independe_da_ordem_e_categoria_da_declaracao() {
    let categorias = [
        "apelido {n} = bombom;",
        "leque {n} { Outro }",
        "ninho {n} { valor: bombom; }",
        "carinho {n}() -> bombom { mimo 0; }",
        "eterno {n}: bombom = 0;",
        "trato {n} { carinho valor(valor: si) -> bombom; }",
    ];
    let uso = r#"trazer lista.criar; trazer processo.executar_estruturado; carinho principal() -> bombom {
        nova muda a: lista<verso> = criar();
        nova muda e: mapa<verso,verso> = mapa_criar();
        executar_estruturado("/bin/true", a, "", "", e, LimiteTempo.SemLimite);
        mimo 0;
    }"#;
    for nome in ["SaidaProcesso", "LimiteTempo"] {
        for categoria in categorias {
            let declaracao = categoria.replace("{n}", nome);
            for fonte in [
                format!("pacote main;\n{declaracao}\n{uso}"),
                format!("pacote main;\n{uso}\n{declaracao}"),
            ] {
                let erro = common::parse(&fonte).expect_err("identidade deveria ser reservada");
                assert!(
                    erro.to_string().contains("identidade builtin reservada"),
                    "nome={nome} declaração={declaracao} erro={erro}"
                );
            }
        }
    }
}

#[test]
fn identidade_reservada_tambem_e_guardada_em_modulo_importado() {
    let dir = common::NativeArtifactDir::create().expect("sandbox de import");
    let main = dir.path().join("main.pink");
    let modulo = dir.path().join("util.pink");
    std::fs::write(
        &main,
        "pacote main; trazer util.marcador; carinho principal() -> bombom { mimo marcador(); }",
    )
    .expect("main");

    for nome in ["SaidaProcesso", "LimiteTempo"] {
        std::fs::write(
            &modulo,
            format!(
                "pacote util; apelido {nome} = bombom; carinho marcador() -> bombom {{ mimo 0; }}"
            ),
        )
        .expect("módulo");
        let output = Command::new(env!("CARGO_BIN_EXE_pink"))
            .arg("--check")
            .arg(&main)
            .output()
            .expect("pink --check");
        assert!(!output.status.success(), "{nome} foi aceito no import");
        let diagnostico = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostico.contains("identidade builtin reservada"),
            "{nome}: {diagnostico}"
        );
    }
}
// @pinker-nav:end evidencia.processos.parte-d-representacao
