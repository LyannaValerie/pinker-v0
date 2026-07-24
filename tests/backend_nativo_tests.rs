//! Testes do pipeline nativo (Eixo B do Bloco 20, fase B1):
//! emissão `.s` com init de runtime e build/link/execução de executável real.

use pinker_v0::{
    backend_s, cfg_ir, cfg_ir_validate, instr_select, instr_select_validate, ir, ir_validate,
    lexer::Lexer, parser::Parser, semantic,
};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidencia.backend-nativo.suporte-lowering-memoria
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Helper local lower_to_selected: encadeia Lexer → Parser → semântica → IR → CFG → seleção de instruções inteiramente em memória e devolve um SelectedProgram. Não chama nenhum emissor, não cria processo, não monta, não linka e não executa; não utiliza libpinker_rt.a. Região de suporte, sem ownership direto de testes.
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
// @pinker-nav:end evidencia.backend-nativo.suporte-lowering-memoria

// @pinker-nav:start evidencia.backend-nativo.emissao-init-runtime
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Comparação textual entre os dois emissores sobre o mesmo programa: emit_external_toolchain_subset_nativo usa runtime_init=true e emit_external_toolchain_subset usa runtime_init=false; a única diferença comprovada é a presença de `call pinker_rt_iniciar` no prólogo de `main`. Nenhuma montagem, linkedição ou execução ocorre — `contains` sobre o texto emitido não prova comportamento de máquina.
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
// @pinker-nav:end evidencia.backend-nativo.emissao-init-runtime

// @pinker-nav:start evidencia.backend-nativo.emissao-abi-e-fluxo-textual
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Cinco testes que chamam emit_external_toolchain_subset — caminho HOSPEDADO, runtime_init=false — e verificam apenas o texto emitido para a ABI SysV (seis registradores de argumento e passagem por pilha), o padding de alinhamento de pilha, a recursão direta, o `cmov` do ternário e os saltos dos construtos de controle de fluxo. Nenhuma toolchain externa é invocada, nenhum runtime é ligado e nada é executado.
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
// @pinker-nav:end evidencia.backend-nativo.emissao-abi-e-fluxo-textual

// @pinker-nav:start evidencia.backend-nativo.emissao-simbolos-runtime-textual
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Sete testes que, apesar dos nomes “nativo”, chamam emit_external_toolchain_subset — caminho HOSPEDADO, runtime_init=false — e apenas procuram referências textuais a símbolos `pinker_*` para verso, listas, mapas, leques, texto, arquivos, tempo, acaso, ambiente e processos. Não localizam nem ligam libpinker_rt.a, não montam, não linkam e não executam; a presença textual de um símbolo não prova a implementação funcional do runtime.
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

