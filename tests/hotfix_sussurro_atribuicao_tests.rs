//! Hotfix pós-PR #412 — atribuição de símbolo em `sussurro`.
//!
//! O scanner estrutural recusava as duas formas de statement do GNU as que
//! começam com `.` (diretiva) ou terminam em `:` (label nominal), mas a terceira
//! forma que define símbolo — a atribuição `nome = expressão` — atravessava:
//! `nome` era reconhecido como mnemônico e `= expressão` era preservado como
//! texto de operando, de modo que o assembler real definia o símbolo.
//!
//! Esta suíte cobre as duas camadas da correção: a política estrutural sobre a
//! fonte e o invariante sobre o objeto realmente produzido.

mod common;

use common::ControlledCommand as Command;
use pinker_v0::inline_asm::{self, E_ASM_ARTIFACT, E_ASM_SYMBOL_ASSIGN};
use pinker_v0::{
    abstract_machine, abstract_machine_validate, backend_s, cfg_ir, cfg_ir_validate, elf,
    instr_select, instr_select_validate, interpreter, ir, ir_validate, semantic,
};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn sussurro_source(chunks: &str) -> String {
    format!(
        "pacote main;\ncarinho principal() -> bombom {{\n    sussurro({chunks});\n    mimo 0;\n}}\n"
    )
}

fn recusa(chunks: &str) -> String {
    let ast = common::parse(&sussurro_source(chunks)).expect("parse");
    semantic::check_program(&ast)
        .expect_err("política de 'sussurro' deve recusar")
        .to_string()
}

fn aceita(chunks: &str) {
    let ast = common::parse(&sussurro_source(chunks)).expect("parse");
    semantic::check_program(&ast)
        .unwrap_or_else(|error| panic!("'sussurro' deveria aceitar {chunks}: {error}"));
}

fn diretorio_temporario(rotulo: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    std::env::temp_dir().join(format!("pinker_hf_sussurro_{rotulo}_{nanos}"))
}

// @pinker-nav:start evidencia.hotfix.sussurro-atribuicao-scanner
// @pinker-nav:domain sussurro
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência da política estrutural: a atribuição de símbolo do GNU as é recusada com `E-SEMANTIC-ASM-SYMBOL-ASSIGN` em todas as formas que o assembler real aceita — espaçamento livre, tab, sem espaço, alias para símbolo existente, expressão composta, depois de `;`, depois de comentário de linha e de bloco, depois de CRLF normalizado, depois de label local numérico, em qualquer statement de um bloco com vários, e na forma `==` do dialeto —, enquanto formas apenas parecidas mantêm a classificação própria (`nome:` label nominal, `.set` diretiva, `= 1` token inesperado, `nome + 1` entregue ao assembler); cobre também o span e o texto do diagnóstico, o bloco com vários `sussurro` no mesmo carinho, o erro explícito do interpretador (`E-RUNTIME-SUSSURRO-NATIVO`) e a preservação integral dos exemplos válidos da Fase 247 e das aceitações históricas (labels locais, `Nf`/`Nb`, comentários, segment override).

/// Toda forma de atribuição que o assembler real aceitaria define um símbolo.
///
/// A tabela é a mesma matriz confirmada contra o `as` do build antes da
/// correção: cada entrada assemblava e produzia um símbolo novo no objeto.
#[test]
fn atribuicao_de_simbolo_e_recusada_em_toda_forma() {
    for chunk in [
        // Forma simples.
        r#""meu_simbolo = 1""#,
        // Espaçamento: nenhum, largo, tab dos dois lados.
        r#""meu_simbolo=1""#,
        r#""meu_simbolo   =   1""#,
        "\"meu_simbolo\\t=\\t1\"",
        // Alias para um símbolo existente e expressão composta.
        r#""alias_de_principal = principal""#,
        r#""meu_simbolo = 1 + 2""#,
        // Depois de separador de statement, em qualquer posição.
        r#""nop; meu_simbolo = 1""#,
        r#""meu_simbolo = 1; nop""#,
        r#""nop; nop; meu_simbolo = 1""#,
        // Depois de comentário de bloco (removido pelo scanner) e antes de um
        // comentário de linha (que não protege a atribuição).
        r#""/* comentario */ meu_simbolo = 1""#,
        r#""nop /* c */ ; meu_simbolo = 1""#,
        r#""meu_simbolo = 1 # comentario""#,
        // Depois de label local numérico.
        r#""1: meu_simbolo = 1""#,
        // A forma `==` também define símbolo no dialeto aceito pelo build.
        r#""meu_simbolo == 1""#,
    ] {
        let error = recusa(chunk);
        assert!(
            error.contains(E_ASM_SYMBOL_ASSIGN),
            "{chunk} deveria ser recusado como atribuição de símbolo => {error}"
        );
    }
}

