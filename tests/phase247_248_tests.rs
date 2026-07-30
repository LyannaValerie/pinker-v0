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
    // A fronteira vale para identificadores originados da fonte. Depois de HR1
    // o `encaixe` de união não fabrica identificador algum: tag e extração são
    // operações internas tipadas da IR, não chamadas de função.
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

// ---------------------------------------------------------------------------
// Correções da revisão humana da PR #411 — HR1 (`encaixe` de união tipado, com
// tags exclusivamente do registry) e a parcela residual de HR5 (operações
// internas tipadas, sem chamadas fabricadas na AST/IR).
// ---------------------------------------------------------------------------

const HR1_ALIAS_INVERSAO: &str = r#"
    pacote main;
    apelido aa = u8;
    apelido zz = u64;
    carinho principal() -> bombom {
        nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
        encaixe valor {
            caso aa(numero) { falar(1000 + (numero virar bombom)); }
            caso zz(numero) { falar(2000 + (numero virar bombom)); }
        }
        mimo 0;
    }
"#;

/// Nomes das antigas intrínsecas de união. Nenhuma delas pode reaparecer como
/// chamada comum na AST ou na IR estruturada.
const HR1_INTRINSECAS_PROIBIDAS: [&str; 3] = [
    "__pinker_internal_uniao_tag",
    "__pinker_internal_uniao_payload_b",
    "__pinker_internal_uniao_payload_v",
];

fn hr1_union_match_stmts(
    program: &pinker_v0::ast::Program,
) -> Vec<&pinker_v0::ast::UnionMatchStmt> {
    fn scan<'a>(
        block: &'a pinker_v0::ast::Block,
        out: &mut Vec<&'a pinker_v0::ast::UnionMatchStmt>,
    ) {
        use pinker_v0::ast::{ElseBlock, Stmt};
        for stmt in &block.stmts {
            match stmt {
                Stmt::UnionMatch(union_match) => {
                    out.push(union_match);
                    for arm in &union_match.arms {
                        scan(&arm.body, out);
                    }
                }
                Stmt::If(if_stmt) => {
                    scan(&if_stmt.then_branch, out);
                    let mut branch = if_stmt.else_branch.as_ref();
                    while let Some(else_branch) = branch {
                        match else_branch {
                            ElseBlock::Block(block) => {
                                scan(block, out);
                                branch = None;
                            }
                            ElseBlock::If(inner) => {
                                scan(&inner.then_branch, out);
                                branch = inner.else_branch.as_ref();
                            }
                        }
                    }
                }
                Stmt::While(while_stmt) => scan(&while_stmt.body, out),
                _ => {}
            }
        }
    }

    let mut out = Vec::new();
    for item in &program.items {
        if let pinker_v0::ast::Item::Function(function) = item {
            scan(&function.body, &mut out);
        }
    }
    out
}

fn hr1_interpreta(source: &str) -> Option<i32> {
    let (_, _, _, machine) = lower(source);
    interpreter::run_program_with_args(&machine, &[])
        .expect("interpretar encaixe de união")
        .exit_status
}

fn hr1_erro_semantico(source: &str) -> String {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast)
        .expect_err("encaixe inválido deve falhar na semântica")
        .to_string()
}

fn hr1_registry_tags(source: &str) -> Vec<(u64, String)> {
    let (ir, _, _, _) = lower(source);
    assert_eq!(
        ir.union_types.len(),
        1,
        "esperava uma única união internada"
    );
    ir.union_types[0]
        .members
        .iter()
        .map(|member| (member.tag, member.canonical_member_key.clone()))
        .collect()
}

