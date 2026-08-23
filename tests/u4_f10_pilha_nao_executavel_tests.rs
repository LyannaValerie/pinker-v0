//! U4/F-10: o backend nativo não pode exigir pilha executável.
//!
//! A evidência aqui é o artefato ELF real, não substring do `.s`: o objeto é
//! montado pelo driver C e lido por `pinker_v0::elf`, e o executável final é
//! lido pela tabela de segmentos. Um teste que só procurasse `.note.GNU-stack`
//! no texto emitido continuaria verde se o assembler passasse a ignorar a
//! diretiva.

mod common;

use common::ControlledCommand as Command;
use pinker_v0::{
    backend_s, cfg_ir, cfg_ir_validate, elf, instr_select, instr_select_validate, ir, ir_validate,
    lexer::Lexer, parser::Parser, semantic,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.u4-f10.suporte
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Suporte da evidência F-10: lower_to_selected encadeia Lexer → Parser → semântica → IR → CFG → seleção em memória; diretorio_efemero cria um diretório único por caso; montar_objeto invoca o driver C detectado com `-c` sobre um `.s` e devolve os bytes do objeto produzido; secoes_do_objeto delega a leitura a pinker_v0::elf::parse. Região de suporte, sem ownership direto de testes; nenhuma asserção vive aqui.
fn lower_to_selected(code: &str) -> instr_select::SelectedProgram {
    let mut lexer = Lexer::new(code);
    let tokens = lexer.tokenize().expect("lex");
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("parse");
    semantic::check_program(&program).expect("semantic");
    let program_ir = ir::lower_program(&program).expect("ir");
    ir_validate::validate_program(&program_ir).expect("ir validate");
    let cfg = cfg_ir::lower_program(&program_ir).expect("cfg");
    cfg_ir_validate::validate_program(&cfg).expect("cfg validate");
    let selected = instr_select::lower_program(&cfg).expect("select");
    instr_select_validate::validate_program(&selected).expect("select validate");
    selected
}

fn diretorio_efemero(rotulo: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("pinker_u4_f10_{rotulo}_{nanos}"));
    fs::create_dir_all(&dir).expect("criar diretório efêmero");
    dir
}

fn montar_objeto(driver: &str, dir: &Path, asm: &str) -> Vec<u8> {
    let asm_path = dir.join("unidade.s");
    let obj_path = dir.join("unidade.o");
    fs::write(&asm_path, asm).expect("gravar .s");
    let saida = Command::new(driver)
        .arg("-c")
        .arg(&asm_path)
        .arg("-o")
        .arg(&obj_path)
        .output()
        .expect("invocar driver C");
    assert!(
        saida.status.success(),
        "montagem falhou: {}",
        String::from_utf8_lossy(&saida.stderr)
    );
    fs::read(&obj_path).expect("ler objeto")
}

fn secoes_do_objeto(bytes: &[u8]) -> Vec<String> {
    elf::parse(bytes).expect("ler objeto ELF").sections
}
// @pinker-nav:end evidencia.u4-f10.suporte

// @pinker-nav:start evidencia.u4-f10.objeto-nao-pede-pilha-executavel
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova, sobre o objeto REAL montado pelo driver C, que os dois emissores montáveis — emit_external_toolchain_subset (hospedado) e emit_external_toolchain_subset_nativo — produzem uma unidade com a seção `.note.GNU-stack`, e que ela aparece exatamente uma vez por unidade, não uma vez por função. Exercita quatro formas de programa (trivial, função de usuário além de `principal`, ABI completa e controle de fluxo). Não linka nem executa; a propagação para o executável é provada em outra região. Guarda silenciosa de plataforma/driver via require_native_evidence.
const FONTES_DE_FORMA: &[(&str, &str)] = &[
    (
        "trivial",
        include_str!("../examples/fase212_build_nativo_fumaca_valido.pink"),
    ),
    (
        "abi_completa",
        include_str!("../examples/fase213_abi_completa_valido.pink"),
    ),
    (
        "controle_fluxo",
        include_str!("../examples/fase214_controle_fluxo_geral_valido.pink"),
    ),
];

#[test]
fn objeto_gerado_declara_nao_exigir_pilha_executavel() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };
    let dir = diretorio_efemero("objeto");

    for (rotulo, fonte) in FONTES_DE_FORMA {
        let selected = lower_to_selected(fonte);
        for (caminho, asm) in [
            (
                "hospedado",
                backend_s::emit_external_toolchain_subset(&selected).expect("emit hospedado"),
            ),
            (
                "nativo",
                backend_s::emit_external_toolchain_subset_nativo(&selected).expect("emit nativo"),
            ),
        ] {
            let caso = dir.join(format!("{rotulo}_{caminho}"));
            fs::create_dir_all(&caso).expect("criar caso");
            let secoes = secoes_do_objeto(&montar_objeto(&driver, &caso, &asm));
            let ocorrencias = secoes.iter().filter(|s| *s == ".note.GNU-stack").count();
            assert_eq!(
                ocorrencias, 1,
                "objeto {rotulo}/{caminho} deveria declarar `.note.GNU-stack` \
                 exatamente uma vez por unidade; seções: {secoes:?}"
            );
        }
    }

    let _ = fs::remove_dir_all(&dir);
}
// @pinker-nav:end evidencia.u4-f10.objeto-nao-pede-pilha-executavel

