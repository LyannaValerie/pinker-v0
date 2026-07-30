//! HR3 — payloads estruturais de união.
//!
//! Cobre a classificação exaustiva das representações de payload, a rejeição
//! semântica antecipada dos tipos sem representação conhecida, o transporte do
//! layout pela IR e a independência entre origem, snapshot e binding extraído,
//! no interpretador e no caminho nativo.

mod common;

use std::collections::HashMap;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use pinker_v0::ast::{StructDecl, Type};
use pinker_v0::union_payload::{
    classify_union_payload, UnionPayloadRepresentation, MAX_UNION_PAYLOAD_ALIGN,
    MAX_UNION_PAYLOAD_BYTES, MAX_UNION_TOTAL_PAYLOAD_BYTES, UNION_DESCRIPTOR_METADATA_BYTES,
};
use pinker_v0::{
    abstract_machine, abstract_machine_validate, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, ir, ir_validate, semantic,
};

const POSICAO: pinker_v0::token::Position = pinker_v0::token::Position { line: 1, col: 1 };
const SPAN: pinker_v0::token::Span = pinker_v0::token::Span {
    start: POSICAO,
    end: POSICAO,
};

fn lower(source: &str) -> (ir::ProgramIR, abstract_machine::MachineProgram) {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let ir_program = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&ir_program).expect("ir validate");
    let cfg = cfg_ir::lower_program(&ir_program).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");
    (ir_program, machine)
}

fn semantic_error(source: &str) -> String {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast)
        .expect_err("programa deveria ser recusado pela semântica")
        .to_string()
}

fn array(element: Type, size: u64) -> Type {
    Type::FixedArray {
        element: Box::new(element),
        size,
        span: SPAN,
    }
}

fn empty_context() -> (HashMap<String, Type>, HashMap<String, StructDecl>) {
    (HashMap::new(), HashMap::new())
}

// ---------------------------------------------------------------------------
// Classificação exaustiva
// ---------------------------------------------------------------------------

#[test]
fn hr3_classifica_escalares_com_largura_real() {
    let (aliases, structs) = empty_context();
    let casos = [
        (Type::U8(SPAN), 1_u64, 1_u64),
        (Type::I8(SPAN), 1, 1),
        (Type::Logica(SPAN), 1, 1),
        (Type::U16(SPAN), 2, 2),
        (Type::I16(SPAN), 2, 2),
        (Type::U32(SPAN), 4, 4),
        (Type::I32(SPAN), 4, 4),
        (Type::U64(SPAN), 8, 8),
        (Type::I64(SPAN), 8, 8),
        (Type::Bombom(SPAN), 8, 8),
    ];
    for (ty, size, align) in casos {
        let layout = classify_union_payload(&ty, &aliases, &structs).unwrap_or_else(|error| {
            panic!("{} deveria classificar: {}", ty.name(), error.message())
        });
        assert_eq!(
            layout.representation,
            UnionPayloadRepresentation::Scalar,
            "{}",
            ty.name()
        );
        assert_eq!(layout.size, size, "{}", ty.name());
        assert_eq!(layout.align, align, "{}", ty.name());
    }
}

#[test]
fn hr3_classifica_leque_nominal_como_escalar() {
    let (aliases, structs) = empty_context();
    let leque = Type::Enum {
        name: "Cor".to_string(),
        span: SPAN,
    };
    let layout = classify_union_payload(&leque, &aliases, &structs).expect("leque classifica");
    assert_eq!(layout.representation, UnionPayloadRepresentation::Scalar);
    assert_eq!(layout.size, 8);
}

