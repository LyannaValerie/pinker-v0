mod common;

use common::render_backend_s_external_subset;
use common::ControlledCommand as Command;
use pinker_v0::backend_s::emit_external_toolchain_subset;
use pinker_v0::cfg_ir::OperandIR;
use pinker_v0::instr_select::{
    SelectedBlock, SelectedFunction, SelectedGlobal, SelectedProgram, SelectedTerminator,
};
use pinker_v0::ir::TypeIR;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// @pinker-nav:start evidence.external-backend-s.rendering-versioned-slices
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Supplies inline source or versioned examples (phase111–125) to the render_backend_s_external_subset helper, which runs parse, semantics, IR, CFG and selection in memory and emits assembly via emit_external_toolchain_subset; it validates by contains the subset header, `.globl main` for the entrypoint and `.local <name>` for every non-entrypoint definition, the injective local labels `.Lp<len>_<fn><len>_<block>`, `jmp`/`cmpq`/`jne`/`setb`, the `.rodata` section, argument moves and deref instructions. No external process is created: it does not assemble, does not link and does not execute; the evidence is about the emitted text, not about the correctness of the machine code.
#[test]
fn asm_s_external_subset_emite_main_montavel() {
    let code = "pacote main; carinho principal() -> bombom { mimo 42; }";
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains("# pinker v0 external toolchain subset (fase 135"));
    assert!(out.contains("composto minimo com deref_store/deref_load heterogeneo camada 4"));
    assert!(out.contains(".globl main"));
    assert!(out.contains("jmp .Lp9_principal5_entry"));
    assert!(out.contains(".Lp9_principal5_entry:"));
    assert!(out.contains("movabsq $42, %rax"));
}

#[test]
fn asm_s_external_subset_fase111_exemplo_versionado_emite_labels_e_jmp_incondicional() {
    let code = include_str!("../examples/fase111_blocos_labels_salto_incondicional_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lp9_principal5_entry:"));
    assert!(out.contains("jmp .Lp9_principal5_entry"));
}

#[test]
fn asm_s_external_subset_fase112_exemplo_versionado_emite_cmp_e_jcc() {
    let code = include_str!("../examples/fase112_branch_condicional_minimo_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lp9_principal5_entry:"));
    assert!(out.contains("cmpq %r10, %rax"));
    assert!(out.contains("cmpq $0, %rax"));
    assert!(out.contains("jne .Lp9_principal"));
}

#[test]
fn asm_s_external_subset_fase113_exemplo_versionado_emite_ciclo_com_label_de_loop() {
    let code = include_str!("../examples/fase113_loops_reais_minimos_validos.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(out.contains("setb %al"));
    assert!(out.contains("jmp .Lp9_principal11_loop_cond_0"));
}

#[test]
fn asm_s_external_subset_fase114_exemplo_versionado_emite_rodata_e_load_global() {
    let code = include_str!("../examples/fase114_globais_minimas_rodata_base_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".section .rodata"));
    assert!(out.contains(".local BASE"));
    assert!(out.contains("BASE:"));
    assert!(out.contains("movq BASE(%rip), %rax"));
}

#[test]
fn asm_s_external_subset_fase115_exemplo_versionado_emite_terceiro_argumento() {
    let code = include_str!("../examples/fase115_abi_minima_mais_larga_camada1_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains("movq %rdx"));
    assert!(out.contains("call soma3"));
}

#[test]
fn asm_s_external_subset_fase116_exemplo_versionado_emite_deref_load_minimo() {
    let code = include_str!("../examples/fase116_compostos_minimos_camada1_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local soma_par_minimo"));
    assert!(out.contains("movq (%rax), %rax"));
    assert!(out.contains("call pinker_ponteiro_derivar_tipado"));
    assert!(out.contains("movq $8, %rdx"));
}

#[test]
fn asm_s_external_subset_fase117_exemplo_versionado_emite_offset_explicito_em_local_ponteiro() {
    let code = include_str!("../examples/fase117_compostos_minimos_camada2_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local soma_par_offset_local"));
    assert!(out.contains("call pinker_ponteiro_derivar_tipado"));
    assert!(out.contains("movq $8, %rdx"));
    assert!(out.matches("movq (%rax), %rax").count() >= 2);
}

#[test]
fn asm_s_external_subset_fase118_exemplo_versionado_emite_deref_store_minimo() {
    let code = include_str!("../examples/fase118_compostos_minimos_camada3_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local soma_tripla_com_store_minimo"));
    assert!(out.contains("movq %r10, (%rax)"));
    assert!(out.matches("movq (%rax), %rax").count() >= 3);
}