fn hr1_arm_tags(program: &ir::ProgramIR) -> Vec<(u64, String)> {
    fn scan(block: &ir::BlockIR, out: &mut Vec<(u64, String)>) {
        for instruction in &block.instructions {
            match instruction {
                ir::InstructionIR::UnionMatch(union_match) => {
                    for arm in &union_match.arms {
                        out.push((arm.tag, arm.canonical_member_key.clone()));
                        scan(&arm.body, out);
                    }
                }
                ir::InstructionIR::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    scan(then_block, out);
                    if let Some(else_block) = else_block {
                        scan(else_block, out);
                    }
                }
                ir::InstructionIR::While { body_block, .. } => scan(body_block, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for function in &program.functions {
        scan(&function.entry, &mut out);
    }
    out
}

#[test]
fn hr1_parser_preserva_encaixe_como_no_proprio_sem_calcular_tags() {
    let program = common::parse(HR1_ALIAS_INVERSAO).expect("parse");
    let matches = hr1_union_match_stmts(&program);
    assert_eq!(matches.len(), 1, "esperava um `Stmt::UnionMatch`");
    let union_match = matches[0];

    // Ordem de fonte preservada, tipos textuais preservados, bindings e corpos
    // preservados — e nenhuma tag, porque o parser não conhece tags.
    assert_eq!(union_match.arms.len(), 2);
    // O tipo do braço é o tipo **como escrito**: o apelido cru, ainda não
    // resolvido. A resolução é da semântica.
    let escritos: Vec<&str> = union_match
        .arms
        .iter()
        .map(|arm| match &arm.member_type {
            pinker_v0::ast::Type::Alias { name, .. } => name.as_str(),
            outro => panic!("esperava apelido preservado, recebi {}", outro.name()),
        })
        .collect();
    assert_eq!(escritos, vec!["aa", "zz"]);
    assert_eq!(union_match.arms[0].binding, "numero");
    assert_eq!(union_match.arms[1].binding, "numero");
    assert!(union_match.span.start.line > 0);
    for arm in &union_match.arms {
        assert!(arm.span.start.line > 0, "span do braço preservado");
        assert!(!arm.body.stmts.is_empty(), "corpo do braço preservado");
    }

    // O parser não desdobra o construto em `talvez` aninhado nem sintetiza
    // âncoras: o corpo da função tem exatamente `nova`, `encaixe` e `mimo`.
    let principal = program
        .items
        .iter()
        .find_map(|item| match item {
            pinker_v0::ast::Item::Function(function) if function.name == "principal" => {
                Some(function)
            }
            _ => None,
        })
        .expect("função principal");
    assert_eq!(principal.body.stmts.len(), 3);
    assert!(matches!(
        principal.body.stmts[1],
        pinker_v0::ast::Stmt::UnionMatch(_)
    ));
    assert!(
        !principal
            .body
            .stmts
            .iter()
            .any(|stmt| matches!(stmt, pinker_v0::ast::Stmt::If(_))),
        "o parser não gera `talvez` para `encaixe` de união"
    );
}

#[test]
fn hr1_ast_json_expoe_no_proprio_sem_nomes_internos() {
    let json = common::render_json_ast(HR1_ALIAS_INVERSAO).expect("json ast");
    assert!(json.contains("\"node\": \"UnionMatchStmt\""), "{json}");
    assert!(json.contains("\"node\": \"UnionMatchArm\""), "{json}");
    assert!(json.contains("\"binding\": \"numero\""), "{json}");
    assert!(json.contains("\"member_type\""), "{json}");
    assert!(json.contains("\"scrutinee\""), "{json}");
    for nome in HR1_INTRINSECAS_PROIBIDAS {
        assert!(!json.contains(nome), "AST JSON não pode conter {nome}");
    }
    // Nenhuma tag literal fabricada pelo parser: o desugaring antigo emitia
    // `IntLit` de tag em cada braço.
    assert!(
        !json.contains("__encaixe_uniao_"),
        "AST JSON não pode conter âncora sintética: {json}"
    );
}

#[test]
fn hr1_reproducao_original_executa_o_braco_correto() {
    // A reprodução da revisão humana: `aa` é lexicalmente anterior mas
    // canonicamente posterior (`u64` < `u8` em ordem de chave).
    let (ir_program, _, _, machine) = lower(HR1_ALIAS_INVERSAO);
    let registry = &ir_program.union_types[0];
    assert_eq!(registry.members[0].canonical_member_key, "u64");
    assert_eq!(registry.members[1].canonical_member_key, "u8");

    // O braço `aa` (u8) recebe a tag 1 do registry, não a tag 0 da ordenação
    // lexical do apelido.
    let arms = hr1_arm_tags(&ir_program);
    assert_eq!(
        arms,
        vec![(1, "u8".to_string()), (0, "u64".to_string())],
        "tags dos braços devem vir do registry"
    );

    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar");
    assert_eq!(outcome.exit_status, Some(0));
}

#[test]
fn hr1_stdout_interpretado_seleciona_o_braco_do_apelido_escrito() {
    let pink = env!("CARGO_BIN_EXE_pink");
    let run = std::process::Command::new(pink)
        .arg("--run")
        .arg("examples/hr1_encaixe_uniao_apelidos_valido.pink")
        .output()
        .expect("invocar pink --run");
    assert!(
        run.status.success(),
        "execução interpretada falhou: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "1007",
        "o braço `aa` (u8) deve executar; stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn hr1_cada_membro_seleciona_o_seu_proprio_braco() {
    // Os dois sentidos da mesma união: nenhum braço é privilegiado por posição
    // e nenhuma tag é assumida constante.
    let pink = env!("CARGO_BIN_EXE_pink");
    for (exemplo, esperado) in [
        ("examples/hr1_encaixe_uniao_apelidos_valido.pink", "1007"),
        (
            "examples/hr1_encaixe_uniao_segundo_membro_valido.pink",
            "2008",
        ),
    ] {
        let run = std::process::Command::new(pink)
            .arg("--run")
            .arg(exemplo)
            .output()
            .expect("invocar pink --run");
        assert!(
            run.status.success(),
            "execução de {exemplo} falhou: {}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&run.stdout).trim(),
            esperado,
            "{exemplo} deve executar o braço do membro injetado; stderr={}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
}

#[test]
fn hr1_ordem_de_apelidos_membros_e_bracos_nao_altera_tags() {
    let esperado = vec![(0, "u64".to_string()), (1, "u8".to_string())];

    // Ordem textual da união invertida.
    let uniao_invertida = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho principal() -> bombom {
            nova valor: uniao<zz, aa> = (7 virar aa) virar uniao<zz, aa>;
            encaixe valor {
                caso aa(numero) { falar(1000 + (numero virar bombom)); }
                caso zz(numero) { falar(2000 + (numero virar bombom)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_registry_tags(uniao_invertida), esperado);

    // Ordem dos braços invertida.
    let bracos_invertidos = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho principal() -> bombom {
            nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
            encaixe valor {
                caso zz(numero) { falar(2000 + (numero virar bombom)); }
                caso aa(numero) { falar(1000 + (numero virar bombom)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_registry_tags(bracos_invertidos), esperado);

    // Sem apelido nenhum: mesma tabela.
    let sem_apelidos = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(numero) { falar(1000 + (numero virar bombom)); }
                caso u64(numero) { falar(2000 + (numero virar bombom)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_registry_tags(sem_apelidos), esperado);

    // Em todos os casos o braço executado é o do tipo escrito.
    for source in [uniao_invertida, bracos_invertidos, sem_apelidos] {
        assert_eq!(hr1_interpreta(source), Some(0));
    }
}

#[test]
fn hr1_cadeia_de_apelidos_e_apelido_de_apelido_resolvem_ao_tipo_canonico() {
    let cadeia = r#"
        pacote main;
        apelido base_estreito = u8;
        apelido aa = base_estreito;
        apelido base_largo = u64;
        apelido zz = base_largo;
        carinho principal() -> bombom {
            nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
            encaixe valor {
                caso aa(numero) { falar(1000 + (numero virar bombom)); }
                caso zz(numero) { falar(2000 + (numero virar bombom)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(
        hr1_registry_tags(cadeia),
        vec![(0, "u64".to_string()), (1, "u8".to_string())]
    );
    assert_eq!(hr1_interpreta(cadeia), Some(0));
}

#[test]
fn hr1_apelido_de_verso_seleciona_a_extracao_pelo_tipo_resolvido() {
    let source = r#"
        pacote main;
        apelido texto_curto = verso;
        apelido byte = u8;
        carinho principal() -> bombom {
            nova valor: uniao<texto_curto, byte> =
                (9 virar byte) virar uniao<texto_curto, byte>;
            encaixe valor {
                caso texto_curto(t) { falar(t); }
                caso byte(n) { falar(2000 + (n virar bombom)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(
        hr1_registry_tags(source),
        vec![(0, "u8".to_string()), (1, "verso".to_string())]
    );
    assert_eq!(hr1_interpreta(source), Some(0));
}

#[test]
fn hr1_apelido_de_leque_resolve_ao_membro_nominal() {
    let source = r#"
        pacote main;
        leque Cor { Rosa, Azul }
        apelido paleta = Cor;
        carinho principal() -> bombom {
            nova valor: uniao<paleta, verso> =
                Cor.Rosa virar uniao<paleta, verso>;
            encaixe valor {
                caso paleta(c) { falar(1); }
                caso verso(t) { falar(t); }
            }
            mimo 0;
        }
    "#;
    let tags = hr1_registry_tags(source);
    assert!(
        tags.iter().any(|(_, key)| key == "enum:3:Cor"),
        "chave nominal do leque preservada: {tags:?}"
    );
    assert_eq!(hr1_interpreta(source), Some(0));
}

#[test]
fn hr1_dois_apelidos_equivalentes_sao_o_mesmo_membro_canonico() {
    let source = r#"
        pacote main;
        apelido byte_a = u8;
        apelido byte_b = u8;
        carinho principal() -> bombom {
            nova valor: uniao<byte_a, u64> = (7 virar byte_a) virar uniao<byte_a, u64>;
            encaixe valor {
                caso byte_a(a) { falar(1000 + (a virar bombom)); }
                caso byte_b(b) { falar(2000 + (b virar bombom)); }
            }
            mimo 0;
        }
    "#;
    let error = hr1_erro_semantico(source);
    assert!(error.contains("repetido"), "{error}");
    assert!(error.contains("apelidos"), "{error}");
}

#[test]
fn hr1_cobertura_canonica_e_exigida_apos_a_resolucao() {
    // Membro ausente.
    let ausente = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64, verso> =
                (7 virar u8) virar uniao<u8, u64, verso>;
            encaixe valor {
                caso u8(n) { falar(n); }
                caso u64(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let error = hr1_erro_semantico(ausente);
    assert!(error.contains("exaustivo"), "{error}");
    assert!(error.contains("verso"), "ausência nomeada: {error}");

    // Membro externo à união.
    let externo = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(n) { falar(n); }
                caso verso(t) { falar(t); }
            }
            mimo 0;
        }
    "#;
    let error = hr1_erro_semantico(externo);
    assert!(error.contains("não é membro da união"), "{error}");

    // Duplicata textual.
    let duplicata = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(a) { falar(a); }
                caso u8(b) { falar(b); }
            }
            mimo 0;
        }
    "#;
    let error = hr1_erro_semantico(duplicata);
    assert!(error.contains("repetido"), "{error}");

    // Scrutinee que não é união.
    let nao_uniao = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: bombom = 7;
            encaixe valor {
                caso u8(n) { falar(n); }
                caso u64(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let error = hr1_erro_semantico(nao_uniao);
    assert!(error.contains("scrutinee de união estrutural"), "{error}");
}

#[test]
fn hr1_uniao_aninhada_e_achatada_antes_da_cobertura() {
    let source = r#"
        pacote main;
        apelido par = uniao<u8, u64>;
        carinho principal() -> bombom {
            nova valor: uniao<par, verso> = (7 virar u8) virar uniao<par, verso>;
            encaixe valor {
                caso u8(a) { falar(1000 + (a virar bombom)); }
                caso u64(b) { falar(2000 + (b virar bombom)); }
                caso verso(t) { falar(t); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(
        hr1_registry_tags(source),
        vec![
            (0, "u64".to_string()),
            (1, "u8".to_string()),
            (2, "verso".to_string()),
        ]
    );
    assert_eq!(hr1_interpreta(source), Some(0));
}

#[test]
fn hr1_senao_continua_recusado_no_encaixe_de_uniao() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(n) { falar(n); }
                senao { falar(0); }
            }
            mimo 0;
        }
    "#;
    let error = common::parse(source)
        .expect_err("'senao' deve ser recusado")
        .to_string();
    assert!(error.contains("'senao' não substitui"), "{error}");
}

#[test]
fn hr1_um_unico_braco_continua_recusado() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let error = common::parse(source)
        .expect_err("um braço deve ser recusado")
        .to_string();
    assert!(error.contains("ao menos dois membros"), "{error}");
}

#[test]
fn hr1_binding_escopo_retorno_e_aninhamentos() {
    // Retorno dentro do braço, `talvez` aninhado e `encaixe` aninhado.
    let source = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho escolher(valor: uniao<aa, zz>) -> bombom {
            encaixe valor {
                caso aa(numero) {
                    talvez numero > (3 virar aa) {
                        mimo 1000 + (numero virar bombom);
                    }
                    mimo 1;
                }
                caso zz(numero) {
                    nova interno: uniao<aa, zz> = (1 virar aa) virar uniao<aa, zz>;
                    encaixe interno {
                        caso aa(x) { mimo 2000 + (x virar bombom); }
                        caso zz(y) { mimo 3000 + (y virar bombom); }
                    }
                    mimo 2;
                }
            }
            mimo 0;
        }
        carinho principal() -> bombom {
            nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
            nova resultado: bombom = escolher(valor);
            falar(resultado);
            mimo 0;
        }
    "#;
    assert_eq!(hr1_interpreta(source), Some(0));

    // `encaixe` dentro de laço, com `quebrar` no braço.
    let em_laco = r#"
        pacote main;
        carinho principal() -> bombom {
            nova muda i: bombom = 0;
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            sempre que i < 3 {
                encaixe valor {
                    caso u8(n) { falar(1000 + (n virar bombom)); quebrar; }
                    caso u64(n) { falar(2000 + (n virar bombom)); continuar; }
                }
                i = i + 1;
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_interpreta(em_laco), Some(0));
}

#[test]
fn hr1_dois_matches_no_mesmo_bloco_e_em_funcoes_distintas() {
    let source = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho outro() -> bombom {
            nova b: uniao<u8, verso> = (1 virar u8) virar uniao<u8, verso>;
            encaixe b {
                caso u8(n) { falar(n); }
                caso verso(t) { falar(t); }
            }
            mimo 0;
        }
        carinho principal() -> bombom {
            nova primeiro: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
            nova segundo: uniao<aa, zz> = (8 virar aa) virar uniao<aa, zz>;
            encaixe primeiro {
                caso aa(n) { falar(1000 + (n virar bombom)); }
                caso zz(n) { falar(2000 + (n virar bombom)); }
            }
            encaixe segundo {
                caso zz(n) { falar(3000 + (n virar bombom)); }
                caso aa(n) { falar(4000 + (n virar bombom)); }
            }
            nova _ignorado: bombom = outro();
            mimo 0;
        }
    "#;
    let (ir_program, _, _, machine) = lower(source);
    // Duas uniões distintas internadas: <u64,u8> e <u8,verso>.
    assert_eq!(ir_program.union_types.len(), 2);
    let ids: Vec<u32> = ir_program
        .union_types
        .iter()
        .map(|union| union.id.0)
        .collect();
    assert_eq!(ids, vec![0, 1]);
    // Os dois matches do mesmo bloco usam as mesmas tags, na mesma união.
    let arms = hr1_arm_tags(&ir_program);
    assert!(arms.contains(&(1, "u8".to_string())));
    assert!(arms.contains(&(0, "u64".to_string())));
    let outcome = interpreter::run_program_with_args(&machine, &[]).expect("interpretar");
    assert_eq!(outcome.exit_status, Some(0));
}

#[test]
fn hr1_match_em_closure_e_apos_callable() {
    let source = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho identidade(x: bombom) -> bombom { mimo x; }
        carinho principal() -> bombom {
            nova f: carinho(bombom) -> bombom = identidade;
            nova pronto: bombom = f(1);
            nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
            encaixe valor {
                caso aa(n) { falar(pronto + (n virar bombom)); }
                caso zz(n) { falar(2000 + (n virar bombom)); }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_interpreta(source), Some(0));
}

#[test]
fn hr1_registry_carrega_a_mesma_tabela_em_todas_as_camadas() {
    let (ir_program, cfg, selected, machine) = lower(HR1_ALIAS_INVERSAO);
    assert_eq!(cfg.union_types, ir_program.union_types);
    assert_eq!(selected.union_types, ir_program.union_types);
    assert_eq!(machine.union_types, ir_program.union_types);
    for (index, member) in ir_program.union_types[0].members.iter().enumerate() {
        assert_eq!(member.tag, index as u64);
        assert!(!member.canonical_member_key.is_empty());
    }
}

#[test]
fn hr1_validadores_recusam_divergencia_entre_braco_e_registry() {
    let (ir_program, _, _, _) = lower(HR1_ALIAS_INVERSAO);
    let unions = &ir_program.union_types;

    // Tag inexistente.
    let error = ir::validate_union_member_reference(
        unions,
        unions[0].id,
        99,
        &unions[0].members[0].canonical_member_key,
        unions[0].members[0].ty,
        unions[0].members[0].payload_layout,
    )
    .expect_err("tag fora do registry deve falhar");
    assert!(error.contains("não pertence"), "{error}");

    // Chave canônica divergente da tag.
    let error = ir::validate_union_member_reference(
        unions,
        unions[0].id,
        0,
        "chave-inventada",
        unions[0].members[0].ty,
        unions[0].members[0].payload_layout,
    )
    .expect_err("chave divergente deve falhar");
    assert!(error.contains("chave canônica divergente"), "{error}");

    // Layout divergente.
    let error = ir::validate_union_member_reference(
        unions,
        unions[0].id,
        0,
        &unions[0].members[0].canonical_member_key,
        unions[0].members[0].ty,
        pinker_v0::union_payload::UnionPayloadLayout {
            size: unions[0].members[0].payload_layout.size + 1,
            ..unions[0].members[0].payload_layout
        },
    )
    .expect_err("layout divergente deve falhar");
    assert!(error.contains("layout de payload divergente"), "{error}");

    // Registry ausente.
    let error =
        ir::validate_union_reference(&[], unions[0].id).expect_err("registry ausente deve falhar");
    assert!(error.contains("ausente do registro internado"), "{error}");

    // Cobertura incompleta e braço repetido.
    let chave = unions[0].members[0].canonical_member_key.clone();
    let error = ir::validate_union_match_coverage(unions, unions[0].id, &[(0, chave.clone())])
        .expect_err("cobertura incompleta deve falhar");
    assert!(error.contains("cobertura incompleta"), "{error}");
    let error =
        ir::validate_union_match_coverage(unions, unions[0].id, &[(0, chave.clone()), (0, chave)])
            .expect_err("braço repetido deve falhar");
    assert!(error.contains("braço repetido"), "{error}");
}

#[test]
fn hr1_validador_de_registry_recusa_chave_vazia_e_ordem_invertida() {
    let (ir_program, _, _, _) = lower(HR1_ALIAS_INVERSAO);

    let mut sem_chave = ir_program.union_types.clone();
    sem_chave[0].members[0].canonical_member_key.clear();
    let error = ir::validate_union_registry(&sem_chave).expect_err("chave vazia deve falhar");
    assert!(error.contains("sem chave canônica"), "{error}");

    let mut ordem_invertida = ir_program.union_types.clone();
    ordem_invertida[0].members.swap(0, 1);
    ordem_invertida[0].members[0].tag = 0;
    ordem_invertida[0].members[1].tag = 1;
    let error = ir::validate_union_registry(&ordem_invertida)
        .expect_err("ordem canônica invertida deve falhar");
    assert!(error.contains("ordem canônica violada"), "{error}");

    let mut chave_duplicada = ir_program.union_types.clone();
    let primeira_chave = chave_duplicada[0].members[0].canonical_member_key.clone();
    chave_duplicada[0].members[1]
        .canonical_member_key
        .clone_from(&primeira_chave);
    let error =
        ir::validate_union_registry(&chave_duplicada).expect_err("chave duplicada deve falhar");
    assert!(error.contains("duplicada"), "{error}");
}

#[test]
fn hr1_scrutinee_e_avaliado_uma_unica_vez() {
    let source = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho fonte() -> uniao<aa, zz> { mimo (7 virar aa) virar uniao<aa, zz>; }
        carinho principal() -> bombom {
            encaixe fonte() {
                caso aa(n) { falar(1000 + (n virar bombom)); }
                caso zz(n) { falar(2000 + (n virar bombom)); }
            }
            mimo 0;
        }
    "#;
    let cfg_text = common::render_cfg_ir(source).expect("cfg");
    assert_eq!(
        cfg_text.matches("call fonte(").count(),
        1,
        "o scrutinee não pode ser reavaliado: {cfg_text}"
    );
    assert_eq!(
        cfg_text.matches("= union_tag ").count(),
        1,
        "a tag é lida uma única vez: {cfg_text}"
    );
    assert_eq!(hr1_interpreta(source), Some(0));
}

#[test]
fn hr1_operacoes_internas_sao_tipadas_em_todas_as_camadas() {
    let ir_text = common::render_ir(HR1_ALIAS_INVERSAO).expect("ir");
    let cfg_text = common::render_cfg_ir(HR1_ALIAS_INVERSAO).expect("cfg");
    let selected_text = common::render_selected(HR1_ALIAS_INVERSAO).expect("selected");
    let machine_text = common::render_machine(HR1_ALIAS_INVERSAO).expect("machine");

    assert!(ir_text.contains("union_match #0"), "{ir_text}");
    assert!(cfg_text.contains("= union_tag #0"), "{cfg_text}");
    assert!(cfg_text.contains("= union_extract #0"), "{cfg_text}");
    assert!(selected_text.contains("= union_tag #0"), "{selected_text}");
    assert!(
        selected_text.contains("= union_extract #0"),
        "{selected_text}"
    );
    assert!(machine_text.contains("union_tag #0"), "{machine_text}");
    assert!(machine_text.contains("union_extract #0"), "{machine_text}");

    // As antigas intrínsecas não aparecem como chamadas comuns em nenhuma
    // camada estruturada.
    for nome in HR1_INTRINSECAS_PROIBIDAS {
        for (camada, texto) in [
            ("ir", &ir_text),
            ("cfg", &cfg_text),
            ("selected", &selected_text),
            ("machine", &machine_text),
        ] {
            assert!(!texto.contains(nome), "{nome} reapareceu em {camada}");
        }
    }
}

#[test]
fn hr1_backend_nativo_escolhe_o_simbolo_de_runtime_no_proprio_backend() {
    let asm = common::render_backend_s_external_subset_nativo(HR1_ALIAS_INVERSAO).expect("asm");
    // O símbolo de ABI existe apenas no backend. Desde HR3 a extração é uma
    // cópia validada para storage do chamador, e não uma leitura de palavra.
    assert!(asm.contains("call pinker_uniao_tag"), "{asm}");
    assert!(asm.contains("call pinker_uniao_copiar_payload"), "{asm}");
    assert!(!asm.contains("pinker_uniao_payload_"), "{asm}");
    for nome in HR1_INTRINSECAS_PROIBIDAS {
        assert!(!asm.contains(nome), "{nome} não pode vazar para o backend");
    }
}

#[test]
fn hr1_backend_nativo_extrai_membro_verso_por_copia_validada() {
    let source = r#"
        pacote main;
        apelido texto = verso;
        carinho principal() -> bombom {
            nova valor: uniao<texto, u8> = "oi" virar uniao<texto, u8>;
            encaixe valor {
                caso texto(t) { falar(t); }
                caso u8(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let asm = common::render_backend_s_external_subset_nativo(source).expect("asm");
    // HR3: um membro `verso` é handle opaco de uma palavra. A extração copia o
    // snapshot para storage novo do binding e só então carrega a palavra; o
    // ponteiro interno do descritor nunca é devolvido.
    assert!(asm.contains("call pinker_uniao_copiar_payload"), "{asm}");
    assert!(asm.contains("movq $8, %rcx"), "{asm}");
    assert!(asm.contains("movq $8, %r8"), "{asm}");
    assert_eq!(hr1_interpreta(source), Some(0));
}

#[test]
fn hr1_execucao_nativa_tem_paridade_de_stdout_com_o_interpretador() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let pink = env!("CARGO_BIN_EXE_pink");
    let exemplo = "examples/hr1_encaixe_uniao_apelidos_valido.pink";

    let interpretado = std::process::Command::new(pink)
        .arg("--run")
        .arg(exemplo)
        .output()
        .expect("invocar pink --run");
    assert!(interpretado.status.success());
    let esperado = String::from_utf8_lossy(&interpretado.stdout).to_string();
    assert_eq!(esperado.trim(), "1007");

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_hr1_{nanos}"));
    let build = std::process::Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("invocar pink build --nativo");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let binario = out_dir.join("hr1_encaixe_uniao_apelidos_valido");
    let nativo = std::process::Command::new(binario)
        .output()
        .expect("executar binário nativo");
    assert_eq!(
        String::from_utf8_lossy(&nativo.stdout),
        esperado,
        "paridade de stdout entre interpretador e nativo"
    );
    assert_eq!(nativo.status.code(), Some(0));

    // Determinismo entre builds.
    let segundo = common::render_backend_s_external_subset_nativo(HR1_ALIAS_INVERSAO).expect("asm");
    let primeiro =
        common::render_backend_s_external_subset_nativo(HR1_ALIAS_INVERSAO).expect("asm");
    assert_eq!(primeiro, segundo);

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn hr1_valores_de_borda_do_payload_atual() {
    // Zero e o topo de cada largura suportada pelo payload de uma palavra.
    let casos = [
        ("u8", "0"),
        ("u8", "255"),
        ("u16", "65535"),
        ("u32", "4294967295"),
    ];
    for (tipo, literal) in casos {
        let source = format!(
            r#"
            pacote main;
            carinho principal() -> bombom {{
                nova valor: uniao<{tipo}, verso> =
                    ({literal} virar {tipo}) virar uniao<{tipo}, verso>;
                encaixe valor {{
                    caso {tipo}(n) {{ falar(n virar bombom); }}
                    caso verso(t) {{ falar(t); }}
                }}
                mimo 0;
            }}
        "#
        );
        assert_eq!(
            hr1_interpreta(&source),
            Some(0),
            "borda {tipo}={literal} deve executar"
        );
    }
}

#[test]
fn hr1_e_hr2_convivem_na_mesma_funcao() {
    let source = r#"
        pacote main;
        apelido aa = u8;
        apelido zz = u64;
        carinho principal() -> bombom {
            sussurro("nop", "1: nop", "jmp 1b");
            nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
            encaixe valor {
                caso aa(n) { falar(1000 + (n virar bombom)); }
                caso zz(n) { falar(2000 + (n virar bombom)); }
            }
            sussurro("nop");
            mimo 0;
        }
    "#;
    // O envelope estrutural de HR2 permanece equilibrado, e o encaixe tipado
    // continua presente na mesma função.
    let (_, _, selected, _) = lower(source);
    let asm = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly");
    assert_eq!(asm.matches(".intel_syntax noprefix").count(), 2);
    assert_eq!(asm.matches(".att_syntax prefix").count(), 2);
    assert!(asm.contains("call pinker_uniao_tag"), "{asm}");
    assert!(asm.contains("1: nop"), "{asm}");
}

#[test]
fn hr1_namespace_reservado_continua_recusado_dentro_do_braco() {
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(n) { nova __pinker_internal_x: bombom = 1; falar(n); }
                caso u64(n) { falar(n); }
            }
            mimo 0;
        }
    "#;
    let error = common::parse_and_check(source)
        .expect_err("namespace reservado deve falhar dentro do braço")
        .to_string();
    assert!(error.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{error}");
}

#[test]
fn hr1_chamada_direta_as_intrinsecas_de_uniao_continua_recusada() {
    for nome in HR1_INTRINSECAS_PROIBIDAS {
        let source = format!(
            "pacote main;\ncarinho principal() -> bombom {{\n    nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;\n    falar({nome}(valor));\n    mimo 0;\n}}"
        );
        let error = common::parse_and_check(&source)
            .expect_err("intrínseca de união não é chamável da fonte")
            .to_string();
        assert!(error.contains("E-SEMANTIC-RESERVED-NAMESPACE"), "{error}");
    }
}

// HR4 — identidade semântica de tipos em uniões.
//
// O defeito corrigido aqui é a seleção do membro de união por categoria
// operacional (`TypeIR`) com desempate por primeira ocorrência: dois `leque`
// distintos, dois `ninho` distintos, duas assinaturas de `carinho` e dois
// `seta<T>` de apontados diferentes colapsam na mesma categoria e faziam a
// injeção escolher o membro errado **silenciosamente**.

/// Programa com dois `leque` homorrepresentados injetando o membro indicado.
fn hr4_fonte_dois_leques(injetado: &str, variante: &str) -> String {
    format!(
        "pacote main;\n\
         leque Cor {{ Rosa, Azul }}\n\
         leque Tom {{ Claro, Escuro }}\n\
         carinho principal() -> bombom {{\n\
         \x20   nova valor: uniao<Cor, Tom> = {injetado}.{variante} virar uniao<Cor, Tom>;\n\
         \x20   encaixe valor {{\n\
         \x20       caso Cor(c) {{ mimo 1; }}\n\
         \x20       caso Tom(t) {{ mimo 2; }}\n\
         \x20   }}\n\
         \x20   mimo 0;\n\
         }}"
    )
}

#[test]
fn hr4_dois_leques_homorrepresentados_injetam_o_membro_exato() {
    // Antes de HR4 ambos os programas caíam no primeiro membro de mesma
    // representação: os dois devolviam o resultado do braço `Cor`.
    assert_eq!(
        hr1_interpreta(&hr4_fonte_dois_leques("Cor", "Azul")),
        Some(1)
    );
    assert_eq!(
        hr1_interpreta(&hr4_fonte_dois_leques("Tom", "Escuro")),
        Some(2)
    );
}

#[test]
fn hr4_dois_leques_tem_identidades_resolvidas_distintas_no_registry() {
    let (ir, _, _, _) = lower(&hr4_fonte_dois_leques("Tom", "Claro"));
    let union = &ir.union_types[0];
    let identidades: Vec<_> = union
        .members
        .iter()
        .map(|member| member.resolved_type_id)
        .collect();
    assert_eq!(identidades.len(), 2);
    assert_ne!(
        identidades[0], identidades[1],
        "dois leques distintos não podem compartilhar identidade resolvida"
    );

    let chaves: Vec<_> = union
        .members
        .iter()
        .map(|member| member.canonical_member_key.as_str())
        .collect();
    assert_eq!(chaves, vec!["enum:3:Cor", "enum:3:Tom"]);

    // Cada identidade do registry existe na tabela do programa, com a chave
    // canônica correspondente e a representação escalar do leque.
    for member in &union.members {
        let entrada = ir
            .resolved_types
            .iter()
            .find(|entrada| entrada.id == member.resolved_type_id)
            .expect("identidade do membro presente na tabela do programa");
        assert_eq!(entrada.canonical_key, member.canonical_member_key);
        assert_eq!(entrada.representation, ir::TypeIR::Bombom);
        assert_eq!(entrada.nominal_kind, Some(ir::NominalTypeKindIR::Leque));
    }
}

#[test]
fn hr4_injecao_carrega_a_identidade_do_membro_escolhido() {
    fn coleta(block: &ir::BlockIR, out: &mut Vec<(u64, ir::ResolvedTypeId, String)>) {
        fn valor(value: &ir::ValueIR, out: &mut Vec<(u64, ir::ResolvedTypeId, String)>) {
            if let ir::ValueIR::UnionInject {
                tag,
                resolved_member_type_id,
                canonical_member_key,
                ..
            } = value
            {
                out.push((*tag, *resolved_member_type_id, canonical_member_key.clone()));
            }
        }
        for instruction in &block.instructions {
            match instruction {
                ir::InstructionIR::Let { value, .. } => valor(value, out),
                ir::InstructionIR::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    coleta(then_block, out);
                    if let Some(else_block) = else_block {
                        coleta(else_block, out);
                    }
                }
                _ => {}
            }
        }
    }

    let (ir_programa, _, _, _) = lower(&hr4_fonte_dois_leques("Tom", "Escuro"));
    let mut injecoes = Vec::new();
    for function in &ir_programa.functions {
        coleta(&function.entry, &mut injecoes);
    }
    assert_eq!(injecoes.len(), 1, "esperava uma única injeção");
    let (tag, identidade, chave) = &injecoes[0];
    assert_eq!(chave, "enum:3:Tom");

    // A tag e a identidade descrevem o **mesmo** membro do registry.
    let membro = ir_programa.union_types[0]
        .members
        .iter()
        .find(|member| member.tag == *tag)
        .expect("tag pertence ao registry");
    assert_eq!(membro.resolved_type_id, *identidade);
    assert_eq!(&membro.canonical_member_key, chave);
}

#[test]
fn hr4_apelido_de_leque_nao_cria_identidade_propria() {
    // `apelido` é transparente: o texto do apelido nunca vira identidade.
    let source = r#"
        pacote main;
        leque Cor { Rosa, Azul }
        leque Tom { Claro, Escuro }
        apelido Apelidada = Cor;
        carinho principal() -> bombom {
            nova valor: uniao<Apelidada, Tom> = Cor.Azul virar uniao<Apelidada, Tom>;
            encaixe valor {
                caso Cor(c) { mimo 1; }
                caso Tom(t) { mimo 2; }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_interpreta(source), Some(1));
    assert_eq!(
        hr1_registry_tags(source),
        vec![(0, "enum:3:Cor".to_string()), (1, "enum:3:Tom".to_string())]
    );
}

#[test]
fn hr4_identidades_do_programa_sao_deterministicas_entre_lowerings() {
    // A tabela de identidades não pode depender da ordem de iteração de nenhum
    // mapa: dois lowerings do mesmo programa, no mesmo processo (onde cada
    // `HashMap` recebe sementes diferentes), produzem a mesma tabela.
    let source = hr4_fonte_dois_leques("Cor", "Rosa");
    let ast = common::parse(&source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let primeiro = ir::lower_program(&ast).expect("primeiro lowering");
    let segundo = ir::lower_program(&ast).expect("segundo lowering");
    assert_eq!(primeiro.resolved_types, segundo.resolved_types);
    assert_eq!(primeiro.union_types, segundo.union_types);
}

#[test]
fn hr4_tabela_de_identidades_e_densa_e_sem_chave_repetida() {
    let (ir_programa, _, _, _) = lower(&hr4_fonte_dois_leques("Tom", "Claro"));
    ir::validate_resolved_type_table(&ir_programa.resolved_types)
        .expect("tabela de identidades válida");
    ir::validate_union_registry_identities(&ir_programa.union_types, &ir_programa.resolved_types)
        .expect("registry coerente com a tabela");
}

#[test]
fn hr4_validador_recusa_identidade_de_membro_divergente() {
    // IR deliberadamente inválida: a tag continua correta, mas a identidade
    // aponta para outro membro. Nenhuma camada pode "consertar" isso escolhendo
    // o membro pela representação.
    let (ir_programa, _, _, _) = lower(&hr4_fonte_dois_leques("Cor", "Azul"));
    let union = &ir_programa.union_types[0];
    let outra = union.members[1].resolved_type_id;
    let erro = ir::validate_union_member_identity(&ir_programa.union_types, union.id, 0, outra)
        .expect_err("identidade divergente deve ser recusada");
    assert!(
        erro.contains("E-IR-UNION-MEMBER-IDENTITY-MISMATCH"),
        "{erro}"
    );
}

#[test]
fn hr4_validador_recusa_identidade_duplicada_no_registry() {
    // Duas tags com a mesma identidade resolvida descrevem a mesma união
    // ambígua que HR4 proíbe.
    let (ir_programa, _, _, _) = lower(&hr4_fonte_dois_leques("Cor", "Azul"));
    let mut unions = ir_programa.union_types.clone();
    let primeira = unions[0].members[0].resolved_type_id;
    unions[0].members[1].resolved_type_id = primeira;
    let erro = ir::validate_union_registry(&unions)
        .expect_err("identidade repetida deve ser recusada no registry");
    assert!(erro.contains("identidade resolvida"), "{erro}");
}

#[test]
fn hr4_encaixe_liga_o_braco_a_identidade_exata_do_membro() {
    // HR1 continua valendo: o braço tipado carrega a identidade do membro, e é
    // ela que permite reinjetar o valor sem reescolher a tag.
    let (ir_programa, _, _, _) = lower(&hr4_fonte_dois_leques("Tom", "Escuro"));
    let union = &ir_programa.union_types[0];
    let mut vistos = Vec::new();
    for function in &ir_programa.functions {
        for instruction in &function.entry.instructions {
            if let ir::InstructionIR::UnionMatch(union_match) = instruction {
                for arm in &union_match.arms {
                    vistos.push((arm.tag, arm.resolved_member_type_id));
                }
            }
        }
    }
    assert_eq!(vistos.len(), union.members.len());
    for (tag, identidade) in vistos {
        let membro = union
            .members
            .iter()
            .find(|member| member.tag == tag)
            .expect("tag do braço pertence ao registry");
        assert_eq!(membro.resolved_type_id, identidade);
    }
}

#[test]
fn hr4_uniao_de_escalares_continua_intacta() {
    // Regressão cruzada: a união puramente escalar de HR1 não muda de tags nem
    // de chaves canônicas por causa da identidade resolvida.
    let source = r#"
        pacote main;
        carinho principal() -> bombom {
            nova valor: uniao<u8, u64> = (7 virar u8) virar uniao<u8, u64>;
            encaixe valor {
                caso u8(n) { mimo 1; }
                caso u64(n) { mimo 2; }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_interpreta(source), Some(1));
    assert_eq!(
        hr1_registry_tags(source),
        vec![(0, "u64".to_string()), (1, "u8".to_string())]
    );
}

/// Prelúdio comum das superfícies de propagação: dois `leque`
/// homorrepresentados, o discriminador que devolve `1` para `Cor` e `2` para
/// `Tom`, produtores já injetados e produtores do **leque cru**.
///
/// Os produtores crus (`cor_crua`/`tom_cru`) existem porque a injeção depois da
/// chamada é a superfície que realmente exercita a identidade do retorno: se o
/// retorno perder a identidade, a injeção passa a escolher o membro errado.
const HR4_PRELUDIO: &str = r#"
    pacote main;
    leque Cor { Rosa, Azul }
    leque Tom { Claro, Escuro }
    carinho so_cor() -> uniao<Cor, Tom> { mimo Cor.Rosa virar uniao<Cor, Tom>; }
    carinho so_tom() -> uniao<Cor, Tom> { mimo Tom.Escuro virar uniao<Cor, Tom>; }
    carinho cor_crua() -> Cor { mimo Cor.Rosa; }
    carinho tom_cru() -> Tom { mimo Tom.Escuro; }
    carinho decide(u: uniao<Cor, Tom>) -> bombom {
        encaixe u {
            caso Cor(c) { mimo 1; }
            caso Tom(t) { mimo 2; }
        }
        mimo 0;
    }
"#;

fn hr4_com_prelude(principal: &str) -> String {
    format!("{HR4_PRELUDIO}\n    carinho principal() -> bombom {{\n{principal}\n    }}\n")
}

#[test]
fn hr4_cadeia_de_apelidos_converge_para_a_mesma_identidade() {
    // `apelido A2 = A1` e `apelido A1 = Cor` precisam produzir exatamente o
    // `ResolvedTypeId` de `Cor`: nenhum degrau da cadeia vira identidade.
    let source = r#"
        pacote main;
        leque Cor { Rosa, Azul }
        leque Tom { Claro, Escuro }
        apelido A1 = Cor;
        apelido A2 = A1;
        carinho principal() -> bombom {
            nova valor: uniao<A2, Tom> = Cor.Azul virar uniao<A2, Tom>;
            encaixe valor {
                caso Cor(c) { mimo 1; }
                caso Tom(t) { mimo 2; }
            }
            mimo 0;
        }
    "#;
    assert_eq!(hr1_interpreta(source), Some(1));
    assert_eq!(
        hr1_registry_tags(source),
        vec![(0, "enum:3:Cor".to_string()), (1, "enum:3:Tom".to_string())]
    );

    // A tabela do programa não guarda nenhuma identidade derivada do texto do
    // apelido.
    let (ir_programa, _, _, _) = lower(source);
    for entrada in &ir_programa.resolved_types {
        assert!(
            entrada.canonical_key != "enum:2:A1" && entrada.canonical_key != "enum:2:A2",
            "apelido virou identidade: {}",
            entrada.canonical_key
        );
    }
}

#[test]
fn hr4_identidade_atravessa_parametro_retorno_e_chamada_direta() {
    // Parâmetro (`decide`), retorno (`so_tom`) e chamada direta compõem a
    // mesma cadeia de identidade: o membro escolhido na injeção sobrevive à
    // fronteira da função.
    let source = hr4_com_prelude("        mimo decide(so_tom());");
    assert_eq!(hr1_interpreta(&source), Some(2));
    let cor = hr4_com_prelude("        mimo decide(so_cor());");
    assert_eq!(hr1_interpreta(&cor), Some(1));
}

#[test]
fn hr4_identidade_atravessa_local_e_atribuicao() {
    let source = hr4_com_prelude(
        "        nova muda v = so_cor();\n\
         \x20       nova antes = decide(v);\n\
         \x20       v = so_tom();\n\
         \x20       mimo antes + decide(v) * 10;",
    );
    // 1 (Cor, antes da atribuição) + 2*10 (Tom, depois) = 21.
    assert_eq!(hr1_interpreta(&source), Some(21));
}

#[test]
fn hr4_identidade_atravessa_ternario() {
    let verdadeiro = hr4_com_prelude("        mimo decide(verdade ? so_cor() : so_tom());");
    assert_eq!(hr1_interpreta(&verdadeiro), Some(1));
    let falso = hr4_com_prelude("        mimo decide(falso ? so_cor() : so_tom());");
    assert_eq!(hr1_interpreta(&falso), Some(2));

    // Ternário **antes** da injeção: a identidade do resultado precisa ser a do
    // leque escolhido, não a representação escalar comum aos dois ramos.
    let cru = hr4_com_prelude(
        "        nova escolhido = verdade ? Tom.Claro : Tom.Escuro;\n\
         \x20       mimo decide(escolhido virar uniao<Cor, Tom>);",
    );
    assert_eq!(hr1_interpreta(&cru), Some(2));
}

#[test]
fn hr4_identidade_do_retorno_de_leque_sobrevive_a_chamada() {
    // A injeção acontece **depois** da chamada: é a identidade do retorno
    // declarado que decide o membro.
    let tom = hr4_com_prelude("        mimo decide(tom_cru() virar uniao<Cor, Tom>);");
    assert_eq!(hr1_interpreta(&tom), Some(2));
    let cor = hr4_com_prelude("        mimo decide(cor_crua() virar uniao<Cor, Tom>);");
    assert_eq!(hr1_interpreta(&cor), Some(1));

    // Mesma cadeia através de um local intermediário.
    let via_local = hr4_com_prelude(
        "        nova valor = tom_cru();\n\
         \x20       mimo decide(valor virar uniao<Cor, Tom>);",
    );
    assert_eq!(hr1_interpreta(&via_local), Some(2));
}

#[test]
fn hr4_identidade_atravessa_callable_e_chamada_indireta() {
    let source = hr4_com_prelude(
        "        nova f = so_tom;\n\
         \x20       mimo decide(f());",
    );
    assert_eq!(hr1_interpreta(&source), Some(2));

    // Chamada indireta que devolve o leque cru: a identidade do retorno do
    // callable é o que permite injetar no membro certo depois.
    let cru = hr4_com_prelude(
        "        nova g = tom_cru;\n\
         \x20       mimo decide(g() virar uniao<Cor, Tom>);",
    );
    assert_eq!(hr1_interpreta(&cru), Some(2));
}

#[test]
fn hr4_identidade_atravessa_closure_e_captura() {
    // `base` é capturada com a identidade de `Tom`; a injeção dentro do corpo
    // da closure precisa escolher o membro `Tom`, não o primeiro escalar.
    let source = hr4_com_prelude(
        "        nova base = Tom.Claro;\n\
         \x20       nova g = carinho() -> uniao<Cor, Tom> { mimo base virar uniao<Cor, Tom>; };\n\
         \x20       mimo decide(g());",
    );
    assert_eq!(hr1_interpreta(&source), Some(2));

    // Closure que devolve o leque cru: a identidade precisa sobreviver ao
    // retorno da closure e chegar à injeção no chamador.
    let cru = hr4_com_prelude(
        "        nova base = Tom.Claro;\n\
         \x20       nova h = carinho() -> Tom { mimo base; };\n\
         \x20       mimo decide(h() virar uniao<Cor, Tom>);",
    );
    assert_eq!(hr1_interpreta(&cru), Some(2));
}

#[test]
fn hr4_objeto_de_trato_sem_anotacao_carrega_identidade() {
    // Sem anotação explícita, a identidade do objeto de trato só pode vir da
    // materialização e da chamada de método. `TypeIR::TraitObject` não
    // distingue dois tratos, então o slot precisa carregar `trato<Nome>`.
    let source = r#"
        pacote main;
        trato Medivel { carinho medir(valor: si) -> bombom; }
        impl Medivel para bombom {
            carinho medir(valor: bombom) -> bombom { mimo valor; }
        }
        carinho principal() -> bombom {
            nova objeto = 7 virar trato<Medivel>;
            nova copia = objeto;
            mimo copia.medir();
        }
    "#;
    let (ir_programa, _, _, _) = lower(source);
    let principal = ir_programa
        .functions
        .iter()
        .find(|function| function.name == "principal")
        .expect("função principal");
    let tratos: Vec<_> = principal
        .locals
        .iter()
        .filter(|local| local.ty == ir::TypeIR::TraitObject)
        .collect();
    assert!(!tratos.is_empty(), "esperava slots de trato");
    for local in tratos {
        let resolved = local
            .resolved
            .unwrap_or_else(|| panic!("slot '{}' sem identidade resolvida", local.slot));
        let entrada = ir_programa
            .resolved_types
            .iter()
            .find(|entrada| entrada.id == resolved)
            .expect("identidade presente na tabela");
        assert_eq!(entrada.representation, ir::TypeIR::TraitObject);
        assert!(
            entrada.canonical_key.contains("Medivel"),
            "{}",
            entrada.canonical_key
        );
    }
    assert_eq!(hr1_interpreta(source), Some(7));
}

#[test]
fn hr4_metodo_de_trato_preserva_a_identidade_do_retorno() {
    // O retorno declarado do método é um `leque`; a injeção acontece depois da
    // chamada dinâmica, então a identidade precisa atravessar o despacho.
    let source = r#"
        pacote main;
        leque Cor { Rosa, Azul }
        leque Tom { Claro, Escuro }
        trato Fonte { carinho tom(valor: si) -> Tom; }
        impl Fonte para bombom {
            carinho tom(valor: bombom) -> Tom { mimo Tom.Escuro; }
        }
        carinho decide(u: uniao<Cor, Tom>) -> bombom {
            encaixe u {
                caso Cor(c) { mimo 1; }
                caso Tom(t) { mimo 2; }
            }
            mimo 0;
        }
        carinho principal() -> bombom {
            nova objeto = 7 virar trato<Fonte>;
            mimo decide(objeto.tom() virar uniao<Cor, Tom>);
        }
    "#;
    assert_eq!(hr1_interpreta(source), Some(2));
}

#[test]
fn hr4_validador_recusa_chave_canonica_repetida() {
    // IR deliberadamente inválida: duas entradas com a mesma chave canônica.
    let entrada = |id: u32, key: &str| ir::ResolvedTypeIR {
        id: ir::ResolvedTypeId(id),
        canonical_key: key.to_string(),
        representation: ir::TypeIR::Bombom,
        nominal_kind: None,
        nominal_name: None,
        pointee: None,
        element: None,
        signature: None,
        union_members: None,
    };
    let erro = ir::validate_resolved_type_table(&[entrada(0, "bombom"), entrada(1, "bombom")])
        .expect_err("chave repetida deve ser recusada");
    assert!(erro.contains("chave canônica repetida"), "{erro}");
}

#[test]
fn hr4_validador_recusa_chave_envenenada() {
    // Uma chave de identidade perdida nunca pode virar identidade internada.
    let envenenada = ir::ResolvedTypeIR {
        id: ir::ResolvedTypeId(0),
        canonical_key: "?apelido-nao-resolvido:3:Cor".to_string(),
        representation: ir::TypeIR::Bombom,
        nominal_kind: None,
        nominal_name: None,
        pointee: None,
        element: None,
        signature: None,
        union_members: None,
    };
    let erro = ir::validate_resolved_type_table(&[envenenada])
        .expect_err("chave envenenada deve ser recusada");
    assert!(erro.contains("identidade perdida"), "{erro}");
}

#[test]
fn hr4_internacao_recusa_representacao_divergente_na_mesma_chave() {
    let mut tabela = ir::ResolvedTypeTable::default();
    let primeiro = tabela
        .intern(
            "bombom".to_string(),
            ir::TypeIR::Bombom,
            ir::ResolvedTypeParts::default(),
        )
        .expect("primeira internação");
    let repetida = tabela
        .intern(
            "bombom".to_string(),
            ir::TypeIR::Bombom,
            ir::ResolvedTypeParts::default(),
        )
        .expect("mesma chave, mesma representação");
    assert_eq!(primeiro, repetida, "a internação é idempotente por chave");

    let erro = tabela
        .intern(
            "bombom".to_string(),
            ir::TypeIR::Verso,
            ir::ResolvedTypeParts::default(),
        )
        .expect_err("representação divergente sob a mesma chave deve ser recusada");
    assert!(erro.contains("representação"), "{erro}");
}

#[test]
fn hr4_tabela_de_identidades_recusa_id_fora_da_posicao() {
    // A densidade dos IDs é o que permite indexar a tabela pela posição; uma
    // tabela reordenada depois da internação deixa de ser válida.
    let entrada = |id: u32, key: &str| ir::ResolvedTypeIR {
        id: ir::ResolvedTypeId(id),
        canonical_key: key.to_string(),
        representation: ir::TypeIR::Bombom,
        nominal_kind: None,
        nominal_name: None,
        pointee: None,
        element: None,
        signature: None,
        union_members: None,
    };
    let erro = ir::validate_resolved_type_table(&[entrada(1, "u8"), entrada(0, "bombom")])
        .expect_err("ID fora da posição deve ser recusado");
    assert!(erro.contains("fora da posição"), "{erro}");
}

#[test]
fn hr4_interpretador_recusa_identidade_de_membro_divergente() {
    // Programa de máquina deliberadamente inválido: a tag continua correta e a
    // identidade do membro é trocada. O interpretador tem de recusar em vez de
    // produzir o valor do membro errado.
    let (_, _, _, mut machine) = lower(&hr4_com_prelude("        mimo decide(so_tom());"));
    let mut trocadas = 0usize;
    for function in machine.functions.iter_mut() {
        for block in function.blocks.iter_mut() {
            for instruction in block.code.iter_mut() {
                if let abstract_machine::MachineInstr::MakeUnion {
                    resolved_member_type_id,
                    ..
                } = instruction
                {
                    resolved_member_type_id.0 += 1;
                    trocadas += 1;
                }
            }
        }
    }
    assert!(trocadas > 0, "esperava ao menos uma instrução make_union");
    let erro = interpreter::run_program_with_args(&machine, &[])
        .expect_err("identidade divergente deve ser recusada em execução")
        .to_string();
    assert!(erro.contains("identidade de membro de união"), "{erro}");
}

#[test]
fn hr4_extracao_e_reinjecao_preservam_o_membro() {
    // O valor desempacotado por `encaixe` reinjetado na mesma união tem de
    // voltar para a **mesma** tag. Antes de HR4 a reinjeção reescolhia o
    // primeiro membro de mesma representação.
    let source = format!(
        "{HR4_PRELUDIO}\n\
         carinho reinjeta(u: uniao<Cor, Tom>) -> uniao<Cor, Tom> {{\n\
         \x20   encaixe u {{\n\
         \x20       caso Cor(c) {{ mimo c virar uniao<Cor, Tom>; }}\n\
         \x20       caso Tom(t) {{ mimo t virar uniao<Cor, Tom>; }}\n\
         \x20   }}\n\
         \x20   mimo u;\n\
         }}\n\
         carinho principal() -> bombom {{\n\
         \x20   mimo decide(reinjeta(so_cor())) + decide(reinjeta(so_tom())) * 10;\n\
         }}\n"
    );
    // 1 (Cor reinjetado como Cor) + 2*10 (Tom reinjetado como Tom) = 21.
    assert_eq!(hr1_interpreta(&source), Some(21));
}

#[test]
fn hr4_dois_ninhos_homorrepresentados_tem_identidades_distintas() {
    // `ninho Alfa` e `ninho Beta` compartilham `TypeIR::Struct`. A linguagem
    // ainda não possui expressão de literal de `ninho`, então a injeção direta
    // de um valor de ninho é NOT_APPLICABLE; o que precisa valer — e vale — é
    // que o registry e a tabela mantenham identidades separadas.
    let source = r#"
        pacote main;
        ninho Alfa { a: bombom; }
        ninho Beta { b: bombom; }
        carinho decide(u: uniao<Alfa, Beta>) -> bombom {
            encaixe u {
                caso Alfa(x) { mimo 1; }
                caso Beta(y) { mimo 2; }
            }
            mimo 0;
        }
        carinho principal() -> bombom { mimo 0; }
    "#;
    let (ir_programa, _, _, _) = lower(source);
    let union = &ir_programa.union_types[0];
    assert_eq!(
        union
            .members
            .iter()
            .map(|member| member.canonical_member_key.as_str())
            .collect::<Vec<_>>(),
        vec!["struct:4:Alfa", "struct:4:Beta"]
    );
    assert_ne!(
        union.members[0].resolved_type_id, union.members[1].resolved_type_id,
        "dois ninhos distintos não podem compartilhar identidade resolvida"
    );
    for member in &union.members {
        assert_eq!(member.ty, ir::TypeIR::Struct, "mesma categoria operacional");
        let entrada = ir_programa
            .resolved_types
            .iter()
            .find(|entrada| entrada.id == member.resolved_type_id)
            .expect("identidade presente na tabela");
        assert_eq!(entrada.nominal_kind, Some(ir::NominalTypeKindIR::Ninho));
    }
}

#[test]
fn hr4_assinaturas_distintas_de_carinho_nao_colidem() {
    // `carinho(u8) -> u8` e `carinho(u64) -> u64` compartilham
    // `TypeIR::Function`; a identidade resolvida separa as duas pela assinatura.
    let source = r#"
        pacote main;
        carinho estreito(v: u8) -> u8 { mimo v; }
        carinho largo(v: u64) -> u64 { mimo v; }
        carinho principal() -> bombom {
            nova a = estreito;
            nova b = largo;
            mimo 0;
        }
    "#;
    let (ir_programa, _, _, _) = lower(source);
    let assinaturas: Vec<_> = ir_programa
        .resolved_types
        .iter()
        .filter(|entrada| entrada.representation == ir::TypeIR::Function)
        .map(|entrada| entrada.canonical_key.as_str())
        .collect();
    assert!(assinaturas.contains(&"fn(2:u8)->2:u8"), "{assinaturas:?}");
    assert!(assinaturas.contains(&"fn(3:u64)->3:u64"), "{assinaturas:?}");
}

#[test]
fn hr4_ponteiros_de_apontados_distintos_nao_colidem() {
    // `seta<u8>` e `seta<u64>` compartilham `TypeIR::Pointer` e precisam de
    // identidades distintas, com o apontado registrado na tabela.
    let source = r#"
        pacote main;
        carinho le_estreito(p: seta<u8>) -> bombom { mimo 0; }
        carinho le_largo(p: seta<u64>) -> bombom { mimo 0; }
        carinho principal() -> bombom {
            nova estreito: seta<u8> = alocar(8);
            liberar(estreito);
            mimo 0;
        }
    "#;
    let (ir_programa, _, _, _) = lower(source);
    let ponteiros: Vec<_> = ir_programa
        .resolved_types
        .iter()
        .filter(|entrada| matches!(entrada.representation, ir::TypeIR::Pointer { .. }))
        .map(|entrada| entrada.canonical_key.as_str())
        .collect();
    assert!(ponteiros.contains(&"ptr:0:u8"), "{ponteiros:?}");
    assert!(ponteiros.contains(&"ptr:0:u64"), "{ponteiros:?}");

    for entrada in ir_programa
        .resolved_types
        .iter()
        .filter(|entrada| matches!(entrada.representation, ir::TypeIR::Pointer { .. }))
    {
        let pointee = entrada.pointee.expect("ponteiro registra o apontado");
        assert!(
            ir_programa
                .resolved_types
                .iter()
                .any(|outro| outro.id == pointee),
            "apontado ausente da tabela"
        );
    }
}