#[test]
fn hr3_classifica_handles_opacos_de_uma_palavra() {
    let (aliases, structs) = empty_context();
    let casos = [
        Type::Verso(SPAN),
        Type::ListBombom(SPAN),
        Type::ListVerso(SPAN),
        Type::MapVersoBombom(SPAN),
        Type::MapVersoVerso(SPAN),
        Type::MapBombomBombom(SPAN),
        Type::MapBombomVerso(SPAN),
        Type::Pointer {
            base: Box::new(Type::Bombom(SPAN)),
            is_volatile: false,
            span: SPAN,
        },
        Type::Function {
            params: vec![Type::Bombom(SPAN)],
            ret: Box::new(Type::Bombom(SPAN)),
            span: SPAN,
        },
    ];
    for ty in casos {
        let layout = classify_union_payload(&ty, &aliases, &structs).unwrap_or_else(|error| {
            panic!("{} deveria classificar: {}", ty.name(), error.message())
        });
        assert_eq!(
            layout.representation,
            UnionPayloadRepresentation::OpaqueHandle,
            "{}",
            ty.name()
        );
        assert_eq!(layout.size, 8, "{}", ty.name());
        assert_eq!(layout.align, 8, "{}", ty.name());
    }
}

#[test]
fn hr3_classifica_agregados_com_layout_real() {
    let (aliases, structs) = empty_context();
    for (elementos, tamanho) in [(2_u64, 16_u64), (3, 24), (9, 72)] {
        let ty = array(Type::Bombom(SPAN), elementos);
        let layout = classify_union_payload(&ty, &aliases, &structs).expect("array classifica");
        assert_eq!(layout.representation, UnionPayloadRepresentation::Aggregate);
        assert_eq!(layout.size, tamanho);
        assert_eq!(layout.align, 8);
    }
    // Array de bytes: tamanho 9, alinhamento 1 — nem tamanho nem alinhamento
    // são arredondados para palavra.
    let bytes = array(Type::U8(SPAN), 9);
    let layout = classify_union_payload(&bytes, &aliases, &structs).expect("array de bytes");
    assert_eq!(layout.size, 9);
    assert_eq!(layout.align, 1);
}

#[test]
fn hr3_apelidos_sao_transparentes_em_profundidade() {
    let mut aliases = HashMap::new();
    aliases.insert("Interno".to_string(), array(Type::Bombom(SPAN), 3));
    aliases.insert(
        "Meio".to_string(),
        Type::Alias {
            name: "Interno".to_string(),
            span: SPAN,
        },
    );
    aliases.insert(
        "Externo".to_string(),
        Type::Alias {
            name: "Meio".to_string(),
            span: SPAN,
        },
    );
    let structs = HashMap::new();

    let direto = classify_union_payload(&array(Type::Bombom(SPAN), 3), &aliases, &structs)
        .expect("alvo classifica");
    for nome in ["Interno", "Meio", "Externo"] {
        let apelido = Type::Alias {
            name: nome.to_string(),
            span: SPAN,
        };
        let layout = classify_union_payload(&apelido, &aliases, &structs)
            .unwrap_or_else(|error| panic!("{nome}: {}", error.message()));
        assert_eq!(layout, direto, "apelido '{nome}' deve resolver ao alvo");
    }
}

#[test]
fn hr3_rejeita_tipos_sem_representacao_conhecida() {
    let (aliases, structs) = empty_context();

    let nulo = classify_union_payload(&Type::Nulo(SPAN), &aliases, &structs)
        .expect_err("nulo não é payload");
    assert_eq!(nulo.code(), "E-SEMANTIC-UNION-PAYLOAD-REPRESENTATION");

    let aplicado = classify_union_payload(
        &Type::Applied {
            name: "Caixa".to_string(),
            args: vec![Type::Bombom(SPAN)],
            span: SPAN,
        },
        &aliases,
        &structs,
    )
    .expect_err("genérico não monomorfizado não é payload");
    assert_eq!(aplicado.code(), "E-SEMANTIC-UNION-PAYLOAD-REPRESENTATION");

    let inexistente = classify_union_payload(
        &Type::Alias {
            name: "NaoExiste".to_string(),
            span: SPAN,
        },
        &aliases,
        &structs,
    )
    .expect_err("apelido inexistente não é payload");
    assert_eq!(inexistente.code(), "E-SEMANTIC-UNION-PAYLOAD-LAYOUT");

    let ninho_inexistente = classify_union_payload(
        &Type::Struct {
            name: "NaoExiste".to_string(),
            span: SPAN,
        },
        &aliases,
        &structs,
    )
    .expect_err("ninho inexistente não é payload");
    assert_eq!(ninho_inexistente.code(), "E-SEMANTIC-UNION-PAYLOAD-LAYOUT");
}