#[test]
fn asm_s_external_subset_fase119_exemplo_versionado_consolida_par_homogeneo_minimo() {
    let code = include_str!("../examples/fase119_compostos_minimos_camada4_valida.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local consolida_par_homogeneo_minimo"));
    assert!(out.matches("movq %r10, (%rax)").count() >= 2);
    assert!(out.matches("movq (%rax), %rax").count() >= 4);
    assert!(out.matches("call pinker_ponteiro_derivar_tipado").count() >= 2);
    assert!(out.contains("movq $8, %rdx"));
}

#[test]
fn asm_s_external_subset_fase120_exemplo_versionado_u32_minimo_em_param_local() {
    let code = include_str!("../examples/fase120_tipos_inteiros_mais_largos_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local eco_u32_minimo"));
    assert!(out.contains("call eco_u32_minimo"));
    assert!(out.contains("movq %rdi, -8(%rbp)"));
}

#[test]
fn asm_s_external_subset_fase121_exemplo_versionado_u64_minimo_em_param_local() {
    let code = include_str!("../examples/fase121_tipos_inteiros_mais_largos_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local eco_u64_minimo"));
    assert!(out.contains("call eco_u64_minimo"));
    assert!(out.contains("movq %rdi, -8(%rbp)"));
}
#[test]
fn asm_s_external_subset_fase122_exemplo_versionado_comparacao_ne_minima() {
    let code = include_str!("../examples/fase122_comparacoes_ampliadas_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local diferente_u64_minimo"));
    assert!(out.contains("setne %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase123_exemplo_versionado_comparacao_gt_minima() {
    let code = include_str!("../examples/fase123_comparacoes_ampliadas_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local maior_u64_minimo"));
    assert!(out.contains("seta %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase124_exemplo_versionado_comparacao_le_minima() {
    let code = include_str!("../examples/fase124_comparacoes_ampliadas_camada3_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local menor_ou_igual_u64_minimo"));
    assert!(out.contains("setbe %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}

#[test]
fn asm_s_external_subset_fase125_exemplo_versionado_comparacao_ge_minima() {
    let code = include_str!("../examples/fase125_comparacoes_ampliadas_camada4_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local maior_ou_igual_u64_minimo"));
    assert!(out.contains("setae %al"));
    assert!(out.contains("cmpq %r10, %rax"));
}
// @pinker-nav:end evidence.external-backend-s.rendering-versioned-slices

// @pinker-nav:start evidence.external-backend-s.boundary-heterogeneous-ninho
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Alternates acceptances and refusals of the heterogeneous `ninho` examples in layers 1–4 (phase129–132): in the accepted cases it checks by contains the offsets and accesses emitted in the assembly; in the refused ones it checks the error message of the external assemblable subset. All the work happens in memory via render_backend_s_external_subset; no external tool is called and nothing is assembled, linked or executed.
#[test]
fn asm_s_external_subset_fase129_exemplo_versionado_ninho_heterogeneo_camada1() {
    let code = include_str!("../examples/fase129_ninho_heterogeneo_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local ler_marca_heterogenea_minima"));
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.contains("sete %al"));
    assert!(out.contains("movabsq $129, %rax"));
}

#[test]
fn asm_s_external_subset_fase130_exemplo_versionado_ninho_heterogeneo_camada2() {
    let code = include_str!("../examples/fase130_ninho_heterogeneo_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local ler_selo_heterogeneo_camada2"));
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.contains("sete %al"));
    assert!(out.contains("movabsq $130, %rax"));
}

#[test]
fn asm_s_external_subset_fase129_recusa_campo_heterogeneo_fora_recorte() {
    let code = include_str!("../examples/fase129_ninho_heterogeneo_camada1_regressao_valido.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    let msg = err.to_string();
    // Fase 219 (B8): locals `logica` passaram a ser aceitos, então o exemplo
    // avança até a recusa real de `deref_load` fora do recorte (Fase 134).
    assert!(msg.contains("Fase 134"), "{}", msg);
    assert!(msg.contains("deref_load"), "{}", msg);
}

#[test]
fn asm_s_external_subset_fase130_recusa_campo_heterogeneo_fora_recorte() {
    let code = include_str!("../examples/fase130_ninho_heterogeneo_camada2_regressao_valido.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    let msg = err.to_string();
    // Fase 219 (B8): locals `logica` passaram a ser aceitos, então o exemplo
    // avança até a recusa real de `deref_load` fora do recorte (Fase 134).
    assert!(msg.contains("Fase 134"), "{}", msg);
    assert!(msg.contains("deref_load"), "{}", msg);
}

#[test]
fn asm_s_external_subset_fase131_exemplo_versionado_ninho_heterogeneo_camada3() {
    let code = include_str!("../examples/fase131_ninho_heterogeneo_camada3_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local escrever_selo_heterogeneo_camada3"));
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.contains("movq %r10, (%rax)"));
    assert!(out.contains("sete %al"));
    assert!(out.contains("movabsq $131, %rax"));
}