// @pinker-nav:start evidencia.u4-f10.executavel-final-sem-pilha-executavel
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Prova sobre o EXECUTÁVEL real produzido por `pink build --nativo` com libpinker_rt.a ligada: a tabela de segmentos contém PT_GNU_STACK e ele NÃO tem PF_X. Como o linker propaga o requisito de pilha por OU entre todas as unidades de entrada, um PT_GNU_STACK não executável também prova que nenhum membro do runtime archive exige pilha executável — a compatibilidade do archive é observada no artefato, não presumida. O binário é executado em seguida para provar que a mudança não quebrou o programa. Guarda silenciosa de plataforma/driver/staticlib.
fn segmento_gnu_stack(binario: &Path) -> elf::ElfProgramHeader {
    let bytes = fs::read(binario).expect("ler executável");
    let segmentos = elf::parse_program_headers(&bytes).expect("ler segmentos");
    *segmentos
        .iter()
        .find(|s| s.p_type == elf::PT_GNU_STACK)
        .unwrap_or_else(|| {
            panic!("executável sem PT_GNU_STACK: o requisito de pilha ficaria indefinido")
        })
}

#[test]
fn executavel_nativo_tem_gnu_stack_nao_executavel() {
    let Some((_driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let pink = env!("CARGO_BIN_EXE_pink");
    let dir = diretorio_efemero("executavel");

    for (exemplo, codigo_esperado) in [
        ("fase212_build_nativo_fumaca_valido", 42),
        ("fase213_abi_completa_valido", 42),
        ("fase219_texto_nativo_valido", 0),
        ("fase247_sussurro_inline_asm_real_valido", 0),
    ] {
        let build = Command::new(pink)
            .arg("build")
            .arg("--nativo")
            .arg("--out-dir")
            .arg(&dir)
            .arg(format!("examples/{exemplo}.pink"))
            .env("PINKER_RT_LIB", &runtime_lib)
            .output()
            .expect("invocar pink build");
        assert!(
            build.status.success(),
            "build nativo de {exemplo} falhou: {}",
            String::from_utf8_lossy(&build.stderr)
        );

        let binario = dir.join(exemplo);
        let gnu_stack = segmento_gnu_stack(&binario);
        assert!(
            !gnu_stack.is_executable(),
            "executável de {exemplo} exige pilha executável: PT_GNU_STACK flags={:#x}",
            gnu_stack.flags
        );

        let execucao = Command::new(&binario)
            .output()
            .expect("executar binário nativo");
        assert_eq!(
            execucao.status.code(),
            Some(codigo_esperado),
            "{exemplo} não executou corretamente: {}",
            String::from_utf8_lossy(&execucao.stderr)
        );
    }

    let _ = fs::remove_dir_all(&dir);
}
// @pinker-nav:end evidencia.u4-f10.executavel-final-sem-pilha-executavel

// @pinker-nav:start evidencia.u4-f10.sensibilidade
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Injeção de sensibilidade: remove a linha `.note.GNU-stack` do assembly REALMENTE emitido, monta e linka a unidade mutilada contra libpinker_rt.a e prova que o executável resultante volta a declarar PT_GNU_STACK executável. Primeiro confirma que a injeção de fato se aplicou (a linha existia e saiu), de modo que um emissor que parasse de emitir a diretiva faria este teste falhar em vez de passar por vacuidade. É a prova de que o gate de artefato tem dentes e de que a diretiva é a causa, não uma coincidência.
#[test]
fn remover_a_diretiva_faz_o_executavel_voltar_a_exigir_pilha_executavel() {
    let Some((driver, Some(runtime_lib))) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), true)
    else {
        return;
    };
    let dir = diretorio_efemero("sensibilidade");

    let fonte = include_str!("../examples/fase212_build_nativo_fumaca_valido.pink");
    let selected = lower_to_selected(fonte);
    let asm = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("emit nativo");

    // A injeção precisa ter se aplicado de verdade antes de o resultado valer.
    let mutilado: String = asm
        .lines()
        .filter(|linha| !linha.contains(".note.GNU-stack"))
        .map(|linha| format!("{linha}\n"))
        .collect();
    assert_ne!(
        asm, mutilado,
        "injeção de sensibilidade não se aplicou: o emissor não produziu `.note.GNU-stack`"
    );

    let objeto = dir.join("mutilado.o");
    let asm_path = dir.join("mutilado.s");
    fs::write(&asm_path, &mutilado).expect("gravar .s mutilado");
    let montagem = Command::new(&driver)
        .arg("-c")
        .arg(&asm_path)
        .arg("-o")
        .arg(&objeto)
        .output()
        .expect("montar objeto mutilado");
    assert!(
        montagem.status.success(),
        "montagem do objeto mutilado falhou: {}",
        String::from_utf8_lossy(&montagem.stderr)
    );

    let secoes = secoes_do_objeto(&fs::read(&objeto).expect("ler objeto mutilado"));
    assert!(
        !secoes.iter().any(|s| s == ".note.GNU-stack"),
        "objeto mutilado ainda declara `.note.GNU-stack`: {secoes:?}"
    );

    let binario = dir.join("mutilado");
    let link = Command::new(&driver)
        .arg(&objeto)
        .arg(&runtime_lib)
        .arg("-lpthread")
        .arg("-ldl")
        .arg("-lm")
        .arg("-o")
        .arg(&binario)
        .output()
        .expect("linkar objeto mutilado");
    assert!(
        link.status.success(),
        "linkedição do objeto mutilado falhou: {}",
        String::from_utf8_lossy(&link.stderr)
    );

    let gnu_stack = segmento_gnu_stack(&binario);
    assert!(
        gnu_stack.is_executable(),
        "sem a diretiva o executável deveria voltar a exigir pilha executável, \
         mas PT_GNU_STACK flags={:#x}; o gate perdeu poder de detecção",
        gnu_stack.flags
    );

    let _ = fs::remove_dir_all(&dir);
}
// @pinker-nav:end evidencia.u4-f10.sensibilidade