#[test]
fn hr3_rejeita_apelido_recursivo_sem_cair_em_fallback() {
    let mut aliases = HashMap::new();
    aliases.insert(
        "Ciclo".to_string(),
        Type::Alias {
            name: "Ciclo".to_string(),
            span: SPAN,
        },
    );
    let structs = HashMap::new();
    let erro = classify_union_payload(
        &Type::Alias {
            name: "Ciclo".to_string(),
            span: SPAN,
        },
        &aliases,
        &structs,
    )
    .expect_err("apelido recursivo não é payload");
    assert_eq!(erro.code(), "E-SEMANTIC-UNION-PAYLOAD-LAYOUT");
    assert!(erro.message().contains("recursivo"), "{}", erro.message());
}

#[test]
fn hr3_limites_de_tamanho_valem_no_limite_e_acima() {
    let (aliases, structs) = empty_context();

    let no_limite = array(Type::U8(SPAN), MAX_UNION_PAYLOAD_BYTES);
    let layout = classify_union_payload(&no_limite, &aliases, &structs)
        .expect("tamanho exatamente no limite é aceito");
    assert_eq!(layout.size, MAX_UNION_PAYLOAD_BYTES);

    let acima = array(Type::U8(SPAN), MAX_UNION_PAYLOAD_BYTES + 1);
    let erro = classify_union_payload(&acima, &aliases, &structs)
        .expect_err("um byte acima do limite é recusado");
    assert_eq!(erro.code(), "E-SEMANTIC-UNION-PAYLOAD-SIZE");
}

#[test]
fn hr3_rejeita_tamanho_zero_e_overflow_de_array() {
    let (aliases, structs) = empty_context();

    let vazio = array(Type::Bombom(SPAN), 0);
    let erro = classify_union_payload(&vazio, &aliases, &structs).expect_err("tamanho zero");
    assert_eq!(erro.code(), "E-SEMANTIC-UNION-PAYLOAD-SIZE");

    let overflow = array(Type::Bombom(SPAN), u64::MAX);
    let erro = classify_union_payload(&overflow, &aliases, &structs).expect_err("overflow");
    assert_eq!(erro.code(), "E-SEMANTIC-UNION-PAYLOAD-LAYOUT");
}

#[test]
fn hr3_limites_documentados_sao_finitos_e_coerentes() {
    // Os limites são constantes; os valores são lidos por variável para que a
    // asserção descreva a política e não seja dobrada em tempo de compilação.
    let por_payload = MAX_UNION_PAYLOAD_BYTES;
    let alinhamento = MAX_UNION_PAYLOAD_ALIGN;
    let total = MAX_UNION_TOTAL_PAYLOAD_BYTES;
    assert_eq!(por_payload, 4096, "teto por payload documentado");
    assert_eq!(alinhamento, 16, "teto de alinhamento documentado");
    assert!(alinhamento.is_power_of_two());
    assert!(
        total >= por_payload,
        "o orçamento agregado precisa comportar ao menos um payload no limite"
    );
    assert_eq!(UNION_DESCRIPTOR_METADATA_BYTES, 64);
}

#[test]
fn hr3_layout_mal_formado_e_recusado_pelo_predicado_central() {
    use pinker_v0::union_payload::UnionPayloadLayout;

    let bem_formado = UnionPayloadLayout {
        size: 24,
        align: 8,
        representation: UnionPayloadRepresentation::Aggregate,
    };
    assert!(bem_formado.is_well_formed());

    // Cada campo isolado torna o layout inválido. Este é o predicado que os
    // validadores de IR, CFG, seleção, máquina e backend repetem em vez de
    // confiar na origem.
    let casos = [
        (
            "tamanho zero",
            UnionPayloadLayout {
                size: 0,
                ..bem_formado
            },
        ),
        (
            "tamanho acima do limite",
            UnionPayloadLayout {
                size: MAX_UNION_PAYLOAD_BYTES + 1,
                ..bem_formado
            },
        ),
        (
            "alinhamento zero",
            UnionPayloadLayout {
                align: 0,
                ..bem_formado
            },
        ),
        (
            "alinhamento não potência de dois",
            UnionPayloadLayout {
                align: 3,
                ..bem_formado
            },
        ),
        (
            "alinhamento acima do limite",
            UnionPayloadLayout {
                align: MAX_UNION_PAYLOAD_ALIGN * 2,
                ..bem_formado
            },
        ),
        (
            "handle com tamanho de agregado",
            UnionPayloadLayout {
                size: 24,
                align: 8,
                representation: UnionPayloadRepresentation::OpaqueHandle,
            },
        ),
        (
            "escalar mais largo que uma palavra",
            UnionPayloadLayout {
                size: 16,
                align: 8,
                representation: UnionPayloadRepresentation::Scalar,
            },
        ),
    ];
    for (nome, layout) in casos {
        assert!(!layout.is_well_formed(), "{nome} deveria ser mal formado");
    }
}