#[test]
fn asm_s_external_subset_fase131_recusa_campo_heterogeneo_fora_recorte() {
    let code = include_str!("../examples/fase131_ninho_heterogeneo_camada3_regressao_valido.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Fase 134")
            || msg.contains("deref_load")
            || msg.contains("deref_store")
            || msg.contains("slot")
            || msg.contains("seta")
            || msg.contains("logica")
    );
}

#[test]
fn asm_s_external_subset_fase132_exemplo_versionado_ninho_heterogeneo_camada4() {
    let code = include_str!("../examples/fase132_ninho_heterogeneo_camada4_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".local compor_ninho_heterogeneo_camada4"));
    assert!(out.matches("movq %r10, (%rax)").count() >= 2);
    assert!(out.matches("movq (%rax), %rax").count() >= 2);
    assert!(out.contains("movabsq $8, %r10"));
    assert!(out.contains("sete %al"));
    assert!(out.contains("movabsq $132, %rax"));
}

#[test]
fn asm_s_external_subset_fase132_recusa_campo_heterogeneo_fora_recorte() {
    let code = include_str!("../examples/fase132_ninho_heterogeneo_camada4_regressao_valido.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("Fase 134")
            || msg.contains("deref_load")
            || msg.contains("deref_store")
            || msg.contains("slot")
            || msg.contains("seta")
            || msg.contains("u16")
    );
}
// @pinker-nav:end evidence.external-backend-s.boundary-heterogeneous-ninho

// @pinker-nav:start evidence.external-backend-s.boundary-virar-conversion
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Supplies versioned examples of `virar` layers 1 and 2 (two acceptances) and one invalid example (one refusal); it checks textually the conversion instructions emitted and, in the invalid case, the refusal message. Execution is in memory only; no external tool is called — no assembler, linker or binary.
#[test]
fn asm_s_external_subset_fase133_exemplo_versionado_virar_camada1() {
    let code = include_str!("../examples/fase133_virar_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains("movabsq $133, %rax"));
    assert!(out.contains("# pinker v0 external toolchain subset (fase 135"));
}

#[test]
fn asm_s_external_subset_fase134_exemplo_versionado_virar_camada2() {
    let code = include_str!("../examples/fase134_virar_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains("movabsq $134, %rax"));
    assert!(out.contains("# pinker v0 external toolchain subset (fase 135"));
}

#[test]
fn asm_s_external_subset_fase134_cast_historico_agora_aceito() {
    let code = include_str!("../examples/fase134_virar_camada2_invalido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movabsq $1, %rax"));
}
// @pinker-nav:end evidence.external-backend-s.boundary-virar-conversion

// @pinker-nav:start evidence.external-backend-s.rendering-verso-rodata
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Supplies `verso` layer 1 examples (including one example historically marked as invalid, accepted today) and checks by contains the length-prefixed layout `[.quad size][.ascii bytes]` in the `.rodata` section. Textual validation only: it does not assemble, does not link and does not execute, and nothing is proven about reading that layout at runtime.
#[test]
fn asm_s_external_subset_fase135_exemplo_versionado_verso_camada1() {
    let code = include_str!("../examples/fase135_verso_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains("# pinker v0 external toolchain subset (fase 135"));
    assert!(out.contains(".Lpinker_verso_0:"));
    // Fase 215 (B4): literais `verso` passaram a ser length-prefixed
    // (`.quad tamanho` + `.ascii`), o layout único de verso nativo.
    assert!(out.contains(".quad 7"));
    assert!(out.contains(".ascii \"fase135\""));
    assert!(out.contains("leaq .Lpinker_verso_0(%rip), %rax"));
    assert!(out.contains("call mede_tag"));
}

#[test]
fn asm_s_external_subset_fase215_aceita_retorno_verso_com_layout_length_prefixed() {
    // Antes da Fase 215 (B4), retorno `verso` era recusado; com o verso
    // dinâmico nativo, funções que devolvem verso emitem normalmente.
    let code = include_str!("../examples/fase135_verso_camada1_invalido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".quad 4"), "{}", out);
    assert!(out.contains(".ascii \"fora\""), "{}", out);
}
// @pinker-nav:end evidence.external-backend-s.rendering-verso-rodata

// @pinker-nav:start evidence.external-backend-s.rendering-quebrar-continuar
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Supplies versioned examples of `quebrar`/`continuar` (phase126–128) in decreasing physical layer order — 3, 2, 1 — and checks textually the labels and jumps emitted. Execution in memory only via render_backend_s_external_subset; no external process, no assembling, linking or execution.
#[test]
fn asm_s_external_subset_fase128_exemplo_versionado_quebrar_continuar_camada3() {
    let code = include_str!("../examples/fase128_quebrar_continuar_camada3_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(out.contains(".Lp9_principal11_loop_cond_2:"));
    assert!(out.contains(".Lp9_principal11_loop_cond_6:"));
    assert!(out.matches("_loop_break_cont_").count() >= 3);
    assert!(out.matches("_loop_continue_cont_").count() >= 3);
}

#[test]
fn asm_s_external_subset_fase127_exemplo_versionado_quebrar_continuar_camada2() {
    let code = include_str!("../examples/fase127_quebrar_continuar_camada2_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(out.contains(".Lp9_principal11_loop_cond_2:"));
    assert!(out.contains("_loop_break_cont_"));
    assert!(out.contains("_loop_continue_cont_"));
    assert!(out.matches("jmp .Lp9_principal11_loop_cond_").count() >= 2);
}

#[test]
fn asm_s_external_subset_fase126_exemplo_versionado_quebrar_continuar_camada1() {
    let code = include_str!("../examples/fase126_quebrar_continuar_camada1_valido.pink");
    let out = render_backend_s_external_subset(code).unwrap();
    assert!(out.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(out.contains("_loop_break_cont_"));
    assert!(out.contains("_loop_continue_cont_"));
    assert!(out.contains("jmp .Lp9_principal11_loop_cond_0"));
}
// @pinker-nav:end evidence.external-backend-s.rendering-quebrar-continuar

// @pinker-nav:start evidence.external-backend-s.real-execution-versioned-slices
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Each test renders the `.s` with render_backend_s_external_subset, writes the file into a unique temporary directory, detects a C driver (`cc`, `gcc` or `clang`) at runtime and invokes it as the party responsible for assembling and linking, then executes the produced binary and validates only `status.code()`. No stdout is validated and stderr is used only as a failure message. The path is hosted with runtime_init=false and without libpinker_rt.a. All are silently skipped outside Linux x86_64 or when there is no C driver — the suite can pass without exercising this evidence.
#[test]
fn asm_s_external_subset_fluxo_real_fase117_composto_minimo_camada2() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase117_compostos_minimos_camada2_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local soma_par_offset_local"));
    assert!(asm.matches("movq (%rax), %rax").count() >= 2);

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(0));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase118_composto_minimo_camada3() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase118_compostos_minimos_camada3_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local soma_tripla_com_store_minimo"));
    assert!(asm.contains("movq %r10, (%rax)"));
    assert!(asm.matches("movq (%rax), %rax").count() >= 3);

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(0));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase119_composto_minimo_camada4() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase119_compostos_minimos_camada4_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local consolida_par_homogeneo_minimo"));
    assert!(asm.matches("movq %r10, (%rax)").count() >= 2);
    assert!(asm.matches("movq (%rax), %rax").count() >= 4);

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(119));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase120_u32_minimo_em_param_local() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase120_tipos_inteiros_mais_largos_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local eco_u32_minimo"));
    assert!(asm.contains("call eco_u32_minimo"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(120));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase121_u64_minimo_em_param_local() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase121_tipos_inteiros_mais_largos_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local eco_u64_minimo"));
    assert!(asm.contains("call eco_u64_minimo"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(121));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase122_comparacao_ne_minima() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase122_comparacoes_ampliadas_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local diferente_u64_minimo"));
    assert!(asm.contains("setne %al"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(122));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase123_comparacao_gt_minima() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase123_comparacoes_ampliadas_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local maior_u64_minimo"));
    assert!(asm.contains("seta %al"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(123));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase124_comparacao_le_minima() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase124_comparacoes_ampliadas_camada3_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local menor_ou_igual_u64_minimo"));
    assert!(asm.contains("setbe %al"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(124));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase125_comparacao_ge_minima() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase125_comparacoes_ampliadas_camada4_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local maior_ou_igual_u64_minimo"));
    assert!(asm.contains("setae %al"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(125));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase129_ninho_heterogeneo_camada1() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase129_ninho_heterogeneo_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local ler_marca_heterogenea_minima"));
    assert!(asm.contains("movabsq $8, %r10"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(129));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase130_ninho_heterogeneo_camada2() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase130_ninho_heterogeneo_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local ler_selo_heterogeneo_camada2"));
    assert!(asm.contains("movabsq $8, %r10"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(130));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase131_ninho_heterogeneo_camada3() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase131_ninho_heterogeneo_camada3_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local escrever_selo_heterogeneo_camada3"));
    assert!(asm.contains("movabsq $8, %r10"));
    assert!(asm.contains("movq %r10, (%rax)"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(131));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase132_ninho_heterogeneo_camada4() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase132_ninho_heterogeneo_camada4_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local compor_ninho_heterogeneo_camada4"));
    assert!(asm.matches("movq %r10, (%rax)").count() >= 2);
    assert!(asm.matches("movq (%rax), %rax").count() >= 2);

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(132));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase133_virar_camada1() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase133_virar_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(133));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase134_virar_camada2() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase134_virar_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(134));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase135_verso_camada1() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase135_verso_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lpinker_verso_0:"));
    assert!(asm.contains("leaq .Lpinker_verso_0(%rip), %rax"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(135));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase128_quebrar_continuar_camada3() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase128_quebrar_continuar_camada3_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(asm.contains(".Lp9_principal11_loop_cond_2:"));
    assert!(asm.contains(".Lp9_principal11_loop_cond_6:"));
    assert!(asm.matches("_loop_break_cont_").count() >= 3);
    assert!(asm.matches("_loop_continue_cont_").count() >= 3);

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(42));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase127_quebrar_continuar_camada2() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase127_quebrar_continuar_camada2_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(asm.contains(".Lp9_principal11_loop_cond_2:"));
    assert!(asm.contains("_loop_break_cont_"));
    assert!(asm.contains("_loop_continue_cont_"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(12));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase126_quebrar_continuar_camada1() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase126_quebrar_continuar_camada1_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("_loop_break_cont_"));
    assert!(asm.contains("_loop_continue_cont_"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(12));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}
#[test]
fn asm_s_external_subset_fluxo_real_fase116_composto_minimo_camada1() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase116_compostos_minimos_camada1_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".local soma_par_minimo"));
    assert!(asm.contains("movq (%rax), %rax"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(0));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_condicional() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase112_branch_condicional_minimo_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");

    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");

    assert_eq!(run.status.code(), Some(7));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_loop_minimo() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase113_loops_reais_minimos_validos.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(asm.contains("jmp .Lp9_principal11_loop_cond_0"));
    assert!(asm.contains("setb %al"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");

    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(3));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}
// @pinker-nav:end evidence.external-backend-s.real-execution-versioned-slices

// @pinker-nav:start evidence.external-backend-s.real-execution-interprocedural-abi-frame
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary The same limits as the previous region — rendering the `.s`, writing to a temporary directory, a C driver (`cc`, `gcc` or `clang`) detected at runtime responsible for assembling and linking, executing the binary, validating only `status.code()`, no stdout validated, stderr only as a failure message, runtime_init=false, without libpinker_rt.a and a silent skip outside Linux x86_64 or without a C driver — applied to locals, arithmetic, calls, parameters, frame, frame memory, interprocedural composition and larger linear programs. The suite can pass without exercising this evidence.
#[test]
fn asm_s_external_subset_fluxo_real_com_locais_e_aritmetica() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase73_backend_externo_locais_aritmetica_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("imulq"));
    assert!(asm.contains("subq"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(59));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_com_call_e_parametro_unico() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase74_backend_externo_call_minimo_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains(".globl main"));
    assert!(asm.contains(".local dobro"));
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("call dobro"));
    assert!(asm.contains("imulq %r10, %rax"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(79));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase75_frame_registradores() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase75_backend_externo_frame_registradores_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("imulq %r10, %rax"));
    assert!(asm.contains("# frame: %rbp base"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(44));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase76_multiplos_parametros() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase76_backend_externo_multiplos_parametros_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("movq %rsi"));
    assert!(asm.contains("call soma2"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(41));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase77_memoria_frame_minima() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase77_backend_externo_memoria_frame_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq -"));
    assert!(asm.contains("(%rbp), %rax"));
    assert!(asm.contains("movq %rax, -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(23));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase78_composicao_interprocedural_linear() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code =
        include_str!("../examples/fase78_backend_externo_composicao_interprocedural_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("call soma2"));
    assert!(asm.contains("call ajusta"));
    assert!(asm.contains("call combina"));
    assert!(asm.contains("movq -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(39));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase79_programa_linear_maior() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase79_backend_externo_programa_linear_maior_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("call etapa"));
    assert!(asm.contains("call refina"));
    assert!(asm.contains("imulq %r10, %rax"));
    assert!(asm.contains("subq %r10, %rax"));
    assert!(asm.contains("movq -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(207));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase80_cobertura_linear_auditavel_mais_ampla() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code =
        include_str!("../examples/fase80_backend_externo_cobertura_linear_ampla_valido.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("call base"));
    assert!(asm.contains("call mistura"));
    assert!(asm.contains("movq %rsi"));
    assert!(asm.contains("imulq %r10, %rax"));
    assert!(asm.contains("movq -"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(167));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}

#[test]
fn asm_s_external_subset_fluxo_real_fase115_abi_minima_mais_larga_camada1() {
    let Some((driver, _)) =
        common::require_native_evidence(concat!(module_path!(), ":", line!()), false)
    else {
        return;
    };

    let code = include_str!("../examples/fase115_abi_minima_mais_larga_camada1_valida.pink");
    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("movq %rdi"));
    assert!(asm.contains("movq %rsi"));
    assert!(asm.contains("movq %rdx"));
    assert!(asm.contains("call soma3"));

    let workdir = unique_temp_dir();
    fs::create_dir_all(&workdir).expect("falha ao criar diretório temporário");
    let asm_path = workdir.join("principal.s");
    let bin_path = workdir.join("principal");
    fs::write(&asm_path, asm).expect("falha ao escrever .s temporário");

    let compile = Command::new(&driver)
        .arg(&asm_path)
        .arg("-o")
        .arg(&bin_path)
        .output()
        .expect("falha ao invocar driver C");
    assert!(
        compile.status.success(),
        "compilação falhou com {}: {}",
        driver,
        String::from_utf8_lossy(&compile.stderr)
    );

    let run = Command::new(&bin_path)
        .output()
        .expect("falha ao executar binário gerado");
    assert_eq!(run.status.code(), Some(32));

    let _ = fs::remove_file(&asm_path);
    let _ = fs::remove_file(&bin_path);
    let _ = fs::remove_dir(&workdir);
}
// @pinker-nav:end evidence.external-backend-s.real-execution-interprocedural-abi-frame

// @pinker-nav:start evidence.external-backend-s.boundary-textual-subset
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Gathers the boundary tests that call render_backend_s_external_subset and inspect the result in memory: refusals with a specific message (source outside the subset, non-`bombom` parameter, loop condition outside the slice, `quebrar` outside a loop, composite outside layers 1–2, fragile store, `u16` parameter), boundary acceptances (four parameters with the full ABI, `talvez`/`senão`) and an auditable matrix of the assemblable subset. It proves messages and text fragments; it does not assemble, does not link and does not execute.
#[test]
fn asm_s_external_subset_falha_clara_fora_do_subset() {
    // Fase 221 (B10) absorveu ambiente/processo; a fronteira de recusa clara
    // passa a ser exercida por stdin interativo (`ouvir`), fora do eixo atual.
    let code = "pacote main; trazer entrada.ouvir;\n\ncarinho principal() -> bombom {\n    nova n: bombom = ouvir();\n    mimo n;\n}\n";

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains("subset externo montável"));
}

#[test]
fn asm_s_external_subset_falha_parametro_nao_bombom() {
    let code =
        include_str!("../examples/fase75_backend_externo_parametro_nao_bombom_invalido.pink");

    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável aceita parâmetro `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase213_aceita_quatro_parametros_com_abi_completa() {
    // Antes da Fase 213 (B2), 4+ parâmetros eram recusados; a ABI SysV
    // completa passa a aceitar N parâmetros (6 registradores + pilha).
    let code =
        include_str!("../examples/fase115_abi_minima_mais_larga_camada1_quatro_args_invalido.pink");

    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("%rcx"), "{}", asm);
}

#[test]
fn asm_s_external_subset_fase112_aceita_talvez_senao_no_recorte_minimo() {
    let code = include_str!(
        "../examples/fase82_backend_externo_recusa_explicita_talvez_senao_invalido.pink",
    );

    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("cmpq $0, %rax"));
    assert!(asm.contains("jne .Lp9_principal"));
}

#[test]
fn asm_s_external_subset_fase113_loop_historico_agora_aceito() {
    let code = include_str!("../examples/fase113_loop_condicao_invalida_invalido.pink");

    let asm = render_backend_s_external_subset(code).unwrap();
    assert!(asm.contains("pinker_erro_divisao_zero"));
    assert!(asm.contains(".Lp9_principal11_loop_cond_0:"));
}

#[test]
fn asm_s_external_subset_fase126_mantem_recusa_de_quebrar_fora_de_loop() {
    let code = include_str!("../examples/check_quebrar_fora_loop.pink");
    let err = render_backend_s_external_subset(code).expect_err("quebrar fora de loop deve falhar");
    assert!(format!("{err}").contains("'quebrar' só pode ser usado dentro de 'sempre que'"));
}

#[test]
fn asm_s_external_subset_fase84_matriz_fronteira_auditavel() {
    let casos_garantidos = [
        include_str!("../examples/fase73_backend_externo_locais_aritmetica_valido.pink"),
        include_str!("../examples/fase77_backend_externo_memoria_frame_valido.pink"),
        include_str!("../examples/fase78_backend_externo_composicao_interprocedural_valido.pink"),
    ];

    for code in casos_garantidos {
        let asm = render_backend_s_external_subset(code).expect("subset garantido deve emitir .s");
        assert!(asm.contains("# pinker v0 external toolchain subset (fase 135"));
    }

    // Fase 213 (B2): o caso de 4 parâmetros passou de fronteira rejeitada a
    // garantido pela ABI SysV completa (4º argumento em %rcx).
    let caso_quatro_params = include_str!(
        "../examples/fase115_abi_minima_mais_larga_camada1_quatro_args_invalido.pink",
    );
    let asm_quatro_params = render_backend_s_external_subset(caso_quatro_params).unwrap();
    assert!(asm_quatro_params.contains("%rcx"));

    let caso_branch_valido =
        include_str!("../examples/fase112_branch_condicional_minimo_valido.pink");
    let asm_branch = render_backend_s_external_subset(caso_branch_valido).unwrap();
    assert!(asm_branch.contains("cmpq $0, %rax"));
    assert!(asm_branch.contains("jne .Lp9_principal"));

    let caso_loop_valido = include_str!("../examples/fase113_loops_reais_minimos_validos.pink");
    let asm_loop = render_backend_s_external_subset(caso_loop_valido).unwrap();
    assert!(asm_loop.contains(".Lp9_principal11_loop_cond_0:"));
    assert!(asm_loop.contains("jmp .Lp9_principal11_loop_cond_0"));

    let caso_historico_sempre_que =
        include_str!("../examples/fase113_loop_condicao_invalida_invalido.pink");
    let asm_sempre_que = render_backend_s_external_subset(caso_historico_sempre_que).unwrap();
    assert!(asm_sempre_que.contains("pinker_erro_divisao_zero"));
}

#[test]
fn asm_s_external_subset_fase116_recusa_composto_fora_da_camada1() {
    let code = include_str!("../examples/fase116_compostos_minimos_camada1_invalida.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável aceita parâmetro `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase117_recusa_local_composto_fora_da_camada2() {
    let code = include_str!("../examples/fase117_compostos_minimos_camada2_invalida.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável só aceita local `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase118_recusa_store_fragil_fora_do_subset() {
    let code = include_str!("../examples/fase118_compostos_minimos_camada3_invalida.pink");
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável aceita parâmetro `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`"
    ));
}

#[test]
fn asm_s_external_subset_fase121_recusa_parametro_u16_fora_do_recorte() {
    let code = r#"
pacote main;
carinho soma_u16(a: u16) -> bombom {
    mimo 0;
}
carinho principal() -> bombom {
    mimo soma_u16(1);
}
"#;
    let err = render_backend_s_external_subset(code).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável aceita parâmetro `bombom`, `u32`, `u64`, `verso` opaco mínimo, `ninho` opaco ou `seta<T>`"
    ));
}
// @pinker-nav:end evidence.external-backend-s.boundary-textual-subset

// @pinker-nav:start evidence.external-backend-s.synthetic-structural-validation
// @pinker-nav:domain backend-s
// @pinker-nav:layer evidence
// @pinker-nav:summary Builds a `SelectedProgram` by hand (globals, functions, blocks, terminators) without going through the front-end and calls emit_external_toolchain_subset directly, requiring a refusal for a duplicate global, a jump to a nonexistent label, a duplicate label and a branch with a nonexistent true or false target, validating the diagnostic message. There is no front-end, file, assembler, linker or execution.
#[test]
fn asm_s_external_subset_fase114_falha_em_global_duplicada() {
    let program = SelectedProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![
            SelectedGlobal {
                name: "BASE".to_string(),
                ty: TypeIR::Bombom,
                value: OperandIR::Int(10),
            },
            SelectedGlobal {
                name: "BASE".to_string(),
                ty: TypeIR::Bombom,
                value: OperandIR::Int(20),
            },
        ],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            internal_pointer_params: Default::default(),
            blocks: vec![SelectedBlock {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: SelectedTerminator::Ret(Some(OperandIR::Int(0))),
            }],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 114) encontrou símbolo global duplicado"));
}

#[test]
fn asm_s_external_subset_fase112_falha_em_jmp_para_label_inexistente() {
    let mut slot_types = HashMap::new();
    slot_types.insert("x".to_string(), TypeIR::Bombom);
    let program = SelectedProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["x".to_string()],
            slot_types,
            internal_pointer_params: Default::default(),
            blocks: vec![SelectedBlock {
                label: "entry".to_string(),
                instructions: vec![],
                terminator: SelectedTerminator::Jmp("sumiu".to_string()),
            }],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 113) encontrou `jmp` para label inexistente"));
}

#[test]
fn asm_s_external_subset_fase112_falha_em_label_duplicado() {
    let mut slot_types = HashMap::new();
    slot_types.insert("x".to_string(), TypeIR::Bombom);
    let program = SelectedProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec!["x".to_string()],
            slot_types,
            internal_pointer_params: Default::default(),
            blocks: vec![
                SelectedBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Jmp("entry".to_string()),
                },
                SelectedBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Ret(Some(OperandIR::Int(0))),
                },
            ],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 113) encontrou label duplicado em função"));
}

#[test]
fn asm_s_external_subset_fase112_falha_em_br_com_alvo_verdadeiro_inexistente() {
    let program = SelectedProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            internal_pointer_params: Default::default(),
            blocks: vec![
                SelectedBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Br {
                        cond: OperandIR::Int(1),
                        then_label: "sumiu".to_string(),
                        else_label: "ok".to_string(),
                    },
                },
                SelectedBlock {
                    label: "ok".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Ret(Some(OperandIR::Int(0))),
                },
            ],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err.to_string().contains(
        "subset externo montável (Fase 113) encontrou `br` com alvo verdadeiro inexistente"
    ));
}

#[test]
fn asm_s_external_subset_fase112_falha_em_br_com_alvo_falso_inexistente() {
    let program = SelectedProgram {
        union_types: vec![],
        module_name: "main".to_string(),
        is_freestanding: false,
        globals: vec![],
        functions: vec![SelectedFunction {
            name: "principal".to_string(),
            ret_type: TypeIR::Bombom,
            params: vec![],
            locals: vec![],
            slot_types: HashMap::new(),
            internal_pointer_params: Default::default(),
            blocks: vec![
                SelectedBlock {
                    label: "entry".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Br {
                        cond: OperandIR::Int(1),
                        then_label: "ok".to_string(),
                        else_label: "sumiu".to_string(),
                    },
                },
                SelectedBlock {
                    label: "ok".to_string(),
                    instructions: vec![],
                    terminator: SelectedTerminator::Ret(Some(OperandIR::Int(0))),
                },
            ],
        }],
    };

    let err = emit_external_toolchain_subset(&program).unwrap_err();
    assert!(err
        .to_string()
        .contains("subset externo montável (Fase 113) encontrou `br` com alvo falso inexistente"));
}
// @pinker-nav:end evidence.external-backend-s.synthetic-structural-validation

fn unique_temp_dir() -> std::path::PathBuf {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("tempo do sistema inválido")
        .as_nanos();
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "pinker_phase113_{}_{}_{}",
        std::process::id(),
        nanos,
        sequence
    ))
}