/// Uma string literal Pinker não quebra linha, então newline e CRLF são
/// exercidos direto no scanner, que é a fronteira real.
#[test]
fn atribuicao_e_recusada_depois_de_newline_e_de_crlf() {
    for chunk in [
        "nop\nmeu_simbolo = 1",
        "nop\r\nmeu_simbolo = 1",
        "nop \\\n    ; meu_simbolo = 1",
        "1:\r\n  meu_simbolo = 1",
    ] {
        let error = inline_asm::scan_chunk(chunk).expect_err("atribuição deve ser recusada");
        assert_eq!(error.code, E_ASM_SYMBOL_ASSIGN, "{chunk:?} => {error}");
    }
}

/// Formas apenas parecidas continuam com a classificação que já tinham: a
/// correção não pode transformar o scanner num reconhecedor por prefixo.
#[test]
fn formas_semelhantes_mantem_a_classificacao_propria() {
    for (chunk, esperado) in [
        (r#""meu_simbolo:""#, "E-SEMANTIC-ASM-NAMED-LABEL"),
        (r#""meu_simbolo: nop""#, "E-SEMANTIC-ASM-NAMED-LABEL"),
        (r#""meu_simbolo := 1""#, "E-SEMANTIC-ASM-NAMED-LABEL"),
        (r#"".set meu_simbolo, 1""#, "E-SEMANTIC-ASM-DIRECTIVE"),
        (r#"".equ meu_simbolo, 1""#, "E-SEMANTIC-ASM-DIRECTIVE"),
        (r#""= 1""#, "E-SEMANTIC-ASM-UNEXPECTED-TOKEN"),
        (r#""nop; = 1""#, "E-SEMANTIC-ASM-UNEXPECTED-TOKEN"),
    ] {
        let error = recusa(chunk);
        assert!(error.contains(esperado), "{chunk} => {error}");
    }

    // Um token desconhecido que não é atribuição continua sendo entregue ao
    // assembler real: o scanner não inventa exceção nem adivinha mnemônico.
    let statements = inline_asm::scan_chunk("meu_simbolo + 1").expect("entregue ao assembler");
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].mnemonic.as_deref(), Some("meu_simbolo"));
    assert_eq!(statements[0].operands, "+ 1");

    // Um `=` dentro dos operandos, depois de um mnemônico legítimo, não é a
    // forma de atribuição — o statement já é uma instrução naquele ponto.
    let statements = inline_asm::scan_chunk("mov rax, 1").expect("instrução comum");
    assert_eq!(statements[0].operands, "rax, 1");
}

/// O diagnóstico precisa nomear o símbolo e apontar o statement de origem.
#[test]
fn diagnostico_e_span_da_atribuicao() {
    let source = sussurro_source(r#""nop", "meu_simbolo = 1""#);
    let ast = common::parse(&source).expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("atribuição deve falhar")
        .to_string();

    assert!(error.contains(E_ASM_SYMBOL_ASSIGN), "{error}");
    assert!(error.contains("meu_simbolo"), "{error}");
    assert!(error.contains("'sussurro'"), "{error}");
    // O span é o do statement `sussurro`, na linha 3 da fonte sintetizada.
    assert!(
        error.contains("3:5.."),
        "span ausente ou deslocado: {error}"
    );
}

/// Vários blocos no mesmo carinho: a política vale em todos, não só no primeiro.
#[test]
fn atribuicao_em_qualquer_um_de_varios_blocos_e_recusada() {
    let com_dois_blocos = |primeiro: &str, segundo: &str| {
        format!(
            "pacote main;\ncarinho principal() -> bombom {{\n    sussurro({primeiro});\n    sussurro({segundo});\n    mimo 0;\n}}\n"
        )
    };
    for (primeiro, segundo) in [
        (r#""meu_simbolo = 1""#, r#""nop""#),
        (r#""nop""#, r#""meu_simbolo = 1""#),
    ] {
        let ast = common::parse(&com_dois_blocos(primeiro, segundo)).expect("parse");
        let error = semantic::check_program(&ast)
            .expect_err("atribuição deve falhar")
            .to_string();
        assert!(error.contains(E_ASM_SYMBOL_ASSIGN), "{error}");
    }

    // O exemplo com dois blocos legítimos continua aceito de ponta a ponta.
    let ast = common::parse(include_str!(
        "../examples/hotfix_sussurro_multiplos_blocos_valido.pink"
    ))
    .expect("parse");
    semantic::check_program(&ast).expect("dois blocos legítimos");
}

/// O exemplo negativo versionado é recusado com o diagnóstico dedicado.
#[test]
fn exemplo_negativo_versionado_e_recusado() {
    let ast = common::parse(include_str!(
        "../examples/hotfix_sussurro_atribuicao_invalido.pink"
    ))
    .expect("parse");
    let error = semantic::check_program(&ast)
        .expect_err("atribuição deve falhar")
        .to_string();
    assert!(error.contains(E_ASM_SYMBOL_ASSIGN), "{error}");
}

/// A correção não pode estreitar o que já era aceito.
#[test]
fn aceitacoes_historicas_sao_preservadas() {
    for chunk in [
        r#""nop""#,
        r#""1:""#,
        r#""1: nop""#,
        r#""jne 1b""#,
        r#""jmp 2f", "2: nop""#,
        r#""mov rax, fs:[0]""#,
        r#""nop # .section .data""#,
        r#""nop /* .section .data */""#,
        r#""nop /* comentario */ ; nop""#,
    ] {
        aceita(chunk);
    }

    // Os exemplos válidos da Fase 247 continuam atravessando a semântica.
    for exemplo in [
        include_str!("../examples/fase247_sussurro_inline_asm_real_valido.pink"),
        include_str!("../examples/check_inline_asm_valido.pink"),
        include_str!("../examples/check_inline_asm_multilinha.pink"),
    ] {
        let ast = common::parse(exemplo).expect("parse");
        semantic::check_program(&ast).expect("exemplo válido preservado");
    }
}

/// O interpretador continua recusando `sussurro` de forma explícita.
#[test]
fn interpretador_mantem_o_erro_explicito() {
    let source = sussurro_source(r#""nop", "1: nop", "jmp 1b""#);
    let ast = common::parse(&source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let program_ir = ir::lower_program(&ast).expect("ir");
    ir_validate::validate_program(&program_ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&program_ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    instr_select_validate::validate_program(&selected).expect("selected validate");
    let machine = abstract_machine::lower_program(&selected).expect("machine");
    abstract_machine_validate::validate_program(&machine).expect("machine validate");

    let error = interpreter::run_program(&machine)
        .expect_err("sussurro não executa no interpretador")
        .to_string();
    assert!(error.contains("E-RUNTIME-SUSSURRO-NATIVO"), "{error}");
}
// @pinker-nav:end evidencia.hotfix.sussurro-atribuicao-scanner

// @pinker-nav:start evidencia.hotfix.sussurro-artefato
// @pinker-nav:domain sussurro
// @pinker-nav:layer evidencia
// @pinker-nav:summary Evidência do invariante de artefato: o leitor de ELF próprio lê seções e símbolos de um objeto real e recusa entrada malformada sem pânico; `strip_envelope_bodies` remove exatamente os envelopes e preserva o resto linha a linha, mantendo sentinelas e wrappers Intel/AT&T no assembly emitido; `compare_artifact_surfaces` acusa símbolo novo, alias novo, seção nova e mudança de ligação/visibilidade sobre objetos realmente montados, e aprova o par derivado do compilador; `verify_native_artifact` — a mesma função chamada pelo build — aprova um envelope legítimo; um `pink build --nativo` real imprime a linha de verificação e o ELF final não ganha nenhum símbolo definido nem seção em relação ao mesmo programa sem `sussurro`; e um guardião estrutural exige que a região `cli.build.nativo` de `src/main.rs` continue chamando a verificação antes de linkar, de modo que remover o cabo produtivo quebre a suíte. Sob `PINKER_EXIGE_NATIVO=1` a ausência do driver C bloqueia em vez de pular em silêncio.

/// Monta um `.s` com o driver C e devolve o objeto lido pelo leitor próprio.
fn montar_e_ler(driver: &str, rotulo: &str, asm: &str) -> elf::ElfObject {
    let dir = diretorio_temporario(rotulo);
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    let source = dir.join("bloco.s");
    let object = dir.join("bloco.o");
    std::fs::write(&source, asm).expect("gravar .s");
    let saida = Command::new(driver)
        .arg("-c")
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .expect("invocar driver C");
    assert!(
        saida.status.success(),
        "driver recusou '{rotulo}': {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    let bytes = std::fs::read(&object).expect("ler objeto");
    let lido = elf::parse(&bytes).expect("ler ELF");
    let _ = std::fs::remove_dir_all(&dir);
    lido
}

const ASM_BASE: &str = ".text\n.globl f\nf:\n  nop\n  ret\n";

#[test]
fn leitor_de_elf_le_secoes_e_simbolos_e_recusa_entrada_malformada() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let objeto = montar_e_ler(&driver, "leitor", ASM_BASE);
    assert!(
        objeto.sections.iter().any(|name| name == ".text"),
        "{:?}",
        objeto.sections
    );
    let f = objeto
        .symbols
        .iter()
        .find(|symbol| symbol.name == "f")
        .expect("símbolo 'f'");
    assert!(f.is_defined());
    assert_eq!(f.bind, 1, "STB_GLOBAL");

    // Entrada malformada devolve Err com detalhe, nunca pânico.
    assert!(elf::parse(b"").is_err());
    assert!(elf::parse(b"nao e um elf").is_err());
    let mut truncado = vec![0u8; 64];
    truncado[0..4].copy_from_slice(b"\x7fELF");
    truncado[4] = 2;
    truncado[5] = 1;
    // `e_shoff` aponta para fora do arquivo.
    truncado[0x28] = 0xff;
    assert!(elf::parse(&truncado).is_err());
    // Classe 32 bits é recusada explicitamente.
    let mut classe32 = truncado.clone();
    classe32[4] = 1;
    assert!(elf::parse(&classe32).is_err());
}

fn asm_do_exemplo(source: &str) -> String {
    let ast = common::parse(source).expect("parse");
    semantic::check_program(&ast).expect("semantic");
    let program_ir = ir::lower_program(&ast).expect("ir");
    let cfg = cfg_ir::lower_program(&program_ir).expect("cfg");
    let selected = instr_select::lower_program(&cfg).expect("selected");
    backend_s::emit_external_toolchain_subset_nativo(&selected).expect("assembly nativo")
}

#[test]
fn baseline_remove_apenas_os_envelopes_e_preserva_sentinelas_no_emitido() {
    let asm = asm_do_exemplo(&sussurro_source(r#""nop", "1: nop", "jmp 1b""#));

    // O emitido preserva sentinelas e os dois wrappers de sintaxe.
    assert_eq!(asm.matches(inline_asm::SENTINEL_BEGIN_PREFIX).count(), 1);
    assert_eq!(asm.matches(inline_asm::SENTINEL_END_PREFIX).count(), 1);
    assert_eq!(asm.matches(inline_asm::INTEL_SYNTAX_WRAPPER).count(), 1);
    assert_eq!(asm.matches(inline_asm::ATT_SYNTAX_WRAPPER).count(), 1);

    let baseline = inline_asm::strip_envelope_bodies(&asm).expect("baseline");
    assert!(!baseline.contains(inline_asm::SENTINEL_BEGIN_PREFIX));
    assert!(!baseline.contains(inline_asm::SENTINEL_END_PREFIX));
    assert!(!baseline.contains(inline_asm::INTEL_SYNTAX_WRAPPER));
    assert!(!baseline.contains(inline_asm::ATT_SYNTAX_WRAPPER));
    assert!(inline_asm::validate_envelopes(&baseline)
        .expect("baseline sem envelope")
        .is_empty());

    // Fora dos envelopes, a baseline é o mesmo texto, na mesma ordem.
    let mut esperado: Vec<&str> = Vec::new();
    let mut dentro = false;
    for line in asm.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(inline_asm::SENTINEL_BEGIN_PREFIX) {
            dentro = true;
            continue;
        }
        if trimmed.starts_with(inline_asm::SENTINEL_END_PREFIX) {
            dentro = false;
            continue;
        }
        if !dentro {
            esperado.push(line);
        }
    }
    assert_eq!(baseline.lines().collect::<Vec<_>>(), esperado);

    // Um programa sem `sussurro` atravessa inalterado.
    let sem_bloco =
        asm_do_exemplo("pacote main;\ncarinho principal() -> bombom {\n    mimo 0;\n}\n");
    assert_eq!(
        inline_asm::strip_envelope_bodies(&sem_bloco).expect("baseline"),
        sem_bloco
    );
}

#[test]
fn superficie_do_artefato_acusa_simbolo_alias_secao_e_ligacao() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let baseline = inline_asm::artifact_surface(&montar_e_ler(&driver, "base", ASM_BASE));

    // Par idêntico: nenhum delta.
    let igual = inline_asm::artifact_surface(&montar_e_ler(&driver, "igual", ASM_BASE));
    let check = inline_asm::compare_artifact_surfaces(&baseline, &igual).expect("sem delta");
    assert!(check.defined_symbols >= 1);

    // Cada violação do contrato aparece como delta atribuível ao bloco.
    for (rotulo, extra, esperado) in [
        ("simbolo", "meu_simbolo = 1\n", "meu_simbolo"),
        ("alias", "alias_de_f = f\n", "alias_de_f"),
        (
            "secao",
            ".section .rodata.sussurro,\"a\"\n.byte 1\n",
            ".rodata.sussurro",
        ),
        ("ligacao", ".globl g\ng:\n  ret\n", "g"),
        ("visibilidade", ".hidden f\n", "f"),
    ] {
        let alterado = inline_asm::artifact_surface(&montar_e_ler(
            &driver,
            rotulo,
            &format!("{ASM_BASE}{extra}"),
        ));
        let error = inline_asm::compare_artifact_surfaces(&baseline, &alterado)
            .expect_err(&format!("delta '{rotulo}' deveria ser recusado"));
        assert_eq!(error.code, E_ASM_ARTIFACT, "{rotulo}");
        assert!(
            error.detail.contains(esperado),
            "{rotulo}: '{esperado}' ausente em {}",
            error.detail
        );
    }
}

#[test]
fn verificacao_produtiva_aprova_envelope_legitimo() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let asm = asm_do_exemplo(include_str!(
        "../examples/hotfix_sussurro_multiplos_blocos_valido.pink"
    ));
    let dir = diretorio_temporario("produtiva");
    let check = inline_asm::verify_native_artifact(&asm, &driver, &dir)
        .expect("envelope legítimo não altera o artefato");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(check.envelopes, 2);
    assert!(check.sections > 0 && check.defined_symbols > 0);

    // Sem envelope, não há nada atribuível ao bloco e nada a inspecionar.
    let sem_bloco =
        asm_do_exemplo("pacote main;\ncarinho principal() -> bombom {\n    mimo 0;\n}\n");
    let dir = diretorio_temporario("produtiva_sem");
    let check =
        inline_asm::verify_native_artifact(&sem_bloco, &driver, &dir).expect("sem envelope");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(check.envelopes, 0);
}

/// Símbolos definidos do ELF final, pelo leitor próprio.
fn simbolos_definidos_do_binario(path: &Path) -> Vec<String> {
    let bytes = std::fs::read(path).expect("ler ELF final");
    let objeto = elf::parse(&bytes).expect("ler ELF final");
    let mut nomes: Vec<String> = inline_asm::artifact_surface(&objeto)
        .defined_symbols
        .into_iter()
        .map(|symbol| symbol.name)
        .collect();
    nomes.sort();
    nomes.dedup();
    nomes
}

fn build_nativo(runtime_lib: &Path, exemplo: &str, rotulo: &str) -> (PathBuf, String) {
    let out_dir = diretorio_temporario(rotulo);
    let build = Command::new(env!("CARGO_BIN_EXE_pink"))
        .args(["build", "--nativo", "--out-dir"])
        .arg(&out_dir)
        .arg(exemplo)
        .env("PINKER_RT_LIB", runtime_lib)
        .output()
        .expect("invocar pink build --nativo");
    assert!(
        build.status.success(),
        "build nativo de '{exemplo}' falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let nome = Path::new(exemplo)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("nome do exemplo");
    (
        out_dir.join(nome),
        String::from_utf8_lossy(&build.stdout).into_owned(),
    )
}

#[test]
fn build_nativo_real_verifica_o_artefato_e_o_elf_final_nao_ganha_simbolo() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };

    let (binario, stdout) = build_nativo(
        &runtime_lib,
        "examples/hotfix_sussurro_multiplos_blocos_valido.pink",
        "com",
    );
    // A verificação roda no caminho produtivo e diz o que inspecionou.
    assert!(
        stdout.contains("Artefato verificado") && stdout.contains("2 envelope(s) de 'sussurro'"),
        "build nativo não relatou a verificação de artefato:\n{stdout}"
    );

    // O mesmo programa sem `sussurro` é a baseline do ELF final.
    let dir = diretorio_temporario("fonte_sem");
    std::fs::create_dir_all(&dir).expect("diretório temporário");
    let sem = dir.join("sem_sussurro.pink");
    std::fs::write(
        &sem,
        "pacote main;\n\ncarinho principal() -> bombom {\n    nova antes: bombom = 20;\n    nova depois: bombom = 22;\n    falar(antes + depois);\n    mimo 0;\n}\n",
    )
    .expect("gravar fonte da baseline");
    let (binario_sem, _) = build_nativo(&runtime_lib, sem.to_str().expect("caminho"), "sem");

    let com_bloco = simbolos_definidos_do_binario(&binario);
    let sem_bloco = simbolos_definidos_do_binario(&binario_sem);
    let novos: Vec<&String> = com_bloco
        .iter()
        .filter(|nome| !sem_bloco.contains(nome))
        .collect();
    assert!(
        novos.is_empty(),
        "o bloco de 'sussurro' introduziu símbolos no ELF final: {novos:?}"
    );

    // O binário produzido continua sendo o programa correto.
    let saida = Command::new(&binario).output().expect("executar ELF final");
    assert_eq!(saida.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&saida.stdout).trim(), "42");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Guardião estrutural do cabo produtivo.
///
/// A verificação de artefato só tem valor se rodar no build real. Um teste que
/// chamasse apenas a função da biblioteca continuaria passando com o cabo
/// removido de `src/main.rs` — por isso a exigência é sobre a fonte da região
/// cartografada do build nativo.
#[test]
fn verificacao_de_artefato_esta_cabeada_no_build_nativo() {
    let fonte =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/main.rs"))
            .expect("ler src/main.rs");
    let inicio = fonte
        .find("@pinker-nav:start cli.build.nativo")
        .expect("região cli.build.nativo ausente");
    let fim = fonte
        .find("@pinker-nav:end cli.build.nativo")
        .expect("fim da região cli.build.nativo ausente");
    let regiao = &fonte[inicio..fim];

    assert!(
        regiao.contains("inline_asm::verify_native_artifact"),
        "a região do build nativo precisa chamar a verificação de artefato da biblioteca"
    );
    assert!(
        regiao.contains("verificar_artefato_sussurro(asm_path"),
        "link_nativo precisa verificar o artefato antes de linkar"
    );
    // A verificação precede a produção do binário: o `.s` é inspecionado antes
    // de o driver ser invocado para linkar.
    let posicao_verificacao = regiao
        .find("verificar_artefato_sussurro(asm_path")
        .expect("chamada da verificação");
    let posicao_link = regiao
        .find("std::process::Command::new(&driver)")
        .expect("invocação do driver de link");
    assert!(
        posicao_verificacao < posicao_link,
        "a verificação do artefato precisa acontecer antes da linkedição"
    );
}
// @pinker-nav:end evidencia.hotfix.sussurro-artefato