#[test]
fn texto_nativo_emite_calls_de_runtime_e_formatar_por_aridade() {
    let code = include_str!("../examples/fase219_texto_nativo_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    for symbol in [
        "call pinker_verso_aparar",
        "call pinker_verso_minusculo",
        "call pinker_verso_maiusculo",
        "call pinker_verso_contem",
        "call pinker_verso_comeca_com",
        "call pinker_verso_termina_com",
        "call pinker_verso_indice_em",
        "call pinker_verso_buscar",
        "call pinker_verso_indice",
        "call pinker_verso_dividir_contar",
        "call pinker_verso_dividir_em",
        "call pinker_verso_substituir",
        "call pinker_verso_juntar_com",
        "call pinker_verso_para_bombom",
        "call pinker_bombom_para_verso",
        "call pinker_verso_vazio",
        "call pinker_verso_nao_vazio",
        "call pinker_formatar_verso_2",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
    assert!(!asm.contains("call formatar_verso"), "{}", asm);
}

#[test]
fn arquivo_tempo_acaso_emitem_calls_de_runtime() {
    let code = include_str!("../examples/fase220_arquivo_tempo_acaso_nativos_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    for symbol in [
        "call pinker_arquivo_criar",
        "call pinker_arquivo_escrever_verso",
        "call pinker_arquivo_fechar",
        "call pinker_arquivo_ler_caminho_verso",
        "call pinker_arquivo_abrir_anexo",
        "call pinker_arquivo_anexar_verso",
        "call pinker_arquivo_ou",
        "call pinker_arquivo_copiar",
        "call pinker_caminho_juntar",
        "call pinker_caminho_existe",
        "call pinker_caminho_e_arquivo",
        "call pinker_caminho_tamanho_arquivo",
        "call pinker_caminho_remover_arquivo",
        "call pinker_formatar_tempo_unix",
        "call pinker_aleatorio_criar",
        "call pinker_aleatorio_proximo",
        "call pinker_aleatorio_entre",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
}

#[test]
fn ambiente_e_processo_emitem_calls_de_runtime() {
    let code = include_str!("../examples/fase221_ambiente_processo_nativos_valido.pink");
    let selected = lower_to_selected(code);
    let asm = backend_s::emit_external_toolchain_subset(&selected).expect("emit");
    for symbol in [
        "call pinker_ambiente_quantos_argumentos",
        "call pinker_ambiente_argumento_ou",
        "call pinker_ambiente_tem_flag",
        "call pinker_ambiente_pedir_argumento",
        "call pinker_ambiente_ou",
        "call pinker_ambiente_buscar_contexto",
        "call pinker_processo_executar_1",
        "call pinker_processo_capturar_stdout_2",
        "call pinker_processo_com_entrada_2",
        "call pinker_processo_pipeline",
    ] {
        assert!(asm.contains(symbol), "faltou {} em:\n{}", symbol, asm);
    }
}
// @pinker-nav:end evidencia.backend-nativo.emissao-simbolos-runtime-textual

// @pinker-nav:start evidencia.backend-nativo.suporte-driver-c
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Helper local detect_cc_driver: sonda `cc`, `gcc` e `clang` por `--version` e devolve Option<String> com o primeiro que responder. É diferente do homônimo produtivo de src/main.rs, que devolve Result; este helper é a origem da guarda silenciosa de driver herdada pelos testes processuais e não monta por si próprio. Região de suporte, sem ownership direto de testes.
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
// @pinker-nav:end evidencia.backend-nativo.suporte-driver-c

// @pinker-nav:start evidencia.backend-nativo.execucao-exit-fumaca-abi
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Dois testes que executam `pink build --nativo` fornecendo PINKER_RT_LIB: o driver C externo monta e linka, libpinker_rt.a é ligada e o ELF resultante é executado, mas somente `status.code() == 42` é validado — stdout não é comparado e stderr aparece apenas como mensagem de falha. Três guardas silenciosas (Linux x86_64, driver C e libpinker_rt.a) fazem com que a suíte possa passar sem exercer esta evidência.
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
// @pinker-nav:end evidencia.backend-nativo.execucao-exit-fumaca-abi

// @pinker-nav:start evidencia.backend-nativo.paridade-stdout-colecoes
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Três testes (verso dinâmico, listas e mapas) que usam o interpretador como oráculo, fazem build nativo e executam o ELF comparando stdout; o exit é comparado apenas à constante 0 e o `strip_suffix("0\n")` pressupõe retorno zero, de modo que não existe paridade de exit contra o retorno observado. Três guardas silenciosas (plataforma, driver C e libpinker_rt.a) — a suíte pode passar sem exercer esta evidência.
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
// @pinker-nav:end evidencia.backend-nativo.paridade-stdout-colecoes

// @pinker-nav:start evidencia.backend-nativo.suporte-matriz-paridade-b11
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Suporte contíguo da matriz B11: ParidadeNativaCaso, ARGVS_FASE221, CASOS_PARIDADE_B11 com 14 casos sobre 13 exemplos distintos (um caso adicional da fase221 com argv), separar_stdout_e_retorno_interpretador — normalização genérica do retorno do interpretador — e paridade_stdout_e_exit, que compara stdout e exit sob as três guardas. Um único #[test] consome todos os casos e uma falha antecipada impede os casos seguintes; região de suporte, sem ownership direto de testes.
#[derive(Clone, Copy)]
struct ParidadeNativaCaso {
    exemplo: &'static str,
    bin_nome: &'static str,
    argv: &'static [&'static str],
}

const ARGVS_FASE221: &[&str] = &[
    "primeiro",
    "--modo",
    "--saida=custom.txt",
    "--nivel",
    "alto",
];

const CASOS_PARIDADE_B11: &[ParidadeNativaCaso] = &[
    ParidadeNativaCaso {
        exemplo: "examples/fase212_build_nativo_fumaca_valido.pink",
        bin_nome: "fase212_build_nativo_fumaca_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase213_abi_completa_valido.pink",
        bin_nome: "fase213_abi_completa_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase214_controle_fluxo_geral_valido.pink",
        bin_nome: "fase214_controle_fluxo_geral_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase215_verso_dinamico_nativo_valido.pink",
        bin_nome: "fase215_verso_dinamico_nativo_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase216_listas_nativas_valido.pink",
        bin_nome: "fase216_listas_nativas_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase217_mapas_nativos_valido.pink",
        bin_nome: "fase217_mapas_nativos_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase218_leques_carga_nativos_valido.pink",
        bin_nome: "fase218_leques_carga_nativos_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase219_texto_nativo_valido.pink",
        bin_nome: "fase219_texto_nativo_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase220_arquivo_tempo_acaso_nativos_valido.pink",
        bin_nome: "fase220_arquivo_tempo_acaso_nativos_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase221_ambiente_processo_nativos_valido.pink",
        bin_nome: "fase221_ambiente_processo_nativos_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase221_ambiente_processo_nativos_valido.pink",
        bin_nome: "fase221_ambiente_processo_nativos_valido",
        argv: ARGVS_FASE221,
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase209_lexer_brinquedo_valido.pink",
        bin_nome: "fase209_lexer_brinquedo_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase210_leque_recursivo_avaliador_valido.pink",
        bin_nome: "fase210_leque_recursivo_avaliador_valido",
        argv: &[],
    },
    ParidadeNativaCaso {
        exemplo: "examples/fase211_compilador_brinquedo_valido.pink",
        bin_nome: "fase211_compilador_brinquedo_valido",
        argv: &[],
    },
];

fn separar_stdout_e_retorno_interpretador(stdout: &str) -> (&str, i32) {
    let sem_quebra_final = stdout
        .strip_suffix('\n')
        .expect("stdout do interpretador deve terminar com quebra de linha");
    let (programa_stdout, retorno) = match sem_quebra_final.rsplit_once('\n') {
        Some((prefixo, ultima)) => (&stdout[..prefixo.len() + 1], ultima),
        None => ("", sem_quebra_final),
    };
    let retorno = retorno
        .parse::<i32>()
        .expect("última linha do interpretador deve ser o retorno numérico de principal");
    (programa_stdout, retorno)
}

fn paridade_stdout_e_exit(caso: ParidadeNativaCaso, marcador: u128) {
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
        eprintln!("libpinker_rt.a ausente; pulando teste de paridade B11");
        return;
    }

    let mut interp_cmd = Command::new(pink);
    interp_cmd.arg("--run").arg(caso.exemplo);
    if !caso.argv.is_empty() {
        interp_cmd.arg("--").args(caso.argv);
    }
    let interp = interp_cmd.output().expect("falha ao rodar interpretador");
    assert!(
        interp.status.success(),
        "interpretador falhou para {}: {}",
        caso.exemplo,
        String::from_utf8_lossy(&interp.stderr)
    );
    let interp_stdout = String::from_utf8_lossy(&interp.stdout);
    let (programa_interp, retorno_interp) = separar_stdout_e_retorno_interpretador(&interp_stdout);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema")
        .as_nanos()
        + marcador;
    let out_dir = std::env::temp_dir().join(format!("pinker_paridade_b11_{}", nanos));
    let build = Command::new(pink)
        .arg("build")
        .arg("--nativo")
        .arg("--out-dir")
        .arg(&out_dir)
        .arg(caso.exemplo)
        .env("PINKER_RT_LIB", &runtime_lib)
        .output()
        .expect("falha ao invocar pink build");
    assert!(
        build.status.success(),
        "build nativo falhou para {}: {}",
        caso.exemplo,
        String::from_utf8_lossy(&build.stderr)
    );

    let bin_path = out_dir.join(caso.bin_nome);
    let run = Command::new(bin_path)
        .args(caso.argv)
        .output()
        .expect("falha ao executar binário nativo");
    let nativo_stdout = String::from_utf8_lossy(&run.stdout);

    assert_eq!(
        run.status.code(),
        Some(retorno_interp),
        "exit deve ser idêntico ao retorno de principal para {}",
        caso.exemplo
    );
    assert_eq!(
        programa_interp, nativo_stdout,
        "stdout deve ser idêntico entre interpretador e nativo para {}",
        caso.exemplo
    );

    let _ = fs::remove_dir_all(&out_dir);
}
// @pinker-nav:end evidencia.backend-nativo.suporte-matriz-paridade-b11

