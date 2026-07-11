//! Testes do pipeline nativo (Eixo B do Bloco 20, fase B1):
//! emissão `.s` com init de runtime e build/link/execução de executável real.

use pinker_v0::{
    backend_s, cfg_ir, cfg_ir_validate, instr_select, instr_select_validate, ir, ir_validate,
    lexer::Lexer, parser::Parser, semantic,
};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn lower_to_selected(code: &str) -> pinker_v0::instr_select::SelectedProgram {
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

#[test]
fn emissao_nativa_inclui_init_do_runtime() {
    let code = include_str!("../examples/fase212_build_nativo_fumaca_valido.pink");
    let selected = lower_to_selected(code);
    let nativo = backend_s::emit_external_toolchain_subset_nativo(&selected).expect("emit nativo");
    assert!(nativo.contains("call pinker_rt_iniciar"), "{}", nativo);
    assert!(nativo.contains(".globl main"), "{}", nativo);
}

#[test]
fn emissao_padrao_nao_inclui_init_do_runtime() {
    let code = include_str!("../examples/fase212_build_nativo_fumaca_valido.pink");
    let selected = lower_to_selected(code);
    let padrao = backend_s::emit_external_toolchain_subset(&selected).expect("emit padrao");
    assert!(!padrao.contains("pinker_rt_iniciar"), "{}", padrao);
}

#[test]
fn abi_completa_oito_args_usa_seis_registradores_e_pilha() {
    let code = include_str!("../examples/fase213_abi_completa_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    // 6 registradores de argumento em uso no spill do callee.
    for reg in ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"] {
        assert!(asm.contains(reg), "faltou {} em:\n{}", reg, asm);
    }
    // 7º e 8º argumentos: dois pushes no caller, dois loads de 16/24(%rbp)
    // no callee e limpeza de 16 bytes após o call.
    assert!(asm.matches("pushq %r10").count() >= 2, "{}", asm);
    assert!(asm.contains("movq 16(%rbp), %r10"), "{}", asm);
    assert!(asm.contains("movq 24(%rbp), %r10"), "{}", asm);
    assert!(asm.contains("addq $16, %rsp"), "{}", asm);
}

#[test]
fn abi_completa_sete_args_aplica_padding_de_alinhamento() {
    let code = r#"
        pacote main;
        carinho soma7(a: bombom, b: bombom, c: bombom, d: bombom, e: bombom, f: bombom, g: bombom) -> bombom {
            mimo a + b + c + d + e + f + g;
        }
        carinho principal() -> bombom {
            mimo soma7(1, 2, 3, 4, 5, 6, 7);
        }
    "#;
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    // 1 argumento de pilha (ímpar) exige padding de 8 bytes antes do push e
    // limpeza total de 16 bytes após o call.
    assert!(asm.contains("subq $8, %rsp"), "{}", asm);
    assert!(asm.contains("addq $16, %rsp"), "{}", asm);
}

#[test]
fn abi_completa_aceita_recursao_direta() {
    let code = r#"
        pacote main;
        carinho fatorial(n: bombom) -> bombom {
            talvez n < 2 {
                mimo 1;
            }
            mimo n * fatorial(n - 1);
        }
        carinho principal() -> bombom {
            mimo fatorial(5);
        }
    "#;
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    assert!(asm.contains("call fatorial"), "{}", asm);
}

#[test]
fn controle_fluxo_geral_ternario_vira_cmov_sem_call() {
    let code = r#"
        pacote main;
        carinho principal() -> bombom {
            nova a: bombom = 5;
            nova r: bombom = a > 3 ? 42 : 7;
            mimo r;
        }
    "#;
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    assert!(asm.contains("cmoveq %r10, %rax"), "{}", asm);
    assert!(!asm.contains("call __ternario"), "{}", asm);
}

#[test]
fn controle_fluxo_geral_emite_todos_os_construtos() {
    let code = include_str!("../examples/fase214_controle_fluxo_geral_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    // repetir/para/sempre que/escolha/encaixe/talvez viram blocos e saltos
    // reais; ternário vira cmov; nenhuma pseudo-função sobra no texto.
    assert!(asm.contains("cmoveq"), "{}", asm);
    assert!(asm.contains("call classifica"), "{}", asm);
    assert!(asm.contains("call pontua"), "{}", asm);
    assert!(!asm.contains("__ternario"), "{}", asm);
}

#[test]
fn verso_dinamico_emite_layout_length_prefixed_e_calls_de_runtime() {
    let code = include_str!("../examples/fase215_verso_dinamico_nativo_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    // Literais com header de tamanho em bytes.
    assert!(asm.contains(".quad 4"), "{}", asm); // "rosa"
    assert!(asm.contains(".ascii \"rosa\""), "{}", asm);
    // Operações e falar viram chamadas ao runtime nativo.
    for symbol in [
        "call pinker_verso_juntar",
        "call pinker_verso_tamanho",
        "call pinker_verso_igual",
        "call pinker_falar_pedaco_verso",
        "call pinker_falar_pedaco_bombom",
        "call pinker_falar_pedaco_logica",
        "call pinker_falar_espaco",
        "call pinker_falar_fim",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
}

#[test]
fn listas_nativas_emitem_calls_unificados_de_runtime() {
    let code = include_str!("../examples/fase216_listas_nativas_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    // lista<bombom>, lista<verso> e lista<Cor> abaixam para a MESMA família
    // de símbolos do runtime (elementos são palavras de 8 bytes).
    for symbol in [
        "call pinker_lista_criar",
        "call pinker_lista_anexar",
        "call pinker_lista_obter",
        "call pinker_lista_tamanho",
        "call pinker_lista_definir",
        "call pinker_lista_inserir",
        "call pinker_lista_tirar_ultimo",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
    // Nenhum nome monomorphizado sobra no texto final.
    assert!(!asm.contains("lista_bombom_"), "{}", asm);
    assert!(!asm.contains("lista_verso_"), "{}", asm);
}

#[test]
fn mapas_nativos_emitem_calls_unificados_de_runtime() {
    let code = include_str!("../examples/fase217_mapas_nativos_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    for symbol in [
        "call pinker_mapa_criar_chave_verso",
        "call pinker_mapa_criar_chave_bombom",
        "call pinker_mapa_definir",
        "call pinker_mapa_obter",
        "call pinker_mapa_tem",
        "call pinker_mapa_tamanho",
        "call pinker_mapa_remover",
        "call pinker_mapa_iterador_criar",
        "call pinker_mapa_iterador_proxima",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
    assert!(!asm.contains("mapa_verso_bombom_"), "{}", asm);
    assert!(!asm.contains("__pinker_internal_mapa"), "{}", asm);
}

#[test]
fn leques_com_carga_emitem_calls_unificados_de_runtime() {
    let code = include_str!("../examples/fase218_leques_carga_nativos_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    for symbol in [
        "call pinker_leque_criar_0",
        "call pinker_leque_anexar",
        "call pinker_leque_tag",
        "call pinker_leque_carga",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
    assert!(!asm.contains("__pinker_internal_leque"), "{}", asm);
}

fn detect_cc_driver() -> Option<String> {
    ["cc", "gcc", "clang"].iter().find_map(|candidate| {
        let probe = Command::new(candidate).arg("--version").output().ok()?;
        if probe.status.success() {
            Some((*candidate).to_string())
        } else {
            None
        }
    })
}

#[test]
fn build_nativo_produz_executavel_real_com_runtime() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        // `make ci` roda `cargo build` antes dos testes, garantindo a staticlib;
        // em invocações avulsas sem o artefato, o teste não é conclusivo.
        eprintln!("libpinker_rt.a ausente; pulando teste de build nativo");
        return;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase212_{}", nanos));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("examples/fase212_build_nativo_fumaca_valido.pink")
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let bin_path = out_dir.join("fase212_build_nativo_fumaca_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(
        run.status.code(),
        Some(42),
        "esperava código 42 do executável nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn abi_completa_executa_nativo_com_oito_args_aninhamento_e_recursao() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        eprintln!("libpinker_rt.a ausente; pulando teste executável da ABI");
        return;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase213_{}", nanos));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("examples/fase213_abi_completa_valido.pink")
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let bin_path = out_dir.join("fase213_abi_completa_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(
        run.status.code(),
        Some(42),
        "esperava soma8(1..7, zero()+8) + fatorial(3) = 42 no executável nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn verso_dinamico_nativo_tem_paridade_de_stdout_com_interpretador() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        eprintln!("libpinker_rt.a ausente; pulando teste de paridade de verso");
        return;
    }

    let exemplo = "examples/fase215_verso_dinamico_nativo_valido.pink";

    let interp = Command::new(pink)
        .arg("--run")
        .arg(exemplo)
        .output()
        .expect("falha ao rodar interpretador");
    assert!(interp.status.success());
    let interp_stdout = String::from_utf8_lossy(&interp.stdout);
    // O CLI imprime o valor de retorno de `principal` como última linha;
    // o stdout do programa em si é tudo antes dela.
    let programa_interp = interp_stdout
        .strip_suffix("0\n")
        .expect("esperava retorno 0 na última linha do interpretador");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase215_{}", nanos));
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

    let bin_path = out_dir.join("fase215_verso_dinamico_nativo_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(run.status.code(), Some(0));
    let nativo_stdout = String::from_utf8_lossy(&run.stdout);

    assert_eq!(
        programa_interp, nativo_stdout,
        "stdout do programa deve ser idêntico entre interpretador e nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn listas_nativas_tem_paridade_de_stdout_com_interpretador() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        eprintln!("libpinker_rt.a ausente; pulando teste de paridade de listas");
        return;
    }

    let exemplo = "examples/fase216_listas_nativas_valido.pink";

    let interp = Command::new(pink)
        .arg("--run")
        .arg(exemplo)
        .output()
        .expect("falha ao rodar interpretador");
    assert!(interp.status.success());
    let interp_stdout = String::from_utf8_lossy(&interp.stdout);
    let programa_interp = interp_stdout
        .strip_suffix("0\n")
        .expect("esperava retorno 0 na última linha do interpretador");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase216_{}", nanos));
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

    let bin_path = out_dir.join("fase216_listas_nativas_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(run.status.code(), Some(0));
    let nativo_stdout = String::from_utf8_lossy(&run.stdout);

    assert_eq!(
        programa_interp, nativo_stdout,
        "stdout de listas deve ser idêntico entre interpretador e nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn mapas_nativos_tem_paridade_de_stdout_com_interpretador() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        eprintln!("libpinker_rt.a ausente; pulando teste de paridade de mapas");
        return;
    }

    let exemplo = "examples/fase217_mapas_nativos_valido.pink";

    let interp = Command::new(pink)
        .arg("--run")
        .arg(exemplo)
        .output()
        .expect("falha ao rodar interpretador");
    assert!(interp.status.success());
    let interp_stdout = String::from_utf8_lossy(&interp.stdout);
    let programa_interp = interp_stdout
        .strip_suffix("0\n")
        .expect("esperava retorno 0 na última linha do interpretador");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase217_{}", nanos));
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

    let bin_path = out_dir.join("fase217_mapas_nativos_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(run.status.code(), Some(0));
    let nativo_stdout = String::from_utf8_lossy(&run.stdout);

    assert_eq!(
        programa_interp, nativo_stdout,
        "stdout de mapas deve ser idêntico entre interpretador e nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}

fn paridade_stdout(exemplo: &str, bin_nome: &str, marcador: u128) {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        eprintln!("libpinker_rt.a ausente; pulando teste de paridade");
        return;
    }

    let interp = Command::new(pink)
        .arg("--run")
        .arg(exemplo)
        .output()
        .expect("falha ao rodar interpretador");
    assert!(interp.status.success());
    let interp_stdout = String::from_utf8_lossy(&interp.stdout);
    let programa_interp = interp_stdout
        .strip_suffix("0\n")
        .expect("esperava retorno 0 na última linha do interpretador");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos()
        + marcador;
    let out_dir = std::env::temp_dir().join(format!("pinker_paridade_{}", nanos));
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

    let bin_path = out_dir.join(bin_nome);
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(run.status.code(), Some(0), "exit do nativo");
    let nativo_stdout = String::from_utf8_lossy(&run.stdout);

    assert_eq!(
        programa_interp, nativo_stdout,
        "stdout deve ser idêntico entre interpretador e nativo para {}",
        exemplo
    );

    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn leques_com_carga_tem_paridade_de_stdout_com_interpretador() {
    paridade_stdout(
        "examples/fase218_leques_carga_nativos_valido.pink",
        "fase218_leques_carga_nativos_valido",
        1,
    );
}

#[test]
fn avaliador_recursivo_da_fase210_executa_nativo_com_paridade() {
    paridade_stdout(
        "examples/fase210_leque_recursivo_avaliador_valido.pink",
        "fase210_leque_recursivo_avaliador_valido",
        2,
    );
}

#[test]
fn controle_fluxo_geral_executa_nativo_com_todos_os_construtos() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    if detect_cc_driver().is_none() {
        return;
    }
    let pink = env!("CARGO_BIN_EXE_pink");
    let runtime_lib = std::path::Path::new(pink)
        .parent()
        .expect("diretório do pink")
        .join("libpinker_rt.a");
    if !runtime_lib.is_file() {
        eprintln!("libpinker_rt.a ausente; pulando teste executável de controle de fluxo");
        return;
    }

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos();
    let out_dir = std::env::temp_dir().join(format!("pinker_fase214_{}", nanos));

    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("examples/fase214_controle_fluxo_geral_valido.pink")
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let bin_path = out_dir.join("fase214_controle_fluxo_geral_valido");
    let run = Command::new(bin_path)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(
        run.status.code(),
        Some(42),
        "repetir/para/sempre que/escolha/encaixe/ternário/talvez aninhado deviam compor 42"
    );

    let _ = fs::remove_dir_all(&out_dir);
}
