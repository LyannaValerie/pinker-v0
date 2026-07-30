mod common;

use pinker_v0::{
    abstract_machine, abstract_machine_validate, backend_s, cfg_ir, cfg_ir_validate, instr_select,
    instr_select_validate, interpreter, ir, ir_validate, semantic,
};

fn lower(
    source: &str,
) -> (
    ir::ProgramIR,
    cfg_ir::ProgramCfgIR,
    instr_select::SelectedProgram,
    abstract_machine::MachineProgram,
) {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let ir = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");
    (ir, cfg, selected, machine)
}

#[test]
fn fase247_sussurro_atravessa_pipeline_e_emite_wrappers_balanceados() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop", "1: nop", "jmp 1b");
            mimo 0;
        }
    "#;
    let (ir, cfg, selected, machine) = lower(source);
    assert!(ir::render_program(&ir).contains("inline_asm"));
    assert!(cfg_ir::render_program(&cfg).contains("inline_asm"));
    assert!(instr_select::render_program(&selected).contains("inline_asm"));
    assert!(abstract_machine::render_program(&machine).contains("inline_asm"));

    let asm = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");
    assert_eq!(asm.matches(".intel_syntax noprefix").count(), 1);
    assert_eq!(asm.matches(".att_syntax prefix").count(), 1);
    assert!(asm.contains("1: nop"));
    assert!(asm.contains("jmp 1b"));
}

#[test]
fn fase247_interpretador_rejeita_execucao_sem_noop() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop");
            mimo 0;
        }
    "#;
    let (_, _, _, machine) = lower(source);
    let error = interpreter::run_program(&machine)
        .expect_err("sussurro não executa no interpretador")
        .to_string();
    assert!(error.contains("E-RUNTIME-SUSSURRO-NATIVO"), "{error}");
}

#[test]
fn fase247_rejeita_diretiva_que_altera_secao() {
    let ast = common::parse(include_str!(
        "../examples/fase247_sussurro_diretiva_invalido.pink"
    ))
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("diretiva deve falhar")
        .to_string();
    assert!(error.contains(".section"), "{error}");
}

#[test]
fn fase248_rejeita_encaixe_inexaustivo() {
    let ast = common::parse(include_str!(
        "../examples/fase248_uniao_inexaustiva_invalido.pink"
    ))
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("encaixe inexaustivo deve falhar")
        .to_string();
    assert!(error.contains("exaustivo"), "{error}");
}

#[test]
fn fase248_registry_estrutural_e_preservado_em_todas_as_camadas() {
    let source = include_str!("../examples/fase248_unioes_estruturais_valido.pink");
    let (ir, cfg, selected, machine) = lower(source);
    assert_eq!(ir.union_types.len(), 1);
    let union = &ir.union_types[0];
    assert_eq!(union.id.0, 0);
    assert_eq!(union.members.len(), 2);
    assert_eq!(union.members[0].tag, 0);
    assert_eq!(union.members[1].tag, 1);
    assert_eq!(cfg.union_types, ir.union_types);
    assert_eq!(selected.union_types, ir.union_types);
    assert_eq!(machine.union_types, ir.union_types);

    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar união");
    assert_eq!(outcome.exit_status, Some(0));
}

#[test]
fn fase248_ordem_textual_produz_mesma_identidade() {
    let source = r#"
        pacote main;
        carinho aceitar(a: uniao<u8, verso>) -> uniao<verso, u8> { mimo a; }
        carinho principal() -> bombom {
            nova x: uniao<verso, u8> = (7 virar u8) virar uniao<u8, verso>;
            nova y: uniao<u8, verso> = aceitar(x);
            encaixe y {
                caso verso(t) { falar(t); }
                caso u8(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let (ir, _, _, machine) = lower(source);
    assert_eq!(ir.union_types.len(), 1);
    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar");
    assert_eq!(outcome.exit_status, Some(0));
}

#[test]
fn fase248_rejeita_menos_de_dois_membros_canonicos() {
    let ast = common::parse(
        r#"pacote main; carinho principal() -> bombom {
            nova x: uniao<u8, u8> = 1;
            mimo 0;
        }"#,
    )
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("duplicata canonical deve colapsar")
        .to_string();
    assert!(error.contains("dois membros"), "{error}");
}

#[test]
fn fase248_rejeita_operacoes_sem_contrato_observavel() {
    let cases = [
        (
            r#"pacote main; carinho principal() -> bombom {
                nova x: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
                nova y: logica = x == x;
                mimo 0;
            }"#,
            "igualdade e desigualdade de união",
        ),
        (
            r#"pacote main; carinho principal() -> bombom {
                nova x: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
                falar(x);
                mimo 0;
            }"#,
            "'falar' não suporta tipo 'uniao'",
        ),
        (
            r#"pacote main; carinho principal() -> bombom {
                nova x: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
                nova bruto: u64 = x virar u64;
                mimo bruto;
            }"#,
            "downcast de união fora de 'encaixe'",
        ),
    ];
    for (source, expected) in cases {
        let ast = common::parse(source).expect("parse");
        let error = semantic::check_program(&ast)
            .expect_err("operação sobre união deve falhar")
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
}