// @pinker-nav:start evidencia.backend-nativo.paridade-marco-b11
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Um único #[test] percorre os 14 casos de CASOS_PARIDADE_B11 sobre 13 exemplos distintos, incluindo um caso com argv: é o único ponto do arquivo que compara o exit do ELF com o retorno realmente observado no interpretador, além de comparar stdout. Não equivale a 14 testes independentes — sob as três guardas o laço inteiro pode virar no-op e permanecer verde.
#[test]
fn b11_marco_de_paridade_executa_exemplos_versionados_compativeis() {
    for (indice, caso) in CASOS_PARIDADE_B11.iter().copied().enumerate() {
        paridade_stdout_e_exit(caso, 10_000 + indice as u128);
    }
}
// @pinker-nav:end evidencia.backend-nativo.paridade-marco-b11

// @pinker-nav:start evidencia.backend-nativo.suporte-paridade-stdout
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Helper paridade_stdout, com 25 chamadores: executa o interpretador, faz o build nativo e roda o ELF comparando stdout, mas fixa o exit esperado em 0 usando `strip_suffix("0\n")` e nunca passa argv. Concentra as três guardas silenciosas herdadas pelos chamadores; um retorno terminado em zero, como 10, pode gerar diagnóstico enganoso. Região de suporte, sem ownership direto de testes.
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
// @pinker-nav:end evidencia.backend-nativo.suporte-paridade-stdout