// ---------------------------------------------------------------------------
// Diagnóstico semântico antes da IR validada
// ---------------------------------------------------------------------------

#[test]
fn hr3_semantica_recusa_payload_acima_do_limite() {
    let error = semantic_error(include_str!(
        "../examples/hr3_uniao_payload_sem_representacao_invalido.pink"
    ));
    assert!(
        error.contains("E-SEMANTIC-UNION-PAYLOAD-SIZE"),
        "diagnóstico deve ser semântico e ter código estável: {error}"
    );
}

// ---------------------------------------------------------------------------
// Registry e IR
// ---------------------------------------------------------------------------

#[test]
fn hr3_registry_transporta_representacao_e_layout_reais() {
    let (ir_program, _) = lower(include_str!(
        "../examples/hr3_uniao_agregado_imutavel_valido.pink"
    ));
    let union = &ir_program.union_types[0];
    let agregado = union
        .members
        .iter()
        .find(|member| {
            member.payload_layout.representation == UnionPayloadRepresentation::Aggregate
        })
        .expect("a união tem um membro agregado");
    assert_eq!(agregado.payload_layout.size, 24);
    assert_eq!(agregado.payload_layout.align, 8);

    let escalar = union
        .members
        .iter()
        .find(|member| member.payload_layout.representation == UnionPayloadRepresentation::Scalar)
        .expect("a união tem um membro escalar");
    assert_eq!(escalar.payload_layout.size, 1, "u8 usa a largura real");
    assert_eq!(escalar.payload_layout.align, 1);

    // Cada membro tem identidade resolvida própria: HR4 permanece intacto.
    assert_ne!(agregado.resolved_type_id, escalar.resolved_type_id);
    ir::validate_union_registry(&ir_program.union_types).expect("registry válido");
}

#[test]
fn hr3_validador_recusa_layout_de_outro_membro() {
    let (ir_program, _) = lower(include_str!(
        "../examples/hr3_uniao_agregado_imutavel_valido.pink"
    ));
    let unions = &ir_program.union_types;
    let alvo = &unions[0].members[0];
    let outro = &unions[0].members[1];
    let error = ir::validate_union_member_reference(
        unions,
        unions[0].id,
        alvo.tag,
        &alvo.canonical_member_key,
        alvo.ty,
        outro.payload_layout,
    )
    .expect_err("layout de outro membro deve falhar");
    assert!(error.contains("layout de payload divergente"), "{error}");
}

// ---------------------------------------------------------------------------
// Imutabilidade e independência no interpretador
// ---------------------------------------------------------------------------

/// Executa o exemplo pelo CLI para observar exatamente o stdout do
/// interpretador, que é o mesmo canal comparado com o binário nativo.
fn interpretado(exemplo: &str) -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["--run", exemplo])
        .output()
        .expect("execução do interpretador");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr interpretado deveria ser vazio: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn hr3_interpretador_ignora_mutacao_da_origem() {
    let linhas = interpretado("examples/hr3_uniao_agregado_imutavel_valido.pink");
    assert_eq!(
        linhas,
        vec!["111".to_string(), "222".to_string()],
        "o encaixe deve observar o snapshot anterior, e a origem deve manter a mudança"
    );
}

#[test]
fn hr3_interpretador_mantem_extracoes_independentes() {
    let linhas = interpretado("examples/hr3_uniao_extracoes_independentes_valido.pink");
    assert_eq!(
        linhas,
        vec!["7".to_string(), "7".to_string(), "42".to_string()],
        "duas extrações devem ver o mesmo snapshot, independente da origem"
    );
}