// ---------------------------------------------------------------------------
// Correções da revisão humana da PR #411 — HR2 (envelope estrutural de
// `sussurro`) e HR5 (namespace interno reservado ao compilador).
// ---------------------------------------------------------------------------

fn sussurro_source(chunk: &str) -> String {
    format!(
        "pacote main;\ncarinho principal() -> bombom {{\n    sussurro({chunk});\n    mimo 0;\n}}\n"
    )
}

fn sussurro_error(chunk: &str) -> String {
    let ast = common::parse(&sussurro_source(chunk)).expect("parse");
    semantic::check_program(&ast)
        .expect_err("política de 'sussurro' deve recusar")
        .to_string()
}

fn sussurro_aceita(chunk: &str) {
    let ast = common::parse(&sussurro_source(chunk)).expect("parse");
    semantic::check_program(&ast).unwrap_or_else(|error| {
        panic!("'sussurro' deveria aceitar {chunk}: {error}");
    });
}

#[test]
fn hr2_rejeita_diretiva_depois_de_separador_de_statement() {
    // O bypass da revisão humana: o `;` é um separador de statement do
    // assembler, logo a diretiva começa um statement novo.
    for chunk in [
        r#""nop; .section .data""#,
        r#""nop; .att_syntax prefix""#,
        r#""nop; .pushsection .data""#,
        r#""nop; .popsection""#,
        r#""nop; .previous""#,
        r#""nop; .subsection 1""#,
        r#""nop; .macro nome""#,
        r#""nop; .endm""#,
        r#""nop; .rept 2""#,
        r#""nop; .irp x,1""#,
        r#""nop; .irpc x,ab""#,
        r#""nop; .set nome,1""#,
        r#""nop; .equ nome,1""#,
        r#""nop; .comm nome,8""#,
        r#""nop; .lcomm nome,8""#,
        r#""nop; .code32""#,
        r#""nop; .code64""#,
        r#""nop; .symver nome,nome@VERSAO""#,
        r#""nop; .include \"arquivo\"""#,
        r#""nop; nop; .data""#,
    ] {
        let error = sussurro_error(chunk);
        assert!(
            error.contains("E-SEMANTIC-ASM-DIRECTIVE"),
            "{chunk} => {error}"
        );
    }
}

#[test]
fn hr2_rejeita_diretiva_depois_de_label() {
    // Um label antes da diretiva não muda o fato de que o statement começa
    // com `.` depois da remoção do label.
    for chunk in [
        r#""rotulo: .section .data""#,
        r#""1: .section .data""#,
        r#""1: .macro m""#,
    ] {
        let error = sussurro_error(chunk);
        assert!(
            error.contains("E-SEMANTIC-ASM-DIRECTIVE")
                || error.contains("E-SEMANTIC-ASM-NAMED-LABEL"),
            "{chunk} => {error}"
        );
    }
}