// @pinker-nav:start evidencia.backend-nativo.paridade-stdout-programas-maiores
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Sete programas versionados maiores — leques com carga, avaliador recursivo da fase210, texto, compilador de brinquedo da fase211, lexer de brinquedo da fase209, fase220 com arquivos, tempo fixo e RNG de semente fixa, e fase221 dependente de `true`, `false`, `echo` e `cat` — delegam a paridade_stdout: stdout é comparado e o exit é fixado na constante 0. Três guardas silenciosas — a suíte pode passar sem exercer esta evidência.
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
fn texto_nativo_tem_paridade_de_stdout_com_interpretador() {
    paridade_stdout(
        "examples/fase219_texto_nativo_valido.pink",
        "fase219_texto_nativo_valido",
        3,
    );
}

#[test]
fn compilador_de_brinquedo_da_fase211_executa_nativo_com_paridade() {
    paridade_stdout(
        "examples/fase211_compilador_brinquedo_valido.pink",
        "fase211_compilador_brinquedo_valido",
        4,
    );
}

#[test]
fn lexer_de_brinquedo_da_fase209_executa_nativo_com_paridade() {
    paridade_stdout(
        "examples/fase209_lexer_brinquedo_valido.pink",
        "fase209_lexer_brinquedo_valido",
        5,
    );
}

#[test]
fn arquivo_tempo_acaso_tem_paridade_de_stdout_com_interpretador() {
    paridade_stdout(
        "examples/fase220_arquivo_tempo_acaso_nativos_valido.pink",
        "fase220_arquivo_tempo_acaso_nativos_valido",
        6,
    );
}

#[test]
fn ambiente_processo_tem_paridade_de_stdout_sem_args() {
    paridade_stdout(
        "examples/fase221_ambiente_processo_nativos_valido.pink",
        "fase221_ambiente_processo_nativos_valido",
        7,
    );
}
// @pinker-nav:end evidencia.backend-nativo.paridade-stdout-programas-maiores

// @pinker-nav:start evidencia.backend-nativo.paridade-argv
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Único teste que passa o mesmo argv ao interpretador e ao ELF, comprovando a captura de argc/argv pelo runtime para este exemplo: compara stdout, mantém o exit fixado na constante 0 e depende dos processos externos usados pela fase221. Sob as três guardas silenciosas, pode passar sem exercer a evidência.
#[test]
fn ambiente_nativo_le_argv_com_paridade() {
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
        eprintln!("libpinker_rt.a ausente; pulando teste de argv");
        return;
    }

    let exemplo = "examples/fase221_ambiente_processo_nativos_valido.pink";
    let argv = [
        "primeiro",
        "--modo",
        "--saida=custom.txt",
        "--nivel",
        "alto",
    ];

    let interp = Command::new(pink)
        .arg("--run")
        .arg(exemplo)
        .arg("--")
        .args(argv)
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
    let out_dir = std::env::temp_dir().join(format!("pinker_fase221_argv_{}", nanos));
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

    let bin_path = out_dir.join("fase221_ambiente_processo_nativos_valido");
    let run = Command::new(bin_path)
        .args(argv)
        .output()
        .expect("falha ao executar binário nativo");
    assert_eq!(run.status.code(), Some(0));
    let nativo_stdout = String::from_utf8_lossy(&run.stdout);

    assert_eq!(
        programa_interp, nativo_stdout,
        "argv/env devem produzir stdout idêntico entre interpretador e nativo"
    );

    let _ = fs::remove_dir_all(&out_dir);
}
// @pinker-nav:end evidencia.backend-nativo.paridade-argv