// ---------------------------------------------------------------------------
// Forma do código nativo
// ---------------------------------------------------------------------------

#[test]
fn hr3_backend_materializa_storage_novo_para_agregado() {
    let asm = common::render_backend_s_external_subset_nativo(include_str!(
        "../examples/hr3_uniao_agregado_imutavel_valido.pink"
    ))
    .expect("assembly");

    // Criação por endereço e extração por cópia validada; os símbolos de leitura
    // de palavra deixaram de existir.
    assert!(asm.contains("call pinker_uniao_criar"), "{asm}");
    assert!(asm.contains("call pinker_uniao_copiar_payload"), "{asm}");
    assert!(!asm.contains("pinker_uniao_payload_"), "{asm}");
    assert!(
        asm.contains("movq $24, %rdx"),
        "tamanho real do agregado\n{asm}"
    );

    // A extração devolve o endereço do storage novo do frame, nunca o handle
    // recebido nem o ponteiro interno do descritor.
    let depois_da_copia: Vec<&str> = asm
        .lines()
        .map(str::trim)
        .skip_while(|linha| !linha.contains("call pinker_uniao_copiar_payload"))
        .take(3)
        .collect();
    assert!(
        depois_da_copia
            .iter()
            .any(|linha| linha.starts_with("leaq -") && linha.ends_with("(%rbp), %rax")),
        "a extração de agregado deve devolver o storage novo do frame: {depois_da_copia:?}"
    );
    assert!(
        !depois_da_copia
            .iter()
            .any(|linha| *linha == "movq %rdi, %rax"),
        "a extração não pode devolver o handle recebido: {depois_da_copia:?}"
    );
}

#[test]
fn hr3_backend_materializa_escalar_com_largura_real() {
    let asm = common::render_backend_s_external_subset_nativo(include_str!(
        "../examples/fase248_unioes_estruturais_valido.pink"
    ))
    .expect("assembly");

    // O membro injetado é `u8`: um byte de largura real, tanto na escrita do
    // scratch quanto na leitura do storage de extração.
    assert!(
        asm.contains("movq $1, %rdx"),
        "tamanho real do payload u8\n{asm}"
    );
    assert!(
        asm.contains("movb %al,"),
        "escalar de 1 byte deve gravar um byte\n{asm}"
    );
    assert!(
        asm.contains("movzbq"),
        "escalar de 1 byte deve ser lido com extensão de zero\n{asm}"
    );
    // O scratch é zerado antes da escrita para não vazar bytes do frame.
    assert!(asm.contains("movq $0, -"), "scratch zerado\n{asm}");
}

// ---------------------------------------------------------------------------
// Paridade nativa
// ---------------------------------------------------------------------------

fn paridade_nativa(exemplo: &str, esperado: &[&str]) {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let pink = env!("CARGO_BIN_EXE_pink");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_hr3_{nanos}"));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let nome = std::path::Path::new(exemplo)
        .file_stem()
        .expect("nome do exemplo");
    let run = Command::new(out_dir.join(nome))
        .output()
        .expect("falha ao executar binário nativo");
    assert!(run.status.success(), "binário nativo falhou");
    let stdout = String::from_utf8_lossy(&run.stdout);
    let linhas: Vec<&str> = stdout.lines().collect();
    assert_eq!(linhas, esperado, "stdout nativo divergente");
    assert!(
        run.stderr.is_empty(),
        "stderr nativo deveria ser vazio: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn hr3_nativo_tem_paridade_na_imutabilidade_de_agregado() {
    paridade_nativa(
        "examples/hr3_uniao_agregado_imutavel_valido.pink",
        &["111", "222"],
    );
}

#[test]
fn hr3_nativo_tem_paridade_em_extracoes_independentes() {
    paridade_nativa(
        "examples/hr3_uniao_extracoes_independentes_valido.pink",
        &["7", "7", "42"],
    );
}

#[test]
fn hr3_nativo_preserva_uniao_escalar_e_handle() {
    paridade_nativa("examples/fase248_unioes_estruturais_valido.pink", &["42"]);
}