#[test]
fn hr2_rejeita_diretiva_desconhecida_sem_blacklist() {
    // A completude não vem de uma lista de nomes: uma diretiva inventada,
    // que nenhuma blacklist conteria, é recusada pela mesma regra estrutural.
    let error = sussurro_error(r#""nop; .diretiva_que_nao_existe 1,2,3""#);
    assert!(error.contains("E-SEMANTIC-ASM-DIRECTIVE"), "{error}");
    let error = sussurro_error(r#"".DIRETIVA_MAIUSCULA""#);
    assert!(error.contains("E-SEMANTIC-ASM-DIRECTIVE"), "{error}");
}

#[test]
fn hr2_rejeita_label_nominal_e_aceita_label_numerico() {
    for chunk in [r#""nome:""#, r#""nome: nop""#, r#""_local: nop""#] {
        let error = sussurro_error(chunk);
        assert!(
            error.contains("E-SEMANTIC-ASM-NAMED-LABEL"),
            "{chunk} => {error}"
        );
    }
    // `.Lnome:` começa com `.`, portanto é recusado como diretiva.
    let error = sussurro_error(r#"".Lnome:""#);
    assert!(error.contains("E-SEMANTIC-ASM-DIRECTIVE"), "{error}");

    for chunk in [
        r#""1:""#,
        r#""1: nop""#,
        r#""jne 1b""#,
        r#""jmp 2f", "2: nop""#,
    ] {
        sussurro_aceita(chunk);
    }
}

#[test]
fn hr2_trata_comentarios_quotes_e_continuations() {
    // Comentário: o conteúdo comentado não é interpretado, então uma diretiva
    // comentada não é uma diretiva — mas também não pode esconder um `;`.
    sussurro_aceita(r#""nop # .section .data""#);
    sussurro_aceita(r#""nop /* .section .data */""#);
    sussurro_aceita(r#""nop /* comentario */ ; nop""#);
    // Continuação reconhecida. Uma string literal Pinker não quebra linha, então
    // o caso multi-linha é exercido direto no scanner, que é a fronteira real.
    let statements = pinker_v0::inline_asm::scan_chunk("nop \\\n    nop").expect("continuação");
    assert_eq!(statements.len(), 1, "{statements:?}");
    assert_eq!(statements[0].mnemonic.as_deref(), Some("nop"));
    assert_eq!(statements[0].operands, "nop");
    // Sem a continuação, as duas linhas são dois statements.
    let statements = pinker_v0::inline_asm::scan_chunk("nop\n    nop").expect("duas linhas");
    assert_eq!(statements.len(), 2, "{statements:?}");
    // Uma diretiva depois de uma continuação continua sendo diretiva.
    let error = pinker_v0::inline_asm::scan_chunk("nop \\\n    ; .section .data")
        .expect_err("diretiva após continuação");
    assert_eq!(error.code, pinker_v0::inline_asm::E_ASM_DIRECTIVE);
    // Segment override em operando não é label.
    sussurro_aceita(r#""mov rax, fs:[0]""#);

    // Um comentário de bloco não terminado não pode virar aceitação.
    let error = sussurro_error(r#""nop /* sem fim""#);
    assert!(
        error.contains("E-SEMANTIC-ASM-UNTERMINATED-COMMENT"),
        "{error}"
    );
    // Uma região citada não terminada tampouco.
    let error = sussurro_error(r#""mov rax, \"aberto""#);
    assert!(
        error.contains("E-SEMANTIC-ASM-UNTERMINATED-QUOTE"),
        "{error}"
    );
    // Um separador dentro de região citada faria o scanner e o assembler
    // discordarem sobre o fim do statement; a divergência é recusada.
    let error = sussurro_error(r#""nop \"; .section .data\"""#);
    assert!(
        error.contains("E-SEMANTIC-ASM-SEPARATOR-IN-QUOTE"),
        "{error}"
    );
}

#[test]
fn hr2_envelope_do_backend_e_balanceado_e_validado() {
    use pinker_v0::inline_asm;

    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop", "1: nop", "jmp 1b");
            mimo 0;
        }
    "#;
    let (_, _, selected, _) = lower(source);
    let asm = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");

    // As sentinelas são geradas pelo compilador, não vêm da fonte.
    assert_eq!(asm.matches(inline_asm::SENTINEL_BEGIN_PREFIX).count(), 1);
    assert_eq!(asm.matches(inline_asm::SENTINEL_END_PREFIX).count(), 1);
    assert_eq!(asm.matches(inline_asm::INTEL_SYNTAX_WRAPPER).count(), 1);
    assert_eq!(asm.matches(inline_asm::ATT_SYNTAX_WRAPPER).count(), 1);

    let envelopes = inline_asm::validate_envelopes(&asm).expect("envelope válido");
    assert_eq!(envelopes.len(), 1);
    let envelope = &envelopes[0];
    assert!(envelope.id.starts_with("principal#"));
    assert!(envelope.source_lines.iter().any(|line| line == "1: nop"));
    assert!(envelope.source_lines.iter().any(|line| line == "jmp 1b"));

    // Nenhum texto da fonte aparece fora do envelope.
    let fora: Vec<&str> = asm
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed == "1: nop" || trimmed == "jmp 1b"
        })
        .collect();
    assert_eq!(fora.len(), 2, "texto da fonte apenas dentro do envelope");
}

#[test]
fn hr2_envelope_desbalanceado_e_recusado() {
    use pinker_v0::inline_asm;

    let begin = inline_asm::SENTINEL_BEGIN_PREFIX;
    let end = inline_asm::SENTINEL_END_PREFIX;
    let intel = inline_asm::INTEL_SYNTAX_WRAPPER;
    let att = inline_asm::ATT_SYNTAX_WRAPPER;

    // Begin sem end.
    let error = inline_asm::validate_envelopes(&format!("{begin}a\n{intel}\nnop\n{att}\n"))
        .expect_err("begin sem end");
    assert_eq!(error.code, inline_asm::E_ASM_ENVELOPE);

    // End sem begin.
    let error = inline_asm::validate_envelopes(&format!("{end}a\n")).expect_err("end sem begin");
    assert_eq!(error.code, inline_asm::E_ASM_ENVELOPE);

    // Identificadores trocados.
    let error = inline_asm::validate_envelopes(&format!("{begin}a\n{intel}\nnop\n{att}\n{end}b\n"))
        .expect_err("ids divergentes");
    assert_eq!(error.code, inline_asm::E_ASM_ENVELOPE);

    // Wrapper AT&T removido: a sintaxe não é restaurada.
    let error = inline_asm::validate_envelopes(&format!("{begin}a\n{intel}\nnop\n{end}a\n"))
        .expect_err("att não restaurado");
    assert_eq!(error.code, inline_asm::E_ASM_ENVELOPE);

    // Envelope duplicado.
    let bloco = format!("{begin}a\n{intel}\nnop\n{att}\n{end}a\n");
    let error =
        inline_asm::validate_envelopes(&format!("{bloco}{bloco}")).expect_err("envelope duplicado");
    assert_eq!(error.code, inline_asm::E_ASM_ENVELOPE);

    // Um envelope bem formado passa.
    let envelopes = inline_asm::validate_envelopes(&bloco).expect("envelope válido");
    assert_eq!(envelopes.len(), 1);
}

#[test]
fn hr5_rejeita_namespace_reservado_em_toda_declaracao_e_referencia() {
    let casos: [(&str, &str); 10] = [
        (
            "funcao",
            "pacote main;\ncarinho __pinker_internal_f() -> bombom { mimo 1; }\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "variavel",
            "pacote main;\ncarinho principal() -> bombom { nova __pinker_internal_v: bombom = 1; mimo 0; }",
        ),
        (
            "parametro",
            "pacote main;\ncarinho f(__pinker_internal_p: bombom) -> bombom { mimo 1; }\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "apelido",
            "pacote main;\napelido __pinker_internal_a = u8;\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "ninho",
            "pacote main;\nninho __pinker_internal_n { x: bombom; }\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "campo",
            "pacote main;\nninho N { __pinker_internal_c: bombom; }\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "leque",
            "pacote main;\nleque __pinker_internal_l { A, B }\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "trato",
            "pacote main;\ntrato __pinker_internal_t { carinho m(self) -> bombom; }\ncarinho principal() -> bombom { mimo 0; }",
        ),
        (
            "referencia",
            "pacote main;\ncarinho principal() -> bombom { falar(__pinker_internal_x); mimo 0; }",
        ),
        (
            "chamada-intrinseca",
            "pacote main;\ncarinho principal() -> bombom {\n    nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;\n    falar(__pinker_internal_uniao_tag(valor));\n    mimo 0;\n}",
        ),
    ];

    for (categoria, source) in casos {
        let error = common::parse_and_check(source)
            .expect_err(&format!("namespace reservado deve falhar em {categoria}"))
            .to_string();
        assert!(
            error.contains("E-SEMANTIC-RESERVED-NAMESPACE"),
            "{categoria} => {error}"
        );
    }
}

#[test]
fn hr5_intrinsecas_sinteticas_do_compilador_seguem_funcionando() {
    // A fronteira vale para identificadores originados da fonte. Os
    // identificadores sintéticos do desugaring não são lexados e por isso
    // continuam válidos — `encaixe` de união segue funcionando.
    let source = include_str!("../examples/fase248_unioes_estruturais_valido.pink");
    let (_, _, _, machine) = lower(source);
    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar união");
    assert_eq!(outcome.exit_status, Some(0));
}

/// Ferramentas de inspeção estrutural do objeto.
///
/// A ausência é fatal sob `PINKER_EXIGE_NATIVO=1`: uma evidência indispensável
/// não pode ser pulada em silêncio.
fn require_object_inspection_tools(test: &str) -> Option<(&'static str, &'static str)> {
    fn probe(tool: &str) -> bool {
        std::process::Command::new(tool)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    let missing = if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("unsupported_platform")
    } else if !probe("as") {
        Some("as_not_found")
    } else if !probe("readelf") {
        Some("readelf_not_found")
    } else {
        None
    };

    if let Some(reason) = missing {
        eprintln!(
            "{{\"event\":\"object_inspection\",\"reason\":\"{reason}\",\"status\":\"unavailable\",\"test\":\"{test}\"}}"
        );
        assert_ne!(
            std::env::var("PINKER_EXIGE_NATIVO").as_deref(),
            Ok("1"),
            "inspeção estrutural do objeto é indispensável: {reason} ({test})"
        );
        return None;
    }
    Some(("as", "readelf"))
}

fn assemble_and_inspect(
    assembler: &str,
    reader: &str,
    label: &str,
    asm: &str,
) -> (Vec<String>, Vec<String>) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("pinker_hr2_{label}_{nanos}"));
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    let source = dir.join("bloco.s");
    let object = dir.join("bloco.o");
    std::fs::write(&source, asm).expect("escrever .s");

    let assembled = std::process::Command::new(assembler)
        .arg("-o")
        .arg(&object)
        .arg(&source)
        .output()
        .expect("invocar assembler");
    assert!(
        assembled.status.success(),
        "assembler recusou o envelope de '{label}': {}",
        String::from_utf8_lossy(&assembled.stderr)
    );

    let sections = std::process::Command::new(reader)
        .arg("-SW")
        .arg(&object)
        .output()
        .expect("invocar leitor de seções");
    let mut section_names: Vec<String> = String::from_utf8_lossy(&sections.stdout)
        .lines()
        .filter_map(|line| line.split_once("] "))
        .filter_map(|(_, tail)| tail.split_whitespace().next())
        .filter(|name| name.starts_with('.'))
        .map(str::to_string)
        .collect();
    section_names.sort();
    section_names.dedup();

    let symbols = std::process::Command::new(reader)
        .arg("-sW")
        .arg(&object)
        .output()
        .expect("invocar leitor de símbolos");
    let mut symbol_names: Vec<String> = String::from_utf8_lossy(&symbols.stdout)
        .lines()
        .skip(3)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            // Índice de seção `UND` significa símbolo apenas referenciado.
            if fields.len() < 8 || fields[6] == "UND" {
                return None;
            }
            let name = fields[7];
            (!name.starts_with('.')).then(|| name.to_string())
        })
        .collect();
    symbol_names.sort();
    symbol_names.dedup();

    let _ = std::fs::remove_dir_all(&dir);
    (section_names, symbol_names)
}

#[test]
fn hr2_objeto_montado_nao_recebe_secao_nem_simbolo_do_bloco() {
    let Some((assembler, reader)) =
        require_object_inspection_tools(concat!(module_path!(), ":", line!()))
    else {
        return;
    };

    let com_sussurro = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop", "1: nop", "jne 1b");
            mimo 0;
        }
    "#;
    let sem_sussurro = r#"
        pacote main;
        carinho principal() -> bombom {
            mimo 0;
        }
    "#;

    let (_, _, selected_com, _) = lower(com_sussurro);
    let asm_com =
        backend_s::emit_external_toolchain_subset_nativo(&selected_com).expect("assembly");
    let (_, _, selected_sem, _) = lower(sem_sussurro);
    let asm_sem =
        backend_s::emit_external_toolchain_subset_nativo(&selected_sem).expect("assembly");

    // O envelope precisa estar íntegro antes de montar.
    let envelopes = pinker_v0::inline_asm::validate_envelopes(&asm_com).expect("envelope válido");
    assert_eq!(envelopes.len(), 1);
    assert!(pinker_v0::inline_asm::validate_envelopes(&asm_sem)
        .expect("sem envelope")
        .is_empty());

    let (secoes_com, simbolos_com) = assemble_and_inspect(assembler, reader, "com", &asm_com);
    let (secoes_sem, simbolos_sem) = assemble_and_inspect(assembler, reader, "sem", &asm_sem);

    // O bloco de `sussurro` não criou seção nem símbolo nomeado adicional.
    assert_eq!(
        secoes_com, secoes_sem,
        "bloco de 'sussurro' alterou o conjunto de seções"
    );
    assert_eq!(
        simbolos_com, simbolos_sem,
        "bloco de 'sussurro' alterou o conjunto de símbolos"
    );
    assert!(
        !secoes_com.iter().any(|name| name == ".rodata.sussurro"),
        "{secoes_com:?}"
    );
}

#[test]
fn hr2_build_do_envelope_e_deterministico() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            sussurro("nop", "1: nop", "jne 1b");
            mimo 0;
        }
    "#;
    let (_, _, selected, _) = lower(source);
    let primeiro = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");
    let segundo = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");
    assert_eq!(
        primeiro, segundo,
        "emissão do envelope deve ser determinística"
    );
}