// @pinker-nav:start evidencia.backend-nativo.execucao-exit-controle-fluxo
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Único teste que faz build nativo e execução real cobrindo construções gerais de controle de fluxo, validando somente `status.code() == 42` e não comparando stdout. Está fisicamente entre blocos de paridade, mas não pertence a eles; sob as três guardas silenciosas pode passar sem exercer a evidência.
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
// @pinker-nav:end evidencia.backend-nativo.execucao-exit-controle-fluxo

// @pinker-nav:start evidencia.backend-nativo.paridade-stdout-fases-avancadas
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidencia
// @pinker-nav:summary Dezoito testes das fases 223–240 — error handling, funções anônimas, tratos, impls, mapas e funções genéricas — em ordem física não monotônica, todos delegando a paridade_stdout: stdout é comparado e o exit é fixado na constante 0. Três guardas silenciosas fazem com que a suíte possa passar sem exercer esta evidência; o conjunto não declara paridade completa da linguagem.
#[test]
fn fase223_tentar_error_handling_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase223_error_handling_tentar_valido.pink",
        "fase223_error_handling_tentar_valido",
        22_300,
    );
}

#[test]
fn fase224_propagar_error_handling_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase224_error_handling_propagar_valido.pink",
        "fase224_error_handling_propagar_valido",
        22_400,
    );
}

#[test]
fn fase225_carinho_anonimo_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase225_carinho_anonimo_valido.pink",
        "fase225_carinho_anonimo_valido",
        22_500,
    );
}

#[test]
fn fase226_trato_metodo_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase226_trato_metodo_valido.pink",
        "fase226_trato_metodo_valido",
        22_600,
    );
}

#[test]
fn fase227_impl_trato_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase227_impl_trato_valido.pink",
        "fase227_impl_trato_valido",
        22_700,
    );
}

#[test]
fn fase228_impl_resolucao_nominal_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase228_impl_resolucao_nominal_valido.pink",
        "fase228_impl_resolucao_nominal_valido",
        22_800,
    );
}

#[test]
fn fase229_impl_ninho_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase229_impl_ninho_valido.pink",
        "fase229_impl_ninho_valido",
        22_900,
    );
}

#[test]
fn fase230_impl_cobertura_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase230_impl_cobertura_valido.pink",
        "fase230_impl_cobertura_valido",
        23_000,
    );
}

#[test]
fn fase231_propagar_valor_nomeado_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase231_propagar_valor_nomeado_valido.pink",
        "fase231_propagar_valor_nomeado_valido",
        23_100,
    );
}

#[test]
fn fase237_propagar_curto_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase237_propagar_curto_valido.pink",
        "fase237_propagar_curto_valido",
        23_700,
    );
}

#[test]
fn fase238_funcao_local_valor_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase238_funcao_local_valor_valido.pink",
        "fase238_funcao_local_valor_valido",
        23_800,
    );
}

#[test]
fn fase239_funcao_parametro_estatica_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase239_funcao_parametro_estatica_valido.pink",
        "fase239_funcao_parametro_estatica_valido",
        23_900,
    );
}

#[test]
fn fase240_leque_generico_resultado_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase240_leque_generico_resultado_valido.pink",
        "fase240_leque_generico_resultado_valido",
        24_000,
    );
}

#[test]
fn fase232_impl_multiplos_contratos_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase232_impl_multiplos_contratos_valido.pink",
        "fase232_impl_multiplos_contratos_valido",
        23_200,
    );
}

#[test]
fn fase233_mapa_generico_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase233_mapa_generico_valido.pink",
        "fase233_mapa_generico_valido",
        23_300,
    );
}

#[test]
fn fase234_impl_homonimos_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase234_impl_homonimos_valido.pink",
        "fase234_impl_homonimos_valido",
        23_400,
    );
}

#[test]
fn fase235_mapa_generico_expressoes_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase235_mapa_generico_expressoes_valido.pink",
        "fase235_mapa_generico_expressoes_valido",
        23_500,
    );
}

#[test]
fn fase236_funcao_generica_usuario_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase236_funcao_generica_usuario_valido.pink",
        "fase236_funcao_generica_usuario_valido",
        23_600,
    );
}

#[test]
fn fase241_resultado_predeclarado_tem_paridade_nativa() {
    paridade_stdout(
        "examples/fase241_resultado_predeclarado_valido.pink",
        "fase241_resultado_predeclarado_valido",
        24_100,
    );
}
// @pinker-nav:end evidencia.backend-nativo.paridade-stdout-fases-avancadas
